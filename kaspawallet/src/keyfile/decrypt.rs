//! Argon2id + XChaCha20-Poly1305 mnemonic decryption. v0
//! keyfiles brute-force the Argon2id parallelism parameter; v1
//! keyfiles use a fixed `DEFAULT_NUM_THREADS`.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use super::error::KeyfileError;
use super::types::{
    ARGON2_MEMORY_KIB, ARGON2_OUTPUT_LEN, ARGON2_TIME_COST, DEFAULT_NUM_THREADS, EncryptedMnemonic, KeysFile, MAX_NUM_THREADS_GUESS,
    XCHACHA_NONCE_LEN,
};

/// Decrypt every encrypted mnemonic in `keyfile`, returning the
/// plaintext mnemonic strings in the same order. The returned
/// `Zeroizing<Vec<String>>` scrubs every byte of every plaintext on
/// drop, so callers do not need to add their own zeroize wrapper at
/// the boundary.
///
/// A single thread-count resolution applies to every mnemonic in
/// the file (one brute-force pass on v0 against the first
/// mnemonic, then reused). The on-disk keyfile is **not** mutated
/// here -- writing the resolved value back to disk on v0 is
/// deliberately omitted so fixture-driven tests remain
/// idempotent.
pub fn decrypt_mnemonics(keyfile: &KeysFile, password: &[u8]) -> Result<Zeroizing<Vec<String>>, KeyfileError> {
    if keyfile.encrypted_mnemonics.is_empty() {
        return Err(KeyfileError::NoMnemonics);
    }

    let threads = resolve_num_threads(keyfile, password)?;

    let mut out = Vec::with_capacity(keyfile.encrypted_mnemonics.len());
    for em in &keyfile.encrypted_mnemonics {
        out.push(decrypt_one(em, password, threads)?);
    }
    Ok(Zeroizing::new(out))
}

/// Decrypt one encrypted mnemonic. Used internally by
/// `decrypt_mnemonics` and by the v0 brute-force resolver below.
pub(crate) fn decrypt_one(em: &EncryptedMnemonic, password: &[u8], threads: u8) -> Result<String, KeyfileError> {
    if em.cipher.len() < XCHACHA_NONCE_LEN {
        return Err(KeyfileError::CiphertextTooShort);
    }
    let key = derive_key(password, &em.salt, threads)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let nonce = XNonce::from_slice(&em.cipher[..XCHACHA_NONCE_LEN]);
    let ct = &em.cipher[XCHACHA_NONCE_LEN..];
    let plaintext = cipher.decrypt(nonce, ct).map_err(|_| KeyfileError::MacFailure)?;
    Ok(String::from_utf8(plaintext)?)
}

fn derive_key(password: &[u8], salt: &[u8], threads: u8) -> Result<Zeroizing<[u8; ARGON2_OUTPUT_LEN]>, KeyfileError> {
    let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_TIME_COST, threads as u32, Some(ARGON2_OUTPUT_LEN))
        .map_err(KeyfileError::Argon2Params)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; ARGON2_OUTPUT_LEN]);
    argon.hash_password_into(password, salt, out.as_mut_slice()).map_err(KeyfileError::Argon2Derive)?;
    Ok(out)
}

/// Resolve the Argon2id parallelism parameter. v1 keyfiles use a
/// fixed constant; v0 keyfiles brute-force against the first
/// mnemonic. The brute-force exits on the first guess that
/// produces a successful AEAD open and propagates any non-MAC
/// error verbatim.
fn resolve_num_threads(keyfile: &KeysFile, password: &[u8]) -> Result<u8, KeyfileError> {
    if keyfile.version != 0 {
        return Ok(DEFAULT_NUM_THREADS);
    }

    let first = &keyfile.encrypted_mnemonics[0];
    let first_guess = if keyfile.num_threads == 0 {
        // Cap the CPU-derived guess at MAX_NUM_THREADS_GUESS so the
        // u8 conversion stays lossless on hosts with absurd CPU
        // counts.
        std::cmp::min(num_cpus::get(), MAX_NUM_THREADS_GUESS as usize) as u8
    } else {
        keyfile.num_threads
    };

    match try_threads(first, password, first_guess) {
        TryResult::Ok => return Ok(first_guess),
        TryResult::MacFailure => {}
        TryResult::Other(e) => return Err(e),
    }

    for guess in 1..=MAX_NUM_THREADS_GUESS {
        if guess == first_guess {
            continue;
        }
        match try_threads(first, password, guess) {
            TryResult::Ok => return Ok(guess),
            TryResult::MacFailure => {
                if guess == MAX_NUM_THREADS_GUESS {
                    return Err(KeyfileError::MacFailure);
                }
            }
            TryResult::Other(e) => return Err(e),
        }
    }
    Err(KeyfileError::MacFailure)
}

enum TryResult {
    Ok,
    MacFailure,
    Other(KeyfileError),
}

fn try_threads(em: &EncryptedMnemonic, password: &[u8], threads: u8) -> TryResult {
    match decrypt_one(em, password, threads) {
        Ok(_) => TryResult::Ok,
        Err(KeyfileError::MacFailure) => TryResult::MacFailure,
        Err(e) => TryResult::Other(e),
    }
}
