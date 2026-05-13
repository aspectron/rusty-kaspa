//! Platform-aware default-key-path resolver.
//!
//! Default keyfile path: `<app-dir>/<network-name>/keys.json`
//! where the app-dir is per-OS:
//!
//! - POSIX (Linux/BSD): `$HOME/.kaspawallet`
//! - macOS: `$HOME/Library/Application Support/Kaspawallet`
//! - Windows: `%LOCALAPPDATA%\Kaspawallet` (fallback `%APPDATA%`)
//!
//! Home-dir / data-local-dir lookup uses the `dirs` crate, which
//! handles `$HOME` / `$XDG_*` / `%LOCALAPPDATA%` / `%APPDATA%`
//! resolution per platform.

use std::path::{Path, PathBuf};

use super::error::KeySourceError;

/// Application name segment used in the default-path tree. The
/// platform-specific case adjustment is lowercase on POSIX,
/// capitalised on macOS / Windows.
const APP_NAME: &str = "kaspawallet";

/// Keyfile filename.
const KEYS_FILE_NAME: &str = "keys.json";

/// Compute the default keyfile path for the given network name.
///
/// `network_name` MUST be the canonical kaspa network name
/// string (`"kaspa-mainnet"`, `"kaspa-testnet-10"`,
/// `"kaspa-simnet"`, `"kaspa-devnet"`). Callers derive it from
/// the merged [`crate::cli::network::NetworkFlags`] via
/// [`crate::cli::network::NetworkFlags::network_name`].
pub fn default_keys_file(network_name: &str) -> Result<PathBuf, KeySourceError> {
    let app_dir = default_app_dir()?;
    Ok(app_dir.join(network_name).join(KEYS_FILE_NAME))
}

/// Resolve the effective keyfile path: the operator-supplied
/// `--keys-file` override if present, otherwise the
/// platform-aware default.
///
/// Returns the resolved path WITHOUT touching the filesystem; the
/// caller is responsible for opening it. Use
/// [`require_existing_keyfile`] when "missing-at-resolved-path
/// is an error" semantics are needed (e.g. `parse`,
/// `dump-unencrypted-data`).
pub fn resolve_keys_file_path(override_path: Option<&Path>, network_name: &str) -> Result<PathBuf, KeySourceError> {
    if let Some(p) = override_path {
        return Ok(p.to_path_buf());
    }
    default_keys_file(network_name)
}

/// As [`resolve_keys_file_path`], but additionally returns
/// [`KeySourceError::DefaultPathMissing`] if the resolved path
/// does not exist on disk.
pub fn require_existing_keyfile(override_path: Option<&Path>, network_name: &str) -> Result<PathBuf, KeySourceError> {
    let resolved = resolve_keys_file_path(override_path, network_name)?;
    if !resolved.exists() {
        return Err(KeySourceError::DefaultPathMissing { default: resolved.display().to_string() });
    }
    Ok(resolved)
}

/// Compute the application-root directory (the parent of the
/// per-network keyfile subdirectory) per OS:
///
/// - Windows: `%LOCALAPPDATA%\Kaspawallet` (or `%APPDATA%\Kaspawallet`
///   if `LOCALAPPDATA` is unset; `dirs::data_local_dir()` already
///   prefers `LOCALAPPDATA` and falls back to `APPDATA`).
/// - macOS: `$HOME/Library/Application Support/Kaspawallet`
///   (`dirs::data_local_dir()` returns
///   `~/Library/Application Support`).
/// - Other Unix-like (Linux, BSD): `$HOME/.kaspawallet` (POSIX
///   convention joins the lowercase app name with a leading `.`;
///   the path is hard-coded here since `dirs` has no
///   "POSIX dot-prefix in $HOME" helper).
fn default_app_dir() -> Result<PathBuf, KeySourceError> {
    #[cfg(target_os = "windows")]
    {
        let base = dirs::data_local_dir().ok_or(KeySourceError::NoAppDataDir)?;
        Ok(base.join(APP_NAME_CAP))
    }
    #[cfg(target_os = "macos")]
    {
        let base = dirs::data_local_dir().ok_or(KeySourceError::NoAppDataDir)?;
        Ok(base.join(APP_NAME_CAP))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let home = dirs::home_dir().ok_or(KeySourceError::NoAppDataDir)?;
        Ok(home.join(format!(".{APP_NAME}")))
    }
}

/// Capitalised form of the application name. macOS + Windows use
/// the `Appname` form; POSIX uses `.appname`.
#[cfg(any(target_os = "windows", target_os = "macos"))]
const APP_NAME_CAP: &str = "Kaspawallet";

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the POSIX default
    /// (`$HOME/.kaspawallet/<network>/keys.json`). Skipped on
    /// non-Unix targets via cfg gate so the same test asserts
    /// platform-specific shape.
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn test_default_key_path_posix_shape() {
        let path = default_keys_file("kaspa-mainnet").expect("default path");
        let s = path.to_string_lossy();
        assert!(s.ends_with("/.kaspawallet/kaspa-mainnet/keys.json"), "POSIX default-key-path suffix mismatch; got {s}");
        let home = dirs::home_dir().expect("$HOME available in test env");
        assert!(s.starts_with(&*home.to_string_lossy()), "expected POSIX path under $HOME; got {s}");
    }

    /// Exercises the macOS default
    /// (`$HOME/Library/Application Support/Kaspawallet/<network>/keys.json`).
    #[cfg(target_os = "macos")]
    #[test]
    fn test_default_key_path_macos_shape() {
        let path = default_keys_file("kaspa-mainnet").expect("default path");
        let s = path.to_string_lossy();
        assert!(
            s.ends_with("/Library/Application Support/Kaspawallet/kaspa-mainnet/keys.json"),
            "macOS default-key-path suffix mismatch; got {s}"
        );
    }

    /// Exercises the Windows default
    /// (`%LOCALAPPDATA%\Kaspawallet\<network>\keys.json`).
    #[cfg(target_os = "windows")]
    #[test]
    fn test_default_key_path_windows_shape() {
        let path = default_keys_file("kaspa-mainnet").expect("default path");
        let s = path.to_string_lossy();
        assert!(
            s.ends_with("Kaspawallet\\kaspa-mainnet\\keys.json") || s.ends_with("Kaspawallet/kaspa-mainnet/keys.json"),
            "Windows default-key-path suffix mismatch; got {s}"
        );
    }

    #[test]
    fn test_resolve_returns_override_when_provided() {
        let override_path = PathBuf::from("/tmp/some-explicit-path/keys.json");
        let resolved = resolve_keys_file_path(Some(&override_path), "kaspa-mainnet").expect("resolve");
        assert_eq!(resolved, override_path, "operator-supplied override must win over the default-path resolver");
    }

    #[test]
    fn test_resolve_returns_default_when_override_omitted() {
        let resolved = resolve_keys_file_path(None, "kaspa-testnet-10").expect("resolve");
        let expected = default_keys_file("kaspa-testnet-10").expect("default");
        assert_eq!(resolved, expected, "absent override must trigger the default-path resolver");
    }

    #[test]
    fn test_per_network_subdir_in_path() {
        let mainnet = default_keys_file("kaspa-mainnet").expect("mainnet");
        let testnet = default_keys_file("kaspa-testnet-10").expect("testnet");
        let simnet = default_keys_file("kaspa-simnet").expect("simnet");
        let devnet = default_keys_file("kaspa-devnet").expect("devnet");

        assert_ne!(mainnet, testnet);
        assert_ne!(mainnet, simnet);
        assert_ne!(mainnet, devnet);
        assert_ne!(testnet, simnet);

        let parent_mainnet = mainnet.parent().expect("parent").parent().expect("grandparent");
        let parent_testnet = testnet.parent().expect("parent").parent().expect("grandparent");
        assert_eq!(parent_mainnet, parent_testnet, "all networks share the same application root");

        assert!(mainnet.to_string_lossy().contains("kaspa-mainnet"));
        assert!(testnet.to_string_lossy().contains("kaspa-testnet-10"));
    }

    #[test]
    fn test_require_existing_keyfile_returns_error_when_resolved_path_absent() {
        let bogus = PathBuf::from("/this/path/should/not/exist/keys.json");
        let err = require_existing_keyfile(Some(&bogus), "kaspa-mainnet").expect_err("must fail");
        assert!(matches!(err, KeySourceError::DefaultPathMissing { .. }), "got: {err:?}");
    }

    #[test]
    fn test_require_existing_keyfile_succeeds_when_override_exists() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let resolved = require_existing_keyfile(Some(&manifest), "kaspa-mainnet").expect("resolve existing");
        assert_eq!(resolved, manifest);
    }
}
