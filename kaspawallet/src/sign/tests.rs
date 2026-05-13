//! Tests for the single-cosigner sign flows (Schnorr + ECDSA).
//!
//! Schnorr coverage: decrypt the keyfile's mnemonic, sign the
//! captured PST fixture, verify the resulting signature against
//! the cosigner's derived leaf x-only public key on the same
//! sighash.
//!
//! ECDSA coverage: (a) determinism gate proving the underlying
//! `secp256k1` crate uses RFC 6979 deterministic nonces; (b)
//! end-to-end sign + verify against a derived leaf pubkey; (c)
//! cosigner-mismatch rejection.

use std::path::PathBuf;
use std::str::FromStr;

use kaspa_bip32::{DerivationPath, ExtendedPrivateKey, ExtendedPublicKey, Language, Mnemonic, SecretKey};
use kaspa_consensus_core::hashing::sighash::{SigHashReusedValuesUnsync, calc_ecdsa_signature_hash, calc_schnorr_signature_hash};
use kaspa_consensus_core::hashing::sighash_type::SIG_HASH_ALL;
use kaspa_consensus_core::tx::SignableTransaction;

use super::ecdsa::sign_pst_ecdsa_with_mnemonic;
use super::error::SignError;
use super::schnorr::sign_pst_schnorr_with_mnemonic;
use super::wire::{build_utxo_entries, wire_to_consensus_tx};
use crate::keyfile;
use crate::serialization::{deserialize_partially_signed_transaction, wire};

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn read_pst_fixture() -> wire::PartiallySignedTransaction {
    let hex_str = std::fs::read_to_string(fixture("go_emitted_pst.hex")).unwrap();
    let bytes = hex::decode(hex_str.trim()).unwrap();
    deserialize_partially_signed_transaction(&bytes).unwrap()
}

/// Decode the keyfile, decrypt the mnemonic with the documented
/// fixture passphrase, and return the mnemonic phrase.
fn cosigner_mnemonic() -> String {
    let kf = keyfile::read_from_path(fixture("legacy_go_v1_singlekey.json")).unwrap();
    let mnemonics = keyfile::decrypt::decrypt_mnemonics(&kf, b"test fixture passphrase").unwrap();
    mnemonics.first().cloned().unwrap()
}

/// Independently derive the leaf xpriv for the input via master
/// xpriv -> single-signer cosigner prefix `m/44'/111111'/0'` ->
/// relative `psi.derivation_path` walk. Used by the per-mode
/// end-to-end tests to recover the leaf pubkey against which the
/// signature is verified.
fn derive_leaf_for_test(mnemonic_phrase: &str, psi: &wire::PartiallySignedInput) -> ExtendedPrivateKey<SecretKey> {
    let mnemonic = Mnemonic::new(mnemonic_phrase, Language::English).unwrap();
    let seed = mnemonic.to_seed("");
    let master = ExtendedPrivateKey::<SecretKey>::new(seed.as_bytes()).unwrap();
    let cosigner_prefix = DerivationPath::from_str("m/44'/111111'/0'").unwrap();
    let cosigner = master.derive_path(&cosigner_prefix).unwrap();
    let relative = DerivationPath::from_str(&psi.derivation_path).unwrap();
    cosigner.derive_path(&relative).unwrap()
}

#[test]
fn test_sign_schnorr_single_signer_against_fixture() {
    let mnemonic_phrase = cosigner_mnemonic();
    let mut pst = read_pst_fixture();
    let expected_xpub_prefix = "ktub";

    assert_eq!(pst.partially_signed_inputs.len(), 1);
    let pre_pair = &pst.partially_signed_inputs[0].pub_key_signature_pairs[0];
    assert!(pre_pair.extended_pub_key.starts_with(expected_xpub_prefix));
    assert!(pre_pair.signature.is_empty(), "fixture is the pre-sign baseline");

    sign_pst_schnorr_with_mnemonic(&mut pst, &mnemonic_phrase, "").expect("sign succeeds");

    let pair = &pst.partially_signed_inputs[0].pub_key_signature_pairs[0];
    assert_eq!(pair.signature.len(), 65, "Schnorr signature is 64 sig bytes plus 1 sigHashType byte");
    assert_eq!(pair.signature[64], 0x01, "sigHashType byte is SIG_HASH_ALL (0x01)");

    let tx_msg = pst.tx.as_ref().unwrap();
    let psi = &pst.partially_signed_inputs[0];
    let consensus_tx = wire_to_consensus_tx(tx_msg).unwrap();
    let entries = build_utxo_entries(&pst).unwrap();
    let signable = SignableTransaction::with_entries(consensus_tx, entries);
    let reused = SigHashReusedValuesUnsync::new();
    let sighash = calc_schnorr_signature_hash(&signable.as_verifiable(), 0, SIG_HASH_ALL, &reused);
    let msg = secp256k1::Message::from_digest_slice(sighash.as_bytes().as_slice()).unwrap();

    let leaf = derive_leaf_for_test(&mnemonic_phrase, psi);
    let leaf_xpub: ExtendedPublicKey<secp256k1::PublicKey> = (&leaf).into();
    let xonly = leaf_xpub.public_key().x_only_public_key().0;

    let sig = secp256k1::schnorr::Signature::from_slice(&pair.signature[..64]).expect("64 sig bytes parse");
    sig.verify(&msg, &xonly).expect("signature verifies under leaf cosigner pubkey");
}

#[test]
fn test_sign_schnorr_rejects_when_cosigner_xpub_does_not_match() {
    let mnemonic_phrase = cosigner_mnemonic();
    let mut pst = read_pst_fixture();
    pst.partially_signed_inputs[0].pub_key_signature_pairs[0].extended_pub_key =
        "ktub2deliberatelybogusxpubstringthatcannotpossiblymatch".into();

    match sign_pst_schnorr_with_mnemonic(&mut pst, &mnemonic_phrase, "") {
        Ok(_) => panic!("must not sign when no pair matches the cosigner"),
        Err(SignError::CosignerMismatch { input_index }) => assert_eq!(input_index, 0),
        Err(other) => panic!("expected CosignerMismatch, got {other:?}"),
    }
}

#[test]
fn test_sign_propagates_invalid_derivation_path() {
    let mnemonic_phrase = cosigner_mnemonic();
    let mut pst = read_pst_fixture();
    pst.partially_signed_inputs[0].derivation_path = "not a derivation path".into();

    let err = sign_pst_schnorr_with_mnemonic(&mut pst, &mnemonic_phrase, "").expect_err("invalid path must fail");
    let msg = format!("{err}");
    assert!(msg.contains("bip32") || msg.contains("path"), "error must surface the bip32 cause: {msg}");
}

/// Determinism gate proving the underlying `secp256k1` crate
/// produces byte-identical ECDSA signatures on independent
/// invocations with the same `(privkey, message)` pair. If this
/// property is ever lost (e.g. the crate switches to
/// randomized-nonce signing for the same call shape), parity
/// tests against reference ECDSA-mode signatures will start
/// failing -- catching it here at unit-test time keeps the
/// cross-binary contract tractable.
#[test]
fn test_ecdsa_uses_rfc6979() {
    let sk = secp256k1::SecretKey::from_slice(&[
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
        0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
    ])
    .expect("test vector is a valid scalar");
    let msg = secp256k1::Message::from_digest([0xaa; 32]);

    let sig_a = sk.sign_ecdsa(msg).serialize_compact();
    let sig_b = sk.sign_ecdsa(msg).serialize_compact();
    let sig_c = sk.sign_ecdsa(msg).serialize_compact();

    assert_eq!(sig_a, sig_b, "RFC 6979 deterministic nonce -- repeated sign returns identical bytes");
    assert_eq!(sig_b, sig_c, "RFC 6979 deterministic nonce -- third sign also identical");
}

#[test]
fn test_sign_ecdsa_single_signer_against_fixture() {
    let mnemonic_phrase = cosigner_mnemonic();
    let mut pst = read_pst_fixture();

    assert_eq!(pst.partially_signed_inputs.len(), 1);
    assert!(pst.partially_signed_inputs[0].pub_key_signature_pairs[0].signature.is_empty(), "fixture is the pre-sign baseline");

    sign_pst_ecdsa_with_mnemonic(&mut pst, &mnemonic_phrase, "").expect("ECDSA sign succeeds");

    let pair = &pst.partially_signed_inputs[0].pub_key_signature_pairs[0];
    assert_eq!(pair.signature.len(), 65, "compact ECDSA signature is 64 sig bytes plus 1 sigHashType byte");
    assert_eq!(pair.signature[64], 0x01, "sigHashType byte is SIG_HASH_ALL (0x01)");

    let tx_msg = pst.tx.as_ref().unwrap();
    let psi = &pst.partially_signed_inputs[0];
    let consensus_tx = wire_to_consensus_tx(tx_msg).unwrap();
    let entries = build_utxo_entries(&pst).unwrap();
    let signable = SignableTransaction::with_entries(consensus_tx, entries);
    let reused = SigHashReusedValuesUnsync::new();
    let sighash = calc_ecdsa_signature_hash(&signable.as_verifiable(), 0, SIG_HASH_ALL, &reused);
    let msg = secp256k1::Message::from_digest_slice(sighash.as_bytes().as_slice()).unwrap();

    let leaf = derive_leaf_for_test(&mnemonic_phrase, psi);
    let leaf_xpub: ExtendedPublicKey<secp256k1::PublicKey> = (&leaf).into();
    let pubkey = leaf_xpub.public_key();

    let sig = secp256k1::ecdsa::Signature::from_compact(&pair.signature[..64]).expect("64 compact bytes parse");
    sig.verify(&msg, pubkey).expect("ECDSA signature verifies under leaf cosigner pubkey");
}

#[test]
fn test_sign_ecdsa_repeated_invocation_is_byte_identical() {
    let mnemonic_phrase = cosigner_mnemonic();
    let mut pst_a = read_pst_fixture();
    let mut pst_b = read_pst_fixture();

    sign_pst_ecdsa_with_mnemonic(&mut pst_a, &mnemonic_phrase, "").expect("ECDSA sign A succeeds");
    sign_pst_ecdsa_with_mnemonic(&mut pst_b, &mnemonic_phrase, "").expect("ECDSA sign B succeeds");

    let sig_a = &pst_a.partially_signed_inputs[0].pub_key_signature_pairs[0].signature;
    let sig_b = &pst_b.partially_signed_inputs[0].pub_key_signature_pairs[0].signature;
    assert_eq!(sig_a, sig_b, "ECDSA signing on the same PST + same key must be byte-identical (RFC 6979)");
}

#[test]
fn test_sign_ecdsa_rejects_when_cosigner_xpub_does_not_match() {
    let mnemonic_phrase = cosigner_mnemonic();
    let mut pst = read_pst_fixture();
    pst.partially_signed_inputs[0].pub_key_signature_pairs[0].extended_pub_key =
        "ktub2deliberatelybogusxpubstringthatcannotpossiblymatch".into();

    match sign_pst_ecdsa_with_mnemonic(&mut pst, &mnemonic_phrase, "") {
        Ok(_) => panic!("must not sign when no pair matches the cosigner"),
        Err(SignError::CosignerMismatch { input_index }) => assert_eq!(input_index, 0),
        Err(other) => panic!("expected CosignerMismatch, got {other:?}"),
    }
}

/// Multisig over-sign guard: a 2-of-3 wallet whose keyfile holds
/// all three cosigner mnemonics must produce a PST with exactly
/// `minimum_signatures` non-empty signatures after the
/// per-mnemonic sign loop, NOT the full set of three. An
/// over-signed PST would cause `extract_transaction` to push all
/// three sigs into the on-chain sigscript, leaving an unconsumed
/// extra item on the stack after `OpCheckMultiSig` ("stack
/// contains 1 unexpected items"). The fix is the
/// `is_pst_fully_signed` early-return at the head of both
/// per-mnemonic sign flows.
#[test]
fn test_multisig_2of3_over_sign_guard() {
    let kf = keyfile::read_from_path(fixture("legacy_go_v1_multisig_2of3.json")).unwrap();
    let mnemonics = keyfile::decrypt::decrypt_mnemonics(&kf, b"multisig test passphrase").unwrap();
    assert_eq!(mnemonics.len(), 3, "fixture must hold 3 mnemonics");
    assert_eq!(kf.minimum_signatures, 2, "fixture must be a 2-of-3 wallet");
    assert_eq!(kf.extended_public_keys.len(), 3, "fixture must store 3 cosigner xpubs");

    // Derive the leaf-level xpub for each mnemonic at the
    // multisig cosigner prefix + relative derivation path
    // `m/45'/111111'/0'/0/0`. Building the PST with leaf-level
    // xpubs (not the cosigner-level xpubs stored in the keyfile)
    // mirrors what the unsigned-transaction-creation path does
    // for production multisig.
    const RELATIVE_PATH: &str = "m/0/0";
    let leaf_xpub_strs: Vec<String> = mnemonics
        .iter()
        .map(|m| {
            let mnemonic = Mnemonic::new(m, Language::English).unwrap();
            let seed = mnemonic.to_seed("");
            let master = ExtendedPrivateKey::<SecretKey>::new(seed.as_bytes()).unwrap();
            let cosigner = master.derive_path(&DerivationPath::from_str("m/45'/111111'/0'").unwrap()).unwrap();
            let leaf = cosigner.derive_path(&DerivationPath::from_str(RELATIVE_PATH).unwrap()).unwrap();
            let leaf_xpub: ExtendedPublicKey<secp256k1::PublicKey> = (&leaf).into();
            leaf_xpub.to_string(Some(kaspa_bip32::Prefix::KTUB))
        })
        .collect();

    // Build a synthetic multisig 2-of-3 PST with one input. The
    // input's `prev_output.scriptPublicKey` is a placeholder
    // (sighash computation does NOT execute the script -- it only
    // hashes the bytes); the load-bearing assertion is on the
    // sign-flow's over-sign guard.
    let pst_before = wire::PartiallySignedTransaction {
        tx: Some(wire::TransactionMessage {
            version: 0,
            inputs: vec![wire::TransactionInput {
                previous_outpoint: Some(wire::Outpoint {
                    transaction_id: Some(wire::TransactionId { bytes: vec![0u8; 32] }),
                    index: 0,
                }),
                signature_script: Vec::new(),
                sequence: 0,
                sig_op_count: 0,
            }],
            outputs: vec![wire::TransactionOutput {
                value: 1_000,
                script_public_key: Some(wire::ScriptPublicKey { version: 0, script: vec![0x51] }),
            }],
            lock_time: 0,
            subnetwork_id: Some(wire::SubnetworkId { bytes: vec![0u8; 20] }),
            gas: 0,
            payload: Vec::new(),
        }),
        partially_signed_inputs: vec![wire::PartiallySignedInput {
            redeem_script: Vec::new(),
            prev_output: Some(wire::TransactionOutput {
                value: 1_000,
                script_public_key: Some(wire::ScriptPublicKey { version: 0, script: vec![0x51] }),
            }),
            minimum_signatures: 2,
            pub_key_signature_pairs: leaf_xpub_strs
                .iter()
                .map(|x| wire::PubKeySignaturePair { extended_pub_key: x.clone(), signature: Vec::new() })
                .collect(),
            derivation_path: RELATIVE_PATH.to_owned(),
        }],
    };

    // Iterate every mnemonic in the keyfile unconditionally. The
    // over-sign guard inside `sign_pst_schnorr_with_mnemonic`
    // must early-return on the 3rd call, leaving the 3rd pair's
    // signature empty.
    let mut pst = pst_before.clone();
    for mnemonic in mnemonics.iter() {
        sign_pst_schnorr_with_mnemonic(&mut pst, mnemonic, "").expect("sign succeeds");
    }

    let filled: usize = pst.partially_signed_inputs[0].pub_key_signature_pairs.iter().filter(|p| !p.signature.is_empty()).count();
    assert_eq!(
        filled, 2,
        "after 3 sign calls on a 2-of-3 wallet the PST must hold exactly 2 non-empty signatures (= minimum_signatures), not 3"
    );

    assert!(super::is_pst_fully_signed(&pst), "PST must report fully-signed after minimum_signatures cosigners signed");
}
