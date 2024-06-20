
cfg_if::cfg_if! {
    if #[cfg(feature = "py-sdk")] {
        use pyo3::prelude::*;

        #[pymodule]
        fn kaspapy(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<kaspa_wallet_keys::privkeygen::PrivateKeyGenerator>()?;
            // m.add_class::<kaspa_wallet_keys::privatekey::PrivateKey>()?;

            Ok(())
        }
    }
}