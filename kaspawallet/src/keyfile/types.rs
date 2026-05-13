//! Domain types for the on-disk keyfile format.

use serde::{Deserialize, Serialize};

/// Format-version tag emitted in new keyfiles. Existing field
/// keyfiles carry this value; reading older formats is handled by
/// the v0 decoder in `decrypt.rs`.
pub const LATEST_VERSION: u32 = 1;

/// Default `numThreads` value emitted in v1 keyfiles.
pub(crate) const DEFAULT_NUM_THREADS: u8 = 8;

/// Argon2id memory cost in KiB (64 MiB).
pub(crate) const ARGON2_MEMORY_KIB: u32 = 64 * 1024;

/// Argon2id time cost.
pub(crate) const ARGON2_TIME_COST: u32 = 1;

/// Argon2id output length in bytes (XChaCha20-Poly1305 key size).
pub(crate) const ARGON2_OUTPUT_LEN: usize = 32;

/// XChaCha20-Poly1305 nonce size in bytes.
pub(crate) const XCHACHA_NONCE_LEN: usize = 24;

/// Maximum `numThreads` value searched during the v0 brute-force.
pub(crate) const MAX_NUM_THREADS_GUESS: u8 = 255;

/// Wire-level encrypted-mnemonic record. JSON values are hex-encoded
/// byte strings; decoded into raw bytes here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptedMnemonic {
    /// Nonce (24 bytes) concatenated with the XChaCha20-Poly1305
    /// AEAD output (ciphertext || tag).
    pub cipher: Vec<u8>,
    /// Per-mnemonic Argon2id salt.
    pub salt: Vec<u8>,
}

/// Decoded keyfile content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeysFile {
    pub version: u32,
    /// `numThreads` is interpreted for v0 (brute-forced) and
    /// constant for v1. v1 keyfiles still emit this field
    /// (default 8); whatever value the JSON carried is preserved
    /// so the round-trip is exact.
    pub num_threads: u8,
    pub encrypted_mnemonics: Vec<EncryptedMnemonic>,
    pub extended_public_keys: Vec<String>,
    pub minimum_signatures: u32,
    pub cosigner_index: u32,
    pub last_used_external_index: u32,
    pub last_used_internal_index: u32,
    pub ecdsa: bool,
}

/// On-disk JSON shape: hex-encoded byte fields and `omitempty`
/// semantics on `numThreads`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeysFileJson {
    pub version: u32,
    #[serde(default, rename = "numThreads", skip_serializing_if = "is_zero_u8")]
    pub num_threads: u8,
    #[serde(rename = "encryptedMnemonics")]
    pub encrypted_mnemonics: Vec<EncryptedMnemonicJson>,
    #[serde(rename = "publicKeys")]
    pub public_keys: Vec<String>,
    #[serde(rename = "minimumSignatures")]
    pub minimum_signatures: u32,
    #[serde(rename = "cosignerIndex")]
    pub cosigner_index: u32,
    #[serde(rename = "lastUsedExternalIndex")]
    pub last_used_external_index: u32,
    #[serde(rename = "lastUsedInternalIndex")]
    pub last_used_internal_index: u32,
    pub ecdsa: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncryptedMnemonicJson {
    pub cipher: String,
    pub salt: String,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}
