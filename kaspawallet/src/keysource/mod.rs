//! Platform-aware default keyfile path resolution.
//!
//! The integrated wallet binary embeds a single keyfile-backed
//! source today: the operator's encrypted keyfile under the
//! per-OS application data directory. Future cycles may add
//! alternative backends (hardware-wallet seed import, etc.) by
//! extending this module; for now it exposes only the
//! default-path resolver and the matching error type.

mod default_path;
mod error;

pub use default_path::{default_keys_file, require_existing_keyfile};
