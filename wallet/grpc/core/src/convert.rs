use crate::kaspawalletd::{Outpoint, ScriptPublicKey, UtxoEntry, UtxosByAddressesEntry};
use crate::protoserialization;
// use std::num::TryFromIntError;
use crate::protoserialization::{
    PartiallySignedInput, PartiallySignedTransaction, SubnetworkId, TransactionMessage, TransactionOutput,
};
use kaspa_rpc_core::{
    RpcScriptPublicKey, RpcScriptVec, RpcSubnetworkId, RpcTransaction, RpcTransactionInput, RpcTransactionOutpoint,
    RpcTransactionOutput,
};
use kaspa_txscript::script_builder::ScriptBuilder;
use kaspa_txscript::{multisig_redeem_script, multisig_redeem_script_ecdsa};
use kaspa_wallet_core::api::{ScriptPublicKeyWrapper, TransactionOutpointWrapper, UtxoEntryWrapper};
use prost::Message;
use std::num::TryFromIntError;
use tonic::Status;

/// Deserializes a vector of transaction byte arrays into RpcTransaction.
///
/// # Arguments
/// * `txs` - Vector of transaction byte arrays to deserialize
/// * `is_domain` - Boolean flag indicating whether the transactions are domain transactions
///
/// # Returns
/// * `Result<Vec<RpcTransaction>, Status>` - Vector of deserialized transactions or error status
pub fn deserialize_txs(txs: Vec<Vec<u8>>, is_domain: bool, ecdsa: bool) -> Result<Vec<RpcTransaction>, Status> {
    txs.into_iter()
        .map(|tx| if is_domain { deserialize_domain_tx(tx.as_slice()) } else { extract_tx(tx.as_slice(), ecdsa) })
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
fn extract_tx(tx: &[u8], ecdsa: bool) -> Result<RpcTransaction, Status> {
    let tx = PartiallySignedTransaction::decode(tx).map_err(|err| Status::invalid_argument(err.to_string()))?;
    let tx_message = extract_tx_deserialized(tx, ecdsa)?;
    RpcTransaction::try_from(tx_message)
}

/// Extracts and processes a partially signed transaction into a regular transaction message.
/// Handles both single-signature and multi-signature inputs, constructing appropriate signature scripts.
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

            let redeem_script = partially_signed_input_multisig_redeem_script(input, ecdsa)?;
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

/// Generates a multi-signature redeem script for a partially signed input.
/// Supports both ECDSA and Schnorr signature schemes based on the ecdsa parameter.
fn partially_signed_input_multisig_redeem_script(input: &PartiallySignedInput, ecdsa: bool) -> Result<Vec<u8>, Status> {
    let pub_keys: Vec<_> = input.pub_key_signature_pairs.iter().map(|key| key.extended_pub_key.as_bytes()).collect();

    let redeem_script = if ecdsa {
        let extended_pub_keys: Vec<[u8; 33]> = pub_keys.into_iter().map(|bytes| <[u8; 33]>::try_from(bytes).unwrap()).collect();
        multisig_redeem_script_ecdsa(extended_pub_keys.iter(), input.minimum_signatures as usize)
    } else {
        let extended_pub_keys: Vec<[u8; 32]> = pub_keys.into_iter().map(|bytes| <[u8; 32]>::try_from(bytes).unwrap()).collect();
        multisig_redeem_script(extended_pub_keys.iter(), input.minimum_signatures as usize)
    };

    redeem_script.map_err(|err| Status::invalid_argument(err.to_string()))
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

impl TryFrom<TransactionMessage> for RpcTransaction {
    type Error = Status;

    fn try_from(
        // protoserialization::TransactionMessage { version, inputs, outputs, lock_time, subnetwork_id, gas, payload }: protoserialization::TransactionMessage,
        value: TransactionMessage,
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
        Ok(RpcScriptPublicKey::new(version, RpcScriptVec::from(value.script)))
    }
}

impl TryFrom<SubnetworkId> for RpcSubnetworkId {
    type Error = Status;
    fn try_from(value: SubnetworkId) -> Result<Self, Self::Error> {
        let bytes = value.bytes;
        if bytes.len() != 20 {
            return Err(Status::invalid_argument("SubnetworkId must be 20 bytes long"));
        }
        let mut fixed_bytes = [0u8; 20];
        fixed_bytes.copy_from_slice(&bytes);
        Ok(RpcSubnetworkId::from_bytes(fixed_bytes))
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
