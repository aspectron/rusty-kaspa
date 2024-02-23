use crate::imports::*;
use crate::utils::script_hashes;
use kaspa_consensus_client::{Transaction, TransactionInput, TransactionOutput, UtxoEntries, UtxoEntry};
use kaspa_consensus_core::tx::{self, VerifiableTransaction};
use serde_wasm_bindgen::to_value;
//use std::str::FromStr;

/// Represents a generic mutable transaction
/// @category Consensus
#[derive(Clone, Debug, Serialize, Deserialize)]
#[wasm_bindgen(inspectable)]
pub struct SignableTransaction {
    tx: Arc<Mutex<Transaction>>,
    /// UTXO entry data
    #[wasm_bindgen(getter_with_clone)]
    pub entries: UtxoEntries,
}

#[wasm_bindgen]
impl SignableTransaction {
    #[wasm_bindgen(constructor)]
    pub fn new_from_refs(tx: &Transaction, entries: &UtxoEntries) -> Self {
        Self { tx: Arc::new(Mutex::new(tx.clone())), entries: entries.clone() }
    }

    #[wasm_bindgen(getter=tx)]
    pub fn tx_getter(&self) -> Transaction {
        self.tx.lock().unwrap().clone()
    }

    // /// Serialize transaction as json string
    // /// @see {@link SignableTransaction.deserialize}
    // #[wasm_bindgen(js_name=serialize)]
    // pub fn serialize_json(&self) -> Result<String, JsError> {
    //     //Ok(SerializableTransaction::try_from(self)?.serialize(serde_json::value::Serializer)?.to_string())
    //     Ok("TODO::".to_string())
    // }

    // /// Deserialize transaction from json string
    // /// @see {@link SignableTransaction.serialize}
    // #[wasm_bindgen(js_name=deserialize)]
    // pub fn deserialize_json(_json: &str) -> Result<SignableTransaction, JsError> {
    //     let pskt: PartiallySignedTransaction = serde_json::from_value(serde_json::Value::from_str(json)?)?;
    //     // Ok((&pskt).try_into()?)
    //     Err(JsError::new("TODO ERROR :"))
    // }

    #[wasm_bindgen(js_name=getScriptHashes)]
    pub fn script_hashes(&self) -> Result<JsValue, JsError> {
        let hashes = script_hashes(self.clone().into())?;
        Ok(to_value(&hashes)?)
    }

    #[wasm_bindgen(js_name=setSignatures)]
    pub fn set_signatures(&self, signatures: js_sys::Array) -> Result<JsValue, JsError> {
        let signatures =
            signatures.iter().map(|s| s.try_as_vec_u8()).collect::<Result<Vec<Vec<u8>>, workflow_wasm::error::Error>>()?;

        {
            let mut locked = self.tx.lock();
            let tx = locked.as_mut().unwrap();

            if signatures.len() != tx.inner().inputs.len() {
                return Err(Error::Custom("Signature counts don't match input counts".to_string()).into());
            }
            let len = tx.inner().inputs.len();
            for (i, signature) in signatures.into_iter().enumerate().take(len) {
                tx.inner().inputs[i].inner().sig_op_count = 1;
                tx.inner().inputs[i].inner().signature_script = signature;
            }
        }

        Ok(to_value(self)?)
    }

    #[wasm_bindgen(getter=inputs)]
    pub fn get_inputs(&self) -> Result<js_sys::Array, JsError> {
        let inputs = self.tx.lock()?.get_inputs_as_js_array();
        Ok(inputs)
    }

    #[wasm_bindgen(getter=outputs)]
    pub fn get_outputs(&self) -> Result<js_sys::Array, JsError> {
        let outputs = self.tx.lock()?.get_outputs_as_js_array();
        Ok(outputs)
    }

    // TODO (aspect) - discuss - either remove this or make it utilize wasm MassCalculator (address this as a part of MassCalculator refactoring).
    // pub fn mass(&self, network_type: NetworkType, estimate_signature_mass: bool, minimum_signatures: u16) -> Result<u64, JsError> {
    //     let params = get_consensus_params_by_network(&network_type);
    //     let calc = MassCalculator::new(params);
    //     calc.calc_mass_for_tx(tx)
    //     Ok(calculate_mass(&self.tx(), &params, estimate_signature_mass, minimum_signatures))
    // }
}

impl SignableTransaction {
    pub fn new(tx: Transaction, entries: UtxoEntries) -> Self {
        Self { tx: Arc::new(Mutex::new(tx)), entries }
    }

    pub fn id(&self) -> TransactionId {
        self.tx.lock().unwrap().id()
    }

    pub fn tx(&self) -> MutexGuard<'_, Transaction> {
        self.tx.lock().unwrap()
    }
    pub fn inputs(&self) -> Result<Vec<TransactionInput>, crate::error::Error> {
        Ok(self.tx.lock().unwrap().inner().inputs.clone())
    }

    pub fn outputs(&self) -> Result<Vec<TransactionOutput>, crate::error::Error> {
        Ok(self.tx.lock().unwrap().inner().outputs.clone())
    }

    pub fn total_input_amount(&self) -> Result<u64, crate::error::Error> {
        let amount = self.entries.items().iter().map(|entry| entry.amount()).sum();
        Ok(amount)
    }

    pub fn total_output_amount(&self) -> Result<u64, crate::error::Error> {
        let amount = self.outputs()?.iter().map(|output| output.get_value()).sum();
        Ok(amount)
    }
}

impl From<SignableTransaction> for tx::SignableTransaction {
    fn from(mtx: SignableTransaction) -> Self {
        let tx = &mtx.tx.lock().unwrap().clone();
        Self { tx: tx.into(), entries: mtx.entries.into(), calculated_fee: None, calculated_compute_mass: None }
    }
}

impl TryFrom<(tx::SignableTransaction, UtxoEntries)> for SignableTransaction {
    type Error = Error;
    fn try_from(value: (tx::SignableTransaction, UtxoEntries)) -> Result<Self, Self::Error> {
        Ok(Self { tx: Arc::new(Mutex::new(value.0.tx.into())), entries: value.1 })
    }
}

impl TryFrom<tx::SignableTransaction> for SignableTransaction {
    type Error = Error;
    fn try_from(tx: tx::SignableTransaction) -> Result<Self, Self::Error> {
        let mut entries = vec![];
        let verifiable_tx = tx.as_verifiable();
        let transaction = tx.as_ref();
        for index in 0..transaction.inputs.len() {
            let (input, entry) = verifiable_tx.populated_input(index);
            entries.push(UtxoEntry { address: None, outpoint: input.previous_outpoint.into(), entry: entry.clone() });
        }

        Ok(Self { tx: Arc::new(Mutex::new(tx.tx.clone().into())), entries: entries.into() })
    }
}

impl From<SignableTransaction> for Transaction {
    fn from(mtx: SignableTransaction) -> Self {
        mtx.tx.lock().unwrap().clone()
    }
}

impl TryFrom<JsValue> for SignableTransaction {
    type Error = Error;
    fn try_from(js_value: JsValue) -> Result<Self, Self::Error> {
        SignableTransaction::try_from(&js_value)
    }
}

impl TryFrom<&JsValue> for SignableTransaction {
    type Error = Error;
    fn try_from(js_value: &JsValue) -> Result<Self, Self::Error> {
        Ok(ref_from_abi!(SignableTransaction, js_value)?)
    }
}
