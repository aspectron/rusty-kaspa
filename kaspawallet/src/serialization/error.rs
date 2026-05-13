//! Errors raised by the wire codec.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("proto decode error: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("proto encode error: {0}")]
    Encode(#[from] prost::EncodeError),
    // Constructed by the domain-transaction decode helper which
    // is reserved for the binary-side broadcast flow and the
    // test surface.
    #[allow(dead_code)]
    #[error("invalid wire {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
}
