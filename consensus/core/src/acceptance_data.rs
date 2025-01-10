use crate::tx::TransactionId;
use consensus_core::tx::Transaction;
use kaspa_hashes::Hash;
use serde::{Deserialize, Serialize};

pub type AcceptanceData = Vec<MergesetBlockAcceptanceData>;

pub type AcceptanceDataWithTx = Vec<MergesetBlockAcceptanceDataWithTx>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergesetBlockAcceptanceData {
    pub block_hash: Hash,
    pub accepted_transactions: Vec<AcceptedTxEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergesetBlockAcceptanceDataWithTx {
    pub block_hash: Hash,
    pub block_timestamp: u64,
    pub accepted_transactions: Vec<TransactionWithFee>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionWithFee {
    pub tx: Transaction,
    pub fee: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedTxEntry {
    pub transaction_id: TransactionId,
    pub index_within_block: u32,
}
