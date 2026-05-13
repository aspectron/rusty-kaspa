//! Estimate post-signing mass of an unsigned
//! `PartiallySignedTransaction`.
//!
//! Each `PubKeySignaturePair.Signature` is padded with a junk byte
//! vector of the same length the eventual real signature will
//! occupy; the input's `SigOpCount` is set to the cosigner count;
//! the resulting bytes are packed into a signature script; the
//! transaction is then handed to the consensus mass calculator for
//! the **overall** mass (`max(compute_mass, storage_mass)`). That
//! number is what `parse` reports as "Mass: N grams" and what
//! `send` uses to decide the fee-rate.
//!
//! This module is integration glue between `crate::sign::wire`
//! (which lifts the proto-wire PST into consensus-core types) and
//! `kaspa_consensus_core::mass::MassCalculator`; no mass formula
//! is re-implemented here.

use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::mass::MassCalculator;
use kaspa_consensus_core::tx::{SignableTransaction, Transaction};

use crate::serialization::wire;
use crate::sign::SignError;
use crate::sign::extract_transaction;
use crate::sign::wire::build_utxo_entries;

/// Compact signature size in bytes for both Schnorr (BIP-340)
/// and kaspa's compact ECDSA wire form. The on-chain signature
/// script also carries a 1-byte sigHashType appendage, so the
/// total payload pushed for one signature is `SIGNATURE_LEN + 1`
/// bytes.
pub const SIGNATURE_LEN: usize = 64;

/// Estimate the overall mass of the transaction the input PST
/// will produce once every cosigner up to `minimum_signatures`
/// has contributed a signature. The estimator runs:
///
/// 1. Clone the PST.
/// 2. For each input, walk its `PubKeySignaturePairs`. For the
///    first `minimum_signatures` of them, write a placeholder
///    signature blob of `SIGNATURE_LEN + 1` bytes (the +1 for the
///    eventual sigHashType byte). Set `Tx.Inputs[i].SigOpCount =
///    len(PubKeySignaturePairs)`.
/// 3. Run `ExtractTransactionDeserialized` on the junk-filled PST to
///    produce a fully populated `DomainTransaction`.
/// 4. Ask the mass calculator for the **overall** mass:
///    `max(compute_mass, storage_mass)`.
///
/// The Rust port handles steps 1-3 inline (we don't reuse a
/// generic `ExtractTransaction` here because we only need the
/// shape, not the actual signature-script bytes; building a
/// signature-script-shaped buffer of the correct LENGTH is
/// sufficient for the mass calculation).
pub fn estimate_mass_after_signatures(pst: &wire::PartiallySignedTransaction, params: &Params, ecdsa: bool) -> Result<u64, SignError> {
    let (consensus_tx, entries) = junk_filled_consensus_tx(pst, ecdsa)?;
    let (compute, storage) = compute_and_storage_mass(params, consensus_tx, entries);
    Ok(compute.max(storage))
}

/// Clones the PST, fills the first `minimum_signatures` pairs per
/// input with a placeholder signature blob of `SIGNATURE_LEN + 1`
/// bytes (the +1 for the sigHashType byte), and routes the junk
/// PST through `extract_transaction` so the resulting consensus
/// tx carries the same sigscript shape (push opcodes + redeem-
/// script push for multisig) the post-real-signing tx will have.
/// Counting bytes against a hand-built
/// `sig_script = vec![0u8; SIGNATURE_LEN + 1]` underestimates the
/// mass by 1 byte per signature (the `OP_DATA_65` push opcode)
/// and -- for multisig -- by the entire redeem-script push (which
/// `ScriptBuilder::add_data(redeem_script)` adds via
/// `OP_PUSHDATA1` + 1-byte length + the redeem-script bytes). The
/// under-count was operationally fatal on fee-tight outputs
/// (2-sompi shortfall on tn-10).
fn junk_filled_consensus_tx(
    pst: &wire::PartiallySignedTransaction,
    ecdsa: bool,
) -> Result<(Transaction, Vec<kaspa_consensus_core::tx::UtxoEntry>), SignError> {
    let entries = build_utxo_entries(pst)?;
    let mut junk = pst.clone();

    // Set `Tx.Inputs[i].SigOpCount = len(PubKeySignaturePairs)`.
    // The matching pre-sighash sync in the sign module
    // (`apply_sig_op_count_from_psi`) covers the real-signing
    // path; this junk-fill duplicates it because the PST passed
    // here is typically unsigned (just emerged from
    // `create_unsigned_transaction`).
    let tx = junk.tx.as_mut().ok_or(SignError::Missing("PartiallySignedTransaction.tx"))?;
    if tx.inputs.len() != junk.partially_signed_inputs.len() {
        return Err(SignError::Invalid {
            field: "PartiallySignedTransaction",
            reason: format!(
                "tx.inputs.len()={} != partially_signed_inputs.len()={}",
                tx.inputs.len(),
                junk.partially_signed_inputs.len()
            ),
        });
    }
    for (input, psi) in tx.inputs.iter_mut().zip(junk.partially_signed_inputs.iter()) {
        let count: u32 = u32::try_from(psi.pub_key_signature_pairs.len()).map_err(|_| SignError::Invalid {
            field: "PartiallySignedInput.pairs",
            reason: format!("count {} exceeds u32::MAX", psi.pub_key_signature_pairs.len()),
        })?;
        input.sig_op_count = count;
    }

    // Fill the first `minimum_signatures` pairs with junk
    // `SIGNATURE_LEN + 1`-byte blobs. The remaining pairs stay
    // empty.
    let junk_sig: Vec<u8> = vec![0u8; SIGNATURE_LEN + 1];
    for psi in junk.partially_signed_inputs.iter_mut() {
        let min_sigs = psi.minimum_signatures as usize;
        let upper = min_sigs.min(psi.pub_key_signature_pairs.len());
        for pair in psi.pub_key_signature_pairs.iter_mut().take(upper) {
            pair.signature = junk_sig.clone();
        }
    }

    // The `ecdsa` flag selects the multisig redeem-script's opcode
    // (`OpCheckMultiSig` vs `OpCheckMultiSigECDSA`) and the
    // cosigner pubkey serialization (32-byte x-only Schnorr vs
    // 33-byte compressed ECDSA). Both shapes affect the assembled
    // sigscript size by 1-2 bytes per cosigner, so the mass calc
    // must use the right one. The flag is plumbed through from the
    // wallet's keyfile via [`crate::coinsel::WalletConfig`].
    let consensus_tx = extract_transaction(&junk, ecdsa)?;

    Ok((consensus_tx, entries))
}

/// Returns `(compute_mass, storage_mass)` for the lifted consensus
/// transaction. Both estimators use this; only the combiner differs.
fn compute_and_storage_mass(
    params: &Params,
    consensus_tx: Transaction,
    entries: Vec<kaspa_consensus_core::tx::UtxoEntry>,
) -> (u64, u64) {
    let mc = MassCalculator::new_with_consensus_params(params);
    let signable = SignableTransaction::with_entries(consensus_tx, entries);
    let nc = mc.calc_non_contextual_masses(&signable.tx);
    let cm = mc.calc_contextual_masses(&signable.as_verifiable()).map(|m| m.storage_mass).unwrap_or(0);
    (nc.compute_mass, cm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspa_consensus_core::network::NetworkType;

    fn load_fixture_pst() -> wire::PartiallySignedTransaction {
        use crate::serialization::deserialize_partially_signed_transaction;
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("fixtures");
        p.push("go_emitted_pst.hex");
        let hex_str = std::fs::read_to_string(p).unwrap();
        let bytes = hex::decode(hex_str.trim()).unwrap();
        deserialize_partially_signed_transaction(&bytes).unwrap()
    }

    #[test]
    fn test_estimate_mass_after_signatures_returns_positive_for_singlekey_pst() {
        let pst = load_fixture_pst();
        let params = Params::from(NetworkType::Testnet);
        let mass = estimate_mass_after_signatures(&pst, &params, false).expect("mass calc succeeds");
        assert!(mass > 0, "mass for a non-coinbase tx with one input and two outputs must be positive");
    }

    #[test]
    fn test_estimate_mass_is_deterministic() {
        let pst = load_fixture_pst();
        let params = Params::from(NetworkType::Testnet);
        let m1 = estimate_mass_after_signatures(&pst, &params, false).expect("mass calc 1");
        let m2 = estimate_mass_after_signatures(&pst, &params, false).expect("mass calc 2");
        assert_eq!(m1, m2, "mass calc must be deterministic on the same PST + params");
    }
}
