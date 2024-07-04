use kaspa_python_macros::py_async;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::model::*;
use kaspa_rpc_macros::build_wrpc_python_interface;
use kaspa_notify::listener::ListenerId;
use kaspa_wrpc_client::{
    client::{ConnectOptions, ConnectStrategy},
    KaspaRpcClient, 
    result::Result,
    WrpcEncoding,
};
use pyo3::{prelude::*, types::PyDict};
use std::{
    sync::{
        atomic::{AtomicBool},
        Arc, Mutex,
    },
    time::Duration,
};
pub use workflow_core::channel::{Channel, DuplexChannel};

pub struct Inner {
    client: Arc<KaspaRpcClient>,
    // resolver TODO
    notification_task: AtomicBool,
    notification_ctl: DuplexChannel,
    // callbacks TODO
    listener_id: Arc<Mutex<Option<ListenerId>>>,
    notification_channel: Channel<kaspa_rpc_core::Notification>,
}

#[pyclass]
pub struct RpcClient {
    inner: Arc<Inner>,
    // url: String,
    // encoding TODO
    // verbose TODO
    // timeout TODO
}

impl RpcClient {
    fn new(url: Option<String>, encoding: Option<WrpcEncoding>) -> Result<RpcClient> {
        let encoding = encoding.unwrap_or(WrpcEncoding::Borsh);

        let client = Arc::new(
            KaspaRpcClient::new(encoding, url.as_deref(), None, None, None)
                .unwrap()
        );

        let rpc_client = RpcClient {
            inner: Arc::new(Inner {
                client,
                notification_task: AtomicBool::new(false),
                notification_ctl: DuplexChannel::oneshot(),
                listener_id: Arc::new(Mutex::new(None)),
                notification_channel: Channel::unbounded()
            })
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

    fn connect(&self, py: Python) -> PyResult<Py<PyAny>> {
        // TODO expose args to Python similar to WASM wRPC Client IConnectOptions

        let options = ConnectOptions {
            block_async_connect: true,
            connect_timeout: Some(Duration::from_millis(5_000)),
            strategy: ConnectStrategy::Fallback,
            ..Default::default()
        };

        let client = self.inner.client.clone();
        py_async! {py, async move {
            let _ = client.connect(Some(options)).await.map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()));
            Ok(())
        }}
    }

    fn is_connected(&self) -> bool {
        self.inner.client.is_connected()
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
}

#[pymethods]
impl RpcClient {
    fn is_connected_test(&self) -> bool {
        self.inner.client.is_connected()
    }
}

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
