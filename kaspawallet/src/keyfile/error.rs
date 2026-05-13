//! Keyfile-module error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyfileError {
    #[error("keyfile io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("keyfile json decode error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("keyfile hex decode error in field {field}: {source}")]
    Hex { field: &'static str, source: hex::FromHexError },
    #[error("ciphertext too short")]
    CiphertextTooShort,
    #[error("argon2 parameter error: {0}")]
    Argon2Params(argon2::Error),
    #[error("argon2 key-derivation error: {0}")]
    Argon2Derive(argon2::Error),
    #[error("message authentication failed")]
    MacFailure,
    #[error("decrypted mnemonic was not utf-8: {0}")]
    MnemonicUtf8(#[from] std::string::FromUtf8Error),
    #[error("keyfile contains no encrypted mnemonics")]
    NoMnemonics,
    #[error("invalid keyfile field {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
}
