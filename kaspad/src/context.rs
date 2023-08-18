extern crate kaspa_consensus;
extern crate kaspa_core;
extern crate kaspa_hashes;

use kaspa_consensus_core::config::{Config, ConfigBuilder};
use kaspa_consensus_core::networktype::{NetworkId, NetworkType};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use crate::args::Args;
use crate::utils::*;

#[derive(Debug, Clone)]
pub struct Context {
    pub app_dir: Option<PathBuf>,
    pub db_dir: Option<PathBuf>,
    pub config: Arc<Config>,
    pub network: Option<NetworkId>,
}

impl Context {
    pub fn new_with_args(args: &Args) -> Self {
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

        Self { app_dir: Some(app_dir), db_dir: Some(db_dir), config, network: Some(network) }
    }
}
