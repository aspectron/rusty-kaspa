use crate::error::Error;
use crate::pskt::{Inner as PSKTInner, PSKT};
use hex;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub inner_list: Vec<PSKTInner>,
}

impl<ROLE> From<PSKT<ROLE>> for Bundle {
    fn from(pskt: PSKT<ROLE>) -> Self {
        Bundle { inner_list: vec![pskt.deref().clone()] }
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

    // todo: field support for bincode
    pub fn to_hex(&self) -> Result<String, Box<dyn std::error::Error>> {
        // Serialize the bundle to JSON
        let json_string = serde_json::to_string(self)?;

        // Encode the JSON string to hexadecimal
        Ok(hex::encode(json_string))
    }

    pub fn from_hex(hex_data: &str) -> Result<Self, Error> {
        // Decode the hexadecimal string to JSON string
        let json_string = hex::decode(hex_data)?;

        // Deserialize the JSON string to a bundle
        let bundle: Bundle = serde_json::from_slice(&json_string).unwrap();
        Ok(bundle)
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
