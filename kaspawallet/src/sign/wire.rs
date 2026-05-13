//! Wire <-> consensus type conversion.
//!
//! Lifts a proto-wire `TransactionMessage` into the
//! consensus-core `Transaction` type so consumers can compute
//! transaction IDs, run hashing routines, and otherwise interact
//! with the consensus domain types.
//!
//! The lift re-applies the same input-validation guards
//! (`version <= u16`, `subnetwork_id.bytes == 20`,
//! `sig_op_count <= u8`, `txid.bytes == 32`) that the
//! serialization deserialize entry points enforce on decode.
//! Defense-in-depth: a consumer that receives an externally-
//! constructed wire transaction built without going through the
//! local decoder still cannot bypass the wire-shape contract.

use kaspa_consensus_core::subnets::SubnetworkId;
use kaspa_consensus_core::tx::{ScriptPublicKey, Transaction, TransactionInput, TransactionOutpoint, TransactionOutput, UtxoEntry};

use super::error::SignError;
use crate::serialization::wire;

/// Length of the kaspa subnetwork-id field.
const SUBNETWORK_ID_LEN: usize = 20;

/// Length of a kaspa transaction-id field.
const TRANSACTION_ID_LEN: usize = 32;

/// Convert a proto-wire `TransactionMessage` into the
/// consensus-core `Transaction` type that hashing and ID
/// routines accept.
pub(crate) fn wire_to_consensus_tx(tx_msg: &wire::TransactionMessage) -> Result<Transaction, SignError> {
    let version: u16 = u16::try_from(tx_msg.version)
        .map_err(|_| SignError::Invalid { field: "tx.version", reason: format!("version {} exceeds u16::MAX", tx_msg.version) })?;
    let subnetwork_bytes = tx_msg.subnetwork_id.as_ref().ok_or(SignError::Missing("tx.subnetworkId"))?.bytes.as_slice();
    if subnetwork_bytes.len() != SUBNETWORK_ID_LEN {
        return Err(SignError::Invalid {
            field: "tx.subnetworkId.bytes",
            reason: format!("expected {SUBNETWORK_ID_LEN} bytes, got {}", subnetwork_bytes.len()),
        });
    }
    let mut sub_arr = [0u8; SUBNETWORK_ID_LEN];
    sub_arr.copy_from_slice(subnetwork_bytes);
    let subnetwork_id = SubnetworkId::from_bytes(sub_arr);

    let inputs = tx_msg.inputs.iter().map(wire_input_to_consensus).collect::<Result<Vec<_>, _>>()?;
    let outputs = tx_msg.outputs.iter().map(wire_output_to_consensus).collect::<Result<Vec<_>, _>>()?;

    Ok(Transaction::new(version, inputs, outputs, tx_msg.lock_time, subnetwork_id, tx_msg.gas, tx_msg.payload.clone()))
}

fn wire_input_to_consensus(input: &wire::TransactionInput) -> Result<TransactionInput, SignError> {
    let outpoint = input.previous_outpoint.as_ref().ok_or(SignError::Missing("tx.input.previousOutpoint"))?;
    let txid_bytes = outpoint.transaction_id.as_ref().ok_or(SignError::Missing("tx.input.previousOutpoint.transactionId"))?;
    let id_arr: [u8; TRANSACTION_ID_LEN] = txid_bytes.bytes.as_slice().try_into().map_err(|_| SignError::Invalid {
        field: "tx.input.previousOutpoint.transactionId.bytes",
        reason: format!("expected {TRANSACTION_ID_LEN} bytes, got {}", txid_bytes.bytes.len()),
    })?;
    let prev = TransactionOutpoint::new(kaspa_consensus_core::Hash::from_bytes(id_arr), outpoint.index);
    let sig_op_count: u8 = u8::try_from(input.sig_op_count).map_err(|_| SignError::Invalid {
        field: "tx.input.sigOpCount",
        reason: format!("exceeds u8::MAX: {}", input.sig_op_count),
    })?;
    Ok(TransactionInput::new(prev, input.signature_script.clone(), input.sequence, sig_op_count))
}

fn wire_output_to_consensus(output: &wire::TransactionOutput) -> Result<TransactionOutput, SignError> {
    let spk_msg = output.script_public_key.as_ref().ok_or(SignError::Missing("tx.output.scriptPublicKey"))?;
    let spk = wire_script_to_consensus(spk_msg)?;
    Ok(TransactionOutput::new(output.value, spk))
}

pub(crate) fn wire_script_to_consensus(spk: &wire::ScriptPublicKey) -> Result<ScriptPublicKey, SignError> {
    let version: u16 = u16::try_from(spk.version)
        .map_err(|_| SignError::Invalid { field: "scriptPublicKey.version", reason: format!("exceeds u16::MAX: {}", spk.version) })?;
    Ok(ScriptPublicKey::from_vec(version, spk.script.clone()))
}

/// Sync every `tx.inputs[i].sig_op_count` to the matching PSI's
/// `pub_key_signature_pairs.len()`. Both signing flows call this
/// BEFORE sighash so the digest signed matches the consensus-tx
/// form `extract_transaction` later produces (which sets
/// `sig_op_count` from the same source per `super::combine`).
pub(crate) fn apply_sig_op_count_from_psi(pst: &mut wire::PartiallySignedTransaction) -> Result<(), SignError> {
    let tx = pst.tx.as_mut().ok_or(SignError::Missing("PartiallySignedTransaction.tx"))?;
    if tx.inputs.len() != pst.partially_signed_inputs.len() {
        return Err(SignError::Invalid {
            field: "PartiallySignedTransaction",
            reason: format!(
                "tx.inputs.len()={} != partially_signed_inputs.len()={}",
                tx.inputs.len(),
                pst.partially_signed_inputs.len()
            ),
        });
    }
    for (input, psi) in tx.inputs.iter_mut().zip(pst.partially_signed_inputs.iter()) {
        let count: u32 = u32::try_from(psi.pub_key_signature_pairs.len()).map_err(|_| SignError::Invalid {
            field: "PartiallySignedInput.pubKeySignaturePairs",
            reason: format!("count {} exceeds u32::MAX", psi.pub_key_signature_pairs.len()),
        })?;
        input.sig_op_count = count;
    }
    Ok(())
}

/// Build the `UtxoEntry` list the consensus-core mass + sighash
/// routines require, sourced from each
/// `PartiallySignedInput.prev_output`. `block_daa_score` and
/// `is_coinbase` are not part of the wire format and do not feed
/// into the mass calculation; safe placeholder values keep
/// `SignableTransaction::as_verifiable()` happy.
pub(crate) fn build_utxo_entries(pst: &wire::PartiallySignedTransaction) -> Result<Vec<UtxoEntry>, SignError> {
    pst.partially_signed_inputs
        .iter()
        .map(|psi| {
            let prev = psi.prev_output.as_ref().ok_or(SignError::Missing("PartiallySignedInput.prevOutput"))?;
            let spk_msg =
                prev.script_public_key.as_ref().ok_or(SignError::Missing("PartiallySignedInput.prevOutput.scriptPublicKey"))?;
            let spk = wire_script_to_consensus(spk_msg)?;
            Ok(UtxoEntry::new(prev.value, spk, 0, false))
        })
        .collect()
}
