use crate::error::Error;
use crate::input::{Input, InputBuilder};
use crate::output::{Output, OutputBuilder};
use crate::pskt::{Global, Inner};
use kaspa_consensus_client::{Transaction, TransactionInput, TransactionInputInner, TransactionOutput, TransactionOutputInner};
use kaspa_consensus_core::tx as cctx;

impl From<Transaction> for Inner {
    fn from(_transaction: Transaction) -> Inner {
        let transaction = cctx::Transaction::from(&_transaction);
        Inner::from(transaction)
    }
}

impl TryFrom<TransactionInput> for Input {
    type Error = Error;
    fn try_from(input: TransactionInput) -> std::result::Result<Input, Self::Error> {
        let TransactionInputInner { previous_outpoint, signature_script: _, sequence: _, sig_op_count, utxo } = &*input.inner();

        let input = InputBuilder::default()
        .utxo_entry(utxo.as_ref().ok_or(Error::MissingUtxoEntry)?.into())
        .previous_outpoint(previous_outpoint.into())
        // .sequence(*sequence)
        // min_time
        // partial_sigs
        // sighash_type
        // redeem_script
        .sig_op_count(*sig_op_count)
        // bip32_derivations
        // final_script_sig
        .build()?;

        Ok(input)
    }
}

impl TryFrom<TransactionOutput> for Output {
    type Error = Error;
    fn try_from(output: TransactionOutput) -> std::result::Result<Output, Self::Error> {
        // Self::Transaction(transaction)

        let TransactionOutputInner { value, script_public_key } = &*output.inner();

        let output = OutputBuilder::default()
        .amount(*value)
        .script_public_key(script_public_key.clone())
        // .redeem_script
        // .bip32_derivations
        // .proprietaries
        // .unknowns
        .build()?;

        Ok(output)
    }
}

impl From<(cctx::Transaction, Vec<(&cctx::TransactionInput, &cctx::UtxoEntry)>)> for Inner {
    fn from((transaction, populated_inputs): (cctx::Transaction, Vec<(&cctx::TransactionInput, &cctx::UtxoEntry)>)) -> Inner {
        let inputs = populated_inputs
            .into_iter()
            .map(|(input, utxo)| {
                println!("Populated utxo");
                InputBuilder::default()
                    .utxo_entry(utxo.to_owned().clone())
                    .previous_outpoint(input.previous_outpoint)
                    .sig_op_count(input.sig_op_count)
                    .build()
                    .unwrap()
            })
            .collect();

        let outputs: Vec<Output> = transaction
            .outputs
            .iter()
            .filter_map(|output| Output::try_from(TransactionOutput::from(output.to_owned())).ok())
            .collect();

        Inner { global: Global::default(), inputs, outputs }
    }
}

impl From<cctx::Transaction> for Inner {
    fn from(transaction: cctx::Transaction) -> Inner {
        let inputs: Vec<Input> = transaction
            .inputs
            .iter()
            .filter_map(|input| {
                let tx_input = TransactionInput::from(input.to_owned());
                match Input::try_from(tx_input) {
                    Ok(input) => Some(input),
                    Err(e) => {
                        println!("Error converting input: {:?}", e);
                        None
                    }
                }
            })
            .collect();

        let outputs: Vec<Output> = transaction
            .outputs
            .iter()
            .filter_map(|output| Output::try_from(TransactionOutput::from(output.to_owned())).ok())
            .collect();

        Inner { global: Global::default(), inputs, outputs }
    }
}
