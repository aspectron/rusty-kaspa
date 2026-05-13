//! In-process tests for the local-signing dispatcher
//! (`run_sign`) extracted core, exercising the full read-decrypt-
//! sign-serialize round-trip against the captured test fixtures.
//!
//! The CLI entry point itself returns `ExitCode` and writes to
//! stdout / stderr; `sign_partially_signed_transactions` is the
//! pure-function inner helper the entry point delegates to, and
//! is the test surface here.

use std::path::PathBuf;

use crate::cli::dispatch::sign_partially_signed_transactions;
use crate::keyfile;
use crate::serialization::deserialize_partially_signed_transaction;

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn read_pst_bytes() -> Vec<u8> {
    let hex_str = std::fs::read_to_string(fixture("go_emitted_pst.hex")).unwrap();
    hex::decode(hex_str.trim()).unwrap()
}

#[test]
fn test_run_sign_singlekey_round_trip_marks_fully_signed() {
    let kf = keyfile::read_from_path(fixture("legacy_go_v1_singlekey.json")).unwrap();
    let mnemonics = keyfile::decrypt::decrypt_mnemonics(&kf, b"test fixture passphrase").unwrap();
    let inputs = vec![read_pst_bytes()];

    let (updated, all_fully_signed) =
        sign_partially_signed_transactions(&kf, &mnemonics, &inputs).expect("sign succeeds for singlekey fixture");

    assert!(all_fully_signed, "single-signer singlekey keyfile must reach the threshold after one sign pass");
    assert_eq!(updated.len(), 1, "one input in, one signed output out");

    let post = deserialize_partially_signed_transaction(&updated[0]).unwrap();
    let pair = &post.partially_signed_inputs[0].pub_key_signature_pairs[0];
    assert_eq!(pair.signature.len(), 65, "Schnorr blob is 64 sig bytes plus 1 sigHashType byte");
    assert_eq!(pair.signature[64], 0x01, "sigHashType byte is SIG_HASH_ALL");
}

#[test]
fn test_run_sign_invalid_pst_bytes_propagates_error() {
    let kf = keyfile::read_from_path(fixture("legacy_go_v1_singlekey.json")).unwrap();
    let mnemonics = keyfile::decrypt::decrypt_mnemonics(&kf, b"test fixture passphrase").unwrap();
    let inputs = vec![vec![0xffu8; 16]];

    let err = sign_partially_signed_transactions(&kf, &mnemonics, &inputs).expect_err("invalid bytes must fail");
    assert!(err.contains("PSTX deserialization failed"), "error names the failed stage: {err}");
}

#[test]
fn test_run_sign_zero_mnemonics_reports_threshold_unmet() {
    // Empty mnemonic list against a non-empty input set is the
    // dispatcher's degenerate `all_fully_signed = false` branch:
    // no signing happens, the baseline PST is still empty, and
    // `is_pst_fully_signed` returns false because every input
    // has zero non-empty signatures against minimum_signatures = 1.
    let kf = keyfile::read_from_path(fixture("legacy_go_v1_singlekey.json")).unwrap();
    let inputs = vec![read_pst_bytes()];

    let (_updated, all_fully_signed) =
        sign_partially_signed_transactions(&kf, &[], &inputs).expect("zero-mnemonic pass succeeds (no signing performed)");

    assert!(!all_fully_signed, "zero mnemonics leaves the input below threshold");
}

#[test]
fn test_run_sign_zero_inputs_returns_fully_signed_vacuously() {
    let kf = keyfile::read_from_path(fixture("legacy_go_v1_singlekey.json")).unwrap();
    let mnemonics = keyfile::decrypt::decrypt_mnemonics(&kf, b"test fixture passphrase").unwrap();

    let (updated, all_fully_signed) =
        sign_partially_signed_transactions(&kf, &mnemonics, &[]).expect("zero inputs is a no-op success");

    assert!(updated.is_empty());
    assert!(all_fully_signed, "an empty all-quantifier is vacuously true");
}
