use crate::derivation::gen1::WalletDerivationManager;
use crate::result::Result;
use crate::wasm::keys::PrivateKey;
use kaspa_bip32::{ChildNumber, ExtendedPrivateKey, SecretKey};
use std::str::FromStr;
use wasm_bindgen::prelude::*;

///
/// Helper class to generate private keys from an extended private key (XPrv).
/// This class accepts the master Kaspa XPrv string (e.g. `xprv1...`) and generates
/// private keys for the receive and change paths given the pre-set parameters
/// such as account index, multisig purpose and cosigner index.
///
/// Please note that in Kaspa master private keys use `kprv` prefix.
///
/// @see {@link PublicKeyGenerator}, {@link XPub}, {@link XPrv}, {@link Mnemonic}
/// @category Wallet SDK
///
#[wasm_bindgen]
pub struct PrivateKeyGenerator {
    receive: ExtendedPrivateKey<SecretKey>,
    change: ExtendedPrivateKey<SecretKey>,
}
#[wasm_bindgen]
impl PrivateKeyGenerator {
    #[wasm_bindgen(constructor)]
    pub fn new(xprv: &str, is_multisig: bool, account_index: u64, cosigner_index: Option<u32>) -> Result<PrivateKeyGenerator> {
        let xkey = ExtendedPrivateKey::<SecretKey>::from_str(xprv)?;
        let receive = xkey.clone().derive_path(WalletDerivationManager::build_derivate_path(
            is_multisig,
            account_index,
            cosigner_index,
            Some(kaspa_bip32::AddressType::Receive),
        )?)?;
        let change = xkey.derive_path(WalletDerivationManager::build_derivate_path(
            is_multisig,
            account_index,
            cosigner_index,
            Some(kaspa_bip32::AddressType::Change),
        )?)?;

        Ok(Self { receive, change })
    }

    #[wasm_bindgen(js_name=receiveKey)]
    pub fn receive_key(&self, index: u32) -> Result<PrivateKey> {
        let xkey = self.receive.derive_child(ChildNumber::new(index, false)?)?;
        Ok(PrivateKey::from(xkey.private_key()))
    }

    #[wasm_bindgen(js_name=changeKey)]
    pub fn change_key(&self, index: u32) -> Result<PrivateKey> {
        let xkey = self.change.derive_child(ChildNumber::new(index, false)?)?;
        Ok(PrivateKey::from(xkey.private_key()))
    }
}