//! Keyfile codec: on-disk JSON format with Argon2id-derived
//! XChaCha20-Poly1305 AEAD encryption (24-byte nonce prepended to
//! the ciphertext).

mod codec;
pub(crate) mod decrypt;
mod encrypt;
mod error;
mod types;

#[cfg(test)]
mod tests;

pub use codec::{read_from_path, save_to_path};
pub use encrypt::encrypt_mnemonic;
pub use error::KeyfileError;
pub use types::{EncryptedMnemonic, KeysFile, LATEST_VERSION};
