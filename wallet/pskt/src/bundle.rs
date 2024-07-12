use crate::error::Error;
use crate::prelude::*;
use crate::pskt::{Inner as PSKTInner, PSKT};

use kaspa_addresses::Address;
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutpoint, UtxoEntry};

use hex;
use kaspa_txscript::{extract_script_pub_key_address, pay_to_script_hash_script};
use serde::{Deserialize, Serialize};
use std::ops::Deref;

pub fn lock_script_sig(payload: String, pubkey: Option<String>) -> Result<Vec<u8>, Error> {
    let mut payload_bytes: Vec<u8> = hex::decode(payload)?;

    if let Some(pubkey_hex) = pubkey {
        let pubkey_bytes: Vec<u8> = hex::decode(pubkey_hex)?;

        let placeholder = b"{{pubkey}}";

        // Search for the placeholder in payload bytes
        if let Some(pos) = payload_bytes.windows(placeholder.len()).position(|window| window == placeholder) {
            payload_bytes.splice(pos..pos + placeholder.len(), pubkey_bytes.iter().cloned());
        }
    }

    Ok(payload_bytes)
}

pub fn script_addr(script_sig: &[u8], prefix: kaspa_addresses::Prefix) -> Result<Address, Error> {
    let spk = pay_to_script_hash_script(script_sig);
    let p2sh = extract_script_pub_key_address(&spk, prefix).unwrap();
    Ok(p2sh)
}

pub fn script_public_key(script_sig: &[u8]) -> Result<ScriptPublicKey, Error> {
    let spk = pay_to_script_hash_script(script_sig);
    Ok(spk)
}

pub fn unlock_utxos(
    utxo_references: Vec<(UtxoEntry, TransactionOutpoint)>,
    script_public_key: ScriptPublicKey,
    script_sig: Vec<u8>,
    priority_fee_sompi: u64,
) -> Result<Bundle, Error> {
    let (successes, errors): (Vec<_>, Vec<_>) = utxo_references
        .into_iter()
        .map(|(utxo_entry, outpoint)| unlock_utxo(&utxo_entry, &outpoint, &script_public_key, &script_sig, priority_fee_sompi))
        .partition(Result::is_ok);

    let successful_bundles: Vec<_> = successes.into_iter().filter_map(Result::ok).collect();
    let error_list: Vec<_> = errors.into_iter().filter_map(Result::err).collect();

    if !error_list.is_empty() {
        return Err(Error::MultipleUnlockUtxoError(error_list));
    }

    let merged_bundle = successful_bundles.into_iter().fold(None, |acc: Option<Bundle>, bundle| match acc {
        Some(mut merged_bundle) => {
            merged_bundle.merge(bundle);
            Some(merged_bundle)
        }
        None => Some(bundle),
    });

    match merged_bundle {
        None => Err("Generating an empty PSKB".into()),
        Some(bundle) => Ok(bundle),
    }
}

pub fn unlock_utxo(
    utxo_entry: &UtxoEntry,
    outpoint: &TransactionOutpoint,
    script_public_key: &ScriptPublicKey,
    script_sig: &[u8],
    priority_fee_sompi: u64,
) -> Result<Bundle, Error> {
    let input = InputBuilder::default()
        .utxo_entry(utxo_entry.to_owned())
        .previous_outpoint(outpoint.to_owned())
        .sig_op_count(1)
        .redeem_script(script_sig.to_vec())
        .build()?;

    let output = OutputBuilder::default()
        .amount(utxo_entry.amount - priority_fee_sompi)
        .script_public_key(script_public_key.clone())
        .build()?;

    let pskt: PSKT<Constructor> = PSKT::<Creator>::default().constructor().input(input).output(output);
    Ok(pskt.into())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub inner_list: Vec<PSKTInner>,
}

impl<ROLE> From<PSKT<ROLE>> for Bundle {
    fn from(pskt: PSKT<ROLE>) -> Self {
        Bundle { inner_list: vec![pskt.deref().clone()] }
    }
}

impl<ROLE> From<Vec<PSKT<ROLE>>> for Bundle {
    fn from(pskts: Vec<PSKT<ROLE>>) -> Self {
        let inner_list = pskts.into_iter().map(|pskt| pskt.deref().clone()).collect();
        Bundle { inner_list }
    }
}

impl Bundle {
    pub fn new() -> Self {
        Self { inner_list: Vec::new() }
    }

    /// Adds an Inner instance to the bundle
    pub fn add_inner(&mut self, inner: PSKTInner) {
        self.inner_list.push(inner);
    }

    /// Adds a PSKT instance to the bundle
    pub fn add_pskt<ROLE>(&mut self, pskt: PSKT<ROLE>) {
        self.inner_list.push(pskt.deref().clone());
    }

    /// Merges another bundle into the current bundle
    pub fn merge(&mut self, other: Bundle) {
        for inner in other.inner_list {
            self.inner_list.push(inner);
        }
    }

    pub fn to_hex(&self) -> Result<String, Box<dyn std::error::Error>> {
        let type_marked = TypeMarked::new(self, Marker::Pskb).unwrap();
        Ok(hex::encode(serde_json::to_string(&type_marked)?))
    }

    pub fn from_hex(hex_data: &str) -> Result<Self, Error> {
        let bundle: TypeMarked<Bundle> = serde_json::from_slice(hex::decode(hex_data)?.as_slice())?;
        Ok(bundle.data)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Marker {
    Pskb,
}

impl Marker {
    fn as_str(&self) -> &str {
        match self {
            Marker::Pskb => "pskb",
        }
    }

    fn from_str(marker: &str) -> Result<Self, Error> {
        match marker {
            "pskb" => Ok(Marker::Pskb),
            _ => Err("Invalid pskb type marker".into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct TypeMarked<T> {
    type_marker: String,
    #[serde(flatten)]
    data: T,
}

impl<T> TypeMarked<T> {
    fn new(data: T, marker: Marker) -> Result<Self, Error> {
        let type_marker = marker.as_str().to_string();
        if Marker::from_str(&type_marker)? == marker {
            Ok(Self { type_marker, data })
        } else {
            Err("Invalid pskb type marker".into())
        }
    }
}

impl TryFrom<String> for Bundle {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Bundle::from_hex(&value)
    }
}

impl TryFrom<&str> for Bundle {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Error> {
        Bundle::from_hex(value)
    }
}
impl TryFrom<Bundle> for String {
    type Error = Error;
    fn try_from(value: Bundle) -> Result<String, Error> {
        match Bundle::to_hex(&value) {
            Ok(output) => Ok(output.to_owned()),
            Err(e) => Err(Error::PskbSerializeError(e.to_string())),
        }
    }
}

impl Default for Bundle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::role::Creator;

    #[test]
    fn test_bundle_creation() {
        let bundle = Bundle::new();
        assert!(bundle.inner_list.is_empty());
    }

    #[test]
    fn test_new_with_pskt() {
        let pskt = PSKT::<Creator>::default();
        let bundle = Bundle::from(pskt);
        assert_eq!(bundle.inner_list.len(), 1);
    }

    #[test]
    fn test_add_pskt() {
        let mut bundle = Bundle::new();
        let pskt = PSKT::<Creator>::default();
        bundle.add_pskt(pskt);
        assert_eq!(bundle.inner_list.len(), 1);
    }

    #[test]
    fn test_merge_bundles() {
        let mut bundle1 = Bundle::new();
        let mut bundle2 = Bundle::new();

        let inner1 = PSKTInner::default();
        let inner2 = PSKTInner::default();

        bundle1.add_inner(inner1.clone());
        bundle2.add_inner(inner2.clone());

        bundle1.merge(bundle2);

        assert_eq!(bundle1.inner_list.len(), 2);
    }
}
