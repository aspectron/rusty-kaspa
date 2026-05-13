//! JSON encode / decode for the keyfile format.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::Path;

use super::error::KeyfileError;
use super::types::{EncryptedMnemonic, EncryptedMnemonicJson, KeysFile, KeysFileJson};

/// Read a keyfile from a filesystem path. Strict decode --
/// unknown JSON fields are rejected.
pub fn read_from_path(path: impl AsRef<Path>) -> Result<KeysFile, KeyfileError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    read_from_reader(reader)
}

/// Read a keyfile from any reader. Uses
/// `serde(deny_unknown_fields)` for strict decode.
pub fn read_from_reader(reader: impl Read) -> Result<KeysFile, KeyfileError> {
    let raw: KeysFileJson = serde_json::from_reader(reader)?;
    from_json(raw)
}

fn from_json(raw: KeysFileJson) -> Result<KeysFile, KeyfileError> {
    let encrypted_mnemonics = raw.encrypted_mnemonics.into_iter().map(decode_encrypted_mnemonic).collect::<Result<Vec<_>, _>>()?;

    Ok(KeysFile {
        version: raw.version,
        num_threads: raw.num_threads,
        encrypted_mnemonics,
        extended_public_keys: raw.public_keys,
        minimum_signatures: raw.minimum_signatures,
        cosigner_index: raw.cosigner_index,
        last_used_external_index: raw.last_used_external_index,
        last_used_internal_index: raw.last_used_internal_index,
        ecdsa: raw.ecdsa,
    })
}

fn decode_encrypted_mnemonic(j: EncryptedMnemonicJson) -> Result<EncryptedMnemonic, KeyfileError> {
    let cipher = hex::decode(&j.cipher).map_err(|source| KeyfileError::Hex { field: "cipher", source })?;
    let salt = hex::decode(&j.salt).map_err(|source| KeyfileError::Hex { field: "salt", source })?;
    Ok(EncryptedMnemonic { cipher, salt })
}

/// Atomically persist a `KeysFile` to the supplied path:
/// serialize the JSON envelope, write to a sibling
/// `<path>.tmp`, fsync, then rename onto the target. The rename
/// is the atomic step; on POSIX `rename(2)` replaces the
/// destination atomically, and on Windows `MoveFileExW` with
/// `MOVEFILE_REPLACE_EXISTING` provides the same guarantee. Used
/// by the daemon's mutating handlers (NewAddress,
/// change-address bump in CreateUnsignedTransactions).
pub fn save_to_path(keysfile: &KeysFile, path: impl AsRef<Path>) -> Result<(), KeyfileError> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name =
        path.file_name().ok_or_else(|| KeyfileError::Invalid { field: "path", reason: "missing file name component".to_owned() })?;
    let mut tmp_name = file_name.to_owned();
    tmp_name.push(".tmp");
    let tmp_path = parent.join(tmp_name);

    let json = to_json(keysfile);
    let serialized = serde_json::to_vec_pretty(&json)?;

    let mut tmp = OpenOptions::new().create(true).write(true).truncate(true).open(&tmp_path)?;
    tmp.write_all(&serialized)?;
    tmp.sync_all()?;
    drop(tmp);

    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

fn to_json(keysfile: &KeysFile) -> KeysFileJson {
    KeysFileJson {
        version: keysfile.version,
        num_threads: keysfile.num_threads,
        encrypted_mnemonics: keysfile
            .encrypted_mnemonics
            .iter()
            .map(|em| EncryptedMnemonicJson { cipher: hex::encode(&em.cipher), salt: hex::encode(&em.salt) })
            .collect(),
        public_keys: keysfile.extended_public_keys.clone(),
        minimum_signatures: keysfile.minimum_signatures,
        cosigner_index: keysfile.cosigner_index,
        last_used_external_index: keysfile.last_used_external_index,
        last_used_internal_index: keysfile.last_used_internal_index,
        ecdsa: keysfile.ecdsa,
    }
}
