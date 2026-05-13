//! Multisig partial-signature combination + final-signature-script
//! assembly.
//!
//! Module contract:
//!
//! - Consumes a fully-signed (or junk-filled, for mass estimation)
//!   `PartiallySignedTransaction` -- every input's
//!   `PubKeySignaturePairs` carries the required number of
//!   `Signature` byte blobs.
//! - Produces a consensus-core `Transaction` whose
//!   `inputs[i].signature_script` is the on-chain-broadcastable
//!   sigscript.
//! - For single-cosigner inputs the sigscript is one PUSHDATA of
//!   the signature blob; for multisig inputs the sigscript is N
//!   PUSHDATA signatures followed by one PUSHDATA of the redeem
//!   script.
//!
//! Building blocks reused from the workspace:
//!
//! - `kaspa_bip32::ExtendedPublicKey` for deserializing each
//!   `pair.extended_pub_key` and extracting its serialized public
//!   key.
//! - `kaspa_txscript::script_builder::ScriptBuilder` for every
//!   PUSHDATA wrapping (so length prefixes follow the consensus
//!   script-builder semantics byte-for-byte).
//! - `kaspa_txscript::standard::multisig::{multisig_redeem_script,
//!   multisig_redeem_script_ecdsa}` for the redeem-script
//!   construction.

use kaspa_bip32::ExtendedPublicKey;
use kaspa_consensus_core::tx::{Transaction, TransactionInput};
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspa_txscript::standard::{MultisigCreateError, multisig_redeem_script, multisig_redeem_script_ecdsa};

use super::SignError;
use super::wire::wire_to_consensus_tx;
use crate::serialization::wire;

/// Schnorr (BIP-340) x-only serialized public-key length.
const SCHNORR_PUBKEY_LEN: usize = 32;

/// Compressed ECDSA serialized public-key length.
const ECDSA_PUBKEY_LEN: usize = 33;

/// Assemble the on-chain `Transaction` from a fully-collected (or
/// junk-filled) PST.
///
/// `ecdsa` controls the redeem-script's opcode + pubkey form for
/// multisig inputs (`OpCheckMultiSig` + 32-byte Schnorr pubkeys vs
/// `OpCheckMultiSigECDSA` + 33-byte compressed pubkeys). For
/// single-cosigner inputs the flag is unused (the sigscript is
/// one PUSHDATA of the signature only).
pub fn extract_transaction(pst: &wire::PartiallySignedTransaction, ecdsa: bool) -> Result<Transaction, SignError> {
    let tx_msg = pst.tx.as_ref().ok_or(SignError::Missing("PartiallySignedTransaction.tx"))?;
    let mut consensus_tx = wire_to_consensus_tx(tx_msg)?;

    let mut new_inputs: Vec<TransactionInput> = Vec::with_capacity(consensus_tx.inputs.len());
    for (idx, psi) in pst.partially_signed_inputs.iter().enumerate() {
        let consensus_input = consensus_tx.inputs.get(idx).ok_or(SignError::Missing("PartiallySignedTransaction.tx.inputs[idx]"))?;
        let is_multisig = psi.pub_key_signature_pairs.len() > 1;
        let sig_script =
            if is_multisig { build_multisig_signature_script(psi, ecdsa)? } else { build_singlekey_signature_script(psi)? };
        let sig_op_count = u8::try_from(psi.pub_key_signature_pairs.len()).map_err(|_| SignError::Invalid {
            field: "PartiallySignedInput.pairs",
            reason: format!("count {} exceeds u8::MAX", psi.pub_key_signature_pairs.len()),
        })?;
        new_inputs.push(TransactionInput::new(consensus_input.previous_outpoint, sig_script, consensus_input.sequence, sig_op_count));
    }

    consensus_tx = Transaction::new(
        consensus_tx.version,
        new_inputs,
        consensus_tx.outputs.clone(),
        consensus_tx.lock_time,
        consensus_tx.subnetwork_id.clone(),
        consensus_tx.gas,
        consensus_tx.payload.clone(),
    );
    Ok(consensus_tx)
}

fn build_singlekey_signature_script(psi: &wire::PartiallySignedInput) -> Result<Vec<u8>, SignError> {
    let pair = psi.pub_key_signature_pairs.first().ok_or(SignError::Missing("PartiallySignedInput.pubKeySignaturePairs[0]"))?;
    if pair.signature.is_empty() {
        return Err(SignError::Invalid {
            field: "PubKeySignaturePair.signature",
            reason: "single-cosigner sigscript: signature missing".to_owned(),
        });
    }
    let mut builder = ScriptBuilder::new();
    builder.add_data(&pair.signature).map_err(script_builder_to_sign_err)?;
    Ok(builder.drain())
}

fn build_multisig_signature_script(psi: &wire::PartiallySignedInput, ecdsa: bool) -> Result<Vec<u8>, SignError> {
    let mut builder = ScriptBuilder::new();
    let mut sig_count: u32 = 0;
    for pair in &psi.pub_key_signature_pairs {
        if pair.signature.is_empty() {
            continue;
        }
        builder.add_data(&pair.signature).map_err(script_builder_to_sign_err)?;
        sig_count += 1;
    }
    if sig_count < psi.minimum_signatures {
        return Err(SignError::Invalid {
            field: "PartiallySignedInput",
            reason: format!("missing {} signature(s)", psi.minimum_signatures - sig_count),
        });
    }

    let redeem_script = redeem_script_for_input(psi, ecdsa)?;
    builder.add_data(&redeem_script).map_err(script_builder_to_sign_err)?;
    Ok(builder.drain())
}

/// Build the redeem script bound to a multisig input's pair set:
/// extract each pair's `extended_pub_key`, deserialize it, take
/// its serialized public key (32-byte x-only for Schnorr, 33-byte
/// compressed for ECDSA), and build the M-of-N redeem script via
/// `kaspa_txscript::standard::multisig::multisig_redeem_script*`.
fn redeem_script_for_input(psi: &wire::PartiallySignedInput, ecdsa: bool) -> Result<Vec<u8>, SignError> {
    let required = psi.minimum_signatures as usize;
    if ecdsa {
        let pubkeys = psi
            .pub_key_signature_pairs
            .iter()
            .map(|pair| ecdsa_serialized_pubkey_from_xpub(&pair.extended_pub_key))
            .collect::<Result<Vec<[u8; ECDSA_PUBKEY_LEN]>, SignError>>()?;
        multisig_redeem_script_ecdsa(pubkeys.iter(), required).map_err(multisig_err_to_sign_err)
    } else {
        let pubkeys = psi
            .pub_key_signature_pairs
            .iter()
            .map(|pair| schnorr_serialized_pubkey_from_xpub(&pair.extended_pub_key))
            .collect::<Result<Vec<[u8; SCHNORR_PUBKEY_LEN]>, SignError>>()?;
        multisig_redeem_script(pubkeys.iter(), required).map_err(multisig_err_to_sign_err)
    }
}

fn schnorr_serialized_pubkey_from_xpub(xpub_str: &str) -> Result<[u8; SCHNORR_PUBKEY_LEN], SignError> {
    let xpub: ExtendedPublicKey<secp256k1::PublicKey> = xpub_str.parse()?;
    Ok(xpub.public_key().x_only_public_key().0.serialize())
}

fn ecdsa_serialized_pubkey_from_xpub(xpub_str: &str) -> Result<[u8; ECDSA_PUBKEY_LEN], SignError> {
    let xpub: ExtendedPublicKey<secp256k1::PublicKey> = xpub_str.parse()?;
    Ok(xpub.public_key().serialize())
}

fn script_builder_to_sign_err(err: kaspa_txscript::script_builder::ScriptBuilderError) -> SignError {
    SignError::Invalid { field: "signatureScript", reason: format!("script builder: {err}") }
}

fn multisig_err_to_sign_err(err: MultisigCreateError) -> SignError {
    SignError::Invalid { field: "multisigRedeemScript", reason: err.to_string() }
}
