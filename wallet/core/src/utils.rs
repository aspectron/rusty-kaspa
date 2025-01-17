//!
//! Kaspa value formatting and parsing utilities.
//!

use crate::result::Result;
use kaspa_addresses::Address;
use kaspa_consensus_core::constants::*;
use kaspa_consensus_core::network::NetworkType;
//use kaspa_consensus_core::subnets::SubnetworkId;
use crate::error::Error;
use separator::{separated_float, separated_int, separated_uint_with_output, Separatable};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::JsValue;
use workflow_http::get_json;
use workflow_log::style;
use workflow_log::log_warn;
// use crate::utxo::context::UtxoContext;
// use crate::storage::transaction::record::TransactionRecord;
// use crate::storage::transaction::data::TransactionData;
// use crate::utxo::UtxoEntryReference;
// use crate::storage::transaction::UtxoRecord;
// use std::str::FromStr;
// use kaspa_hashes::Hash;

// Add Transaction struct
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub subnetwork_id: String,
    pub transaction_id: String,
    pub hash: String,
    pub mass: String,
    pub payload: Option<String>,
    pub block_hash: Vec<String>,
    pub block_time: u64,
    pub is_accepted: bool,
    pub accepting_block_hash: String,
    pub accepting_block_blue_score: u64,
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<TransactionOutput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionInput {
    pub transaction_id: String,
    pub index: u32,
    pub previous_outpoint_hash: String,
    pub previous_outpoint_index: String,
    pub previous_outpoint_resolved: PreviousOutpointResolved,
    pub previous_outpoint_address: String,
    pub previous_outpoint_amount: u64,
    pub signature_script: String,
    pub sig_op_count: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreviousOutpointResolved {
    pub transaction_id: String,
    pub index: u32,
    pub amount: u64,
    pub script_public_key: String,
    pub script_public_key_address: String,
    pub script_public_key_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionOutput {
    pub transaction_id: String,
    pub index: u32,
    pub amount: u64,
    pub script_public_key: String,
    pub script_public_key_address: String,
    pub script_public_key_type: String,
}

// impl TryFrom<Transaction> for kaspa_consensus_core::tx::Transaction {
//     type Error = Error;
//     fn try_from(tx: Transaction) -> Result<Self> {
//         Ok(Self::new(0, tx.inputs.try_into()?,
//          tx.outputs.try_into()?, 0, SubnetworkId::from_str(&tx.subnetwork_id)?, 0, tx.payload.as_slice().to_vec()))
//     }
// }

// impl TryFrom<TransactionInput> for kaspa_consensus_core::tx::TransactionInput {
//     type Error = Error;
//     fn try_from(input: TransactionInput) -> Result<Self> {
//         let sequence = 0;//input.sequence.parse::<u64>()?;
//         let previous_outpoint = kaspa_consensus_core::tx::TransactionOutpoint::new(Hash::from_str(&input.previous_outpoint_hash)?, input.previous_outpoint_index.parse::<u32>()?);
//         Ok(Self::new(
//             previous_outpoint,
//             input.signature_script.as_slice().to_vec().into(),
//             sequence,
//              input.sig_op_count.parse::<u32>()?))
//     }
// }

// impl Transaction {

//     pub fn meta_transaction_record(&self, utxo_context: &UtxoContext, utxos: &Vec<UtxoEntryReference>) -> Result<TransactionRecord> {

//         let block_daa_score = utxos[0].utxo.block_daa_score;
//         let utxo_entries = utxos.iter().map(UtxoRecord::from).collect::<Vec<_>>();
//         let aggregate_input_value = utxo_entries.iter().map(|utxo| utxo.amount).sum::<u64>();
//         let aggregate_output_value = self.outputs.iter().map(|output| output.amount).sum::<u64>();

//         let transaction_data = TransactionData::Meta{
//             fees: 0,
//             aggregate_input_value,
//             aggregate_output_value,
//             transaction: self.into(),
//             payment_value: None,
//             change_value: 0,
//             accepted_daa_score: Some(block_daa_score),
//             utxo_entries,
//         };
//         let transaction_record = TransactionRecord{
//             id: Hash::from_str(&self.transaction_id)?,
//             unixtime_msec: Some(self.block_time),
//             value: aggregate_input_value,
//             binding: utxo_context.binding().into(),
//             transaction_data,
//             block_daa_score,
//             network_id: utxo_context.processor().network_id()?,
//             metadata: None,
//             note: None,
//         };

//         Ok(transaction_record)
//     }
// }

//KASPLEX
pub const KASPLEX_HEADER_LC: &[u8] = b"kasplex"; // &[107, 97, 115, 112, 108, 101, 120]
pub const KASPLEX_HEADER_LC_HEX: &str = "6b6173706c6578";
pub const KASPLEX_HEADER_UC: &[u8] = b"KASPLEX"; // &[75, 65, 83, 80, 76, 69, 88]
pub const KASPLEX_HEADER_UC_HEX: &str = "4b4153504c4558";

//KSPR
pub const KSPR_HEADER_LC: &[u8] = b"kspr"; // &[107, 115, 112, 114]
pub const KSPR_HEADER_LC_HEX: &str = "6b737072";
pub const KSPR_HEADER_UC: &[u8] = b"KSPR"; // &[75, 83, 80, 82]
pub const KSPR_HEADER_UC_HEX: &str = "4b535052";

//KRC20
pub const KRC20_HEADER_LC: &[u8] = b"krc-20"; // &[107, 114, 99, 45, 50, 48]
pub const KRC20_HEADER_LC_HEX: &str = "6b72632d3230";
pub const KRC20_HEADER_UC: &[u8] = b"KRC-20"; // &[75, 82, 67, 45, 50, 48]
pub const KRC20_HEADER_UC_HEX: &str = "4b52432d3230";

//KRC721
pub const KRC721_HEADER_LC: &[u8] = b"krc-721"; // &[107, 114, 99, 45, 55, 50, 49]
pub const KRC721_HEADER_LC_HEX: &str = "6b72632d373231";
pub const KRC721_HEADER_UC: &[u8] = b"KRC-721"; // &[75, 82, 67, 45, 55, 50, 49]
pub const KRC721_HEADER_UC_HEX: &str = "4b52432d373231";

pub fn detect_krc20_or_krc721(signature: &str) -> bool {
    signature.contains(KRC20_HEADER_LC_HEX) || signature.contains(KRC721_HEADER_LC_HEX)
}

pub fn detect_kspr_or_kasplex(signature: &str) -> bool {
    signature.contains(KSPR_HEADER_LC_HEX) || signature.contains(KASPLEX_HEADER_LC_HEX)
}

pub fn detect_meta_tokens(signature: &str) -> bool {
    let signature = signature.to_lowercase();
    detect_kspr_or_kasplex(&signature) && detect_krc20_or_krc721(&signature)
}

pub async fn get_transaction_by_id(txid: &str) -> Result<Transaction> {
    let url = format!("https://api.kaspa.org/transactions/{}", txid);

    let res = get_json::<Transaction>(&url).await.map_err(|e| Error::custom(e.to_string()));
    log_warn!("### get_transaction_by_id: {:?}", res);
    return res;
}

pub fn try_kaspa_str_to_sompi<S: Into<String>>(s: S) -> Result<Option<u64>> {
    let s: String = s.into();
    let amount = s.trim();
    if amount.is_empty() {
        return Ok(None);
    }

    Ok(Some(str_to_sompi(amount)?))
}

pub fn try_kaspa_str_to_sompi_i64<S: Into<String>>(s: S) -> Result<Option<i64>> {
    let s: String = s.into();
    let amount = s.trim();
    if amount.is_empty() {
        return Ok(None);
    }

    let amount = amount.parse::<f64>()? * SOMPI_PER_KASPA as f64;
    Ok(Some(amount as i64))
}

#[inline]
pub fn sompi_to_kaspa(sompi: u64) -> f64 {
    sompi as f64 / SOMPI_PER_KASPA as f64
}

#[inline]
pub fn kaspa_to_sompi(kaspa: f64) -> u64 {
    (kaspa * SOMPI_PER_KASPA as f64) as u64
}

#[inline]
pub fn sompi_to_kaspa_string(sompi: u64) -> String {
    sompi_to_kaspa(sompi).separated_string()
}

#[inline]
pub fn sompi_to_kaspa_string_with_trailing_zeroes(sompi: u64) -> String {
    separated_float!(format!("{:.8}", sompi_to_kaspa(sompi)))
}

pub fn kaspa_suffix(network_type: &NetworkType) -> &'static str {
    match network_type {
        NetworkType::Mainnet => "KAS",
        NetworkType::Testnet => "TKAS",
        NetworkType::Simnet => "SKAS",
        NetworkType::Devnet => "DKAS",
    }
}

#[inline]
pub fn sompi_to_kaspa_string_with_suffix(sompi: u64, network_type: &NetworkType) -> String {
    let kas = sompi_to_kaspa_string(sompi);
    let suffix = kaspa_suffix(network_type);
    format!("{kas} {suffix}")
}

#[inline]
pub fn sompi_to_kaspa_string_with_trailing_zeroes_and_suffix(sompi: u64, network_type: &NetworkType) -> String {
    let kas = sompi_to_kaspa_string_with_trailing_zeroes(sompi);
    let suffix = kaspa_suffix(network_type);
    format!("{kas} {suffix}")
}

pub fn format_address_colors(address: &Address, range: Option<usize>) -> String {
    let address = address.to_string();

    let parts = address.split(':').collect::<Vec<&str>>();
    let prefix = style(parts[0]).dim();
    let payload = parts[1];
    let range = range.unwrap_or(6);
    let start = range;
    let finish = payload.len() - range;

    let left = &payload[0..start];
    let center = style(&payload[start..finish]).dim();
    let right = &payload[finish..];

    format!("{prefix}:{left}:{center}:{right}")
}

fn str_to_sompi(amount: &str) -> Result<u64> {
    let Some(dot_idx) = amount.find('.') else {
        return Ok(amount.parse::<u64>()? * SOMPI_PER_KASPA);
    };
    let integer = amount[..dot_idx].parse::<u64>()? * SOMPI_PER_KASPA;
    let decimal = &amount[dot_idx + 1..];
    let decimal_len = decimal.len();
    let decimal = if decimal_len == 0 {
        0
    } else if decimal_len <= 8 {
        decimal.parse::<u64>()? * 10u64.pow(8 - decimal_len as u32)
    } else {
        // TODO - discuss how to handle values longer than 8 decimal places
        // (reject, truncate, ceil(), etc.)
        decimal[..8].parse::<u64>()?
    };
    Ok(integer + decimal)
}

// Helper function to recursively convert `u64` to bigint in `serde_json::Value`.
pub fn convert_u64_to_bigint(value: Value) -> Result<JsValue> {
    match value {
        Value::Number(num) if num.is_u64() => Ok(js_sys::BigInt::from(num.as_u64().unwrap()).into()),
        Value::Array(arr) => {
            let mut values = Vec::new();
            for v in arr {
                values.push(convert_u64_to_bigint(v)?);
            }
            Ok(js_sys::Array::from_iter(values).into())
        }
        Value::Object(map) => {
            let obj = js_sys::Object::new();
            for (k, v) in map {
                js_sys::Reflect::set(&obj, &JsValue::from(k), &convert_u64_to_bigint(v)?)?;
            }
            Ok(obj.into())
        }
        _ => Ok(serde_wasm_bindgen::to_value(&value)?),
    }
}

// Main function to serialize the enum to `JsValue`
pub fn to_js_value_with_u64_as_bigint<T: Serialize>(value: &T) -> Result<JsValue> {
    let json_value = serde_json::to_value(value)?;
    convert_u64_to_bigint(json_value)
}
