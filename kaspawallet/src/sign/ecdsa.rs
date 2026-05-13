//! ECDSA single-cosigner sign flow.
//!
//! Shares the derivation and cosigner-match logic with
//! `super::schnorr`; the differences are the sighash function
//! (`calc_ecdsa_signature_hash` instead of
//! `calc_schnorr_signature_hash`) and the signing surface
//! (`SecretKey::sign_ecdsa` instead of `Keypair::sign_schnorr`).
//!
//! `secp256k1::SecretKey::sign_ecdsa` uses RFC 6979 deterministic
//! nonces (no auxiliary randomness, no `OsRng`-sourced seed), so
//! the per-signature bytes are byte-identical between independent
//! signings of the same `(privkey, message)` pair. The
//! cross-wallet sign-flow contract for ECDSA-mode keyfiles asserts
//! byte-equality on the signature positions; this property is
//! verified by `super::tests::test_ecdsa_uses_rfc6979`.
//!
//! The 64-byte compact signature is stored in the matching
//! `PubKeySignaturePair.signature` field of the
//! `PartiallySignedTransaction`; the assembled `signature_script`
//! is built later at broadcast time when the multisig flow
//! combines the per-cosigner signatures.

use kaspa_bip32::{ExtendedPrivateKey, Mnemonic, SecretKey};
use kaspa_consensus_core::hashing::sighash::{SigHashReusedValuesUnsync, calc_ecdsa_signature_hash};
use kaspa_consensus_core::hashing::sighash_type::SIG_HASH_ALL;
use kaspa_consensus_core::tx::SignableTransaction;

use super::derive::derive_leaf_and_match_pair;
use super::error::SignError;
use super::wire::{build_utxo_entries, wire_to_consensus_tx};
use crate::serialization::wire;

/// Sign every input of a `PartiallySignedTransaction` in
/// ECDSA mode. Updates each matching pair's `signature` field in
/// place with the raw 64-byte compact ECDSA signature.
///
/// `bip39_passphrase` is the BIP-39 passphrase the seed
/// derivation uses (NOT the Argon2id-protected keyfile
/// password). The keyfile-encrypted wallet passes the empty
/// string.
pub fn sign_pst_ecdsa_with_mnemonic(
    pst: &mut wire::PartiallySignedTransaction,
    mnemonic_phrase: &str,
    bip39_passphrase: &str,
) -> Result<(), SignError> {
    // Early-return if every input already has at least
    // `minimum_signatures` non-empty signatures -- the
    // multisig over-sign guard. See the same comment block in
    // `super::schnorr::sign_pst_schnorr_with_mnemonic`.
    if crate::sign::is_pst_fully_signed(pst) {
        return Ok(());
    }

    // Sync each `TransactionInput.sig_op_count` to its
    // PartiallySignedInput pair count BEFORE sighash. See the
    // comment block in
    // `super::schnorr::sign_pst_schnorr_with_mnemonic` for the
    // consensus-tx-vs-wire-tx sighash mismatch this prevents.
    super::wire::apply_sig_op_count_from_psi(pst)?;

    let tx_msg = pst.tx.as_ref().ok_or(SignError::Missing("PartiallySignedTransaction.tx"))?;
    let consensus_tx = wire_to_consensus_tx(tx_msg)?;
    let entries = build_utxo_entries(pst)?;
    let signable = SignableTransaction::with_entries(consensus_tx, entries);

    let mnemonic = Mnemonic::new(mnemonic_phrase, kaspa_bip32::Language::English)?;
    let seed = mnemonic.to_seed(bip39_passphrase);
    let master = ExtendedPrivateKey::<SecretKey>::new(seed.as_bytes())?;

    let reused = SigHashReusedValuesUnsync::new();

    for (input_index, psi) in pst.partially_signed_inputs.iter_mut().enumerate() {
        let (leaf, pair_index) = derive_leaf_and_match_pair(&master, psi, input_index)?;
        let secret_key = *leaf.private_key();

        let sighash = calc_ecdsa_signature_hash(&signable.as_verifiable(), input_index, SIG_HASH_ALL, &reused);
        let msg = secp256k1::Message::from_digest_slice(sighash.as_bytes().as_slice())?;
        let sig = secret_key.sign_ecdsa(msg);
        let compact: [u8; 64] = sig.serialize_compact();

        // Append the sigHashType byte. The 65-byte blob is what
        // the multisig combination layer packs into the on-chain
        // signature script via OP_DATA_65 PUSHDATA framing.
        let mut sig_with_hash_type = Vec::with_capacity(compact.len() + 1);
        sig_with_hash_type.extend_from_slice(&compact);
        sig_with_hash_type.push(SIG_HASH_ALL.to_u8());

        psi.pub_key_signature_pairs[pair_index].signature = sig_with_hash_type;
    }

    Ok(())
}
