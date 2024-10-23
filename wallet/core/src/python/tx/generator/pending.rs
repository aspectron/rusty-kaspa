use crate::imports::*;
use crate::tx::generator as native;
use kaspa_consensus_client::Transaction;
use kaspa_consensus_core::hashing::wasm::SighashType;
use kaspa_python_macros::py_async;
use kaspa_wallet_keys::privatekey::PrivateKey;
use kaspa_wrpc_python::client::RpcClient;

#[pyclass]
pub struct PendingTransaction {
    inner: native::PendingTransaction,
}

#[pymethods]
impl PendingTransaction {
    #[getter]
    fn id(&self) -> String {
        self.inner.id().to_string()
    }

    #[getter]
    #[pyo3(name = "payment_amount")]
    fn payment_value(&self) -> Option<u64> {
        self.inner.payment_value()
    }

    #[getter]
    #[pyo3(name = "change_amount")]
    fn change_value(&self) -> u64 {
        self.inner.change_value()
    }

    #[getter]
    #[pyo3(name = "fee_amount")]
    fn fees(&self) -> u64 {
        self.inner.fees()
    }

    #[getter]
    fn mass(&self) -> u64 {
        self.inner.mass()
    }

    #[getter]
    fn minimum_signatures(&self) -> u16 {
        self.inner.minimum_signatures()
    }

    #[getter]
    #[pyo3(name = "aggregate_input_amount")]
    fn aggregate_input_value(&self) -> u64 {
        self.inner.aggregate_input_value()
    }

    #[getter]
    #[pyo3(name = "aggregate_output_amount")]
    fn aggregate_output_value(&self) -> u64 {
        self.inner.aggregate_output_value()
    }

    #[getter]
    #[pyo3(name = "transaction_type")]
    fn kind(&self) -> String {
        if self.inner.is_batch() {
            "batch".to_string()
        } else {
            "final".to_string()
        }
    }

    fn addresses(&self) -> Vec<Address> {
        self.inner.addresses().clone()
    }

    fn get_utxo_entries(&self) -> Vec<UtxoEntryReference> {
        self.inner.utxo_entries().values().map(|utxo_entry| UtxoEntryReference::from(utxo_entry.clone())).collect()
    }

    fn create_input_signature(&self, input_index: u8, private_key: &PrivateKey, sighash_type: Option<&SighashType>) -> Result<String> {
        let signature = self.inner.create_input_signature(
            input_index.into(),
            &private_key.secret_bytes(),
            sighash_type.cloned().unwrap_or(SighashType::All).into(),
        )?;

        Ok(signature.to_hex())
    }

    fn fill_input(&self, input_index: u8, signature_script: String) -> Result<()> {
        // TODO use PyBinary for signature_script
        let mut bytes = vec![0u8; signature_script.len() / 2];
        faster_hex::hex_decode(signature_script.as_bytes(), &mut bytes).unwrap();
        self.inner.fill_input(input_index.into(), bytes)?;

        Ok(())
    }

    fn sign_input(&self, input_index: u8, private_key: &PrivateKey, sighash_type: Option<&SighashType>) -> Result<()> {
        self.inner.sign_input(
            input_index.into(),
            &private_key.secret_bytes(),
            sighash_type.cloned().unwrap_or(SighashType::All).into(),
        )?;

        Ok(())
    }

    fn sign(&self, private_keys: Vec<PrivateKey>, check_fully_signed: Option<bool>) -> Result<()> {
        let mut keys = private_keys.iter().map(|key| key.secret_bytes()).collect::<Vec<_>>();
        self.inner.try_sign_with_keys(&keys, check_fully_signed)?;
        keys.zeroize();
        Ok(())
    }

    fn submit(&self, py: Python, rpc_client: &RpcClient) -> PyResult<Py<PyAny>> {
        let inner = self.inner.clone();
        let rpc: Arc<DynRpcApi> = rpc_client.client().clone();

        py_async! {py, async move {
            let txid = inner.try_submit(&rpc).await?;
            Ok(txid.to_string())
        }}
    }

    #[getter]
    fn transaction(&self) -> Result<Transaction> {
        Ok(Transaction::from_cctx_transaction(&self.inner.transaction(), self.inner.utxo_entries()))
    }
}

impl From<native::PendingTransaction> for PendingTransaction {
    fn from(pending_transaction: native::PendingTransaction) -> Self {
        Self { inner: pending_transaction }
    }
}
