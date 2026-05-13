//! Errors raised by the wire-lift, transaction-extract, and
//! signing layers.

use thiserror::Error;

use crate::serialization::SerializationError;

#[derive(Debug, Error)]
pub enum SignError {
    #[error("missing wire field {0}")]
    Missing(&'static str),
    #[error("invalid wire {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("bip32 error: {0}")]
    Bip32(#[from] kaspa_bip32::Error),
    #[error("secp256k1 error: {0}")]
    Secp256k1(#[from] secp256k1::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] SerializationError),
    #[error("no PubKeySignaturePair on input #{input_index} matches the cosigner's derived extended key")]
    CosignerMismatch { input_index: usize },
}
