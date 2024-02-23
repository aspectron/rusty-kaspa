use crate::imports::{BorshDeserialize, BorshSchema, BorshSerialize, Deserialize, Result, Serialize};
use ahash::HashMap;
use ahash::HashMapExt;
use kaspa_consensus_core::subnets::SubnetworkId;
use kaspa_consensus_core::tx::{
    SignableTransaction, Transaction, TransactionId, TransactionInput, TransactionMass, TransactionOutpoint, TransactionOutput,
    UtxoEntry, VerifiableTransaction,
};
use kaspa_utils::hex::ToHex;
use kaspa_utils::serde_bytes;

use crate::tx::pskt::{params::*, utils::*};

pub type PartiallySignedTransactionIndexType = u32;

// /// Represents a Kaspa transaction outpoint
// #[derive(Eq, Hash, PartialEq, Debug, Copy, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, BorshSchema)]
// #[serde(rename_all = "camelCase")]
// pub struct PartiallySignedTransactionOutpoint {
//     #[serde(with = "serde_bytes_fixed_ref")]
//     pub transaction_id: TransactionId,
//     pub index: PartiallySignedTransactionIndexType,
// }

// impl PartiallySignedTransactionOutpoint {
//     pub fn new(transaction_id: TransactionId, index: u32) -> Self {
//         Self { transaction_id, index }
//     }
// }

// impl Display for PartiallySignedTransactionOutpoint {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "({}, {})", self.transaction_id, self.index)
//     }
// }

// impl From<TransactionOutpoint> for PartiallySignedTransactionOutpoint{
//     fn from(value: TransactionOutpoint) -> Self {

//     }
// }

/// Represents a Kaspa transaction input
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartiallySignedTransactionInput {
    //pub previous_outpoint: PartiallySignedTransactionOutpoint,
    pub transaction_id: TransactionId,
    pub index: PartiallySignedTransactionIndexType,
    #[serde(with = "serde_bytes")]
    pub signature_script: Vec<u8>, // TODO: Consider using SmallVec
    pub sequence: u64,

    // TODO: Since this field is used for calculating mass context free, and we already commit
    // to the mass in a dedicated field (on the tx level), it follows that this field is no longer
    // needed, and can be removed if we ever implement a v2 transaction
    pub sig_op_count: u8,

    pub utxo: UtxoEntry,
}

impl PartiallySignedTransactionInput {
    pub fn new(
        transaction_id: TransactionId,
        index: PartiallySignedTransactionIndexType,
        signature_script: Vec<u8>,
        sequence: u64,
        sig_op_count: u8,
        utxo: UtxoEntry,
    ) -> Self {
        Self { transaction_id, index, signature_script, sequence, sig_op_count, utxo }
    }
}

impl From<(&TransactionInput, &UtxoEntry)> for PartiallySignedTransactionInput {
    fn from(value: (&TransactionInput, &UtxoEntry)) -> Self {
        Self {
            transaction_id: value.0.previous_outpoint.transaction_id,
            index: value.0.previous_outpoint.index,
            signature_script: value.0.signature_script.clone(),
            sequence: value.0.sequence,
            sig_op_count: value.0.sig_op_count,
            utxo: value.1.clone(),
        }
    }
}

impl From<PartiallySignedTransactionInput> for TransactionInput {
    fn from(input: PartiallySignedTransactionInput) -> Self {
        let PartiallySignedTransactionInput { transaction_id, index, signature_script, sequence, sig_op_count, .. } = input;
        Self { previous_outpoint: TransactionOutpoint::new(transaction_id, index), signature_script, sequence, sig_op_count }
    }
}

impl std::fmt::Debug for PartiallySignedTransactionInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartiallySignedTransactionInput")
            .field("transaction_id", &self.transaction_id)
            .field("index", &self.index)
            .field("signature_script", &self.signature_script.to_hex())
            .field("sequence", &self.sequence)
            .field("sig_op_count", &self.sig_op_count)
            .finish()
    }
}

// /// Represents a Kaspad transaction output
// #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct PartiallySignedTransactionOutput {
//     pub value: u64,
//     pub script_public_key: ScriptPublicKey,
// }

// impl PartiallySignedTransactionOutput {
//     pub fn new(value: u64, script_public_key: ScriptPublicKey) -> Self {
//         Self { value, script_public_key }
//     }
// }

// impl From<TransactionOutput> for PartiallySignedTransactionOutput{
//     fn from(output: TransactionOutput) -> Self {
//         Self{
//             value: output.value,
//             script_public_key: output.script_public_key
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartiallySignedTransaction {
    version: u32,
    inputs: Vec<PartiallySignedTransactionInput>,
    outputs: Vec<TransactionOutput>,
    unknown_fields: HashMap<Vec<u8>, Vec<u8>>,

    tx_id: TransactionId,
    tx_version: u16,
    subnetwork_id: SubnetworkId,
    lock_time: u64,
    gas: u64,
    payload: Vec<u8>,
    mass: TransactionMass,
}

impl PartiallySignedTransaction {
    pub fn new(
        tx_version: u16,
        inputs: Vec<PartiallySignedTransactionInput>,
        outputs: Vec<TransactionOutput>,
        lock_time: u64,
        subnetwork_id: SubnetworkId,
        gas: u64,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            version: 2,
            tx_version,
            subnetwork_id,
            lock_time,
            gas,
            payload,
            inputs,
            outputs,
            unknown_fields: HashMap::new(),
            mass: Default::default(),
            tx_id: Default::default(),
        }
    }

    pub fn from_signable_transaction(tx: &SignableTransaction) -> Result<Self> {
        let verifiable_tx = tx.as_verifiable();
        let mut inputs = vec![];
        let transaction = tx.as_ref();
        for index in 0..transaction.inputs.len() {
            inputs.push(verifiable_tx.populated_input(index).into());
        }

        let outputs = transaction.outputs.clone();

        Ok(Self {
            version: 2,
            inputs,
            outputs,
            tx_version: transaction.version,
            lock_time: transaction.lock_time,
            subnetwork_id: transaction.subnetwork_id.clone(),
            gas: transaction.gas,
            payload: transaction.payload.clone(),
            tx_id: transaction.id(),
            // mass: transaction.mass().into(),
            mass: Default::default(),
            unknown_fields: HashMap::new(),
        })
    }

    pub fn get_version(&self) -> u32 {
        self.version
    }

    pub fn get_unsigned_transaction(&self) -> Option<String> {
        None
    }

    pub fn merge(&mut self, _pskt: &PartiallySignedTransaction) -> bool {
        //TODO:
        true
    }

    pub fn serialize_(&self) -> Vec<u8> {
        let mut v = vec![];

        // magic bytes
        v.extend_from_slice(&PSKT_MAGIC_BYTES);

        if self.get_version() == 0 {
            // unsigned tx flag
            serialize_to_vector(&mut v, &PSKT_GLOBAL_UNSIGNED_TX.compact_size());

            // Write serialized tx
            serialize_to_vector(&mut v, &self.get_unsigned_transaction());
        }
        //TODO
        v
    }
}

impl TryFrom<&SignableTransaction> for PartiallySignedTransaction {
    type Error = crate::error::Error;
    fn try_from(tx: &SignableTransaction) -> std::prelude::v1::Result<Self, Self::Error> {
        Self::from_signable_transaction(tx)
    }
}

impl From<PartiallySignedTransaction> for SignableTransaction {
    fn from(pskt: PartiallySignedTransaction) -> Self {
        let mut entries = vec![];
        let mut inputs = vec![];
        for input in pskt.inputs {
            entries.push(input.utxo.clone());
            inputs.push(input.into());
        }

        let tx = Transaction::new(pskt.tx_version, inputs, pskt.outputs, pskt.lock_time, pskt.subnetwork_id, pskt.gas, pskt.payload);

        Self::with_entries(tx, entries)
    }
}
