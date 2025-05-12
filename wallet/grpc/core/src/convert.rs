use crate::kaspawalletd::{Outpoint, ScriptPublicKey, UtxoEntry, UtxosByAddressesEntry};
use crate::protoserialization;
// use std::num::TryFromIntError;
use crate::protoserialization::{
    PartiallySignedInput, PartiallySignedTransaction, PubKeySignaturePair, SubnetworkId, TransactionMessage, TransactionOutput,
};
use kaspa_bip32::{secp256k1, DerivationPath};
use kaspa_rpc_core::{
    RpcScriptPublicKey, RpcSubnetworkId, RpcTransaction, RpcTransactionInput, RpcTransactionOutpoint, RpcTransactionOutput,
};
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspa_wallet_core::api::{ScriptPublicKeyWrapper, TransactionOutpointWrapper, UtxoEntryWrapper};
use kaspa_wallet_core::derivation::ExtendedPublicKeySecp256k1;
use prost::Message;
use std::num::TryFromIntError;
use std::str::FromStr;
use tonic::Status;

/// Deserializes a vector of transaction byte arrays into RpcTransaction.
///
/// # Arguments
/// * `txs` - Vector of transaction byte arrays to deserialize
/// * `is_domain` - Boolean flag indicating whether the transactions are domain transactions
///
/// # Returns
/// * `Result<Vec<RpcTransaction>, Status>` - Vector of deserialized transactions or error status
pub fn deserialize_txs(txs: Vec<Vec<u8>>, is_domain: bool) -> Result<Vec<RpcTransaction>, Status> {
    txs.into_iter()
        .map(|tx| if is_domain { deserialize_domain_tx(tx.as_slice()) } else { extract_tx(tx.as_slice()) })
        .collect::<Result<Vec<_>, Status>>()
}

/// Deserializes a domain transaction from bytes into an RpcTransaction.
///
/// # Arguments
/// * `tx` - Byte slice containing the domain transaction data
///
/// # Returns
/// * `Result<RpcTransaction, Status>` - Deserialized transaction or error status
fn deserialize_domain_tx(tx: &[u8]) -> Result<RpcTransaction, Status> {
    let tx = TransactionMessage::decode(tx).map_err(|err| Status::invalid_argument(err.to_string()))?;
    RpcTransaction::try_from(tx)
}

/// Extracts and deserializes a partially signed transaction from bytes into an RpcTransaction.
///
/// # Arguments
/// * `tx` - Byte slice containing the partially signed transaction data
///
/// # Returns
/// * `Result<RpcTransaction, Status>` - Deserialized transaction or error status
fn extract_tx(tx: &[u8]) -> Result<RpcTransaction, Status> {
    let tx = PartiallySignedTransaction::decode(tx).map_err(|err| Status::invalid_argument(err.to_string()))?;
    // TODO: ecdsa param
    let tx_message = extract_tx_deserialized(tx, false)?;
    RpcTransaction::try_from(tx_message)
}

fn extract_tx_deserialized(mut partially_signed_tx: PartiallySignedTransaction, ecdsa: bool) -> Result<TransactionMessage, Status> {
    for (i, input) in partially_signed_tx.partially_signed_inputs.iter().enumerate() {
        let is_multi_sig = input.pub_key_signature_pairs.len() > 1;
        if is_multi_sig {
            let mut script_builder = &mut ScriptBuilder::new();
            let mut signature_counter = 0;
            for pair in input.pub_key_signature_pairs.iter() {
                script_builder = script_builder.add_data(pair.signature.as_slice()).unwrap();
                signature_counter += 1;
            }

            if signature_counter < input.minimum_signatures {
                return Err(Status::invalid_argument(format!("missing {} signatures", input.minimum_signatures - signature_counter)));
            }

            let redeem_script = partially_signed_input_multisig_redeem_script(input, ecdsa);
            script_builder = script_builder.add_data(redeem_script.as_slice()).unwrap();
            let sig_script = script_builder.script();
            partially_signed_tx.tx.as_mut().unwrap().inputs[i].signature_script = Vec::from(sig_script);
        } else {
            // TODO: check signature on nil
            if input.pub_key_signature_pairs.first().is_none() {
                return Err(Status::invalid_argument("missing signature"));
            }
            let mut script_builder = ScriptBuilder::new();
            let sig_script = script_builder.add_data(input.pub_key_signature_pairs[0].signature.as_slice()).unwrap().script();
            partially_signed_tx.tx.as_mut().unwrap().inputs[i].signature_script = Vec::from(sig_script);
        }
    }
    Ok(partially_signed_tx.tx.unwrap())
}

fn partially_signed_input_multisig_redeem_script(input: &PartiallySignedInput, ecdsa: bool) -> Vec<u8> {
    let mut extended_pub_keys = Vec::with_capacity(input.pub_key_signature_pairs.len());
    for key in input.pub_key_signature_pairs.iter() {
        extended_pub_keys.push(key);
    }
    multi_sig_redeem_script(extended_pub_keys, input.minimum_signatures, "m", ecdsa)
}

fn multi_sig_redeem_script(pub_keys: Vec<&PubKeySignaturePair>, minimum_sig: u32, path: &str, ecdsa: bool) -> Vec<u8> {
    let mut script_builder = &mut ScriptBuilder::new();
    script_builder.add_i64(minimum_sig as i64).unwrap();
    for key in pub_keys.iter() {
        // let extended_key:ExtendedPublicKey<>  = key.extended_pub_key.as_str().parse::<ExtendedKey>().unwrap();
        // let extended_key: ExtendedPublicKey<ExtendedPublicKeySecp256k1> = ExtendedPublicKey::from_str(key.extended_pub_key.as_str()).unwrap();
        let extended_key: ExtendedPublicKeySecp256k1 = ExtendedPublicKeySecp256k1::from_str(key.extended_pub_key.as_str()).unwrap();
        let derived_key = extended_key.derive_path(&path.parse::<DerivationPath>().unwrap()).unwrap();
        let public_key = derived_key.public_key;
        let serialized_pub_key = if ecdsa {
            public_key.serialize()
        } else {
            let schorr_pub_key = secp256k1::Keypair::from_str(public_key.to_string().as_str()).unwrap().public_key();
            schorr_pub_key.serialize()
        };

        script_builder = script_builder.add_data(serialized_pub_key.as_slice()).unwrap();
    }
    script_builder = script_builder.add_i64(pub_keys.len() as i64).unwrap();

    if ecdsa {
        // TODO: create constants for op code
        script_builder = script_builder.add_op(0xa9).unwrap();
    } else {
        script_builder = script_builder.add_op(0xae).unwrap();
    }

    Vec::from(script_builder.script())
}

impl From<TransactionOutpointWrapper> for Outpoint {
    fn from(wrapper: kaspa_wallet_core::api::TransactionOutpointWrapper) -> Self {
        Outpoint { transaction_id: wrapper.transaction_id.to_string(), index: wrapper.index }
    }
}

impl From<ScriptPublicKeyWrapper> for ScriptPublicKey {
    fn from(script_pub_key: ScriptPublicKeyWrapper) -> Self {
        ScriptPublicKey { script_public_key: script_pub_key.script_public_key, version: script_pub_key.version.into() }
    }
}

impl From<UtxoEntryWrapper> for UtxosByAddressesEntry {
    fn from(wrapper: UtxoEntryWrapper) -> Self {
        UtxosByAddressesEntry {
            address: wrapper.address.map(|addr| addr.to_string()).unwrap_or_default(),
            outpoint: Some(wrapper.outpoint.into()),
            utxo_entry: Some(UtxoEntry {
                amount: wrapper.amount,
                script_public_key: Some(wrapper.script_public_key.into()),
                block_daa_score: wrapper.block_daa_score,
                is_coinbase: wrapper.is_coinbase,
            }),
        }
    }
}

impl TryFrom<protoserialization::TransactionMessage> for RpcTransaction {
    type Error = Status;

    fn try_from(
        // protoserialization::TransactionMessage { version, inputs, outputs, lock_time, subnetwork_id, gas, payload }: protoserialization::TransactionMessage,
        value: protoserialization::TransactionMessage,
    ) -> Result<Self, Self::Error> {
        let version: u16 = value.version.try_into().map_err(|e: TryFromIntError| Status::invalid_argument(e.to_string()))?;
        let inputs: Result<Vec<RpcTransactionInput>, Status> = value
            .inputs
            .into_iter()
            .map(|i| RpcTransactionInput::try_from(i).map_err(|e| Status::invalid_argument(e.to_string())))
            .collect();
        let outputs: Result<Vec<RpcTransactionOutput>, Status> = value
            .outputs
            .into_iter()
            .map(|i| RpcTransactionOutput::try_from(i).map_err(|e| Status::invalid_argument(e.to_string())))
            .collect();
        Ok(RpcTransaction {
            version,
            inputs: inputs?,
            outputs: outputs?,
            lock_time: value.lock_time,
            subnetwork_id: value.subnetwork_id.unwrap().try_into()?,
            gas: value.gas,
            payload: value.payload,
            mass: 0,
            verbose_data: None,
        })
    }
}

impl TryFrom<protoserialization::TransactionInput> for RpcTransactionInput {
    type Error = Status;
    fn try_from(value: protoserialization::TransactionInput) -> Result<Self, Self::Error> {
        let previous_outpoint = value.previous_outpoint.unwrap().try_into()?;
        let sig_op_count: u8 = value.sig_op_count.try_into().map_err(|e: TryFromIntError| Status::invalid_argument(e.to_string()))?;
        Ok(RpcTransactionInput {
            previous_outpoint,
            signature_script: value.signature_script,
            sequence: value.sequence,
            sig_op_count,
            verbose_data: None,
        })
    }
}

impl TryFrom<TransactionOutput> for RpcTransactionOutput {
    type Error = Status;

    fn try_from(value: TransactionOutput) -> Result<Self, Self::Error> {
        Ok(RpcTransactionOutput {
            value: value.value,
            script_public_key: value.script_public_key.unwrap().try_into()?,
            verbose_data: None,
        })
    }
}

impl TryFrom<protoserialization::ScriptPublicKey> for RpcScriptPublicKey {
    type Error = Status;

    fn try_from(value: protoserialization::ScriptPublicKey) -> Result<Self, Self::Error> {
        let version: u16 = value.version.try_into().map_err(|e: TryFromIntError| Status::invalid_argument(e.to_string()))?;
        // TODO: convert ScriptVec
        Ok(RpcScriptPublicKey::new(version, Default::default()))
    }
}

impl TryFrom<SubnetworkId> for RpcSubnetworkId {
    type Error = Status;
    fn try_from(_value: SubnetworkId) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryFrom<protoserialization::Outpoint> for RpcTransactionOutpoint {
    type Error = Status;

    fn try_from(
        _: protoserialization::Outpoint, /*protoserialization::Outpoint{ transaction_id, index }: protoserialization::Outpoint*/
    ) -> Result<Self, Self::Error> {
        todo!()
        // Ok(RpcTransactionOutpoint { transaction_id: Default::default(), index: 0 })
    }
}

impl TryFrom<protoserialization::PartiallySignedTransaction> for RpcTransaction {
    type Error = Status;

    fn try_from(_value: PartiallySignedTransaction) -> Result<Self, Self::Error> {
        todo!()
    }
}
