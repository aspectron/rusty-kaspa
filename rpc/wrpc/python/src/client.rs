use ahash::AHashMap;
use futures::*;
use kaspa_addresses::Address;
use kaspa_notify::listener::ListenerId;
use kaspa_notify::notification::Notification;
use kaspa_notify::scope::{Scope, UtxosChangedScope, VirtualChainChangedScope, VirtualDaaScoreChangedScope};
use kaspa_notify::{connection::ChannelType, events::EventType};
use kaspa_python_macros::py_async;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::model::*;
use kaspa_rpc_core::notify::connection::ChannelConnection;
use kaspa_rpc_macros::{build_wrpc_python_interface, build_wrpc_python_subscriptions};
use kaspa_wrpc_client::{
    client::{ConnectOptions, ConnectStrategy},
    error::Error,
    prelude::*,
    result::Result,
    KaspaRpcClient, WrpcEncoding,
};
use pyo3::{
    exceptions::PyException,
    prelude::*,
    types::{PyDict, PyTuple},
};
use std::str::FromStr;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use workflow_core::channel::{Channel, DuplexChannel};
use workflow_log::*;
use workflow_rpc::client::Ctl;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum NotificationEvent {
    All,
    Notification(EventType),
    RpcCtl(Ctl),
}

impl FromStr for NotificationEvent {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        if s == "all" {
            Ok(NotificationEvent::All)
        } else if let Ok(ctl) = Ctl::from_str(s) {
            Ok(NotificationEvent::RpcCtl(ctl))
        } else if let Ok(event) = EventType::from_str(s) {
            Ok(NotificationEvent::Notification(event))
        } else {
            Err(Error::custom(format!("Invalid notification event type: `{}`", s)))
        }
    }
}

#[derive(Clone)]
struct PyCallback {
    callback: PyObject,
    args: Option<Py<PyTuple>>,
    kwargs: Option<Py<PyDict>>,
}

impl PyCallback {
    fn append_to_args(&self, py: Python, event: Bound<PyDict>) -> PyResult<Py<PyTuple>> {
        match &self.args {
            Some(existing_args) => {
                let tuple_ref = existing_args.bind(py);

                let mut new_args: Vec<PyObject> = tuple_ref.iter().map(|arg| arg.to_object(py)).collect();
                new_args.push(event.into());

                Ok(Py::from(PyTuple::new_bound(py, new_args)))
            }
            None => Ok(Py::from(PyTuple::new_bound(py, [event]))),
        }
    }

    fn execute(&self, py: Python, event: Bound<PyDict>) -> PyResult<PyObject> {
        let args = self.append_to_args(py, event).unwrap();
        let kwargs = self.kwargs.as_ref().map(|kw| kw.bind(py));

        let result = self
            .callback
            .call_bound(py, args.bind(py), kwargs)
            .map_err(|e| pyo3::exceptions::PyException::new_err(format!("Error while executing RPC notification callback: {}", e)))
            .unwrap();

        Ok(result)
    }
}

pub struct Inner {
    client: Arc<KaspaRpcClient>,
    // resolver TODO
    notification_task: Arc<AtomicBool>,
    notification_ctl: DuplexChannel,
    callbacks: Arc<Mutex<AHashMap<NotificationEvent, Vec<PyCallback>>>>,
    listener_id: Arc<Mutex<Option<ListenerId>>>,
    notification_channel: Channel<kaspa_rpc_core::Notification>,
}

impl Inner {
    fn notification_callbacks(&self, event: NotificationEvent) -> Option<Vec<PyCallback>> {
        let notification_callbacks = self.callbacks.lock().unwrap();
        let all = notification_callbacks.get(&NotificationEvent::All).cloned();
        let target = notification_callbacks.get(&event).cloned();
        match (all, target) {
            (Some(mut vec_all), Some(vec_target)) => {
                vec_all.extend(vec_target);
                Some(vec_all)
            }
            (Some(vec_all), None) => Some(vec_all),
            (None, Some(vec_target)) => Some(vec_target),
            (None, None) => None,
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct RpcClient {
    inner: Arc<Inner>,
}

impl RpcClient {
    fn new(url: Option<String>, encoding: Option<WrpcEncoding>) -> Result<RpcClient> {
        let encoding = encoding.unwrap_or(WrpcEncoding::Borsh);

        let client = Arc::new(KaspaRpcClient::new(encoding, url.as_deref(), None, None, None).unwrap());

        let rpc_client = RpcClient {
            inner: Arc::new(Inner {
                client,
                notification_task: Arc::new(AtomicBool::new(false)),
                notification_ctl: DuplexChannel::oneshot(),
                callbacks: Arc::new(Default::default()),
                listener_id: Arc::new(Mutex::new(None)),
                notification_channel: Channel::unbounded(),
            }),
        };

        Ok(rpc_client)
    }
}

#[pymethods]
impl RpcClient {
    #[new]
    fn ctor(url: Option<String>) -> PyResult<RpcClient> {
        // TODO expose args to Python similar to WASM wRPC Client IRpcConfig

        Ok(Self::new(url, None)?)
    }

    fn url(&self) -> Option<String> {
        self.inner.client.url()
    }

    fn is_connected(&self) -> bool {
        self.inner.client.is_connected()
    }

    fn encoding(&self) -> String {
        self.inner.client.encoding().to_string()
    }

    fn connect(&self, py: Python) -> PyResult<Py<PyAny>> {
        // TODO expose args to Python similar to WASM wRPC Client IConnectOptions
        let options = ConnectOptions {
            block_async_connect: true,
            connect_timeout: Some(Duration::from_millis(5_000)),
            strategy: ConnectStrategy::Fallback,
            ..Default::default()
        };

        self.start_notification_task(py).unwrap();

        let client = self.inner.client.clone();
        py_async! {py, async move {
            let _ = client.connect(Some(options)).await.map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()));
            Ok(())
        }}
    }

    fn disconnect(&self, py: Python) -> PyResult<Py<PyAny>> {
        let client = self.clone();
        
        py_async! {py, async move {
            client.inner.client.disconnect().await?;
            client.stop_notification_task().await?;
            Ok(())
        }}
    }

    fn get_server_info(&self, py: Python) -> PyResult<Py<PyAny>> {
        let client = self.inner.client.clone();
        py_async! {py, async move {
            let response = client.get_server_info_call(GetServerInfoRequest { }).await?;
            Python::with_gil(|py| {
                Ok(serde_pyobject::to_pyobject(py, &response).unwrap().to_object(py))
            })
        }}
    }

    fn get_block_dag_info(&self, py: Python) -> PyResult<Py<PyAny>> {
        let client = self.inner.client.clone();
        py_async! {py, async move {
            let response = client.get_block_dag_info_call(GetBlockDagInfoRequest { }).await?;
            Python::with_gil(|py| {
                Ok(serde_pyobject::to_pyobject(py, &response).unwrap().to_object(py))
            })
        }}
    }

    #[pyo3(signature = (event, callback, *args, **kwargs))]
    fn add_event_listener(
        &self,
        py: Python,
        event: String,
        callback: PyObject,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let event = NotificationEvent::from_str(event.as_str()).unwrap();

        let args = args.to_object(py).extract::<Py<PyTuple>>(py).unwrap();
        let kwargs = kwargs.unwrap().to_object(py).extract::<Py<PyDict>>(py).unwrap();

        let py_callback = PyCallback { callback, args: Some(args), kwargs: Some(kwargs) };

        self.inner.callbacks.lock().unwrap().entry(event).or_default().push(py_callback);
        Ok(())
    }

    // fn remove_event_listener() TODO
    // fn clear_event_listener() TODO

    fn remove_all_event_listeners(&self) -> PyResult<()> {
        *self.inner.callbacks.lock().unwrap() = Default::default();
        Ok(())
    }
}

impl RpcClient {
    pub fn listener_id(&self) -> Option<ListenerId> {
        *self.inner.listener_id.lock().unwrap()
    }

    async fn stop_notification_task(&self) -> Result<()> {
        if self.inner.notification_task.load(Ordering::SeqCst) {
            self.inner.notification_ctl.signal(()).await?;
            self.inner.notification_task.store(false, Ordering::SeqCst);
        }
        Ok(())
    }

    fn start_notification_task(&self, py: Python) -> Result<()> {
        if self.inner.notification_task.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.inner.notification_task.store(true, Ordering::SeqCst);

        let ctl_receiver = self.inner.notification_ctl.request.receiver.clone();
        let ctl_sender = self.inner.notification_ctl.response.sender.clone();
        let notification_receiver = self.inner.notification_channel.receiver.clone();
        let ctl_multiplexer_channel =
            self.inner.client.rpc_client().ctl_multiplexer().as_ref().expect("Python RpcClient ctl_multiplexer is None").channel();
        let this = self.clone();

        let _ = pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            loop {
                select_biased! {
                    msg = ctl_multiplexer_channel.recv().fuse() => {
                        if let Ok(ctl) = msg {

                            match ctl {
                                Ctl::Connect => {
                                    let listener_id = this.inner.client.register_new_listener(ChannelConnection::new(
                                        "kaspapy-wrpc-client-python",
                                        this.inner.notification_channel.sender.clone(),
                                        ChannelType::Persistent,
                                    ));
                                    *this.inner.listener_id.lock().unwrap() = Some(listener_id);
                                }
                                Ctl::Disconnect => {
                                    let listener_id = this.inner.listener_id.lock().unwrap().take();
                                    if let Some(listener_id) = listener_id {
                                        if let Err(err) = this.inner.client.unregister_listener(listener_id).await {
                                            log_error!("Error in unregister_listener: {:?}",err);
                                        }
                                    }
                                }
                            }

                            let event = NotificationEvent::RpcCtl(ctl);
                            if let Some(handlers) = this.inner.notification_callbacks(event) {
                                for handler in handlers.into_iter() {
                                    Python::with_gil(|py| {
                                        let event = PyDict::new_bound(py);
                                        event.set_item("type", ctl.to_string()).unwrap();
                                        // objectdict.set_item("rpc", ).unwrap(); TODO

                                        handler.execute(py, event).unwrap();
                                    });
                                }
                            }
                        }
                    },
                    msg = notification_receiver.recv().fuse() => {
                        if let Ok(notification) = &msg {
                            match &notification {
                                kaspa_rpc_core::Notification::UtxosChanged(utxos_changed_notification) => {
                                    let event_type = notification.event_type();
                                    let notification_event = NotificationEvent::Notification(event_type);
                                    if let Some(handlers) = this.inner.notification_callbacks(notification_event) {
                                        let UtxosChangedNotification { added, removed } = utxos_changed_notification;

                                        for handler in handlers.into_iter() {
                                            Python::with_gil(|py| {
                                                let added = serde_pyobject::to_pyobject(py, added).unwrap();
                                                let removed = serde_pyobject::to_pyobject(py, removed).unwrap();

                                                let event = PyDict::new_bound(py);
                                                event.set_item("type", event_type.to_string()).unwrap();
                                                event.set_item("added", &added.to_object(py)).unwrap();
                                                event.set_item("removed", &removed.to_object(py)).unwrap();

                                                handler.execute(py, event).unwrap();
                                            })
                                        }
                                    }
                                },
                                _ => {
                                    let event_type = notification.event_type();
                                    let notification_event = NotificationEvent::Notification(event_type);
                                    if let Some(handlers) = this.inner.notification_callbacks(notification_event) {
                                        for handler in handlers.into_iter() {
                                            Python::with_gil(|py| {
                                                let event = PyDict::new_bound(py);
                                                event.set_item("type", event_type.to_string()).unwrap();
                                                event.set_item("data", &notification.to_pyobject(py).unwrap()).unwrap();

                                                handler.execute(py, event).unwrap();
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ = ctl_receiver.recv().fuse() => {
                        break;
                    },

                }
            }

            if let Some(listener_id) = this.listener_id() {
                this.inner.listener_id.lock().unwrap().take();
                if let Err(err) = this.inner.client.unregister_listener(listener_id).await {
                    log_error!("Error in unregister_listener: {:?}", err);
                }
            }

            ctl_sender.send(()).await.ok();

            Python::with_gil(|_| Ok(()))
        });

        Ok(())
    }
}

#[pymethods]
impl RpcClient {
    fn subscribe_utxos_changed(&self, py: Python, addresses: Vec<Address>) -> PyResult<Py<PyAny>> {
        if let Some(listener_id) = self.listener_id() {
            let client = self.inner.client.clone();
            py_async! {py, async move {
                client.start_notify(listener_id, Scope::UtxosChanged(UtxosChangedScope { addresses })).await?;
                Ok(())
            }}
        } else {
            Err(PyErr::new::<PyException, _>("RPC subscribe on a closed connection"))
        }
    }

    fn unsubscribe_utxos_changed(&self, py: Python, addresses: Vec<Address>) -> PyResult<Py<PyAny>> {
        if let Some(listener_id) = self.listener_id() {
            let client = self.inner.client.clone();
            py_async! {py, async move {
                client.stop_notify(listener_id, Scope::UtxosChanged(UtxosChangedScope { addresses })).await?;
                Ok(())
            }}
        } else {
            Err(PyErr::new::<PyException, _>("RPC unsubscribe on a closed connection"))
        }
    }

    fn subscribe_virtual_chain_changed(&self, py: Python, include_accepted_transaction_ids: bool) -> PyResult<Py<PyAny>> {
        if let Some(listener_id) = self.listener_id() {
            let client = self.inner.client.clone();
            py_async! {py, async move {
                client.start_notify(listener_id, Scope::VirtualChainChanged(VirtualChainChangedScope { include_accepted_transaction_ids })).await?;
                Ok(())
            }}
        } else {
            Err(PyErr::new::<PyException, _>("RPC subscribe on a closed connection"))
        }
    }

    fn unsubscribe_virtual_chain_changed(&self, py: Python, include_accepted_transaction_ids: bool) -> PyResult<Py<PyAny>> {
        if let Some(listener_id) = self.listener_id() {
            let client = self.inner.client.clone();
            py_async! {py, async move {
                client.stop_notify(listener_id, Scope::VirtualChainChanged(VirtualChainChangedScope { include_accepted_transaction_ids })).await?;
                Ok(())
            }}
        } else {
            Err(PyErr::new::<PyException, _>("RPC unsubscribe on a closed connection"))
        }
    }
}

#[pymethods]
impl RpcClient {
    fn is_connected_test(&self) -> bool {
        self.inner.client.is_connected()
    }
}

build_wrpc_python_subscriptions!([
    // UtxosChanged - added above due to parameter `addresses: Vec<Address>``
    // VirtualChainChanged - added above due to paramter `include_accepted_transaction_ids: bool`
    BlockAdded,
    FinalityConflict,
    FinalityConflictResolved,
    NewBlockTemplate,
    PruningPointUtxoSetOverride,
    SinkBlueScoreChanged,
    VirtualDaaScoreChanged,
]);

build_wrpc_python_interface!([
    AddPeer,
    Ban,
    EstimateNetworkHashesPerSecond,
    GetBalanceByAddress,
    GetBalancesByAddresses,
    GetBlock,
    GetBlockCount,
    GetBlockDagInfo,
    GetBlocks,
    GetBlockTemplate,
    GetCoinSupply,
    GetConnectedPeerInfo,
    GetDaaScoreTimestampEstimate,
    GetServerInfo,
    GetCurrentNetwork,
    GetHeaders,
    GetInfo,
    GetMempoolEntries,
    GetMempoolEntriesByAddresses,
    GetMempoolEntry,
    GetPeerAddresses,
    GetMetrics,
    GetSink,
    GetSyncStatus,
    GetSubnetwork,
    GetUtxosByAddresses,
    GetSinkBlueScore,
    GetVirtualChainFromBlock,
    Ping,
    ResolveFinalityConflict,
    Shutdown,
    SubmitBlock,
    SubmitTransaction,
    Unban,
]);
