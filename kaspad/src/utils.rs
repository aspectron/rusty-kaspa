extern crate kaspa_consensus;
extern crate kaspa_core;
extern crate kaspa_hashes;

use kaspa_consensus_core::config::Config;
use kaspa_consensus_core::errors::config::{ConfigError, ConfigResult};

use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use crate::args::Args;

pub const DEFAULT_DATA_DIR: &str = "datadir";
pub const CONSENSUS_DB: &str = "consensus";
pub const UTXOINDEX_DB: &str = "utxoindex";
pub const META_DB: &str = "meta";
pub const DEFAULT_LOG_DIR: &str = "logs";

pub fn get_home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    return dirs::data_local_dir().unwrap();
    #[cfg(not(target_os = "windows"))]
    return dirs::home_dir().unwrap();
}

pub fn get_app_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    return get_home_dir().join("rusty-kaspa");
    #[cfg(not(target_os = "windows"))]
    return get_home_dir().join(".rusty-kaspa");
}

pub fn validate_config_and_args(_config: &Arc<Config>, args: &Args) -> ConfigResult<()> {
    if !args.connect_peers.is_empty() && !args.add_peers.is_empty() {
        return Err(ConfigError::MixedConnectAndAddPeers);
    }
    if args.logdir.is_some() && args.no_log_files {
        return Err(ConfigError::MixedLogDirAndNoLogFiles);
    }
    Ok(())
}

pub fn get_user_approval_or_exit(message: &str, approve: bool) {
    if approve {
        return;
    }
    println!("{}", message);
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => {
            let lower = input.to_lowercase();
            let answer = lower.as_str().strip_suffix("\r\n").or(lower.as_str().strip_suffix('\n')).unwrap_or(lower.as_str());
            if answer == "y" || answer == "yes" {
                // return
            } else {
                println!("Operation was rejected ({}), exiting..", answer);
                exit(1);
            }
        }
        Err(error) => {
            println!("Error reading from console: {error}, exiting..");
            exit(1);
        }
    }
}
