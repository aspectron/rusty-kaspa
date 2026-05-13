//! Argon2id + XChaCha20-Poly1305 mnemonic encryption. Used by the
//! standalone `create` subcommand to produce v1-format encrypted
//! mnemonic records. Parameters are pinned to the v1 keyfile
//! format so a Rust-emitted keyfile decrypts cleanly under any
//! v1-aware reader using the same passphrase.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, OsRng};
use chacha20poly1305::{AeadCore, KeyInit, XChaCha20Poly1305};
use zeroize::Zeroizing;

use super::error::KeyfileError;
use super::types::{ARGON2_MEMORY_KIB, ARGON2_OUTPUT_LEN, ARGON2_TIME_COST, DEFAULT_NUM_THREADS, EncryptedMnemonic};

/// Argon2 salt size used by the v1 keyfile format.
pub(crate) const ARGON2_SALT_LEN: usize = 16;

/// Encrypt one mnemonic plaintext under `password` using the v1
/// keyfile parameters (Argon2id m=64 MiB, t=1, threads=8;
/// XChaCha20-Poly1305 AEAD; per-record 16-byte salt and 24-byte
/// nonce). The returned record's `cipher` field is
/// `nonce || ciphertext_with_tag`.
pub fn encrypt_mnemonic(mnemonic: &str, password: &[u8]) -> Result<EncryptedMnemonic, KeyfileError> {
    let mut salt = vec![0u8; ARGON2_SALT_LEN];
    let mut rng = OsRng;
    rng.fill_bytes(&mut salt);

    let key = derive_key(password, &salt, DEFAULT_NUM_THREADS)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ct = cipher.encrypt(&nonce, mnemonic.as_bytes()).map_err(|_| KeyfileError::MacFailure)?;
    let mut combined = Vec::with_capacity(nonce.len() + ct.len());
    combined.extend_from_slice(nonce.as_slice());
    combined.extend_from_slice(&ct);
    Ok(EncryptedMnemonic { cipher: combined, salt })
}

fn derive_key(password: &[u8], salt: &[u8], threads: u8) -> Result<Zeroizing<[u8; ARGON2_OUTPUT_LEN]>, KeyfileError> {
    let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_TIME_COST, threads as u32, Some(ARGON2_OUTPUT_LEN))
        .map_err(KeyfileError::Argon2Params)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; ARGON2_OUTPUT_LEN]);
    argon.hash_password_into(password, salt, out.as_mut_slice()).map_err(KeyfileError::Argon2Derive)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyfile::decrypt::decrypt_one;

    #[test]
    fn round_trip_encrypts_and_decrypts() {
        let plaintext = "spirit obvious melody example hello tone";
        let password = b"correct horse battery staple";
        let record = encrypt_mnemonic(plaintext, password).expect("encrypt succeeds");
        assert_eq!(record.salt.len(), ARGON2_SALT_LEN);
        let recovered = decrypt_one(&record, password, DEFAULT_NUM_THREADS).expect("decrypt succeeds");
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn wrong_password_fails_mac() {
        let record = encrypt_mnemonic("phrase", b"correct").expect("encrypt");
        match decrypt_one(&record, b"wrong", DEFAULT_NUM_THREADS) {
            Err(KeyfileError::MacFailure) => {}
            other => panic!("expected MacFailure, got {other:?}"),
        }
    }
}
