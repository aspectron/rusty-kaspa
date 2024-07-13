use kaspa_consensus_core::network::{NetworkId, NetworkType};
use kaspa_python_macros::py_async;
use kaspa_wrpc_client::{Resolver as NativeResolver, WrpcEncoding};
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use std::{str::FromStr, sync::Arc};

use crate::client::RpcClient;

#[derive(Debug, Clone)]
#[pyclass]
pub struct Resolver {
    resolver: NativeResolver,
}

impl Resolver {
    pub fn new(resolver: NativeResolver) -> Self {
        Self { resolver }
    }
}

#[pymethods]
impl Resolver {
    #[new]
    pub fn ctor(urls: Option<Vec<String>>) -> PyResult<Resolver> {
        if let Some(urls) = urls {
            Ok(Self { resolver: NativeResolver::new(urls.into_iter().map(|url| Arc::new(url)).collect::<Vec<_>>()) })
        } else {
            Ok(Self { resolver: NativeResolver::default() })
        }
    }
}

#[pymethods]
impl Resolver {
    fn urls(&self) -> Vec<String> {
        self.resolver.urls().into_iter().map(|url| String::clone(&url)).collect::<Vec<_>>()
    }

    fn get_node(&self, py: Python, encoding: String, network: String, network_suffix: Option<u32>) -> PyResult<Py<PyAny>> {
        let encoding = WrpcEncoding::from_str(encoding.as_str()).unwrap();

        // TODO find better way of accepting NetworkId type from Python
        let network_type = NetworkType::from_str(network.as_str()).unwrap();
        let network_id = NetworkId::try_from(network_type).unwrap_or({
            if network_suffix == None {
                return Err(PyErr::new::<PyException, _>("Network suffix required for this network"));
            };
            NetworkId::with_suffix(network_type, network_suffix.unwrap())
        });

        let resolver = self.resolver.clone();
        py_async! {py, async move {
            resolver.get_node(encoding, network_id).await?;
            Ok(())
        }}
    }

    fn get_url(&self, py: Python, encoding: String, network: String, network_suffix: Option<u32>) -> PyResult<Py<PyAny>> {
        let encoding = WrpcEncoding::from_str(encoding.as_str()).unwrap();

        // TODO find better way of accepting NetworkId type from Python
        let network_type = NetworkType::from_str(network.as_str()).unwrap();
        let network_id = NetworkId::try_from(network_type).unwrap_or({
            if network_suffix == None {
                return Err(PyErr::new::<PyException, _>("Network suffix required for this network"));
            };
            NetworkId::with_suffix(network_type, network_suffix.unwrap())
        });

        let resolver = self.resolver.clone();
        py_async! {py, async move {
            resolver.get_node(encoding, network_id).await?;
            Ok(())
        }}
    }

    // fn connect() TODO
}

impl From<Resolver> for NativeResolver {
    fn from(resolver: Resolver) -> Self {
        resolver.resolver
    }
}
