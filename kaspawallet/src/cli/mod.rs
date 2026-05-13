//! Command-line surface for the `kaspawallet` binary.
//!
//! `args` defines the clap-derive structs that model the
//! subcommand set and per-subcommand flag groups. `network`
//! defines the shared `--testnet` / `--simnet` / `--devnet` /
//! `--override-dag-params-file` group that flattens onto every
//! subcommand and onto the top-level CLI.

pub mod args;
pub mod dispatch;
pub mod network;

#[cfg(test)]
mod dispatch_tests;
#[cfg(test)]
mod tests;
