use kaspa_python_macros::py_async;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::model::*;
use kaspa_rpc_macros::build_wrpc_python_interface;
use kaspa_wrpc_client::{
    client::{ConnectOptions, ConnectStrategy},
    KaspaRpcClient, WrpcEncoding,
};
use pyo3::{
    exceptions::PyValueError,
    prelude::*, types::PyDict,
};
use std::time::Duration;

#[pyclass]
pub struct RpcClient {
    client: KaspaRpcClient,
    // url: String,
    // encoding: Option<WrpcEncoding>,
    // verbose : Option<bool>,
    // timeout: Option<u64>,
}

#[pymethods]
impl RpcClient {
    #[staticmethod]
    fn connect(py: Python, url: Option<String>) -> PyResult<Bound<PyAny>> {
        let client = KaspaRpcClient::new(WrpcEncoding::Borsh, url.as_deref(), None, None, None)?;

        let options = ConnectOptions {
            block_async_connect: true,
            connect_timeout: Some(Duration::from_millis(5_000)),
            strategy: ConnectStrategy::Fallback,
            ..Default::default()
        };

        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            client.connect(Some(options)).await.map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))?;

            Python::with_gil(|py| {
                Py::new(py, RpcClient { client })
                    .map(|py_rpc_client| py_rpc_client.into_py(py))
                    .map_err(|e| pyo3::exceptions::PyException::new_err(e.to_string()))
            })
        })
    }

    fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    fn get_server_info(&self, py: Python) -> PyResult<Py<PyAny>> {
        let client = self.client.clone();
        py_async! {py, async move {
            let response = client.get_server_info_call(GetServerInfoRequest { }).await?;
            Python::with_gil(|py| {
                Ok(serde_pyobject::to_pyobject(py, &response).unwrap().to_object(py))
            })
        }}
    }

    fn get_block_dag_info(&self, py: Python) -> PyResult<Py<PyAny>> {
        let client = self.client.clone();
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
        self.client.is_connected()
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
