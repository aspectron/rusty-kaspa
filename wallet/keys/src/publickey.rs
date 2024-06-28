//!
//! [`keypair`](mod@self) module encapsulates [`Keypair`] and [`PrivateKey`].
//! The [`Keypair`] provides access to the secret and public keys.
//!
//! ```javascript
//!
//! let keypair = Keypair.random();
//! let privateKey = keypair.privateKey;
//! let publicKey = keypair.publicKey;
//!
//! // to obtain an address from a keypair
//! let address = keypair.toAddress(NetworkType.Mainnnet);
//!
//! // to obtain a keypair from a private key
//! let keypair = privateKey.toKeypair();
//!
//! ```
//!

use kaspa_consensus_core::network::NetworkType;

use crate::imports::*;

/// Data structure that envelopes a PublicKey.
/// Only supports Schnorr-based addresses.
/// @category Wallet SDK
#[derive(Clone, Debug, CastFromJs)]
#[cfg_attr(feature = "py-sdk", pyclass)]
#[wasm_bindgen(js_name = PublicKey)]
pub struct PublicKey {
    #[wasm_bindgen(skip)] // PY-NOTE: not exposed to Python by default, nothing needed here
    pub xonly_public_key: secp256k1::XOnlyPublicKey,
    #[wasm_bindgen(skip)] // PY-NOTE: not exposed to Python by default, nothing needed here
    pub public_key: Option<secp256k1::PublicKey>,
}

// PY-NOTE: WASM specific fn implementations
#[wasm_bindgen(js_class = PublicKey)]
impl PublicKey {
    /// Create a new [`PublicKey`] from a hex-encoded string.
    #[wasm_bindgen(constructor)]
    pub fn try_new(key: &str) -> Result<PublicKey> {
        match secp256k1::PublicKey::from_str(key) {
            Ok(public_key) => Ok((&public_key).into()),
            Err(_e) => Ok(Self { xonly_public_key: secp256k1::XOnlyPublicKey::from_str(key)?, public_key: None }),
        }
    }

    /// Get the [`Address`] of this PublicKey.
    /// Receives a [`NetworkType`] to determine the prefix of the address.
    /// JavaScript: `let address = publicKey.toAddress(NetworkType.MAINNET);`.
    #[wasm_bindgen(js_name = toAddress)]
    pub fn to_address_js(&self, network: &NetworkTypeT) -> Result<Address> {
        self.to_address(network.try_into()?)
    }

    /// Get `ECDSA` [`Address`] of this PublicKey.
    /// Receives a [`NetworkType`] to determine the prefix of the address.
    /// JavaScript: `let address = publicKey.toAddress(NetworkType.MAINNET);`.
    #[wasm_bindgen(js_name = toAddressECDSA)]
    pub fn to_address_ecdsa_js(&self, network: &NetworkTypeT) -> Result<Address> {
        self.to_address_ecdsa(network.try_into()?)
    }
}

// PY-NOTE: fns exposed to both WASM and Python
#[cfg_attr(feature = "py-sdk", pymethods)]
#[wasm_bindgen]
impl PublicKey {
    // PY-NOTE: would like to `#[pyo3(name = "to_string")]` for this fn. But cannot use #[pyo3())] inside of a block that has pymethods applied via cfg_attr
    #[wasm_bindgen(js_name = "toString")]
    pub fn to_string_impl(&self) -> String {
        self.public_key.as_ref().map(|pk| pk.to_string()).unwrap_or_else(|| self.xonly_public_key.to_string())
    }

    #[wasm_bindgen(js_name = toXOnlyPublicKey)]
    pub fn to_x_only_public_key(&self) -> XOnlyPublicKey {
        self.xonly_public_key.into()
    }
}

// PY-NOTE: Python specific fn implementations
#[cfg(feature = "py-sdk")]
#[pymethods]
impl PublicKey {
    // PY-NOTE: #[new] has to be in block that has #[pymethods] applied directly. applying via #[cfg_attr()] does not work (PyO3 limitation).
    #[new]
    pub fn try_new_py(key: &str) -> Result<PublicKey> {
        match secp256k1::PublicKey::from_str(key) {
            Ok(public_key) => Ok((&public_key).into()),
            Err(_e) => Ok(Self { xonly_public_key: secp256k1::XOnlyPublicKey::from_str(key)?, public_key: None }),
        }
    }

    // PY-NOTE: #[pyo3()] can only be used in block that has #[pymethods] applied directly. applying via #[cfg_attr()] does not work (PyO3 limitation).
    #[pyo3(name = "to_address")]
    pub fn to_address_py(&self, network: &str) -> Result<Address> {
        // PY-NOTE: arg type of `network: &str` instead of `network: NetworkTypeT`
        self.to_address(NetworkType::from_str(network)?)
    }

    // PY-NOTE: #[pyo3()] can only be used in block that has #[pymethods] applied directly. applying via #[cfg_attr()] does not work (PyO3 limitation).
    #[pyo3(name = "to_address_ecdsa")]
    pub fn to_address_ecdsa_py(&self, network: &str) -> Result<Address> {
        // PY-NOTE: arg type of `network: &str` instead of `network: NetworkTypeT`
        self.to_address_ecdsa(NetworkType::from_str(network)?)
    }
}

impl PublicKey {
    #[inline]
    pub fn to_address(&self, network_type: NetworkType) -> Result<Address> {
        let payload = &self.xonly_public_key.serialize();
        let address = Address::new(network_type.into(), AddressVersion::PubKey, payload);
        Ok(address)
    }

    #[inline]
    pub fn to_address_ecdsa(&self, network_type: NetworkType) -> Result<Address> {
        if let Some(public_key) = self.public_key.as_ref() {
            let payload = &public_key.serialize();
            let address = Address::new(network_type.into(), AddressVersion::PubKeyECDSA, payload);
            Ok(address)
        } else {
            Err(Error::InvalidXOnlyPublicKeyForECDSA)
        }
    }
}

impl std::fmt::Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_impl())
    }
}

impl From<PublicKey> for secp256k1::XOnlyPublicKey {
    fn from(value: PublicKey) -> Self {
        value.xonly_public_key
    }
}

impl TryFrom<PublicKey> for secp256k1::PublicKey {
    type Error = Error;
    fn try_from(value: PublicKey) -> std::result::Result<Self, Self::Error> {
        value.public_key.ok_or(Error::InvalidPublicKey)
    }
}

impl TryFrom<&PublicKey> for secp256k1::PublicKey {
    type Error = Error;
    fn try_from(value: &PublicKey) -> std::result::Result<Self, Self::Error> {
        value.public_key.ok_or(Error::InvalidPublicKey)
    }
}

impl From<&secp256k1::PublicKey> for PublicKey {
    fn from(public_key: &secp256k1::PublicKey) -> Self {
        let (xonly_public_key, _) = public_key.x_only_public_key();
        Self { xonly_public_key, public_key: Some(*public_key) }
    }
}

impl From<secp256k1::PublicKey> for PublicKey {
    fn from(public_key: secp256k1::PublicKey) -> Self {
        let (xonly_public_key, _) = public_key.x_only_public_key();
        Self { xonly_public_key, public_key: Some(public_key) }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "PublicKey | string")]
    pub type PublicKeyT;

    #[wasm_bindgen(extends = Array, typescript_type = "(PublicKey | string)[]")]
    pub type PublicKeyArrayT;
}

impl TryCastFromJs for PublicKey {
    type Error = Error;
    fn try_cast_from<'a, R>(value: &'a R) -> Result<Cast<Self>, Self::Error>
    where
        R: AsRef<JsValue> + 'a,
    {
        Self::resolve(value, || {
            let value = value.as_ref();
            if let Some(hex_str) = value.as_string() {
                Ok(PublicKey::try_new(hex_str.as_str())?)
            } else {
                Err(Error::custom("Invalid PublicKey"))
            }
        })
    }
}

impl TryFrom<&PublicKeyArrayT> for Vec<secp256k1::PublicKey> {
    type Error = Error;
    fn try_from(value: &PublicKeyArrayT) -> Result<Self> {
        if value.is_array() {
            let array = Array::from(value);
            let pubkeys = array.iter().map(PublicKey::try_owned_from).collect::<Result<Vec<_>>>()?;
            Ok(pubkeys.iter().map(|pk| pk.try_into()).collect::<Result<Vec<_>>>()?)
        } else {
            Err(Error::InvalidPublicKeyArray)
        }
    }
}

///
/// Data structure that envelopes a XOnlyPublicKey.
///
/// XOnlyPublicKey is used as a payload part of the {@link Address}.
///
/// @see {@link PublicKey}
/// @category Wallet SDK
#[wasm_bindgen]
#[cfg_attr(feature = "py-sdk", pyclass)]
#[derive(Clone, Debug, CastFromJs)]
pub struct XOnlyPublicKey {
    #[wasm_bindgen(skip)]
    pub inner: secp256k1::XOnlyPublicKey,
}

impl XOnlyPublicKey {
    pub fn new(inner: secp256k1::XOnlyPublicKey) -> Self {
        Self { inner }
    }
}

// PY-NOTE: WASM specific fn implementations
#[wasm_bindgen]
impl XOnlyPublicKey {
    #[wasm_bindgen(constructor)]
    pub fn try_new(key: &str) -> Result<XOnlyPublicKey> {
        Ok(secp256k1::XOnlyPublicKey::from_str(key)?.into())
    }

    /// Get the [`Address`] of this XOnlyPublicKey.
    /// Receives a [`NetworkType`] to determine the prefix of the address.
    /// JavaScript: `let address = xOnlyPublicKey.toAddress(NetworkType.MAINNET);`.
    #[wasm_bindgen(js_name = toAddress)]
    pub fn to_address(&self, network: &NetworkTypeT) -> Result<Address> {
        let payload = &self.inner.serialize();
        let address = Address::new(network.try_into()?, AddressVersion::PubKey, payload);
        Ok(address)
    }

    /// Get `ECDSA` [`Address`] of this XOnlyPublicKey.
    /// Receives a [`NetworkType`] to determine the prefix of the address.
    /// JavaScript: `let address = xOnlyPublicKey.toAddress(NetworkType.MAINNET);`.
    #[wasm_bindgen(js_name = toAddressECDSA)]
    pub fn to_address_ecdsa(&self, network: &NetworkTypeT) -> Result<Address> {
        let payload = &self.inner.serialize();
        let address = Address::new(network.try_into()?, AddressVersion::PubKeyECDSA, payload);
        Ok(address)
    }

    #[wasm_bindgen(js_name = fromAddress)]
    pub fn from_address(address: &Address) -> Result<XOnlyPublicKey> {
        Ok(secp256k1::XOnlyPublicKey::from_slice(&address.payload)?.into())
    }
}

// PY-NOTE: fns exposed to both WASM and Python
#[cfg_attr(feature = "py-sdk", pymethods)]
#[wasm_bindgen]
impl XOnlyPublicKey {
    // PY-NOTE: would like to `#[pyo3(name = "to_string")]` for this fn
    // but cannot use that inside of a block that has pymethods applied via cfg_attr
    #[wasm_bindgen(js_name = "toString")]
    pub fn to_string_impl(&self) -> String {
        self.inner.to_string()
    }
}

// PY-NOTE: Python specific fn implementations
#[cfg(feature = "py-sdk")]
#[pymethods]
impl XOnlyPublicKey {
    // PY-NOTE: #[new] can only be used in block that has #[pymethods] applied directly. applying via #[cfg_attr()] does not work (PyO3 limitation).
    #[new]
    pub fn try_new_py(key: &str) -> Result<XOnlyPublicKey> {
        Ok(secp256k1::XOnlyPublicKey::from_str(key)?.into())
    }

    pub fn to_address_py(&self, network: &str) -> PyResult<Address> {
        // PY-NOTE: arg type of `network: &str` instead of `network: NetworkTypeT`
        let payload = &self.inner.serialize();
        let address = Address::new(network.try_into()?, AddressVersion::PubKey, payload);
        Ok(address)
    }

    pub fn to_address_ecdsa_py(&self, network: &str) -> PyResult<Address> {
        // PY-NOTE: arg type of `network: &str` instead of `network: NetworkTypeT`
        let payload = &self.inner.serialize();
        let address = Address::new(network.try_into()?, AddressVersion::PubKeyECDSA, payload);
        Ok(address)
    }

    // PY-NOTE: #[pyo3] and #[staticmethod] can only be used in block that has #[pymethods] applied directly. applying via #[cfg_attr()] does not work (PyO3 limitation).
    #[pyo3(name = "from_address")]
    #[staticmethod]
    pub fn from_address_py(address: &Address) -> Result<XOnlyPublicKey> {
        Ok(secp256k1::XOnlyPublicKey::from_slice(&address.payload)?.into())
    }
}

impl std::fmt::Display for XOnlyPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_impl())
    }
}

impl From<secp256k1::XOnlyPublicKey> for XOnlyPublicKey {
    fn from(inner: secp256k1::XOnlyPublicKey) -> Self {
        Self { inner }
    }
}

impl From<XOnlyPublicKey> for secp256k1::XOnlyPublicKey {
    fn from(xonly_public_key: XOnlyPublicKey) -> Self {
        xonly_public_key.inner
    }
}

impl TryFrom<JsValue> for XOnlyPublicKey {
    type Error = Error;
    fn try_from(js_value: JsValue) -> std::result::Result<Self, Self::Error> {
        if let Some(hex_str) = js_value.as_string() {
            Ok(secp256k1::XOnlyPublicKey::from_str(hex_str.as_str())?.into())
        } else {
            Ok(XOnlyPublicKey::try_ref_from_js_value(js_value.as_ref())?.clone())
        }
    }
}
