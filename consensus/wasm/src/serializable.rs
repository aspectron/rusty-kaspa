use crate::imports::*;
use crate::{SignableTransaction, TransactionOutpoint, UtxoEntries, UtxoEntry};
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::Transaction;

#[derive(Clone, Serialize, Deserialize)]
pub struct SerializableTransaction<T: AsRef<Transaction> = std::sync::Arc<Transaction>> {
    /// The inner transaction
    pub tx: T,
    /// Partially filled UTXO entry data
    pub entries: Vec<Option<SerializableUtxoEntry>>,
}
impl<T: AsRef<Transaction>> SerializableTransaction<T> {
    pub fn new(tx: T, entries: Vec<Option<SerializableUtxoEntry>>) -> Self {
        Self { tx, entries }
    }
}

impl TryFrom<&SignableTransaction> for SerializableTransaction<Transaction> {
    type Error = Error;
    fn try_from(signable_tx: &SignableTransaction) -> Result<Self, Self::Error> {
        let tx = Transaction::from(&signable_tx.tx_getter());
        Ok(SerializableTransaction::new(tx, signable_tx.entries.clone().into()))
    }
}

impl<T: AsRef<Transaction>> TryFrom<&SerializableTransaction<T>> for SignableTransaction {
    type Error = Error;
    fn try_from(serializable_tx: &SerializableTransaction<T>) -> Result<Self, Self::Error> {
        let tx = serializable_tx.tx.as_ref().clone();
        Ok(SignableTransaction::new_from_refs(&tx.into(), &serializable_tx.entries.clone().try_into()?))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableUtxoEntry {
    pub address: Option<Address>,
    pub transaction_id: TransactionId,
    pub index: TransactionIndexType,
    pub amount: u64,
    pub script_public_key: ScriptPublicKey,
    pub block_daa_score: u64,
    pub is_coinbase: bool,
}

impl From<UtxoEntry> for SerializableUtxoEntry {
    fn from(value: UtxoEntry) -> Self {
        Self {
            address: value.address,
            transaction_id: value.outpoint.transaction_id(),
            index: value.outpoint.index(),
            amount: value.entry.amount,
            script_public_key: value.entry.script_public_key,
            block_daa_score: value.entry.block_daa_score,
            is_coinbase: value.entry.is_coinbase,
        }
    }
}

impl From<SerializableUtxoEntry> for UtxoEntry {
    fn from(value: SerializableUtxoEntry) -> Self {
        Self {
            address: value.address,
            outpoint: TransactionOutpoint::new(value.transaction_id, value.index),
            entry: cctx::UtxoEntry::new(value.amount, value.script_public_key, value.block_daa_score, value.is_coinbase),
        }
    }
}

impl From<UtxoEntries> for Vec<Option<SerializableUtxoEntry>> {
    fn from(value: UtxoEntries) -> Self {
        value.iter().map(|entry| Some(SerializableUtxoEntry::from(entry.as_ref().clone()))).collect()
    }
}

impl TryFrom<Vec<Option<SerializableUtxoEntry>>> for UtxoEntries {
    type Error = Error;
    fn try_from(value: Vec<Option<SerializableUtxoEntry>>) -> Result<Self, Self::Error> {
        let entries: Vec<Option<UtxoEntry>> = value.iter().map(|entry| entry.clone().map(UtxoEntry::from)).collect();

        entries.try_into()
    }
}
