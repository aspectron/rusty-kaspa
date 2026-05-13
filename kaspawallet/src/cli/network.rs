//! Network-flag group exposed on every subcommand: `--testnet`,
//! `--simnet`, `--devnet`, `--override-dag-params-file`. The group
//! is embedded on every subcommand and merged with the top-level
//! value via [`NetworkFlags::combine`]. Clap models the group as a
//! flattened struct with a mutually-exclusive arg group covering
//! the three boolean flags.

use clap::Args;

/// Canonical names of the network-flag long forms; the
/// subcommand-registry tests cross-check `clap`'s registered
/// long-flag set against this list to protect against silent
/// drift.
#[cfg(test)]
pub const NETWORK_FLAG_NAMES: &[&str] = &["testnet", "simnet", "devnet", "override-dag-params-file"];

#[derive(Args, Debug, Clone, Default)]
#[command(next_help_heading = "Network options")]
pub struct NetworkFlags {
    /// Use the test network.
    #[arg(long, group = "network")]
    pub testnet: bool,

    /// Use the simulation test network.
    #[arg(long, group = "network")]
    pub simnet: bool,

    /// Use the development test network.
    #[arg(long, group = "network")]
    pub devnet: bool,

    /// Overrides DAG params (allowed only on devnet).
    #[arg(long = "override-dag-params-file", value_name = "PATH")]
    pub override_dag_params_file: Option<String>,
}

impl NetworkFlags {
    /// Combine `self` with `other`, treating any truthy boolean on
    /// either side as truthy on the result.
    pub fn combine(&mut self, other: &NetworkFlags) {
        self.testnet = self.testnet || other.testnet;
        self.simnet = self.simnet || other.simnet;
        self.devnet = self.devnet || other.devnet;
        if self.override_dag_params_file.is_none() {
            self.override_dag_params_file = other.override_dag_params_file.clone();
        }
    }

    /// Canonical kaspa address prefix used to encode addresses on
    /// the selected network.
    pub fn address_prefix(&self) -> kaspa_addresses::Prefix {
        if self.simnet {
            kaspa_addresses::Prefix::Simnet
        } else if self.devnet {
            kaspa_addresses::Prefix::Devnet
        } else if self.testnet {
            kaspa_addresses::Prefix::Testnet
        } else {
            kaspa_addresses::Prefix::Mainnet
        }
    }

    /// Canonical kaspa network-name string. Used by the keyfile
    /// default-path resolver to compose
    /// `<app-dir>/<network-name>/keys.json`. The mainnet name is
    /// `kaspa-mainnet`; the testnet-10 name is `kaspa-testnet-10`;
    /// simnet and devnet follow the same `kaspa-<network>` pattern.
    pub fn network_name(&self) -> &'static str {
        // Mutually-exclusive `clap` group means at most one of
        // simnet / devnet / testnet is set; mainnet is the default
        // when none is set. The testnet target is testnet-10.
        if self.simnet {
            "kaspa-simnet"
        } else if self.devnet {
            "kaspa-devnet"
        } else if self.testnet {
            "kaspa-testnet-10"
        } else {
            "kaspa-mainnet"
        }
    }
}
