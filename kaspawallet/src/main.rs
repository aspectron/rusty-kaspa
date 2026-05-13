//! `kaspawallet` - operator-facing CLI client for the wallet daemon.
//!
//! The binary parses operator arguments, dispatches the requested
//! subcommand, opens a gRPC connection to the wallet daemon for
//! daemon-backed subcommands (via `kaspa-wallet-grpc-client`), and
//! prints the response to stdout. All wallet state and signing for
//! the daemon-backed subcommands happens inside the daemon; this
//! client is a thin transport. Offline subcommands (`create`,
//! `dump-unencrypted-data`, `sign`, `parse`, `version`) operate
//! locally against keyfiles or hex inputs.

use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;

use crate::cli::args::{Cli, StartDaemonArgs, Subcommand};
use crate::cli::dispatch;

mod cli;
mod create;
mod keyfile;
mod keysource;
mod mass;
mod parse;
mod serialization;
mod sign;
mod sweep;
mod transactions_hex;
mod version;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Subcommand::Balance(args) => dispatch::run_balance(args),
        Subcommand::ShowAddresses(args) => dispatch::run_show_addresses(args),
        Subcommand::NewAddress(args) => dispatch::run_new_address(args),
        Subcommand::GetDaemonVersion(args) => dispatch::run_get_daemon_version(args),
        Subcommand::Parse(args) => dispatch::run_parse(args, &cli.network),
        Subcommand::DumpUnencryptedData(args) => dispatch::run_dump_unencrypted_data(args, &cli.network),
        Subcommand::Broadcast(args) => dispatch::run_broadcast(args),
        Subcommand::BroadcastReplacement(args) => dispatch::run_broadcast_replacement(args),
        Subcommand::CreateUnsignedTransaction(args) => dispatch::run_create_unsigned_transaction(args),
        Subcommand::BumpFeeUnsigned(args) => dispatch::run_bump_fee_unsigned(args),
        Subcommand::Sign(args) => dispatch::run_sign(args, &cli.network),
        Subcommand::Send(args) => dispatch::run_send(args, &cli.network),
        Subcommand::BumpFee(args) => dispatch::run_bump_fee(args, &cli.network),
        Subcommand::Sweep(args) => sweep::run_sweep(args, &cli.network),
        Subcommand::StartDaemon(args) => run_start_daemon(args),
        Subcommand::Create(args) => create::run_create(args, &cli.network),
        Subcommand::Version(_) => {
            version::print();
            ExitCode::SUCCESS
        }
    }
}

fn run_start_daemon(args: StartDaemonArgs) -> ExitCode {
    let password = match fs::read_to_string(&args.password) {
        Ok(raw) => raw.trim_end_matches(['\r', '\n']).to_owned(),
        Err(err) => {
            let _ = writeln!(io::stderr(), "read --password {}: {err}", args.password.display());
            return ExitCode::from(1);
        }
    };
    let opts = kaspa_wallet_daemon::ServeOptions {
        password,
        name: args.name,
        rpc_server: args.rpc_server,
        network_id: args.network_id,
        listen: args.listen,
        tls_cert: args.tls_cert,
        tls_key: args.tls_key,
        client_ca: args.client_ca,
        auth_token: args.auth_token,
        insecure: args.insecure,
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(err) => {
            let _ = writeln!(io::stderr(), "failed to build tokio runtime: {err}");
            return ExitCode::from(1);
        }
    };
    match runtime.block_on(kaspa_wallet_daemon::run(opts)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = writeln!(io::stderr(), "{err}");
            ExitCode::from(1)
        }
    }
}
