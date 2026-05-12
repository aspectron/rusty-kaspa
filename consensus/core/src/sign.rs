use crate::{
    hashing::{
        sighash::{SigHashReusedValuesUnsync, calc_ecdsa_signature_hash, calc_schnorr_signature_hash},
        sighash_type::{SIG_HASH_ALL, SigHashType},
    },
    tx::{SignableTransaction, VerifiableTransaction},
};
use itertools::Itertools;
use std::collections::BTreeMap;
use std::iter::once;
use thiserror::Error;

/// Standard P2PK script templates a transaction input may carry.
/// Schnorr inputs use `OpData32 <xonly-32> OpCheckSig` (35 bytes).
/// ECDSA inputs use `OpData33 <compressed-33> OpCheckSigECDSA` (35 bytes).
const OP_DATA_32: u8 = 0x20;
const OP_DATA_33: u8 = 0x21;
const OP_CHECK_SIG: u8 = 0xac;
const OP_CHECK_SIG_ECDSA: u8 = 0xab;

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("{0}")]
    Message(String),

    #[error("Secp256k1 -> {0}")]
    Secp256k1Error(#[from] secp256k1::Error),

    #[error("The transaction is partially signed")]
    PartiallySigned,

    #[error("The transaction is fully signed")]
    FullySigned,
}

/// A wrapper enum that represents the transaction signed state. A transaction
/// contained by this enum can be either fully signed or partially signed.
pub enum Signed {
    Fully(SignableTransaction),
    Partially(SignableTransaction),
}

impl Signed {
    /// Returns the transaction if it is fully signed, otherwise returns an error
    pub fn fully_signed(self) -> std::result::Result<SignableTransaction, Error> {
        match self {
            Signed::Fully(tx) => Ok(tx),
            Signed::Partially(_) => Err(Error::PartiallySigned),
        }
    }

    /// Returns the transaction if it is fully signed, otherwise returns the
    /// transaction as an error `Err(tx)`.
    #[allow(clippy::result_large_err)]
    pub fn try_fully_signed(self) -> std::result::Result<SignableTransaction, SignableTransaction> {
        match self {
            Signed::Fully(tx) => Ok(tx),
            Signed::Partially(tx) => Err(tx),
        }
    }

    /// Returns the transaction if it is partially signed, otherwise fail with an error
    pub fn partially_signed(self) -> std::result::Result<SignableTransaction, Error> {
        match self {
            Signed::Fully(_) => Err(Error::FullySigned),
            Signed::Partially(tx) => Ok(tx),
        }
    }

    /// Returns the transaction if it is partially signed, otherwise returns the
    /// transaction as an error `Err(tx)`.
    #[allow(clippy::result_large_err)]
    pub fn try_partially_signed(self) -> std::result::Result<SignableTransaction, SignableTransaction> {
        match self {
            Signed::Fully(tx) => Err(tx),
            Signed::Partially(tx) => Ok(tx),
        }
    }

    /// Returns the transaction regardless of whether it is fully or partially signed
    pub fn unwrap(self) -> SignableTransaction {
        match self {
            Signed::Fully(tx) => tx,
            Signed::Partially(tx) => tx,
        }
    }
}

/// Sign a transaction using schnorr
pub fn sign(mut signable_tx: SignableTransaction, schnorr_key: secp256k1::Keypair) -> SignableTransaction {
    for i in 0..signable_tx.tx.inputs.len() {
        signable_tx.tx.inputs[i].sig_op_count = 1;
    }

    let reused_values = SigHashReusedValuesUnsync::new();
    for i in 0..signable_tx.tx.inputs.len() {
        let sig_hash = calc_schnorr_signature_hash(&signable_tx.as_verifiable(), i, SIG_HASH_ALL, &reused_values);
        let msg = secp256k1::Message::from_digest_slice(sig_hash.as_bytes().as_slice()).unwrap();
        let sig: [u8; 64] = *schnorr_key.sign_schnorr(msg).as_ref();
        // This represents OP_DATA_65 <SIGNATURE+SIGHASH_TYPE> (since signature length is 64 bytes and SIGHASH_TYPE is one byte)
        signable_tx.tx.inputs[i].signature_script = std::iter::once(65u8).chain(sig).chain([SIG_HASH_ALL.to_u8()]).collect();
    }
    signable_tx
}

/// Sign a transaction using schnorr
pub fn sign_with_multiple(mut mutable_tx: SignableTransaction, privkeys: Vec<[u8; 32]>) -> SignableTransaction {
    let mut map = BTreeMap::new();
    for privkey in privkeys {
        let schnorr_key = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, &privkey).unwrap();
        map.insert(schnorr_key.public_key().serialize(), schnorr_key);
    }
    for i in 0..mutable_tx.tx.inputs.len() {
        mutable_tx.tx.inputs[i].sig_op_count = 1;
    }

    let reused_values = SigHashReusedValuesUnsync::new();
    for i in 0..mutable_tx.tx.inputs.len() {
        let script = mutable_tx.entries[i].as_ref().unwrap().script_public_key.script();
        if let Some(schnorr_key) = map.get(script) {
            let sig_hash = calc_schnorr_signature_hash(&mutable_tx.as_verifiable(), i, SIG_HASH_ALL, &reused_values);
            let msg = secp256k1::Message::from_digest_slice(sig_hash.as_bytes().as_slice()).unwrap();
            let sig: [u8; 64] = *schnorr_key.sign_schnorr(msg).as_ref();
            // This represents OP_DATA_65 <SIGNATURE+SIGHASH_TYPE> (since signature length is 64 bytes and SIGHASH_TYPE is one byte)
            mutable_tx.tx.inputs[i].signature_script = std::iter::once(65u8).chain(sig).chain([SIG_HASH_ALL.to_u8()]).collect();
        }
    }
    mutable_tx
}

/// A signing key paired with the signature scheme its inputs expect.
/// Built once per `privkey` so the per-input loop only has to look up
/// the precomputed entry and dispatch.
enum SchemeKey {
    Schnorr(secp256k1::Keypair),
    Ecdsa(secp256k1::SecretKey),
}

/// Sign every input of `mutable_tx` that matches a P2PK script - Schnorr
/// (`OpData32 <xonly-32> OpCheckSig`) or ECDSA (`OpData33 <compressed-33>
/// OpCheckSigECDSA`). Each `privkey` produces two candidate `script_public_key`
/// templates (one per scheme); the entry whose template matches the input's
/// `script_public_key` selects the signing scheme for that input. Inputs
/// whose script matches no candidate (e.g. multisig redeem scripts) are
/// left unsigned and surface as `Signed::Partially`.
#[allow(clippy::result_large_err)]
pub fn sign_with_multiple_v2(mut mutable_tx: SignableTransaction, privkeys: &[[u8; 32]]) -> Signed {
    let mut map = BTreeMap::new();
    for privkey in privkeys {
        let secret_key = secp256k1::SecretKey::from_slice(privkey).unwrap();
        let keypair = secp256k1::Keypair::from_secret_key(secp256k1::SECP256K1, &secret_key);
        let xonly_pubkey = keypair.public_key().x_only_public_key().0;
        let schnorr_script = once(OP_DATA_32).chain(xonly_pubkey.serialize()).chain(once(OP_CHECK_SIG)).collect_vec();
        map.insert(schnorr_script, SchemeKey::Schnorr(keypair));

        let compressed_pubkey = keypair.public_key().serialize();
        let ecdsa_script = once(OP_DATA_33).chain(compressed_pubkey).chain(once(OP_CHECK_SIG_ECDSA)).collect_vec();
        map.insert(ecdsa_script, SchemeKey::Ecdsa(secret_key));
    }

    let reused_values = SigHashReusedValuesUnsync::new();
    let mut additional_signatures_required = false;
    for i in 0..mutable_tx.tx.inputs.len() {
        let script = mutable_tx.entries[i].as_ref().unwrap().script_public_key.script();
        let Some(key) = map.get(script) else {
            additional_signatures_required = true;
            continue;
        };
        let sig: [u8; 64] = match key {
            SchemeKey::Schnorr(keypair) => {
                let sig_hash = calc_schnorr_signature_hash(&mutable_tx.as_verifiable(), i, SIG_HASH_ALL, &reused_values);
                let msg = secp256k1::Message::from_digest_slice(sig_hash.as_bytes().as_slice()).unwrap();
                *keypair.sign_schnorr(msg).as_ref()
            }
            SchemeKey::Ecdsa(secret_key) => {
                let sig_hash = calc_ecdsa_signature_hash(&mutable_tx.as_verifiable(), i, SIG_HASH_ALL, &reused_values);
                let msg = secp256k1::Message::from_digest_slice(sig_hash.as_bytes().as_slice()).unwrap();
                secret_key.sign_ecdsa(msg).serialize_compact()
            }
        };
        // OP_DATA_65 <SIGNATURE(64) + SIGHASH_TYPE(1)>
        mutable_tx.tx.inputs[i].signature_script = std::iter::once(65u8).chain(sig).chain([SIG_HASH_ALL.to_u8()]).collect();
    }
    if additional_signatures_required { Signed::Partially(mutable_tx) } else { Signed::Fully(mutable_tx) }
}

/// Sign a transaction input with a sighash_type using schnorr
pub fn sign_input(tx: &impl VerifiableTransaction, input_index: usize, private_key: &[u8; 32], hash_type: SigHashType) -> Vec<u8> {
    let reused_values = SigHashReusedValuesUnsync::new();

    let hash = calc_schnorr_signature_hash(tx, input_index, hash_type, &reused_values);
    let msg = secp256k1::Message::from_digest_slice(hash.as_bytes().as_slice()).unwrap();
    let schnorr_key = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, private_key).unwrap();
    let sig: [u8; 64] = *schnorr_key.sign_schnorr(msg).as_ref();

    // This represents OP_DATA_65 <SIGNATURE+SIGHASH_TYPE> (since signature length is 64 bytes and SIGHASH_TYPE is one byte)
    std::iter::once(65u8).chain(sig).chain([hash_type.to_u8()]).collect()
}

pub fn verify(tx: &impl VerifiableTransaction) -> Result<(), Error> {
    let reused_values = SigHashReusedValuesUnsync::new();
    for (i, (input, entry)) in tx.populated_inputs().enumerate() {
        if input.signature_script.is_empty() {
            return Err(Error::Message(format!("Signature is empty for input: {i}")));
        }
        let pk = &entry.script_public_key.script()[1..33];
        let pk = secp256k1::XOnlyPublicKey::from_slice(pk)?;
        let sig = secp256k1::schnorr::Signature::from_slice(&input.signature_script[1..65])?;
        let sig_hash = calc_schnorr_signature_hash(tx, i, SIG_HASH_ALL, &reused_values);
        let msg = secp256k1::Message::from_digest_slice(sig_hash.as_bytes().as_slice())?;
        sig.verify(&msg, &pk)?;
    }

    Ok(())
}

/// Per-input verifier that dispatches on the P2PK script template byte
/// (`OpCheckSig` vs `OpCheckSigECDSA`), enabling end-to-end test coverage
/// of `sign_with_multiple_v2` for mixed-scheme transactions.
pub fn verify_v2(tx: &impl VerifiableTransaction) -> Result<(), Error> {
    let reused_values = SigHashReusedValuesUnsync::new();
    for (i, (input, entry)) in tx.populated_inputs().enumerate() {
        if input.signature_script.is_empty() {
            return Err(Error::Message(format!("Signature is empty for input: {i}")));
        }
        let script = entry.script_public_key.script();
        match script.last().copied() {
            Some(OP_CHECK_SIG) => {
                let pk = secp256k1::XOnlyPublicKey::from_slice(&script[1..33])?;
                let sig = secp256k1::schnorr::Signature::from_slice(&input.signature_script[1..65])?;
                let sig_hash = calc_schnorr_signature_hash(tx, i, SIG_HASH_ALL, &reused_values);
                let msg = secp256k1::Message::from_digest_slice(sig_hash.as_bytes().as_slice())?;
                sig.verify(&msg, &pk)?;
            }
            Some(OP_CHECK_SIG_ECDSA) => {
                let pk = secp256k1::PublicKey::from_slice(&script[1..34])?;
                let sig = secp256k1::ecdsa::Signature::from_compact(&input.signature_script[1..65])?;
                let sig_hash = calc_ecdsa_signature_hash(tx, i, SIG_HASH_ALL, &reused_values);
                let msg = secp256k1::Message::from_digest_slice(sig_hash.as_bytes().as_slice())?;
                secp256k1::SECP256K1.verify_ecdsa(&msg, &sig, &pk)?;
            }
            _ => return Err(Error::Message(format!("Unsupported script template for input: {i}"))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{subnets::SubnetworkId, tx::*};
    use secp256k1::{Secp256k1, rand};
    use std::str::FromStr;

    #[test]
    fn test_and_verify_sign() {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let script_pub_key = ScriptVec::from_slice(&public_key.serialize());

        let (secret_key2, public_key2) = secp.generate_keypair(&mut rand::thread_rng());
        let script_pub_key2 = ScriptVec::from_slice(&public_key2.serialize());

        let prev_tx_id = TransactionId::from_str("880eb9819a31821d9d2399e2f35e2433b72637e393d71ecc9b8d0250f49153c3").unwrap();
        let unsigned_tx = Transaction::new(
            0,
            vec![
                TransactionInput {
                    previous_outpoint: TransactionOutpoint { transaction_id: prev_tx_id, index: 0 },
                    signature_script: vec![],
                    sequence: 0,
                    sig_op_count: 0,
                },
                TransactionInput {
                    previous_outpoint: TransactionOutpoint { transaction_id: prev_tx_id, index: 1 },
                    signature_script: vec![],
                    sequence: 1,
                    sig_op_count: 0,
                },
                TransactionInput {
                    previous_outpoint: TransactionOutpoint { transaction_id: prev_tx_id, index: 2 },
                    signature_script: vec![],
                    sequence: 2,
                    sig_op_count: 0,
                },
            ],
            vec![
                TransactionOutput { value: 300, script_public_key: ScriptPublicKey::new(0, script_pub_key.clone()) },
                TransactionOutput { value: 300, script_public_key: ScriptPublicKey::new(0, script_pub_key.clone()) },
            ],
            1615462089000,
            SubnetworkId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            0,
            vec![],
        );

        let entries = vec![
            UtxoEntry {
                amount: 100,
                script_public_key: ScriptPublicKey::new(0, script_pub_key.clone()),
                block_daa_score: 0,
                is_coinbase: false,
            },
            UtxoEntry {
                amount: 200,
                script_public_key: ScriptPublicKey::new(0, script_pub_key),
                block_daa_score: 0,
                is_coinbase: false,
            },
            UtxoEntry {
                amount: 300,
                script_public_key: ScriptPublicKey::new(0, script_pub_key2),
                block_daa_score: 0,
                is_coinbase: false,
            },
        ];
        let signed_tx = sign_with_multiple(
            SignableTransaction::with_entries(unsigned_tx, entries),
            vec![secret_key.secret_bytes(), secret_key2.secret_bytes()],
        );

        assert!(verify(&signed_tx.as_verifiable()).is_ok());
    }

    /// Builds a single-input single-output transaction whose UTXO entry uses
    /// the supplied `script_public_key`. The output is irrelevant for signing
    /// and reuses the same script.
    fn build_one_input_tx(script_pub_key: &[u8]) -> (Transaction, Vec<UtxoEntry>) {
        let prev_tx_id = TransactionId::from_str("880eb9819a31821d9d2399e2f35e2433b72637e393d71ecc9b8d0250f49153c3").unwrap();
        let spk = ScriptPublicKey::new(0, ScriptVec::from_slice(script_pub_key));
        let tx = Transaction::new(
            0,
            vec![TransactionInput {
                previous_outpoint: TransactionOutpoint { transaction_id: prev_tx_id, index: 0 },
                signature_script: vec![],
                sequence: 0,
                sig_op_count: 0,
            }],
            vec![TransactionOutput { value: 50, script_public_key: spk.clone() }],
            1615462089000,
            SubnetworkId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            0,
            vec![],
        );
        let entries = vec![UtxoEntry { amount: 100, script_public_key: spk, block_daa_score: 0, is_coinbase: false }];
        (tx, entries)
    }

    #[test]
    fn test_v2_signs_ecdsa_p2pk_input() {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let compressed = public_key.serialize();
        let mut spk = Vec::with_capacity(35);
        spk.push(OP_DATA_33);
        spk.extend_from_slice(&compressed);
        spk.push(OP_CHECK_SIG_ECDSA);

        let (tx, entries) = build_one_input_tx(&spk);
        let signed = sign_with_multiple_v2(SignableTransaction::with_entries(tx, entries), &[secret_key.secret_bytes()]);
        let fully = signed.fully_signed().expect("ecdsa input should be signed");
        assert!(verify_v2(&fully.as_verifiable()).is_ok(), "ecdsa signature must verify under verify_v2");
    }

    #[test]
    fn test_v2_signs_schnorr_p2pk_input() {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let xonly = public_key.x_only_public_key().0.serialize();
        let mut spk = Vec::with_capacity(34);
        spk.push(OP_DATA_32);
        spk.extend_from_slice(&xonly);
        spk.push(OP_CHECK_SIG);

        let (tx, entries) = build_one_input_tx(&spk);
        let signed = sign_with_multiple_v2(SignableTransaction::with_entries(tx, entries), &[secret_key.secret_bytes()]);
        let fully = signed.fully_signed().expect("schnorr input should be signed");
        assert!(verify_v2(&fully.as_verifiable()).is_ok(), "schnorr signature must verify under verify_v2");
    }

    #[test]
    fn test_v2_signs_mixed_scheme_inputs() {
        let secp = Secp256k1::new();
        // Key A drives a Schnorr input; key B drives an ECDSA input. Both
        // privkeys are passed in one shot - dispatch happens by script template.
        let (sk_a, pk_a) = secp.generate_keypair(&mut rand::thread_rng());
        let (sk_b, pk_b) = secp.generate_keypair(&mut rand::thread_rng());
        let xonly_a = pk_a.x_only_public_key().0.serialize();
        let compressed_b = pk_b.serialize();

        let mut spk_a = Vec::with_capacity(34);
        spk_a.push(OP_DATA_32);
        spk_a.extend_from_slice(&xonly_a);
        spk_a.push(OP_CHECK_SIG);

        let mut spk_b = Vec::with_capacity(35);
        spk_b.push(OP_DATA_33);
        spk_b.extend_from_slice(&compressed_b);
        spk_b.push(OP_CHECK_SIG_ECDSA);

        let prev_tx_id = TransactionId::from_str("880eb9819a31821d9d2399e2f35e2433b72637e393d71ecc9b8d0250f49153c3").unwrap();
        let spk_a_pub = ScriptPublicKey::new(0, ScriptVec::from_slice(&spk_a));
        let spk_b_pub = ScriptPublicKey::new(0, ScriptVec::from_slice(&spk_b));
        let tx = Transaction::new(
            0,
            vec![
                TransactionInput {
                    previous_outpoint: TransactionOutpoint { transaction_id: prev_tx_id, index: 0 },
                    signature_script: vec![],
                    sequence: 0,
                    sig_op_count: 0,
                },
                TransactionInput {
                    previous_outpoint: TransactionOutpoint { transaction_id: prev_tx_id, index: 1 },
                    signature_script: vec![],
                    sequence: 1,
                    sig_op_count: 0,
                },
            ],
            vec![TransactionOutput { value: 150, script_public_key: spk_a_pub.clone() }],
            1615462089000,
            SubnetworkId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            0,
            vec![],
        );
        let entries = vec![
            UtxoEntry { amount: 100, script_public_key: spk_a_pub, block_daa_score: 0, is_coinbase: false },
            UtxoEntry { amount: 100, script_public_key: spk_b_pub, block_daa_score: 0, is_coinbase: false },
        ];
        let signed =
            sign_with_multiple_v2(SignableTransaction::with_entries(tx, entries), &[sk_a.secret_bytes(), sk_b.secret_bytes()]);
        let fully = signed.fully_signed().expect("mixed-scheme tx should be fully signed");
        assert!(verify_v2(&fully.as_verifiable()).is_ok(), "mixed-scheme signatures must verify");
    }

    #[test]
    fn test_v2_unmatched_script_returns_partial() {
        // A redeem-script-style template (no matching P2PK signer in the
        // privkey set) should leave the input unsigned and surface
        // `Signed::Partially`.
        let opaque_script = vec![0x51, 0x52]; // OP_1 OP_2 - does not match either P2PK template.
        let (tx, entries) = build_one_input_tx(&opaque_script);
        let secp = Secp256k1::new();
        let (sk, _pk) = secp.generate_keypair(&mut rand::thread_rng());
        let signed = sign_with_multiple_v2(SignableTransaction::with_entries(tx, entries), &[sk.secret_bytes()]);
        match signed {
            Signed::Partially(_) => {}
            Signed::Fully(_) => panic!("non-P2PK input must NOT be reported as fully signed"),
        }
    }
}
