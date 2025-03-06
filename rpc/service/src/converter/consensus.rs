use async_trait::async_trait;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionOutpoint;
use kaspa_consensus_core::{
    acceptance_data::MergesetBlockAcceptanceData,
    block::Block,
    config::Config,
    hashing::tx::hash,
    header::Header,
    tx::{MutableTransaction, Transaction, TransactionId, TransactionInput, TransactionOutput},
    ChainPath,
};
use kaspa_consensus_notify::notification::{self as consensus_notify, Notification as ConsensusNotification};
use kaspa_consensusmanager::{ConsensusManager, ConsensusProxy};
use kaspa_math::Uint256;
use kaspa_mining::model::{owner_txs::OwnerTransactions, TransactionIdSet};
use kaspa_notify::converter::Converter;
use kaspa_rpc_core::{
    BlockAddedNotification, Notification, RpcAcceptanceData, RpcBlock, RpcBlockVerboseData, RpcError, RpcHash, RpcMempoolEntry,
    RpcMempoolEntryByAddress, RpcMergesetBlockAcceptanceData, RpcResult, RpcTransaction, RpcTransactionInput, RpcTransactionOutput,
    RpcTransactionOutputVerboseData, RpcTransactionVerboseData,
};
use kaspa_txscript::{extract_script_pub_key_address, script_class::ScriptClass};
use std::collections::BTreeMap;
use std::{collections::HashMap, fmt::Debug, sync::Arc};

/// Conversion of consensus_core to rpc_core structures
pub struct ConsensusConverter {
    consensus_manager: Arc<ConsensusManager>,
    config: Arc<Config>,
}

impl ConsensusConverter {
    pub fn new(consensus_manager: Arc<ConsensusManager>, config: Arc<Config>) -> Self {
        Self { consensus_manager, config }
    }

    /// Returns the proof-of-work difficulty as a multiple of the minimum difficulty using
    /// the passed bits field from the header of a block.
    pub fn get_difficulty_ratio(&self, bits: u32) -> f64 {
        // The minimum difficulty is the max possible proof-of-work limit bits
        // converted back to a number. Note this is not the same as the proof of
        // work limit directly because the block difficulty is encoded in a block
        // with the compact form which loses precision.
        let target = Uint256::from_compact_target_bits(bits);
        self.config.max_difficulty_target_f64 / target.as_f64()
    }

    /// Converts a consensus [`Block`] into an [`RpcBlock`], optionally including transaction verbose data.
    ///
    /// _GO-KASPAD: PopulateBlockWithVerboseData_
    pub async fn get_block(
        &self,
        consensus: &ConsensusProxy,
        block: &Block,
        include_transactions: bool,
        include_transaction_verbose_data: bool,
    ) -> RpcResult<RpcBlock> {
        let hash = block.hash();
        let ghostdag_data = consensus.async_get_ghostdag_data(hash).await?;
        let block_status = consensus.async_get_block_status(hash).await.unwrap();
        let children = consensus.async_get_block_children(hash).await.unwrap_or_default();
        let is_chain_block = consensus.async_is_chain_block(hash).await?;
        let verbose_data = Some(RpcBlockVerboseData {
            hash,
            difficulty: self.get_difficulty_ratio(block.header.bits),
            selected_parent_hash: ghostdag_data.selected_parent,
            transaction_ids: block.transactions.iter().map(|x| x.id()).collect(),
            is_header_only: block_status.is_header_only(),
            blue_score: ghostdag_data.blue_score,
            children_hashes: children,
            merge_set_blues_hashes: ghostdag_data.mergeset_blues,
            merge_set_reds_hashes: ghostdag_data.mergeset_reds,
            is_chain_block,
        });

        let transactions = if include_transactions {
            block
                .transactions
                .iter()
                .map(|x| self.get_transaction(consensus, x, Some(&block.header), include_transaction_verbose_data))
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        Ok(RpcBlock { header: block.header.as_ref().into(), transactions, verbose_data })
    }

    pub fn get_mempool_entry(&self, consensus: &ConsensusProxy, transaction: &MutableTransaction) -> RpcMempoolEntry {
        let is_orphan = !transaction.is_fully_populated();
        let rpc_transaction = self.get_transaction(consensus, &transaction.tx, None, true);
        RpcMempoolEntry::new(transaction.calculated_fee.unwrap_or_default(), rpc_transaction, is_orphan)
    }

    pub fn get_mempool_entries_by_address(
        &self,
        consensus: &ConsensusProxy,
        address: Address,
        owner_transactions: &OwnerTransactions,
        transactions: &HashMap<TransactionId, MutableTransaction>,
    ) -> RpcMempoolEntryByAddress {
        let sending = self.get_owner_entries(consensus, &owner_transactions.sending_txs, transactions);
        let receiving = self.get_owner_entries(consensus, &owner_transactions.receiving_txs, transactions);
        RpcMempoolEntryByAddress::new(address, sending, receiving)
    }

    pub fn get_owner_entries(
        &self,
        consensus: &ConsensusProxy,
        transaction_ids: &TransactionIdSet,
        transactions: &HashMap<TransactionId, MutableTransaction>,
    ) -> Vec<RpcMempoolEntry> {
        transaction_ids.iter().map(|x| self.get_mempool_entry(consensus, transactions.get(x).expect("transaction exists"))).collect()
    }

    /// Converts a consensus [`Transaction`] into an [`RpcTransaction`], optionally including verbose data.
    ///
    /// _GO-KASPAD: PopulateTransactionWithVerboseData
    pub fn get_transaction(
        &self,
        consensus: &ConsensusProxy,
        transaction: &Transaction,
        header: Option<&Header>,
        include_verbose_data: bool,
    ) -> RpcTransaction {
        if include_verbose_data {
            let verbose_data = Some(RpcTransactionVerboseData {
                transaction_id: transaction.id(),
                hash: hash(transaction, false),
                compute_mass: consensus.calculate_transaction_non_contextual_masses(transaction).compute_mass,
                // TODO: make block_hash an option
                block_hash: header.map_or_else(RpcHash::default, |x| x.hash),
                block_time: header.map_or(0, |x| x.timestamp),
            });
            RpcTransaction {
                version: transaction.version,
                inputs: transaction.inputs.iter().map(|x| self.get_transaction_input(x)).collect(),
                outputs: transaction.outputs.iter().map(|x| self.get_transaction_output(x)).collect(),
                lock_time: transaction.lock_time,
                subnetwork_id: transaction.subnetwork_id.clone(),
                gas: transaction.gas,
                payload: transaction.payload.clone(),
                mass: transaction.mass(),
                verbose_data,
                fee: 0,
            }
        } else {
            transaction.into()
        }
    }

    fn get_transaction_input(&self, input: &TransactionInput) -> RpcTransactionInput {
        input.into()
    }

    fn get_transaction_output(&self, output: &TransactionOutput) -> RpcTransactionOutput {
        let script_public_key_type = ScriptClass::from_script(&output.script_public_key);
        let address = extract_script_pub_key_address(&output.script_public_key, self.config.prefix()).ok();
        let verbose_data =
            address.map(|address| RpcTransactionOutputVerboseData { script_public_key_type, script_public_key_address: address });
        RpcTransactionOutput { value: output.value, script_public_key: output.script_public_key.clone(), verbose_data }
    }

    pub async fn get_virtual_chain_accepted_transaction_ids(
        &self,
        consensus: &ConsensusProxy,
        chain_path: &ChainPath,
        merged_blocks_limit: Option<usize>,
    ) -> RpcResult<Vec<RpcAcceptanceData>> {
        let acceptance_data = consensus.async_get_blocks_acceptance_data(chain_path.added.clone(), merged_blocks_limit).await?;
        let mut r = Vec::with_capacity(acceptance_data.len());
        for (mergeset_data, accepting_block) in acceptance_data.into_iter().zip(chain_path.added.iter()) {
            let compact_header_data = consensus.async_get_compact_header_data(*accepting_block).await?;
            // Expected to never fail, since we found the acceptance data and therefore there must be matching diff
            let mut tx_id_input_to_outpoint: BTreeMap<(TransactionId, u32), (TransactionOutpoint, Option<&u64>)> = BTreeMap::new();
            let mut outpoint_to_value = BTreeMap::new();
            let mut acceptance_data = Vec::with_capacity(mergeset_data.len());
            for MergesetBlockAcceptanceData { block_hash, accepted_transactions: accepted_transaction_entries } in mergeset_data.iter()
            {
                let block = consensus.async_get_block_even_if_header_only(*block_hash).await?;
                let block_timestamp = block.header.timestamp;
                let block_txs = block.transactions;
                let mut accepted_transactions = Vec::new();
                block_txs
                    .iter()
                    .enumerate()
                    .filter(|(idx, _tx)| {
                        accepted_transaction_entries.iter().any(|accepted| accepted.index_within_block as usize == *idx)
                    })
                    .map(|(_, tx)| tx)
                    .for_each(|tx| {
                        tx.inputs.iter().enumerate().for_each(|(index, input)| {
                            tx_id_input_to_outpoint.insert((tx.id(), index as u32), (input.previous_outpoint, None));
                        });
                        tx.outputs.iter().enumerate().for_each(|(index, out)| {
                            outpoint_to_value.insert(TransactionOutpoint { transaction_id: tx.id(), index: index as u32 }, out.value);
                        });
                        accepted_transactions.push(RpcTransaction::from(tx));
                    });

                acceptance_data.push(RpcMergesetBlockAcceptanceData {
                    merged_block_hash: *block_hash,
                    merged_block_timestamp: block_timestamp,
                    accepted_transactions,
                })
            }
            let mut outpoints_requested_from_unto_diff = vec![];
            tx_id_input_to_outpoint.values_mut().for_each(|(k, v)| {
                *v = outpoint_to_value.get(k);
                if v.is_none() {
                    outpoints_requested_from_unto_diff.push(*k);
                }
            });
            let outpoints_requested_from_unto_diff = Arc::new(outpoints_requested_from_unto_diff);
            let amounts = consensus
                .async_get_utxo_amounts(*accepting_block, outpoints_requested_from_unto_diff.clone())
                .await
                .map_err(RpcError::UtxoReturnAddressNotFound)?;
            outpoints_requested_from_unto_diff.iter().zip(amounts).for_each(|(outpoint, amount)| {
                outpoint_to_value.insert(*outpoint, amount);
            });
            acceptance_data.iter_mut().for_each(|d| {
                d.accepted_transactions.iter_mut().for_each(|rtx| {
                    let tx = Transaction::try_from(rtx.clone()).unwrap(); // lol
                    let input_sum: u64 = tx
                        .inputs
                        .iter()
                        .map(|input| outpoint_to_value.get(&input.previous_outpoint).cloned().unwrap_or_default())
                        .sum();
                    let output_sum: u64 = tx.outputs.iter().map(|o| o.value).sum();
                    rtx.fee = input_sum.saturating_sub(output_sum);
                })
            });
            r.push(RpcAcceptanceData {
                accepting_blue_score: compact_header_data.blue_score,
                accepting_daa_score: compact_header_data.daa_score,
                mergeset_block_acceptance_data: acceptance_data,
            })
        }
        Ok(r)
        // Ok(acceptance_data
        //     .iter()
        //     .zip(acceptance_daa_scores)
        //     .map(|(block_data, accepting_blue_score)| RpcAcceptanceData {
        //         accepting_blue_score,
        //         mergeset_block_acceptance_data: block_data
        //             .iter()
        //             .map(|MergesetBlockAcceptanceData { block_hash, accepted_transactions }| {
        //                 let mut accepted_transactions = accepted_transactions.clone();
        //                 accepted_transactions.sort_unstable_by_key(|entry| entry.index_within_block);
        //
        //                 // todo filter block transactions based on it
        //                 RpcMergesetBlockAcceptanceData {
        //                     merged_block_hash: *block_hash,
        //                     accepted_transactions: accepted_transactions.into_iter().map(|v| v.transaction_id).collect(),
        //                 }
        //             })
        //             .collect(),
        //     })
        //     .collect())
    }
}

#[async_trait]
impl Converter for ConsensusConverter {
    type Incoming = ConsensusNotification;
    type Outgoing = Notification;

    async fn convert(&self, incoming: ConsensusNotification) -> Notification {
        match incoming {
            consensus_notify::Notification::BlockAdded(msg) => {
                let session = self.consensus_manager.consensus().unguarded_session();
                // If get_block fails, rely on the infallible From implementation which will lack verbose data
                let block = Arc::new(self.get_block(&session, &msg.block, true, true).await.unwrap_or_else(|_| (&msg.block).into()));
                Notification::BlockAdded(BlockAddedNotification { block })
            }
            _ => (&incoming).into(),
        }
    }
}

impl Debug for ConsensusConverter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsensusConverter").field("consensus_manager", &"").field("config", &self.config).finish()
    }
}
