//! Unit tests for the keyfile module. Tests use the fixtures
//! committed under `cmd/kaspawallet/tests/fixtures/`; see that
//! directory's `README.md` for the exact `kaspawallet create`
//! commands that produced each fixture.

use std::io::Cursor;
use std::path::PathBuf;

use super::codec::read_from_reader;
use super::decrypt::{decrypt_mnemonics, decrypt_one};
use super::error::KeyfileError;
use super::types::{ARGON2_MEMORY_KIB, ARGON2_OUTPUT_LEN, ARGON2_TIME_COST, DEFAULT_NUM_THREADS, EncryptedMnemonic, KeysFile};

fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn read_fixture(name: &str) -> KeysFile {
    super::codec::read_from_path(fixture_path(name)).expect("fixture decodes")
}

#[test]
fn test_v1_singlekey_decode_layout() {
    let kf = read_fixture("legacy_go_v1_singlekey.json");
    assert_eq!(kf.version, 1);
    assert_eq!(kf.minimum_signatures, 1);
    assert_eq!(kf.cosigner_index, 0);
    assert_eq!(kf.last_used_external_index, 0);
    assert_eq!(kf.last_used_internal_index, 0);
    assert!(!kf.ecdsa);
    assert_eq!(kf.encrypted_mnemonics.len(), 1);
    assert_eq!(kf.extended_public_keys.len(), 1);
    let em = &kf.encrypted_mnemonics[0];
    assert!(em.cipher.len() > 24, "cipher must be longer than the 24-byte nonce");
    assert_eq!(em.salt.len(), 16, "v1 keyfile format pins the salt at 16 bytes");
}

#[test]
fn test_v1_singlekey_roundtrip() {
    let kf = read_fixture("legacy_go_v1_singlekey.json");
    let plaintexts = decrypt_mnemonics(&kf, b"test fixture passphrase").expect("decrypts");
    assert_eq!(plaintexts.len(), 1);
    let expected = "ethics brand merge engine core arm mail image punch mail absent private \
                    pioneer present enforce sorry another lazy hero alpha little glide fossil virus";
    assert_eq!(plaintexts[0], expected);
}

#[test]
fn test_v1_singlekey_wrong_password_mac_failure() {
    let kf = read_fixture("legacy_go_v1_singlekey.json");
    let err = decrypt_mnemonics(&kf, b"wrong passphrase").expect_err("must fail");
    assert!(matches!(err, KeyfileError::MacFailure), "got {err:?}");
}

#[test]
fn test_ecdsa_flag_decoded() {
    let kf = read_fixture("legacy_go_v1_ecdsa_singlekey.json");
    assert!(kf.ecdsa, "ECDSA flag must round-trip from JSON");
    let plaintexts = decrypt_mnemonics(&kf, b"ecdsa test passphrase").expect("decrypts");
    let expected = "evoke monkey potato feature lobster already casual become kitten kingdom \
                    cake someone awkward picture bird limb salon flee title satoshi educate \
                    depart casino cake";
    assert_eq!(plaintexts[0], expected);
}

#[test]
fn test_v1_multisig_2of3_decode() {
    let kf = read_fixture("legacy_go_v1_multisig_2of3.json");
    assert_eq!(kf.version, 1);
    assert_eq!(kf.minimum_signatures, 2);
    assert_eq!(kf.extended_public_keys.len(), 3);
    assert_eq!(kf.encrypted_mnemonics.len(), 3);
    let plaintexts = decrypt_mnemonics(&kf, b"multisig test passphrase").expect("decrypts");
    assert_eq!(plaintexts.len(), 3);
    let expected = [
        "regular brief palm floor wish win ugly sentence powder skill clump crawl prosper \
         increase garden put else payment coach voyage enforce cigar cream capital",
        "credit junior large vacant journey purpose leader pink stage success sting crack \
         nothing immune island firm ankle problem harsh cloth onion armor snake blood",
        "scout silly solar abuse useless pigeon foot fitness job joke chunk spirit interest \
         require battle deer casual ensure run album vapor leg spawn frame",
    ];
    for (got, want) in plaintexts.iter().zip(expected.iter()) {
        assert_eq!(got, want);
    }
}

#[test]
fn test_deny_unknown_fields() {
    let json = r#"{
        "version": 1,
        "encryptedMnemonics": [],
        "publicKeys": [],
        "minimumSignatures": 1,
        "cosignerIndex": 0,
        "lastUsedExternalIndex": 0,
        "lastUsedInternalIndex": 0,
        "ecdsa": false,
        "extraField": 42
    }"#;
    let err = read_from_reader(Cursor::new(json)).expect_err("unknown field must fail");
    assert!(matches!(err, KeyfileError::Json(_)));
}

#[test]
fn test_short_cipher_rejected() {
    let kf = KeysFile {
        version: 1,
        num_threads: DEFAULT_NUM_THREADS,
        encrypted_mnemonics: vec![EncryptedMnemonic { cipher: vec![0u8; 8], salt: vec![0u8; 16] }],
        extended_public_keys: vec![],
        minimum_signatures: 1,
        cosigner_index: 0,
        last_used_external_index: 0,
        last_used_internal_index: 0,
        ecdsa: false,
    };
    let err = decrypt_mnemonics(&kf, b"any").expect_err("short cipher must fail");
    assert!(matches!(err, KeyfileError::CiphertextTooShort), "got {err:?}");
}

#[test]
fn test_empty_keyfile_rejected() {
    let kf = KeysFile {
        version: 1,
        num_threads: DEFAULT_NUM_THREADS,
        encrypted_mnemonics: vec![],
        extended_public_keys: vec![],
        minimum_signatures: 1,
        cosigner_index: 0,
        last_used_external_index: 0,
        last_used_internal_index: 0,
        ecdsa: false,
    };
    let err = decrypt_mnemonics(&kf, b"any").expect_err("empty mnemonics list must fail");
    assert!(matches!(err, KeyfileError::NoMnemonics), "got {err:?}");
}

/// v0 brute-force coverage. Constructed by encrypting a known
/// plaintext with a chosen `numThreads` (`= 3`) using the same
/// Argon2id + XChaCha20-Poly1305 parameters as the v1 path, then
/// labelling the synthetic keyfile as `version = 0` and pinning
/// the file-level `numThreads` to `0` so the resolver falls back
/// to a CPU-derived first guess. The resolver must brute-force
/// through to `3` and decrypt successfully.
#[test]
fn test_v0_singlekey_numthreads_bruteforce() {
    use argon2::{Algorithm, Argon2, Params, Version};
    use chacha20poly1305::aead::{Aead, OsRng};
    use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305};

    const PLAINTEXT: &[u8] = b"synthetic v0 brute-force mnemonic plaintext";
    const PASSWORD: &[u8] = b"v0 brute force passphrase";
    const THREADS: u8 = 3;
    let salt = [0xA5u8; 16];

    let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_TIME_COST, THREADS as u32, Some(ARGON2_OUTPUT_LEN)).unwrap();
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; ARGON2_OUTPUT_LEN];
    argon.hash_password_into(PASSWORD, &salt, &mut key).unwrap();

    let cipher = XChaCha20Poly1305::new((&key).into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let mut blob = nonce.to_vec();
    let ct = cipher.encrypt(&nonce, PLAINTEXT).unwrap();
    blob.extend_from_slice(&ct);

    let kf = KeysFile {
        version: 0,
        num_threads: 0,
        encrypted_mnemonics: vec![EncryptedMnemonic { cipher: blob, salt: salt.to_vec() }],
        extended_public_keys: vec![],
        minimum_signatures: 1,
        cosigner_index: 0,
        last_used_external_index: 0,
        last_used_internal_index: 0,
        ecdsa: false,
    };

    let plaintexts = decrypt_mnemonics(&kf, PASSWORD).expect("v0 brute force resolves");
    assert_eq!(plaintexts.len(), 1);
    assert_eq!(plaintexts[0].as_bytes(), PLAINTEXT);
}

#[test]
fn test_decrypt_one_direct_smoke() {
    let kf = read_fixture("legacy_go_v1_singlekey.json");
    let em = &kf.encrypted_mnemonics[0];
    let plaintext = decrypt_one(em, b"test fixture passphrase", DEFAULT_NUM_THREADS).expect("decrypts");
    assert!(plaintext.starts_with("ethics brand"));
}

#[test]
fn test_latest_version_is_one() {
    assert_eq!(super::types::LATEST_VERSION, 1);
}

#[test]
fn test_save_to_path_round_trip() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("keys.json");
    let kf = KeysFile {
        version: super::types::LATEST_VERSION,
        num_threads: DEFAULT_NUM_THREADS,
        encrypted_mnemonics: vec![EncryptedMnemonic { cipher: vec![0xAAu8; 40], salt: vec![0x55u8; 16] }],
        extended_public_keys: vec!["kpub-stub".to_owned()],
        minimum_signatures: 1,
        cosigner_index: 0,
        last_used_external_index: 0,
        last_used_internal_index: 0,
        ecdsa: false,
    };
    super::codec::save_to_path(&kf, &path).expect("save_to_path succeeds");
    let read_back = super::codec::read_from_path(&path).expect("read_from_path succeeds");
    assert_eq!(read_back, kf);
}

#[test]
fn test_save_to_path_rejects_path_without_filename() {
    use std::path::Path;
    let kf = KeysFile {
        version: super::types::LATEST_VERSION,
        num_threads: DEFAULT_NUM_THREADS,
        encrypted_mnemonics: vec![],
        extended_public_keys: vec![],
        minimum_signatures: 1,
        cosigner_index: 0,
        last_used_external_index: 0,
        last_used_internal_index: 0,
        ecdsa: false,
    };
    let err = super::codec::save_to_path(&kf, Path::new("/")).expect_err("must fail");
    assert!(matches!(err, KeyfileError::Invalid { .. }), "got {err:?}");
}
