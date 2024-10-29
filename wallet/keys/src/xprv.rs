use kaspa_bip32::{ChainCode, KeyFingerprint};

use crate::imports::*;

///
/// Extended private key (XPrv).
///
/// This class allows accepts a master seed and provides
/// functions for derivation of dependent child private keys.
///
/// Please note that Kaspa extended private keys use `kprv` prefix.
///
/// @see {@link PrivateKeyGenerator}, {@link PublicKeyGenerator}, {@link XPub}, {@link Mnemonic}
/// @category Wallet SDK
///

#[derive(Clone, CastFromJs)]
#[cfg_attr(feature = "py-sdk", pyclass)]
#[wasm_bindgen(inspectable)]
pub struct XPrv {
    inner: ExtendedPrivateKey<SecretKey>,
}

impl XPrv {
    pub fn inner(&self) -> &ExtendedPrivateKey<SecretKey> {
        &self.inner
    }
}

#[wasm_bindgen]
impl XPrv {
    #[wasm_bindgen(constructor)]
    pub fn try_new(seed: HexString) -> Result<XPrv> {
        let seed_bytes = Vec::<u8>::from_hex(String::try_from(seed)?.as_str()).map_err(|_| Error::custom("Invalid seed"))?;

        let inner = ExtendedPrivateKey::<SecretKey>::new(seed_bytes)?;
        Ok(Self { inner })
    }

    /// Create {@link XPrv} from `xprvxxxx..` string
    #[wasm_bindgen(js_name=fromXPrv)]
    pub fn from_xprv_str(xprv: String) -> Result<XPrv> {
        Ok(Self { inner: ExtendedPrivateKey::<SecretKey>::from_str(&xprv)? })
    }

    #[wasm_bindgen(js_name=deriveChild)]
    pub fn derive_child(&self, child_number: u32, hardened: Option<bool>) -> Result<XPrv> {
        let child_number = ChildNumber::new(child_number, hardened.unwrap_or(false))?;
        let inner = self.inner.derive_child(child_number)?;
        Ok(Self { inner })
    }

    #[wasm_bindgen(js_name=derivePath)]
    pub fn derive_path(&self, path: &JsValue) -> Result<XPrv> {
        let path = DerivationPath::try_cast_from(path)?;
        let inner = self.inner.clone().derive_path(path.as_ref().into())?;
        Ok(Self { inner })
    }

    #[wasm_bindgen(js_name = intoString)]
    pub fn into_string(&self, prefix: &str) -> Result<String> {
        let str = self.inner.to_extended_key(prefix.try_into()?).to_string();
        Ok(str)
    }
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string(&self) -> Result<String> {
        let str = self.inner.to_extended_key("kprv".try_into()?).to_string();
        Ok(str)
    }

    #[wasm_bindgen(js_name = toXPub)]
    pub fn to_xpub(&self) -> Result<XPub> {
        let public_key = self.inner.public_key();
        Ok(public_key.into())
    }

    #[wasm_bindgen(js_name = toPrivateKey)]
    pub fn to_private_key(&self) -> Result<PrivateKey> {
        let private_key = self.inner.private_key();
        Ok(private_key.into())
    }

    // ~~~~ Getters ~~~~

    #[wasm_bindgen(getter)]
    pub fn xprv(&self) -> Result<String> {
        let str = self.inner.to_extended_key("kprv".try_into()?).to_string();
        Ok(str)
    }

    #[wasm_bindgen(getter, js_name = "privateKey")]
    pub fn private_key_as_hex_string(&self) -> String {
        use kaspa_bip32::PrivateKey;
        self.inner.private_key().to_bytes().to_vec().to_hex()
    }

    #[wasm_bindgen(getter)]
    pub fn depth(&self) -> u8 {
        self.inner.attrs().depth
    }

    #[wasm_bindgen(getter, js_name = parentFingerprint)]
    pub fn parent_fingerprint_as_hex_string(&self) -> String {
        self.inner.attrs().parent_fingerprint.to_vec().to_hex()
    }

    #[wasm_bindgen(getter, js_name = childNumber)]
    pub fn child_number(&self) -> u32 {
        self.inner.attrs().child_number.into()
    }

    #[wasm_bindgen(getter, js_name = chainCode)]
    pub fn chain_code_as_hex_string(&self) -> String {
        self.inner.attrs().chain_code.to_vec().to_hex()
    }
}

impl XPrv {
    pub fn private_key(&self) -> &SecretKey {
        self.inner.private_key()
    }

    pub fn parent_fingerprint(&self) -> KeyFingerprint {
        self.inner.attrs().parent_fingerprint
    }

    pub fn chain_code(&self) -> ChainCode {
        self.inner.attrs().chain_code
    }
}

#[cfg(feature = "py-sdk")]
#[pymethods]
impl XPrv {
    #[new]
    fn try_new_py(seed: String) -> PyResult<XPrv> {
        let seed_bytes = Vec::<u8>::from_hex(&seed).map_err(|e| PyErr::new::<PyException, _>(format!("{}", e)))?;

        let inner = ExtendedPrivateKey::<SecretKey>::new(seed_bytes)?;
        Ok(Self { inner })
    }

    #[staticmethod]
    #[pyo3(name = "from_xprv")]
    pub fn from_xprv_str_py(xprv: String) -> PyResult<XPrv> {
        Ok(Self { inner: ExtendedPrivateKey::<SecretKey>::from_str(&xprv)? })
    }

    #[pyo3(name = "derive_child")]
    #[pyo3(signature = (child_number, hardened=None))]
    pub fn derive_child_py(&self, child_number: u32, hardened: Option<bool>) -> PyResult<XPrv> {
        let child_number = ChildNumber::new(child_number, hardened.unwrap_or(false))?;
        let inner = self.inner.derive_child(child_number)?;
        Ok(Self { inner })
    }

    #[pyo3(name = "derive_path")]
    pub fn derive_path_py(&self, path: Bound<PyAny>) -> PyResult<XPrv> {
        let path = if let Ok(path_str) = path.extract::<String>() {
            Ok(DerivationPath::new(path_str.as_str())?)
        } else if let Ok(path_obj) = path.extract::<DerivationPath>() {
            Ok(path_obj)
        } else {
            Err(PyException::new_err("`path` must be of type `str` or `DerivationPath`"))
        }?;

        let inner = self.inner.clone().derive_path((&path).into())?;
        Ok(Self { inner })
    }

    #[pyo3(name = "into_string")]
    pub fn into_string_py(&self, prefix: &str) -> PyResult<String> {
        let str = self.inner.to_extended_key(prefix.try_into()?).to_string();
        Ok(str)
    }

    #[pyo3(name = "to_string")]
    pub fn to_string_py(&self) -> PyResult<String> {
        let str = self.inner.to_extended_key("kprv".try_into()?).to_string();
        Ok(str)
    }

    #[pyo3(name = "to_xpub")]
    pub fn to_xpub_py(&self) -> PyResult<XPub> {
        let public_key = self.inner.public_key();
        Ok(public_key.into())
    }

    #[pyo3(name = "to_private_key")]
    pub fn to_private_key_py(&self) -> PyResult<PrivateKey> {
        let private_key = self.inner.private_key();
        Ok(private_key.into())
    }

    #[getter]
    #[pyo3(name = "xprv")]
    pub fn xprv_py(&self) -> PyResult<String> {
        let str = self.inner.to_extended_key("kprv".try_into()?).to_string();
        Ok(str)
    }

    #[getter]
    #[pyo3(name = "private_key")]
    pub fn private_key_as_hex_string_py(&self) -> String {
        use kaspa_bip32::PrivateKey;
        self.inner.private_key().to_bytes().to_vec().to_hex()
    }

    #[getter]
    #[pyo3(name = "depth")]
    pub fn depth_py(&self) -> u8 {
        self.inner.attrs().depth
    }

    #[getter]
    #[pyo3(name = "parent_fingerprint")]
    pub fn parent_fingerprint_as_hex_string_py(&self) -> String {
        self.inner.attrs().parent_fingerprint.to_vec().to_hex()
    }

    #[getter]
    #[pyo3(name = "child_number")]
    pub fn child_number_py(&self) -> u32 {
        self.inner.attrs().child_number.into()
    }

    #[getter]
    #[pyo3(name = "chain_code")]
    pub fn chain_code_as_hex_string_py(&self) -> String {
        self.inner.attrs().chain_code.to_vec().to_hex()
    }
}

impl<'a> From<&'a XPrv> for &'a ExtendedPrivateKey<SecretKey> {
    fn from(xprv: &'a XPrv) -> Self {
        &xprv.inner
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "XPrv | string")]
    pub type XPrvT;
}

impl TryCastFromJs for XPrv {
    type Error = Error;
    fn try_cast_from<'a, R>(value: &'a R) -> Result<Cast<Self>, Self::Error>
    where
        R: AsRef<JsValue> + 'a,
    {
        Self::resolve(value, || {
            if let Some(xprv) = value.as_ref().as_string() {
                Ok(XPrv::from_xprv_str(xprv)?)
            } else {
                Err(Error::InvalidXPrv)
            }
        })
    }
}
