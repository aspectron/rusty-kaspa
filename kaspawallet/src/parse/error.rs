//! Errors raised by the parse layer.

use thiserror::Error;

use crate::serialization::SerializationError;
use crate::sign::SignError;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("either --transaction or --transaction-file is required")]
    MissingInput,
    #[error("both --transaction and --transaction-file cannot be passed at the same time")]
    ConflictingInput,
    #[error("could not read hex from {path}: {source}")]
    InputRead { path: String, source: std::io::Error },
    #[error("invalid hex at transaction #{index}: {source}")]
    InvalidHex { index: usize, source: hex::FromHexError },
    #[error("serialization error at transaction #{index}: {source}")]
    Serialization { index: usize, source: SerializationError },
    #[error("wire-to-consensus conversion error at transaction #{index}: {source}")]
    Conversion { index: usize, source: SignError },
}
