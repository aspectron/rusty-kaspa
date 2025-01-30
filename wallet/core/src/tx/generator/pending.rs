//!
//! Pending transaction encapsulating a
//! transaction generated by the [`Generator`].
//!
#![allow(unused_imports)]

use crate::imports::*;
use crate::result::Result;
use crate::rpc::DynRpcApi;
use crate::tx::{DataKind, Generator, MAXIMUM_STANDARD_TRANSACTION_MASS};
use crate::utxo::{UtxoContext, UtxoEntryId, UtxoEntryReference, UtxoIterator};
use kaspa_consensus_core::hashing::sighash_type::SigHashType;
use kaspa_consensus_core::sign::{sign_input, sign_with_multiple_v2, Signed};
use kaspa_consensus_core::tx::{SignableTransaction, Transaction, TransactionId, TransactionInput, TransactionOutput};
use kaspa_rpc_core::{RpcTransaction, RpcTransactionId};

use super::Signer;

pub(crate) struct PendingTransactionInner {
    pub(crate) signer:Signer,
    /// UtxoContext
    pub(crate) utxo_context: Option<UtxoContext>,
    /// UtxoEntryReferences of the pending transaction
    pub(crate) utxo_entries: AHashMap<UtxoEntryId, UtxoEntryReference>,
    /// Transaction Id (cached in pending to avoid mutex lock)
    pub(crate) id: TransactionId,
    /// Signable transaction (actual transaction that will be signed and sent)
    pub(crate) signable_tx: Mutex<SignableTransaction>,
    /// UTXO addresses used by this transaction
    pub(crate) addresses: Vec<Address>,
    /// Whether the transaction has been committed to the mempool via RPC
    pub(crate) is_submitted: AtomicBool,
    /// Payment value of the transaction (transaction destination amount)
    pub(crate) payment_value: Option<u64>,
    /// The index (position) of the change output in the transaction
    pub(crate) change_output_index: Option<usize>,
    /// Change value of the transaction (transaction change amount)
    pub(crate) change_output_value: u64,
    /// Total aggregate value of all inputs
    pub(crate) aggregate_input_value: u64,
    /// Total aggregate value of all outputs
    pub(crate) aggregate_output_value: u64,
    /// Minimum number of signatures required for the transaction
    /// (passed in during transaction creation). This value is used
    /// to estimate the mass of the transaction.
    pub(crate) minimum_signatures: u16,
    // Transaction mass
    pub(crate) mass: u64,
    /// Fees of the transaction
    pub(crate) fees: u64,
    /// Indicates the type of the transaction
    pub(crate) kind: DataKind,
}

// impl Clone for PendingTransactionInner {
//     fn clone(&self) -> Self {
//         Self {
//             generator: self.generator.clone(),
//             utxo_entries: self.utxo_entries.clone(),
//             id: self.id,
//             signable_tx: Mutex::new(self.signable_tx.lock().unwrap().clone()),
//             addresses: self.addresses.clone(),
//             is_submitted: AtomicBool::new(self.is_submitted.load(Ordering::SeqCst)),
//             payment_value: self.payment_value,
//             change_output_index: self.change_output_index,
//             change_output_value: self.change_output_value,
//             aggregate_input_value: self.aggregate_input_value,
//             aggregate_output_value: self.aggregate_output_value,
//             minimum_signatures: self.minimum_signatures,
//             mass: self.mass,
//             fees: self.fees,
//             kind: self.kind,
//         }
//     }
// }

impl std::fmt::Debug for PendingTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let transaction = self.transaction();
        f.debug_struct("PendingTransaction")
            .field("utxo_entries", &self.inner.utxo_entries)
            .field("addresses", &self.inner.addresses)
            .field("payment_value", &self.inner.payment_value)
            .field("change_output_index", &self.inner.change_output_index)
            .field("change_output_value", &self.inner.change_output_value)
            .field("aggregate_input_value", &self.inner.aggregate_input_value)
            .field("minimum_signatures", &self.inner.minimum_signatures)
            .field("mass", &self.inner.mass)
            .field("fees", &self.inner.fees)
            .field("kind", &self.inner.kind)
            .field("transaction", &transaction)
            .finish()
    }
}

/// Meta transaction encapsulating a transaction generated by the [`Generator`].
/// Contains auxiliary information about the transaction such as aggregate
/// input/output amounts, fees, etc.
#[derive(Clone)]
pub struct PendingTransaction {
    pub(crate) inner: Arc<PendingTransactionInner>,
}

impl PendingTransaction {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        signer:Signer,
        utxo_context: UtxoContext,
        transaction: Transaction,
        utxo_entries: Vec<UtxoEntryReference>,
        addresses: Vec<Address>,
        payment_value: Option<u64>,
        change_output_index: Option<usize>,
        change_output_value: u64,
        aggregate_input_value: u64,
        aggregate_output_value: u64,
        minimum_signatures: u16,
        mass: u64,
        fees: u64,
        kind: DataKind,
    ) -> Result<Self> {
        let id = transaction.id();
        let entries = utxo_entries.iter().map(|e| e.utxo.as_ref().into()).collect::<Vec<_>>();
        let signable_tx = Mutex::new(SignableTransaction::with_entries(transaction, entries));
        let utxo_entries = utxo_entries.into_iter().map(|entry| (entry.id(), entry)).collect::<AHashMap<_, _>>();
        Ok(Self {
            inner: Arc::new(PendingTransactionInner {
                generator: Some(generator.clone()),
                id,
                signable_tx,
                utxo_entries,
                addresses,
                is_submitted: AtomicBool::new(false),
                payment_value,
                change_output_index,
                change_output_value,
                aggregate_input_value,
                aggregate_output_value,
                minimum_signatures,
                mass,
                fees,
                kind,
            }),
        })
    }

    pub fn id(&self) -> TransactionId {
        self.inner.id
    }

    pub fn generator(&self) -> &Generator {
        &self.inner.generator
    }

    pub fn source_utxo_context(&self) -> Result<&Option<UtxoContext>> {
        Ok(self.inner.generator.as_ref().ok_or(Error::GeneratorIsMissing)?.source_utxo_context())
    }

    pub fn destination_utxo_context(&self) -> Result<&Option<UtxoContext>> {
        Ok(self.inner.generator.as_ref().ok_or(Error::GeneratorIsMissing)?.destination_utxo_context())
    }

    /// Addresses used by the pending transaction
    pub fn addresses(&self) -> &Vec<Address> {
        &self.inner.addresses
    }

    /// Get UTXO entries [`AHashSet<UtxoEntryReference>`] of the pending transaction
    pub fn utxo_entries(&self) -> &AHashMap<UtxoEntryId, UtxoEntryReference> {
        &self.inner.utxo_entries
    }

    pub fn fees(&self) -> u64 {
        self.inner.fees
    }

    pub fn mass(&self) -> u64 {
        self.inner.mass
    }

    pub fn minimum_signatures(&self) -> u16 {
        self.inner.minimum_signatures
    }

    pub fn aggregate_input_value(&self) -> u64 {
        self.inner.aggregate_input_value
    }

    pub fn aggregate_output_value(&self) -> u64 {
        self.inner.aggregate_output_value
    }

    pub fn payment_value(&self) -> Option<u64> {
        self.inner.payment_value
    }

    pub fn change_output_index(&self) -> Option<usize> {
        self.inner.change_output_index
    }

    pub fn change_value(&self) -> u64 {
        self.inner.change_output_value
    }

    pub fn is_final(&self) -> bool {
        self.inner.kind.is_final()
    }

    pub fn is_batch(&self) -> bool {
        !self.inner.kind.is_final()
    }

    pub fn network_type(&self) -> Result<NetworkType> {
        Ok(self.inner.generator.as_ref().ok_or(Error::GeneratorIsMissing)?.network_type())
    }

    pub fn transaction(&self) -> Transaction {
        self.inner.signable_tx.lock().unwrap().tx.clone()
    }

    pub fn signable_transaction(&self) -> SignableTransaction {
        self.inner.signable_tx.lock().unwrap().clone()
    }

    pub fn rpc_transaction(&self) -> RpcTransaction {
        self.inner.signable_tx.lock().unwrap().tx.as_ref().into()
    }

    /// Submit the transaction on the supplied rpc
    pub async fn try_submit(&self, rpc: &Arc<DynRpcApi>) -> Result<RpcTransactionId> {
        // sanity check to prevent multiple invocations (for API use)
        self.inner.is_submitted.load(Ordering::SeqCst).then(|| {
            panic!("PendingTransaction::try_submit() called multiple times");
        });
        self.inner.is_submitted.store(true, Ordering::SeqCst);

        let rpc_transaction: RpcTransaction = self.rpc_transaction();

        // if we are running under UtxoProcessor
        if let Some(utxo_context) = self.inner.generator.as_ref().ok_or(Error::GeneratorIsMissing)?.source_utxo_context() {
            // lock UtxoProcessor notification ingest
            let _lock = utxo_context.processor().notification_lock().await;

            // register pending UTXOs with UtxoProcessor
            utxo_context.register_outgoing_transaction(self).await?;

            // try to submit transaction
            match rpc.submit_transaction(rpc_transaction, false).await {
                Ok(id) => {
                    // on successful submit, create a notification
                    utxo_context.notify_outgoing_transaction(self).await?;
                    Ok(id)
                }
                Err(error) => {
                    // in case of failure, remove transaction UTXOs from the consumed list
                    utxo_context.cancel_outgoing_transaction(self).await?;
                    Err(error.into())
                }
            }
        } else {
            // No UtxoProcessor present (API etc)
            Ok(rpc.submit_transaction(rpc_transaction, false).await?)
        }
    }

    pub async fn log(&self) -> Result<()> {
        log_info!("pending transaction: {:?}", self.rpc_transaction());
        Ok(())
    }

    pub fn try_sign(&self) -> Result<()> {
        let signer = self.inner.generator.ok_or(Error::GeneratorIsMissing)?.signer().as_ref().expect("no signer in tx generator");
        let signed_tx = signer.try_sign(self.inner.signable_tx.lock()?.clone(), self.addresses())?;
        *self.inner.signable_tx.lock().unwrap() = signed_tx;
        Ok(())
    }

    pub fn create_input_signature(&self, input_index: usize, private_key: &[u8; 32], hash_type: SigHashType) -> Result<Vec<u8>> {
        let mutable_tx = self.inner.signable_tx.lock()?.clone();
        let verifiable_tx = mutable_tx.as_verifiable();

        Ok(sign_input(&verifiable_tx, input_index, private_key, hash_type))
    }

    pub fn fill_input(&self, input_index: usize, signature_script: Vec<u8>) -> Result<()> {
        let mut mutable_tx = self.inner.signable_tx.lock()?.clone();
        mutable_tx.tx.inputs[input_index].signature_script = signature_script;
        *self.inner.signable_tx.lock().unwrap() = mutable_tx;

        Ok(())
    }

    pub fn sign_input(&self, input_index: usize, private_key: &[u8; 32], hash_type: SigHashType) -> Result<()> {
        let mut mutable_tx = self.inner.signable_tx.lock()?.clone();

        let signature_script = {
            let verifiable_tx = &mutable_tx.as_verifiable();
            sign_input(verifiable_tx, input_index, private_key, hash_type)
        };

        mutable_tx.tx.inputs[input_index].signature_script = signature_script;
        *self.inner.signable_tx.lock().unwrap() = mutable_tx;

        Ok(())
    }

    pub fn try_sign_with_keys(&self, privkeys: &[[u8; 32]], check_fully_signed: Option<bool>) -> Result<()> {
        let mutable_tx = self.inner.signable_tx.lock()?.clone();
        let signed = sign_with_multiple_v2(mutable_tx, privkeys);

        let signed_tx = match signed {
            Signed::Fully(tx) => tx,
            Signed::Partially(_) => {
                if check_fully_signed.unwrap_or(true) {
                    signed.fully_signed()?
                } else {
                    signed.unwrap()
                }
            }
        };

        *self.inner.signable_tx.lock().unwrap() = signed_tx;
        Ok(())
    }

    //pub fn increase_fees_for_rbf(&self, additional_fees: u64) -> Result<PendingTransaction> {
    //    #![allow(unused_mut)]
    //    #![allow(unused_variables)]
    //
    //    let PendingTransactionInner {
    //        generator,
    //        utxo_entries,
    //        id,
    //        signable_tx,
    //        addresses,
    //        is_submitted,
    //        payment_value,
    //        change_output_index,
    //        change_output_value,
    //        aggregate_input_value,
    //        aggregate_output_value,
    //        minimum_signatures,
    //        mass,
    //        fees,
    //        kind,
    //    } = &*self.inner;
    //
    //    let generator = generator.clone();
    //    let utxo_entries = utxo_entries.clone();
    //    let id = *id;
    //    // let signable_tx = Mutex::new(signable_tx.lock()?.clone());
    //    let mut signable_tx = signable_tx.lock()?.clone();
    //    let addresses = addresses.clone();
    //    let is_submitted = AtomicBool::new(false);
    //    let payment_value = *payment_value;
    //    let mut change_output_index = *change_output_index;
    //    let mut change_output_value = *change_output_value;
    //    let mut aggregate_input_value = *aggregate_input_value;
    //    let mut aggregate_output_value = *aggregate_output_value;
    //    let minimum_signatures = *minimum_signatures;
    //    let mass = *mass;
    //    let fees = *fees;
    //    let kind = *kind;
    //
    //    #[allow(clippy::single_match)]
    //    match kind {
    //        DataKind::Final => {
    //            // change output has sufficient amount to cover fee increase
    //            // if change_output_value > fee_increase && change_output_index.is_some() {
    //            if let (Some(index), true) = (change_output_index, change_output_value >= additional_fees) {
    //                change_output_value -= additional_fees;
    //                if generator.mass_calculator().is_dust(change_output_value) {
    //                    aggregate_output_value -= change_output_value;
    //                    signable_tx.tx.outputs.remove(index);
    //                    change_output_index = None;
    //                    change_output_value = 0;
    //                } else {
    //                    signable_tx.tx.outputs[index].value = change_output_value;
    //                }
    //            } else {
    //                // we need more utxos...
    //                let mut utxo_entries_rbf = vec![];
    //                let mut available = change_output_value;
    //
    //                let utxo_context = generator.source_utxo_context().as_ref().ok_or(Error::custom("No utxo context"))?;
    //                let mut context_utxo_entries = UtxoIterator::new(utxo_context);
    //                while available < additional_fees {
    //                    // let utxo_entry = utxo_entries.next().ok_or(Error::InsufficientFunds { additional_needed: additional_fees - available, origin: "increase_fees_for_rbf" })?;
    //                    // let utxo_entry = generator.get_utxo_entry_for_rbf()?;
    //                    if let Some(utxo_entry) = context_utxo_entries.next() {
    //                        // let utxo = utxo_entry.utxo.as_ref();
    //                        let value = utxo_entry.amount();
    //                        available += value;
    //                        // aggregate_input_value += value;
    //
    //                        utxo_entries_rbf.push(utxo_entry);
    //                        // signable_tx.lock().unwrap().tx.inputs.push(utxo.as_input());
    //                    } else {
    //                        // generator.stash(utxo_entries_rbf);
    //                        // utxo_entries_rbf.into_iter().for_each(|utxo_entry|generator.stash(utxo_entry));
    //                        return Err(Error::InsufficientFunds {
    //                            additional_needed: additional_fees - available,
    //                            origin: "increase_fees_for_rbf",
    //                        });
    //                    }
    //                }
    //
    //                let utxo_entries_vec = utxo_entries
    //                    .iter()
    //                    .map(|(_, utxo_entry)| utxo_entry.as_ref().clone())
    //                    .chain(utxo_entries_rbf.iter().map(|utxo_entry| utxo_entry.as_ref().clone()))
    //                    .collect::<Vec<_>>();
    //
    //                let inputs = utxo_entries_rbf
    //                    .into_iter()
    //                    .map(|utxo| TransactionInput::new(utxo.outpoint().clone().into(), vec![], 0, generator.sig_op_count()));
    //
    //                signable_tx.tx.inputs.extend(inputs);
    //
    //                // let transaction_mass = generator.mass_calculator().calc_overall_mass_for_unsigned_consensus_transaction(
    //                //     &signable_tx.tx,
    //                //     &utxo_entries_vec,
    //                //     self.inner.minimum_signatures,
    //                // )?;
    //                // if transaction_mass > MAXIMUM_STANDARD_TRANSACTION_MASS {
    //                //     // this should never occur as we should not produce transactions higher than the mass limit
    //                //     return Err(Error::MassCalculationError);
    //                // }
    //                // signable_tx.tx.set_mass(transaction_mass);
    //
    //                // utxo
    //
    //                // let input = ;
    //            }
    //        }
    //        _ => {}
    //    }
    //
    //    let inner = PendingTransactionInner {
    //        generator,
    //        utxo_entries,
    //        id,
    //        signable_tx: Mutex::new(signable_tx),
    //        addresses,
    //        is_submitted,
    //        payment_value,
    //        change_output_index,
    //        change_output_value,
    //        aggregate_input_value,
    //        aggregate_output_value,
    //        minimum_signatures,
    //        mass,
    //        fees,
    //        kind,
    //    };
    //
    //    Ok(PendingTransaction { inner: Arc::new(inner) })
    //
    //    // let mut mutable_tx = self.inner.signable_tx.lock()?.clone();
    //    // mutable_tx.tx.fee += fees;
    //    // *self.inner.signable_tx.lock().unwrap() = mutable_tx;
    //}
}
