//! Encode / decode entry points for the PST and unsigned-tx
//! wire formats.

use prost::Message;

use super::error::SerializationError;
use super::wire;

/// Encode a `PartiallySignedTransaction` to its protobuf wire
/// form.
pub fn serialize_partially_signed_transaction(pst: &wire::PartiallySignedTransaction) -> Result<Vec<u8>, SerializationError> {
    Ok(pst.encode_to_vec())
}

/// Decode a protobuf-encoded `PartiallySignedTransaction`.
pub fn deserialize_partially_signed_transaction(bytes: &[u8]) -> Result<wire::PartiallySignedTransaction, SerializationError> {
    Ok(wire::PartiallySignedTransaction::decode(bytes)?)
}

/// Encode a `TransactionMessage` (the unsigned-transaction wire
/// shape used by the daemon's `Broadcast` RPC and the
/// `kaspawallet broadcast --transaction <hex>` interop path).
// Exercised through the test surface and reserved for the
// binary-side broadcast flow that consumes the same wire shape.
#[allow(dead_code)]
pub fn serialize_domain_transaction(tx: &wire::TransactionMessage) -> Result<Vec<u8>, SerializationError> {
    Ok(tx.encode_to_vec())
}

/// Decode a protobuf-encoded `TransactionMessage`. Performs an
/// additional `version <= u16::MAX` check on top of the prost
/// decode -- the wire `version` field is encoded as `uint32` but
/// the consensus layer treats it as `uint16`.
// Exercised through the test surface and reserved for the
// binary-side broadcast flow that consumes the same wire shape.
#[allow(dead_code)]
pub fn deserialize_domain_transaction(bytes: &[u8]) -> Result<wire::TransactionMessage, SerializationError> {
    let tx = wire::TransactionMessage::decode(bytes)?;
    if tx.version > u32::from(u16::MAX) {
        return Err(SerializationError::Invalid { field: "version", reason: format!("version {} exceeds u16::MAX", tx.version) });
    }
    if let Some(subnetwork) = tx.subnetwork_id.as_ref()
        && subnetwork.bytes.len() != 20
    {
        return Err(SerializationError::Invalid {
            field: "subnetworkId.bytes",
            reason: format!("expected 20 bytes, got {}", subnetwork.bytes.len()),
        });
    }
    for (i, input) in tx.inputs.iter().enumerate() {
        if input.sig_op_count > u32::from(u8::MAX) {
            return Err(SerializationError::Invalid {
                field: "inputs.sigOpCount",
                reason: format!("input #{i} sigOpCount {} exceeds u8::MAX", input.sig_op_count),
            });
        }
    }
    Ok(tx)
}
