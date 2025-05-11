use crate::kaspawalletd::{Outpoint, ScriptPublicKey, UtxoEntry, UtxosByAddressesEntry};
use crate::protoserialization;
use kaspa_rpc_core::{
    RpcScriptPublicKey, RpcSubnetworkId, RpcTransaction, RpcTransactionInput, RpcTransactionOutpoint, RpcTransactionOutput,
};
use kaspa_wallet_core::api::{ScriptPublicKeyWrapper, TransactionOutpointWrapper, UtxoEntryWrapper};
use prost::Message;
use std::num::TryFromIntError;
// use std::num::TryFromIntError;
use crate::protoserialization::{PartiallySignedTransaction, SubnetworkId, TransactionMessage, TransactionOutput};
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
    RpcTransaction::try_from(tx)
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

    fn try_from(_value: protoserialization::ScriptPublicKey) -> Result<Self, Self::Error> {
        todo!()
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
