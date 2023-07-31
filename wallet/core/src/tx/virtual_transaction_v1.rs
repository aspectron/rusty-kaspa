// work in progress

use crate::imports::*;
use crate::keypair::PrivateKey;
use crate::signer::{sign_mutable_transaction, PrivateKeyArrayOrSigner};
use crate::tx::{
    // create_transaction,
    LimitCalcStrategy,
    // LimitStrategy,
    MutableTransaction,
    PaymentOutputs,
    Transaction,
    TransactionInput,
    // TransactionOutpoint, TransactionOutput,
};
// use crate::tx::{
//     // calculate_mass, calculate_minimum_transaction_fee,
//     get_consensus_params_by_address,
//     MAXIMUM_STANDARD_TRANSACTION_MASS,
// };
// use crate::utxo::selection::UtxoSelectionContextInterface;
use crate::utxo::{UtxoEntry, UtxoEntryReference, UtxoSelectionContext};
use crate::Signer;
use kaspa_addresses::Address;
use kaspa_consensus_core::config::params::Params;
// use kaspa_consensus_core::{subnets::SubnetworkId, tx};
use kaspa_rpc_core::SubmitTransactionRequest;
// use kaspa_txscript::pay_to_address_script;
use kaspa_wrpc_client::wasm::RpcClient;
use workflow_core::abortable::Abortable;
use workflow_wasm::tovalue::to_value;

pub struct TransactionsV1 {
    pub transactions: Vec<MutableTransaction>,
    pub inputs: Vec<TransactionInput>,
    pub utxos: Vec<UtxoEntry>,
    pub amount: u64,
}

impl TransactionsV1 {
    pub async fn merge(
        &mut self,
        _outputs: &PaymentOutputs,
        _change_address: &Address,
        _priority_fee: u64,
        _payload: Vec<u8>,
        _minimum_signatures: u16,
    ) -> crate::Result<bool> {
        todo!()

        // if self.amount < priority_fee {
        //     return Err(format!("final amount({}) < priority fee({priority_fee})", self.amount).into());
        // }

        // let amount_after_priority_fee = self.amount - priority_fee;

        // let mut outputs_ = vec![];
        // let mut output_amount = 0;
        // for output in &outputs.outputs {
        //     output_amount += output.amount;
        //     outputs_.push(TransactionOutput::new(output.amount, &pay_to_address_script(&output.address)));
        // }

        // if output_amount > amount_after_priority_fee {
        //     return Err(format!("output amount({output_amount}) > amount after priority fee({amount_after_priority_fee})").into());
        // }

        // let change = amount_after_priority_fee - output_amount;
        // let mut change_output = None;
        // if change > 0 {
        //     let output = TransactionOutput::new(change, &pay_to_address_script(change_address));
        //     if !output.is_dust() {
        //         change_output = Some(output.clone());
        //         outputs_.push(output);
        //     }
        // }

        // let tx = Transaction::new(
        //     0,
        //     self.inputs.clone(),
        //     outputs_,
        //     0,
        //     SubnetworkId::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        //     0,
        //     payload,
        // )?;

        // let consensus_params = get_consensus_params_by_address(change_address);

        // let fee = calculate_minimum_transaction_fee(&tx, &consensus_params, true, minimum_signatures);
        // if change < fee {
        //     return Err(format!("change({change}) <= minimum fee ({fee})").into());
        // }
        // if let Some(change_output) = change_output {
        //     let new_change = change - fee;
        //     change_output.inner().value = new_change;
        //     if change_output.is_dust() {
        //         let _change_output = tx.inner().outputs.pop();
        //     }

        //     tx.finalize().unwrap();
        // }

        // let mtx = MutableTransaction::new(tx, self.utxos.clone().into());
        // self.transactions.push(mtx);

        // Ok(true)
    }
}

pub async fn calculate_chunk_size(
    _tx: &Transaction,
    _total_mass: u64,
    _params: &Params,
    _estimate_signature_mass: bool,
    _minimum_signatures: u16,
) -> crate::Result<u64> {
    todo!()

    // let (mass_per_input, mass_without_inputs) =
    //     mass_per_input_and_mass_without_inputs(tx, total_mass, params, estimate_signature_mass, minimum_signatures);

    // let output = match tx.inner().outputs.get(0).cloned() {
    //     Some(output) => output,
    //     None => {
    //         return Err("Minimum one output is require to calculate chunk size".to_string().into());
    //     }
    // };

    // let split_tx_without_inputs = Transaction::new(
    //     0,
    //     vec![],
    //     vec![output],
    //     0,
    //     SubnetworkId::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    //     0,
    //     vec![],
    // )
    // .unwrap();

    // let split_tx_mass_without_inputs = calculate_mass(&split_tx_without_inputs, params, estimate_signature_mass, minimum_signatures);

    // log_trace!("mass_per_input: {mass_per_input}");
    // log_trace!("total_mass: {total_mass}");
    // log_trace!("mass_without_inputs: {mass_without_inputs}");
    // log_trace!("split_tx_mass_without_inputs: {split_tx_mass_without_inputs}");
    // let inputs_max_mass = MAXIMUM_STANDARD_TRANSACTION_MASS - split_tx_mass_without_inputs;
    // log_trace!("inputs_max_mass: {inputs_max_mass}");
    // log_trace!("chunk_size: {}", inputs_max_mass / mass_per_input);
    // Ok(inputs_max_mass / mass_per_input)
}

pub fn mass_per_input_and_mass_without_inputs(
    _tx: &Transaction,
    _total_mass: u64,
    _params: &Params,
    _estimate_signature_mass: bool,
    _minimum_signatures: u16,
) -> (u64, u64) {
    todo!()

    // //let total_mass = calculate_mass(tx, params, estimate_signature_mass);
    // let mut tx_inner_clone = tx.inner().clone();
    // tx_inner_clone.inputs = vec![];
    // let tx_clone = Transaction::new_with_inner(tx_inner_clone);

    // let mass_without_inputs = calculate_mass(&tx_clone, params, estimate_signature_mass, minimum_signatures);

    // let input_mass = total_mass - mass_without_inputs;
    // let input_count = tx.inner().inputs.len() as u64;
    // let mut mass_per_input = input_mass / input_count;
    // if input_mass % input_count > 0 {
    //     mass_per_input += 1;
    // }

    // (mass_per_input, mass_without_inputs)
}

/// `VirtualTransaction` envelops a collection of multiple related `kaspa_wallet_core::MutableTransaction` instances.
#[derive(Clone, Debug)]
#[wasm_bindgen]
#[allow(dead_code)] //TODO: remove me
pub struct VirtualTransactionV1 {
    transactions: Vec<MutableTransaction>,
    payload: Vec<u8>,
    // include_fees : bool,
}

#[wasm_bindgen]
impl VirtualTransactionV1 {
    #[wasm_bindgen(constructor)]
    pub async fn constructor(
        sig_op_count: u8,
        minimum_signatures: u16,
        ctx: &mut UtxoSelectionContext,
        outputs: JsValue,
        change_address: &Address,
        priority_fee_sompi: Option<u64>,
        payload: Vec<u8>,
        limit_calc_strategy: LimitCalcStrategy,
        abortable: &Abortable,
    ) -> crate::Result<VirtualTransactionV1> {
        Self::try_new(
            sig_op_count,
            minimum_signatures,
            ctx,
            &PaymentOutputs::try_from(outputs)?,
            change_address,
            priority_fee_sompi,
            payload,
            limit_calc_strategy,
            abortable,
        )
        .await
    }

    #[wasm_bindgen(js_name = "transactions")]
    pub fn transaction_array(&self) -> Array {
        Array::from_iter(self.transactions.clone().into_iter().map(JsValue::from))
    }

    #[wasm_bindgen(js_name = "sign")]
    pub fn js_sign(&mut self, signer: PrivateKeyArrayOrSigner, verify_sig: bool) -> crate::Result<()> {
        if signer.is_array() {
            let mut private_keys: Vec<[u8; 32]> = vec![];
            for key in Array::from(&signer).iter() {
                let key = PrivateKey::try_from(&key).map_err(|_| Error::Custom("Unable to cast PrivateKey".to_string()))?;
                private_keys.push(key.secret_bytes());
            }
            self.sign(&private_keys, verify_sig)?;
        } else {
            let signer = Signer::try_from(&JsValue::from(signer)).map_err(|_| Error::Custom("Unable to cast Signer".to_string()))?;
            log_trace!("\nSigning via Signer: {signer:?}....\n");
            self.sign_with_signer(&signer, verify_sig)?;
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = "submit")]
    pub async fn js_submit(&mut self, rpc: &RpcClient, allow_orphan: bool) -> crate::Result<Array> {
        let result = Array::new();
        for transaction in self.transactions.clone() {
            result.push(&to_value(
                &rpc.submit_transaction(SubmitTransactionRequest { transaction: transaction.try_into()?, allow_orphan }).await?,
            )?);
        }

        Ok(result)
    }
}

impl VirtualTransactionV1 {
    pub async fn try_new(
        _sig_op_count: u8,
        _minimum_signatures: u16,
        _ctx: &mut UtxoSelectionContext,
        _outputs: &PaymentOutputs,
        _change_address: &Address,
        _priority_fee_sompi: Option<u64>,
        _payload: Vec<u8>,
        _limit_calc_strategy: LimitCalcStrategy,
        _abortable: &Abortable,
    ) -> crate::Result<VirtualTransactionV1> {
        todo!()

        // let transaction_amount = outputs.amount() + priority_fee_sompi.as_ref().cloned().unwrap_or_default();
        // ctx.select(transaction_amount)?;
        // let selected_amount = ctx.selected_amount();

        // log_trace!("VirtualTransaction...");
        // log_trace!("utxo_selection.transaction_amount: {:?}", transaction_amount);
        // log_trace!("utxo_selection.total_selected_amount: {:?}", selected_amount);
        // log_trace!("outputs.outputs: {:?}", outputs.outputs);
        // log_trace!("change_address: {:?}", change_address.to_string());

        // let consensus_params = get_consensus_params_by_address(change_address);

        // let priority_fee = priority_fee_sompi.unwrap_or(0);

        // match limit_calc_strategy.strategy {
        //     LimitStrategy::Calculated => {
        //         abortable.check()?;
        //         let mtx = create_transaction(
        //             sig_op_count,
        //             ctx,
        //             outputs,
        //             change_address,
        //             minimum_signatures,
        //             priority_fee_sompi,
        //             Some(payload.clone()),
        //         )?;

        //         let tx = mtx.tx().clone();
        //         abortable.check()?;
        //         let mass = calculate_mass(&tx, &consensus_params, true, minimum_signatures);
        //         if mass <= MAXIMUM_STANDARD_TRANSACTION_MASS {
        //             return Ok(VirtualTransactionV1 { transactions: vec![mtx], payload });
        //         }
        //         abortable.check()?;
        //         let max_inputs = calculate_chunk_size(&tx, mass, &consensus_params, true, minimum_signatures).await? as usize;
        //         abortable.check()?;
        //         let entries = ctx.selection().entries().clone();

        //         let mut txs =
        //             Self::split_utxos(&entries, max_inputs, max_inputs, change_address, sig_op_count, minimum_signatures, abortable)
        //                 .await?;
        //         abortable.check()?;
        //         txs.merge(outputs, change_address, priority_fee, payload.clone(), minimum_signatures).await?;
        //         Ok(VirtualTransactionV1 { transactions: txs.transactions, payload })
        //     }
        //     LimitStrategy::Inputs(inputs) => {
        //         abortable.check()?;
        //         let max_inputs = inputs as usize;
        //         let entries = ctx.selection().entries().clone();

        //         let mut txs =
        //             Self::split_utxos(&entries, max_inputs, max_inputs, change_address, sig_op_count, minimum_signatures, abortable)
        //                 .await?;
        //         abortable.check()?;
        //         txs.merge(outputs, change_address, priority_fee, payload.clone(), minimum_signatures).await?;
        //         Ok(VirtualTransactionV1 { transactions: txs.transactions, payload })
        //     }
        // }
    }

    pub fn sign_with_signer(&mut self, signer: &Signer, verify_sig: bool) -> crate::Result<()> {
        let mut transactions = vec![];
        for mtx in self.transactions.clone() {
            transactions.push(signer.sign_transaction(mtx, verify_sig)?);
        }
        self.transactions = transactions;
        Ok(())
    }

    pub fn sign(&mut self, private_keys: &Vec<[u8; 32]>, verify_sig: bool) -> crate::Result<()> {
        let mut transactions = vec![];
        for mtx in self.transactions.clone() {
            transactions.push(sign_mutable_transaction(mtx, private_keys, verify_sig)?);
        }
        self.transactions = transactions;
        Ok(())
    }

    pub fn transactions(&self) -> &Vec<MutableTransaction> {
        &self.transactions
    }

    pub async fn split_utxos(
        _utxos_entries: &[UtxoEntryReference],
        _chunk_size: usize,
        _max_inputs: usize,
        _change_address: &Address,
        _sig_op_count: u8,
        _minimum_signatures: u16,
        _abortable: &Abortable,
    ) -> crate::Result<TransactionsV1> {
        todo!()

        // let mut final_inputs = vec![];
        // let mut final_utxos = vec![];
        // let mut final_amount = 0;
        // let mut transactions = vec![];

        // if utxos_entries.len() <= max_inputs {
        //     utxos_entries.iter().for_each(|utxo_ref| {
        //         final_amount += utxo_ref.utxo.amount();
        //         final_utxos.push(utxo_ref.data());
        //         final_inputs.push(TransactionInput::new(utxo_ref.utxo.outpoint.clone(), vec![], 0, sig_op_count));
        //         log_debug!(
        //             "final_amount: {final_amount}, transaction_id: {}\r\n",
        //             utxo_ref.utxo.outpoint.get_transaction_id_as_string()
        //         );
        //     });

        //     return Ok(TransactionsV1 { transactions, inputs: final_inputs, utxos: final_utxos, amount: final_amount });
        // }

        // abortable.check()?;

        // let consensus_params = get_consensus_params_by_address(change_address);

        // let chunks = utxos_entries.chunks(chunk_size).collect::<Vec<&[UtxoEntryReference]>>();
        // for chunk in chunks {
        //     abortable.check()?;
        //     let utxos = chunk.iter().map(|reference| reference.utxo.clone()).collect::<Vec<Arc<UtxoEntry>>>();

        //     let mut amount = 0;
        //     let mut entries = vec![];

        //     let inputs = utxos
        //         .iter()
        //         .enumerate()
        //         .map(|(sequence, utxo)| {
        //             //println!("input txid: {}\r\n", utxo.outpoint.get_transaction_id());
        //             amount += utxo.entry.amount;
        //             entries.push(utxo.as_ref().clone());
        //             TransactionInput::new(utxo.outpoint.clone(), vec![], sequence as u64, sig_op_count)
        //         })
        //         .collect::<Vec<TransactionInput>>();

        //     let script_public_key = pay_to_address_script(change_address);
        //     let tx = Transaction::new(
        //         0,
        //         inputs,
        //         vec![TransactionOutput::new(amount, &script_public_key)],
        //         0,
        //         SubnetworkId::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        //         0,
        //         vec![],
        //     )
        //     .unwrap();

        //     let fee = calculate_minimum_transaction_fee(&tx, &consensus_params, true, minimum_signatures);
        //     if amount <= fee {
        //         log_debug!("amount<=fee: {amount}, {fee}\r\n");
        //         continue;
        //     }
        //     let amount_after_fee = amount - fee;

        //     tx.inner().outputs[0].set_value(amount_after_fee);
        //     if tx.inner().outputs[0].is_dust() {
        //         log_debug!("outputs is dust: {}\r\n", amount_after_fee);
        //         continue;
        //     }
        //     abortable.check()?;
        //     let transaction_id = tx.finalize().unwrap(); //.to_str();

        //     final_amount += amount_after_fee;
        //     log_debug!("final_amount: {final_amount}, transaction_id: {}\r\n", transaction_id);
        //     final_utxos.push(UtxoEntry {
        //         address: Some(change_address.clone()),
        //         outpoint: TransactionOutpoint::new(transaction_id, 0),
        //         entry: tx::UtxoEntry { amount: amount_after_fee, script_public_key, block_daa_score: u64::MAX, is_coinbase: false },
        //     });
        //     final_inputs.push(TransactionInput::new(TransactionOutpoint::new(transaction_id, 0), vec![], 0, sig_op_count));

        //     transactions.push(MutableTransaction::new(tx, entries.into()));
        // }

        // Ok(TransactionsV1 { transactions, inputs: final_inputs, utxos: final_utxos, amount: final_amount })
    }
}