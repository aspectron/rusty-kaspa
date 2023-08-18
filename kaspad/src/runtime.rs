use kaspa_consensus_core::config::ConfigBuilder;
use kaspa_consensus_core::networktype::{NetworkId, NetworkType};
#[allow(unused_imports)]
use kaspa_core::{info, trace};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use crate::args::Args;
use crate::utils::*;

pub struct Runtime {
    pub log_dir: Option<String>,
    pub app_dir: Option<PathBuf>,
    pub db_dir: Option<PathBuf>,
    pub network: Option<NetworkId>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn init() {
        // Configure the panic behavior
        kaspa_core::panic::configure_panic();
    }

    pub fn new() -> Self {
        Self::init();

        Self { log_dir: None, app_dir: None, db_dir: None, network: None }
    }

    #[allow(dead_code)]
    pub fn new_with_args(args: &Args) -> Self {
        Self::init();

        let network = match (args.testnet, args.devnet, args.simnet) {
            (false, false, false) => NetworkType::Mainnet.into(),
            (true, false, false) => NetworkId::with_suffix(NetworkType::Testnet, args.testnet_suffix),
            (false, true, false) => NetworkType::Devnet.into(),
            (false, false, true) => NetworkType::Simnet.into(),
            _ => panic!("only a single net should be activated"),
        };

        let config = Arc::new(
            ConfigBuilder::new(network.into())
                .adjust_perf_params_to_consensus_params()
                .apply_args(|config| args.apply_to_config(config))
                .build(),
        );

        // Make sure config and args form a valid set of properties
        if let Err(err) = validate_config_and_args(&config, args) {
            println!("{}", err);
            exit(1);
        }

        // TODO: Refactor all this quick-and-dirty code
        let app_dir = args
            .appdir
            .clone()
            .unwrap_or_else(|| get_app_dir().as_path().to_str().unwrap().to_string())
            .replace('~', get_home_dir().as_path().to_str().unwrap());
        let app_dir = if app_dir.is_empty() { get_app_dir() } else { PathBuf::from(app_dir) };
        let db_dir = app_dir.join(config.network_name()).join(DEFAULT_DATA_DIR);

        // Logs directory is usually under the application directory, unless otherwise specified
        let log_dir = args.logdir.clone().unwrap_or_default().replace('~', get_home_dir().as_path().to_str().unwrap());
        let log_dir =
            if log_dir.is_empty() { app_dir.join(config.network_name()).join(DEFAULT_LOG_DIR) } else { PathBuf::from(log_dir) };
        let log_dir = if args.no_log_files { None } else { log_dir.to_str() };

        // Initialize the logger
        kaspa_core::log::init_logger(log_dir, &args.log_level);

        Self { app_dir: Some(app_dir), log_dir: log_dir.map(String::from), db_dir: Some(db_dir), network: Some(network) }
    }
}
