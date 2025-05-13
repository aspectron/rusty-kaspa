use std::ffi::OsStr;
// use std::path::PathBuf;
// use kaspa_wallet_core::wallet::SingleWalletFileV0;

// pub fn read_keys_from_file(path: PathBuf) -> Result<SingleWalletFileV0<>>

pub fn default_keys_file_path() -> &'static OsStr {
    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        OsStr::new("~/.kaspawallet/keys.json")
    }
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        OsStr::new(r"%USERPROFILE%\AppData\Local\Kaspawallet\keys.json")
    }
}