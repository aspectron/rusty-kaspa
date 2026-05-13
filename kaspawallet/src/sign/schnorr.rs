//! Schnorr-only sign flow.
//!
//! Reuses rusty-kaspa surfaces end-to-end:
//!
//! - `kaspa_bip32::Mnemonic` + `Mnemonic::to_seed` for the
//!   BIP-39 mnemonic -> seed conversion.
//! - `kaspa_bip32::ExtendedPrivateKey::new(seed)` for the BIP-32
//!   master key.
//! - `ExtendedPrivateKey::derive_path(...)` for the
//!   `m/<purpose>'/<coin>'/<account>'` cosigner-prefix walk and
//!   the subsequent relative-path walk to the leaf signing key.
//!   See `super::derive` for the chain shape.
//! - `kaspa_consensus_core::hashing::sighash::calc_schnorr_signature_hash`
//!   for the per-input sighash digest.
//! - `secp256k1::Keypair::sign_schnorr` for the actual Schnorr
//!   signing operation. BIP-340 mixes auxiliary randomness into
//!   nonce derivation, so the per-signature bytes are NOT
//!   byte-identical between independent signings of the same
//!   message; the cross-wallet sign-flow contract's operative
//!   property is signature VALIDITY under the cosigner's derived
//!   x-only pubkey on the canonical sighash, not byte-equality of
//!   signature bytes.

use kaspa_bip32::{ExtendedPrivateKey, Mnemonic, SecretKey};
use kaspa_consensus_core::hashing::sighash::{SigHashReusedValuesUnsync, calc_schnorr_signature_hash};
use kaspa_consensus_core::hashing::sighash_type::SIG_HASH_ALL;
use kaspa_consensus_core::tx::SignableTransaction;

use super::derive::derive_leaf_and_match_pair;
use super::error::SignError;
use super::wire::{apply_sig_op_count_from_psi, build_utxo_entries, wire_to_consensus_tx};
use crate::serialization::wire;

/// Sign every input of a `PartiallySignedTransaction` whose
/// `PubKeySignaturePair` matches a leaf key derived from the
/// given mnemonic at the input's `derivation_path`. Updates the
/// matching pair's `signature` field in place with the raw
/// 64-byte Schnorr signature.
///
/// This is the **single-cosigner Schnorr** entry point. Multisig
/// partial-sig combination is the responsibility of the caller.
///
/// `bip39_passphrase` is the BIP-39 passphrase the seed
/// derivation uses (NOT the Argon2id-protected keyfile password).
/// The keyfile-encrypted wallet passes the empty string -- the
/// keyfile is the at-rest secret and the BIP-39 layer rides
/// empty.
pub fn sign_pst_schnorr_with_mnemonic(
    pst: &mut wire::PartiallySignedTransaction,
    mnemonic_phrase: &str,
    bip39_passphrase: &str,
) -> Result<(), SignError> {
    // Early-return if every input already has at least
    // `minimum_signatures` non-empty signatures. This guard
    // prevents a multisig wallet's keyfile (which may hold N
    // cosigner mnemonics for an M-of-N wallet) from over-signing
    // every input -- the on-chain sigscript assembly later pushes
    // EVERY non-empty signature, so an over-signed PST produces a
    // sigscript with one extra signature push that kaspad's
    // `OpCheckMultiSig` evaluation surfaces as "stack contains 1
    // unexpected items".
    if crate::sign::is_pst_fully_signed(pst) {
        return Ok(());
    }

    // Sync each `TransactionInput.sig_op_count` to its
    // PartiallySignedInput pair count BEFORE sighash computation.
    // The consensus tx that `extract_transaction` ultimately
    // produces carries this updated `sig_op_count`; if sighash
    // were computed against `sig_op_count = 0` from the
    // wire-default the resulting signature would not verify
    // against the script kaspad reconstructs from the broadcasted
    // tx ("false stack entry at end of script execution").
    apply_sig_op_count_from_psi(pst)?;

    let tx_msg = pst.tx.as_ref().ok_or(SignError::Missing("PartiallySignedTransaction.tx"))?;
    let consensus_tx = wire_to_consensus_tx(tx_msg)?;
    let entries = build_utxo_entries(pst)?;
    let signable = SignableTransaction::with_entries(consensus_tx, entries);

    let mnemonic = Mnemonic::new(mnemonic_phrase, kaspa_bip32::Language::English)?;
    let seed = mnemonic.to_seed(bip39_passphrase);
    let master = ExtendedPrivateKey::<SecretKey>::new(seed.as_bytes())?;
    let secp = secp256k1::SECP256K1;

    let reused = SigHashReusedValuesUnsync::new();

    for (input_index, psi) in pst.partially_signed_inputs.iter_mut().enumerate() {
        let (leaf, pair_index) = derive_leaf_and_match_pair(&master, psi, input_index)?;
        let secret_key = *leaf.private_key();
        let keypair = secp256k1::Keypair::from_secret_key(secp, &secret_key);

        let sighash = calc_schnorr_signature_hash(&signable.as_verifiable(), input_index, SIG_HASH_ALL, &reused);
        let msg = secp256k1::Message::from_digest_slice(sighash.as_bytes().as_slice())?;
        let sig: [u8; 64] = *keypair.sign_schnorr(msg).as_ref();

        // Append the sigHashType byte. The resulting 65-byte
        // blob is what the multisig combination layer packs into
        // the on-chain signature script via OP_DATA_65 PUSHDATA
        // framing.
        let mut sig_with_hash_type = Vec::with_capacity(sig.len() + 1);
        sig_with_hash_type.extend_from_slice(&sig);
        sig_with_hash_type.push(SIG_HASH_ALL.to_u8());

        psi.pub_key_signature_pairs[pair_index].signature = sig_with_hash_type;
    }

    Ok(())
}
