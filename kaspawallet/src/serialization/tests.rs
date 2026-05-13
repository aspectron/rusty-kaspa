//! Round-trip tests for the partial-signed-transaction wire
//! format. Cross-implementation byte-identity is exercised by the
//! parity fixture below; the in-tree tests prove encode/decode
//! round-trip, default-value omission, and structural correctness
//! of the wire shape.

use super::codec::{
    deserialize_domain_transaction, deserialize_partially_signed_transaction, serialize_domain_transaction,
    serialize_partially_signed_transaction,
};
use super::error::SerializationError;
use super::wire;

const SAMPLE_TXID: [u8; 32] = [
    0xde, 0xad, 0xbe, 0xef, 0xfe, 0xed, 0xfa, 0xce, 0xca, 0xfe, 0xba, 0xbe, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
];

const SUBNETWORK_NATIVE: [u8; 20] = [0u8; 20];

fn sample_tx() -> wire::TransactionMessage {
    wire::TransactionMessage {
        version: 0,
        inputs: vec![wire::TransactionInput {
            previous_outpoint: Some(wire::Outpoint {
                transaction_id: Some(wire::TransactionId { bytes: SAMPLE_TXID.to_vec() }),
                index: 1,
            }),
            signature_script: vec![],
            sequence: u64::MAX,
            sig_op_count: 1,
        }],
        outputs: vec![wire::TransactionOutput {
            value: 1_000_000,
            script_public_key: Some(wire::ScriptPublicKey { script: vec![0x20, 0x11, 0x22, 0x33], version: 0 }),
        }],
        lock_time: 0,
        subnetwork_id: Some(wire::SubnetworkId { bytes: SUBNETWORK_NATIVE.to_vec() }),
        gas: 0,
        payload: vec![],
    }
}

fn sample_pst() -> wire::PartiallySignedTransaction {
    wire::PartiallySignedTransaction {
        tx: Some(sample_tx()),
        partially_signed_inputs: vec![wire::PartiallySignedInput {
            redeem_script: vec![],
            prev_output: Some(wire::TransactionOutput {
                value: 1_000_000,
                script_public_key: Some(wire::ScriptPublicKey { script: vec![0x20, 0xaa, 0xbb, 0xcc], version: 0 }),
            }),
            minimum_signatures: 1,
            pub_key_signature_pairs: vec![wire::PubKeySignaturePair {
                extended_pub_key:
                    "ktub249YJayoDJS3tDjTW8NG3iAwufiDQ13uEptr8Wz2LgnzdVFLUQiRqFRPyq1xndcJMXFbYx268MSxHwukrnD52gWeshgeYseLmTBcUNHR1Xb"
                        .into(),
                signature: vec![],
            }],
            derivation_path: "m/44'/111111'/0'/0/0".into(),
        }],
    }
}

#[test]
fn test_partially_signed_tx_encode_decode_roundtrip() {
    let pst = sample_pst();
    let bytes = serialize_partially_signed_transaction(&pst).expect("encodes");
    let back = deserialize_partially_signed_transaction(&bytes).expect("decodes");
    assert_eq!(pst, back);
}

#[test]
fn test_partially_signed_tx_encode_is_deterministic() {
    let pst = sample_pst();
    let a = serialize_partially_signed_transaction(&pst).unwrap();
    let b = serialize_partially_signed_transaction(&pst).unwrap();
    assert_eq!(a, b, "proto3 encode is byte-deterministic for the same input on the same run");
}

#[test]
fn test_partially_signed_tx_decode_then_reencode_is_idempotent() {
    let pst = sample_pst();
    let original = serialize_partially_signed_transaction(&pst).unwrap();
    let decoded = deserialize_partially_signed_transaction(&original).unwrap();
    let reencoded = serialize_partially_signed_transaction(&decoded).unwrap();
    assert_eq!(original, reencoded, "decode then re-encode must produce identical wire bytes");
}

#[test]
fn test_domain_transaction_roundtrip() {
    let tx = sample_tx();
    let bytes = serialize_domain_transaction(&tx).expect("encodes");
    let back = deserialize_domain_transaction(&bytes).expect("decodes");
    assert_eq!(tx, back);
}

#[test]
fn test_domain_transaction_rejects_version_overflow() {
    let mut tx = sample_tx();
    tx.version = (u16::MAX as u32) + 1;
    let bytes = tx.encode_to_vec();
    match deserialize_domain_transaction(&bytes) {
        Ok(_) => panic!("must reject version > u16::MAX"),
        Err(SerializationError::Invalid { field, reason }) => {
            assert_eq!(field, "version");
            assert!(reason.contains("exceeds u16::MAX"));
        }
        Err(other) => panic!("expected Invalid version, got {other:?}"),
    }
}

#[test]
fn test_domain_transaction_rejects_subnetwork_id_wrong_length() {
    let mut tx = sample_tx();
    tx.subnetwork_id = Some(wire::SubnetworkId { bytes: vec![0u8; 19] });
    let bytes = tx.encode_to_vec();
    match deserialize_domain_transaction(&bytes) {
        Ok(_) => panic!("must reject malformed subnetwork id"),
        Err(SerializationError::Invalid { field, .. }) => assert_eq!(field, "subnetworkId.bytes"),
        Err(other) => panic!("expected Invalid subnetworkId.bytes, got {other:?}"),
    }
}

#[test]
fn test_domain_transaction_rejects_sigopcount_overflow() {
    let mut tx = sample_tx();
    tx.inputs[0].sig_op_count = u32::from(u8::MAX) + 1;
    let bytes = tx.encode_to_vec();
    match deserialize_domain_transaction(&bytes) {
        Ok(_) => panic!("must reject sigOpCount > u8::MAX"),
        Err(SerializationError::Invalid { field, .. }) => assert_eq!(field, "inputs.sigOpCount"),
        Err(other) => panic!("expected Invalid inputs.sigOpCount, got {other:?}"),
    }
}

#[test]
fn test_decode_garbage_fails_cleanly() {
    let garbage = b"\xffnot a valid proto message\xff\xff\xff";
    match deserialize_partially_signed_transaction(garbage) {
        Ok(_) => panic!("garbage must not decode"),
        Err(SerializationError::Decode(_)) => {}
        Err(other) => panic!("expected Decode error, got {other:?}"),
    }
}

#[test]
fn test_default_values_are_omitted_from_wire() {
    let empty = wire::PartiallySignedTransaction { tx: None, partially_signed_inputs: vec![] };
    let bytes = serialize_partially_signed_transaction(&empty).unwrap();
    assert!(bytes.is_empty(), "all-default proto3 message encodes to empty bytes; got {bytes:?}");
}

/// Cross-wallet byte-identity gate. The fixture
/// `tests/fixtures/go_emitted_pst.hex` is a hex-encoded
/// `PartiallySignedTransaction` produced by the reference
/// implementation's wire serializer on a deterministic synthetic
/// input. This test:
///
///   1. Decodes the reference-emitted bytes via this crate --
///      proves the decoder accepts the canonical wire bytes.
///   2. Re-encodes the decoded structure -- proves the encoder
///      emits the same byte string.
///   3. Re-decodes the re-encoded bytes -- proves the round-trip
///      is wire-stable.
///
/// Together these three assertions are the structural cross-
/// implementation byte-identity claim the cross-wallet interop
/// AC depends on.
#[test]
fn test_reference_emitted_pst_round_trips_byte_identically() {
    let hex_str = include_str!("../../tests/fixtures/go_emitted_pst.hex").trim();
    let reference_bytes = hex::decode(hex_str).expect("fixture is valid hex");

    let pst = deserialize_partially_signed_transaction(&reference_bytes).expect("decoder accepts reference-emitted bytes");

    let local_bytes = serialize_partially_signed_transaction(&pst).expect("encoder runs");
    assert_eq!(
        local_bytes,
        reference_bytes,
        "re-encoded PartiallySignedTransaction must equal the original reference bytes; lengths reference={} re-encoded={}",
        reference_bytes.len(),
        local_bytes.len()
    );

    let pst_again = deserialize_partially_signed_transaction(&local_bytes).expect("decoder accepts re-encoded bytes");
    assert_eq!(pst, pst_again, "decoded structures must be identical after the round-trip");
}

/// The reference fixture should expose every wire-level field
/// that matters for the cross-wallet sign flow. This test reads
/// the fixture and pins each field's expected shape, so any
/// regeneration of the fixture that drifts from the original
/// synthetic input gets flagged.
#[test]
fn test_reference_emitted_pst_decoded_shape() {
    let hex_str = include_str!("../../tests/fixtures/go_emitted_pst.hex").trim();
    let bytes = hex::decode(hex_str).unwrap();
    let pst = deserialize_partially_signed_transaction(&bytes).unwrap();

    let tx = pst.tx.as_ref().expect("tx present");
    assert_eq!(tx.version, 0);
    assert_eq!(tx.inputs.len(), 1);
    assert_eq!(tx.outputs.len(), 2);
    assert_eq!(tx.lock_time, 0);
    assert_eq!(tx.gas, 0);
    assert!(tx.payload.is_empty());
    let sub = tx.subnetwork_id.as_ref().expect("subnetwork id present");
    assert_eq!(sub.bytes.len(), 20);
    assert!(sub.bytes.iter().all(|b| *b == 0), "synthetic fixture uses native subnetwork id");

    assert_eq!(pst.partially_signed_inputs.len(), 1);
    let psi = &pst.partially_signed_inputs[0];
    assert_eq!(psi.minimum_signatures, 1);
    assert_eq!(psi.derivation_path, "m/0/0");
    assert_eq!(psi.pub_key_signature_pairs.len(), 1);
    let pair = &psi.pub_key_signature_pairs[0];
    assert!(pair.extended_pub_key.starts_with("ktub"));
    assert!(pair.signature.is_empty(), "fixture is the pre-sign baseline");
}

#[test]
fn test_pst_with_signature_field_set_round_trips() {
    let mut pst = sample_pst();
    pst.partially_signed_inputs[0].pub_key_signature_pairs[0].signature = vec![0xaa; 64];
    let bytes = serialize_partially_signed_transaction(&pst).unwrap();
    let back = deserialize_partially_signed_transaction(&bytes).unwrap();
    assert_eq!(pst, back);
    assert_eq!(back.partially_signed_inputs[0].pub_key_signature_pairs[0].signature, vec![0xaa; 64]);
}
