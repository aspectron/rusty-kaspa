//! Errors raised by the key-source layer.

use thiserror::Error;

use crate::keyfile::KeyfileError;

#[derive(Debug, Error)]
pub enum KeySourceError {
    #[error("keyfile error: {0}")]
    Keyfile(#[from] KeyfileError),
    /// Neither `--keys-file` nor the platform-aware default
    /// path resolves to an existing keyfile on disk.
    #[error("keyfile not found at default path '{default}' or any operator-supplied override")]
    DefaultPathMissing { default: String },
    /// The platform-aware default-path resolver could not
    /// determine an application data directory (no `$HOME`,
    /// no `%LOCALAPPDATA%`).
    #[error("could not determine default key-path: no application data directory available on this platform")]
    NoAppDataDir,
}
