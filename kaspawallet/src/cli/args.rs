//! Clap derive surface. Subcommand names, long flags, short
//! flags, defaults, and `required` dispositions describe the
//! observable wallet CLI.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand as ClapSubcommand};

use super::network::NetworkFlags;

/// Default daemon listen address and per-subcommand
/// `--daemonaddress` default. Loopback-only binding is the shipped
/// behaviour.
pub const DEFAULT_LISTEN: &str = "localhost:8082";

/// Default `--listen` for `start-daemon`. The daemon binds an
/// `IpAddr` literal, so the loopback default is the IP form
/// rather than the DNS-resolvable client-side default.
pub const DEFAULT_START_DAEMON_LISTEN: &str = "127.0.0.1:8082";

/// Default minimum-signatures for `create`.
pub const DEFAULT_MIN_SIGNATURES: u32 = 1;

/// Default number of private keys for `create`.
pub const DEFAULT_NUM_PRIVATE_KEYS: u32 = 1;

/// Default total number of keys for `create`.
pub const DEFAULT_NUM_PUBLIC_KEYS: u32 = 1;

/// Top-level CLI surface.
#[derive(Parser, Debug)]
#[command(name = "kaspawallet", version, about = "Subcommand-style Kaspa wallet binary.")]
#[command(disable_help_subcommand = true)]
pub struct Cli {
    /// Network flags accepted at the top level as well as on every
    /// subcommand; per-subcommand values merge with the top-level
    /// value via [`NetworkFlags::combine`].
    #[command(flatten)]
    pub network: NetworkFlags,

    #[command(subcommand)]
    pub command: Subcommand,
}

#[derive(ClapSubcommand, Debug)]
pub enum Subcommand {
    /// Create a new wallet keyfile.
    Create(CreateArgs),
    /// Print the unencrypted wallet data (mnemonic and extended
    /// keys). Use only on a trusted environment.
    DumpUnencryptedData(DumpUnencryptedDataArgs),
    /// Start the wallet daemon.
    StartDaemon(StartDaemonArgs),
    /// Show the balance held by the wallet's addresses.
    Balance(BalanceArgs),
    /// Construct, sign, and broadcast a transaction.
    Send(SendArgs),
    /// Construct an unsigned transaction.
    CreateUnsignedTransaction(CreateUnsignedTransactionArgs),
    /// Sign one or more unsigned transactions with a keyfile's
    /// private keys.
    Sign(SignArgs),
    /// Broadcast a signed transaction over the running daemon.
    Broadcast(BroadcastArgs),
    /// Parse a transaction hex and print its contents.
    Parse(ParseArgs),
    /// Show every address the daemon's wallet has generated.
    ShowAddresses(ShowAddressesArgs),
    /// Generate a new external receiving address.
    NewAddress(NewAddressArgs),
    /// Print the binary's semantic version.
    Version(VersionArgs),
    /// Sweep all funds controlled by the supplied private key
    /// into the running daemon's wallet.
    Sweep(SweepArgs),
    /// Broadcast a signed replacement transaction (RBF) over the
    /// running daemon. Same flags as `broadcast`; the daemon
    /// forwards via `kaspad`'s replacement-aware submit RPC.
    BroadcastReplacement(BroadcastArgs),
    /// Bump the fee of a pending mempool transaction. Constructs
    /// a higher-fee replacement, signs it locally with the
    /// keyfile, and broadcasts via the daemon.
    BumpFee(BumpFeeArgs),
    /// Bump the fee of a pending mempool transaction without
    /// signing. Emits the unsigned replacement transaction(s) as
    /// hex for offline signing.
    BumpFeeUnsigned(BumpFeeUnsignedArgs),
    /// Print the running daemon's reported version string.
    GetDaemonVersion(GetDaemonVersionArgs),
}

/// `create` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct CreateArgs {
    /// Keyfile location.
    #[arg(long = "keys-file", short = 'f', value_name = "PATH")]
    pub keys_file: Option<String>,

    /// Wallet password.
    #[arg(long, short = 'p', value_name = "PASSWORD")]
    pub password: Option<String>,

    /// Path to a file holding the wallet password on a single
    /// trimmed line. Preferred over `--password` because the
    /// literal string passed via `--password` is observable to
    /// other users via `ps aux` and `/proc/<pid>/cmdline`. The
    /// file must be owner-only readable on Unix (mode `0600`).
    /// When both `--password` and `--password-file` are given,
    /// `--password-file` wins and a warning is printed.
    #[arg(long = "password-file", value_name = "PATH")]
    pub password_file: Option<PathBuf>,

    /// Assume yes to all interactive prompts.
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Minimum required signatures (multisig threshold).
    #[arg(long = "min-signatures", short = 'm', default_value_t = DEFAULT_MIN_SIGNATURES)]
    pub min_signatures: u32,

    /// Number of locally-held private keys.
    #[arg(long = "num-private-keys", short = 'k', default_value_t = DEFAULT_NUM_PRIVATE_KEYS)]
    pub num_private_keys: u32,

    /// Total number of public keys (cosigners).
    #[arg(long = "num-public-keys", short = 'n', default_value_t = DEFAULT_NUM_PUBLIC_KEYS)]
    pub num_public_keys: u32,

    /// Create an ECDSA wallet instead of the default Schnorr.
    #[arg(long)]
    pub ecdsa: bool,

    /// Import existing private keys instead of generating new ones.
    #[arg(long, short = 'i')]
    pub import: bool,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `dump-unencrypted-data` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct DumpUnencryptedDataArgs {
    #[arg(long = "keys-file", short = 'f', value_name = "PATH")]
    pub keys_file: Option<String>,

    #[arg(long, short = 'p', value_name = "PASSWORD")]
    pub password: Option<String>,

    /// Path to a file holding the wallet password on a single
    /// trimmed line. Preferred over `--password` because the
    /// literal string passed via `--password` is observable to
    /// other users via `ps aux` and `/proc/<pid>/cmdline`. The
    /// file must be owner-only readable on Unix (mode `0600`).
    /// When both `--password` and `--password-file` are given,
    /// `--password-file` wins and a warning is printed.
    #[arg(long = "password-file", value_name = "PATH")]
    pub password_file: Option<PathBuf>,

    #[arg(long, short = 'y')]
    pub yes: bool,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `start-daemon` subcommand arguments.
///
/// The flag surface mirrors the wallet daemon's own CLI rather
/// than the legacy Go-wallet `kaspawallet start-daemon` flags.
/// The legacy flags (`--keys-file`, `--wait-timeout`, `--profile`)
/// have no runtime counterpart in the new daemon, which uses the
/// `kaspa-wallet-core` KV store and exposes TLS, mutual TLS, and
/// static-token authentication knobs that operators need to bind
/// the daemon safely. The daemon-client subcommands keep the
/// legacy Go-wallet flag shapes so cross-binary parity tests hold.
#[derive(clap::Args, Debug)]
pub struct StartDaemonArgs {
    /// Path to a file containing the wallet password (cleartext).
    /// The file is read once at startup; the contents decrypt the
    /// `kaspa-wallet-core` store the daemon serves.
    #[arg(long, short = 'p', value_name = "PATH", required = true)]
    pub password: PathBuf,

    /// Wallet store name. Selects the entry inside the
    /// `kaspa-wallet-core` local store when more than one exists.
    #[arg(long, short = 'n', value_name = "NAME")]
    pub name: Option<String>,

    /// Private kaspad wRPC URL. Mutually exclusive with
    /// `--network-id` at runtime; one of the two MUST be supplied.
    #[arg(long = "rpc-server", short = 's', value_name = "URL")]
    pub rpc_server: Option<String>,

    /// Network id to be connected via the Public Node Network.
    /// Required when `--rpc-server` is not supplied.
    #[arg(long = "network-id", value_name = "NETWORK_ID")]
    pub network_id: Option<String>,

    /// gRPC listen address. Default is loopback-only; widening
    /// requires `--tls-cert`/`--tls-key` or the explicit
    /// `--insecure` opt-in.
    #[arg(long = "listen", short = 'l', default_value = DEFAULT_START_DAEMON_LISTEN, value_name = "HOST:PORT")]
    pub listen: SocketAddr,

    /// Path to the PEM-encoded TLS server certificate. Requires
    /// `--tls-key`.
    #[arg(long = "tls-cert", value_name = "PATH")]
    pub tls_cert: Option<PathBuf>,

    /// Path to the PEM-encoded TLS private key. Requires
    /// `--tls-cert`.
    #[arg(long = "tls-key", value_name = "PATH")]
    pub tls_key: Option<PathBuf>,

    /// Path to a PEM-encoded CA certificate used to verify client
    /// certificates. When set the server requires mutually
    /// authenticated TLS. Requires `--tls-cert` and `--tls-key`.
    #[arg(long = "client-ca", value_name = "PATH")]
    pub client_ca: Option<PathBuf>,

    /// Path to a file containing a static API token. When set,
    /// every request must carry `authorization: Bearer <token>`
    /// metadata matching the token.
    #[arg(long = "auth-token", value_name = "PATH")]
    pub auth_token: Option<PathBuf>,

    /// Allow a non-loopback `--listen` without TLS. Off
    /// by default; required to expose the daemon to a remote host
    /// over plain gRPC.
    #[arg(long)]
    pub insecure: bool,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `balance` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct BalanceArgs {
    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    /// Verbose: show per-address balances.
    #[arg(long, short = 'v')]
    pub verbose: bool,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `send` subcommand arguments. `-v` here is `send-amount`, the
/// same short flag as `balance --verbose`; the two are on
/// different subcommands so there is no global conflict.
#[derive(clap::Args, Debug)]
pub struct SendArgs {
    #[arg(long = "keys-file", short = 'f', value_name = "PATH")]
    pub keys_file: Option<String>,

    #[arg(long, short = 'p', value_name = "PASSWORD")]
    pub password: Option<String>,

    /// Path to a file holding the wallet password on a single
    /// trimmed line. Preferred over `--password` because the
    /// literal string passed via `--password` is observable to
    /// other users via `ps aux` and `/proc/<pid>/cmdline`. The
    /// file must be owner-only readable on Unix (mode `0600`).
    /// When both `--password` and `--password-file` are given,
    /// `--password-file` wins and a warning is printed.
    #[arg(long = "password-file", value_name = "PATH")]
    pub password_file: Option<PathBuf>,

    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    /// Destination address.
    #[arg(long = "to-address", short = 't', required = true, value_name = "KASPA_ADDRESS")]
    pub to_address: String,

    /// Source address. Repeat to accept several.
    #[arg(long = "from-address", short = 'a', value_name = "KASPA_ADDRESS")]
    pub from_address: Vec<String>,

    /// Send amount in KAS (mutually exclusive with `--send-all`).
    #[arg(long = "send-amount", short = 'v', value_name = "AMOUNT_KAS")]
    pub send_amount: Option<String>,

    /// Send every available unit (mutually exclusive with
    /// `--send-amount`).
    #[arg(long = "send-all")]
    pub send_all: bool,

    /// Reuse an existing change address rather than minting a new
    /// one.
    #[arg(long = "use-existing-change-address", short = 'u')]
    pub use_existing_change_address: bool,

    /// Maximum fee rate in sompi/gram.
    #[arg(long = "max-fee-rate", short = 'm', value_name = "SOMPI_PER_GRAM")]
    pub max_fee_rate: Option<f64>,

    /// Override fee-rate estimate (sompi/gram).
    #[arg(long = "fee-rate", short = 'r', value_name = "SOMPI_PER_GRAM")]
    pub fee_rate: Option<f64>,

    /// Maximum total fee in sompi.
    #[arg(long = "max-fee", short = 'x', value_name = "SOMPI")]
    pub max_fee: Option<u64>,

    /// Show hex-encoded sent transactions.
    #[arg(long = "show-serialized", short = 's')]
    pub show_serialized: bool,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `create-unsigned-transaction` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct CreateUnsignedTransactionArgs {
    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    #[arg(long = "to-address", short = 't', required = true, value_name = "KASPA_ADDRESS")]
    pub to_address: String,

    #[arg(long = "from-address", short = 'a', value_name = "KASPA_ADDRESS")]
    pub from_address: Vec<String>,

    #[arg(long = "send-amount", short = 'v', value_name = "AMOUNT_KAS")]
    pub send_amount: Option<String>,

    #[arg(long = "send-all")]
    pub send_all: bool,

    #[arg(long = "use-existing-change-address", short = 'u')]
    pub use_existing_change_address: bool,

    #[arg(long = "max-fee-rate", short = 'm', value_name = "SOMPI_PER_GRAM")]
    pub max_fee_rate: Option<f64>,

    #[arg(long = "fee-rate", short = 'r', value_name = "SOMPI_PER_GRAM")]
    pub fee_rate: Option<f64>,

    #[arg(long = "max-fee", short = 'x', value_name = "SOMPI")]
    pub max_fee: Option<u64>,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `sign` subcommand arguments. Offline; reads the keyfile
/// directly.
#[derive(clap::Args, Debug)]
pub struct SignArgs {
    #[arg(long = "keys-file", short = 'f', value_name = "PATH")]
    pub keys_file: Option<String>,

    #[arg(long, short = 'p', value_name = "PASSWORD")]
    pub password: Option<String>,

    /// Path to a file holding the wallet password on a single
    /// trimmed line. Preferred over `--password` because the
    /// literal string passed via `--password` is observable to
    /// other users via `ps aux` and `/proc/<pid>/cmdline`. The
    /// file must be owner-only readable on Unix (mode `0600`).
    /// When both `--password` and `--password-file` are given,
    /// `--password-file` wins and a warning is printed.
    #[arg(long = "password-file", value_name = "PATH")]
    pub password_file: Option<PathBuf>,

    /// Unsigned transaction(s) as hex.
    #[arg(long, short = 't', value_name = "HEX")]
    pub transaction: Option<String>,

    /// File containing unsigned transaction(s) as hex.
    #[arg(long = "transaction-file", short = 'F', value_name = "PATH")]
    pub transaction_file: Option<String>,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `broadcast` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct BroadcastArgs {
    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    #[arg(long, short = 't', value_name = "HEX")]
    pub transaction: Option<String>,

    #[arg(long = "transaction-file", short = 'F', value_name = "PATH")]
    pub transaction_file: Option<String>,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `parse` subcommand arguments. Offline; the `--keys-file` arg
/// is optional and enables address-ownership annotation.
#[derive(clap::Args, Debug)]
pub struct ParseArgs {
    #[arg(long = "keys-file", short = 'f', value_name = "PATH")]
    pub keys_file: Option<String>,

    #[arg(long, short = 't', value_name = "HEX")]
    pub transaction: Option<String>,

    #[arg(long = "transaction-file", short = 'F', value_name = "PATH")]
    pub transaction_file: Option<String>,

    /// Verbose: show transaction inputs.
    #[arg(long, short = 'v')]
    pub verbose: bool,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `show-addresses` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct ShowAddressesArgs {
    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `new-address` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct NewAddressArgs {
    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `version` subcommand arguments (no flags).
#[derive(clap::Args, Debug)]
pub struct VersionArgs {}

/// `sweep` subcommand arguments.
#[derive(clap::Args, Debug)]
pub struct SweepArgs {
    /// Hex-encoded private key.
    #[arg(long = "private-key", short = 'k', value_name = "HEX")]
    pub private_key: Option<String>,

    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `bump-fee` subcommand arguments. Local-signing flow: the
/// daemon returns unsigned replacement transactions, the CLI
/// signs them client-side, then broadcasts.
#[derive(clap::Args, Debug)]
pub struct BumpFeeArgs {
    /// Transaction ID of the pending mempool entry to replace.
    #[arg(long = "txid", short = 'i', value_name = "TXID")]
    pub txid: Option<String>,

    #[arg(long = "keys-file", short = 'f', value_name = "PATH")]
    pub keys_file: Option<String>,

    #[arg(long, short = 'p', value_name = "PASSWORD")]
    pub password: Option<String>,

    /// Path to a file holding the wallet password on a single
    /// trimmed line. Preferred over `--password` because the
    /// literal string passed via `--password` is observable to
    /// other users via `ps aux` and `/proc/<pid>/cmdline`. The
    /// file must be owner-only readable on Unix (mode `0600`).
    /// When both `--password` and `--password-file` are given,
    /// `--password-file` wins and a warning is printed.
    #[arg(long = "password-file", value_name = "PATH")]
    pub password_file: Option<PathBuf>,

    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    /// Restrict input selection to these source addresses. Repeat
    /// to accept several.
    #[arg(long = "from-address", short = 'a', value_name = "KASPA_ADDRESS")]
    pub from_address: Vec<String>,

    #[arg(long = "use-existing-change-address", short = 'u')]
    pub use_existing_change_address: bool,

    #[arg(long = "max-fee-rate", short = 'm', value_name = "SOMPI_PER_GRAM")]
    pub max_fee_rate: Option<f64>,

    #[arg(long = "fee-rate", short = 'r', value_name = "SOMPI_PER_GRAM")]
    pub fee_rate: Option<f64>,

    #[arg(long = "max-fee", short = 'x', value_name = "SOMPI")]
    pub max_fee: Option<u64>,

    /// Show hex-encoded signed replacement transactions after
    /// broadcast.
    #[arg(long = "show-serialized", short = 's')]
    pub show_serialized: bool,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `bump-fee-unsigned` subcommand arguments. Returns the daemon's
/// unsigned replacement transaction(s) as hex; no keyfile is
/// read.
#[derive(clap::Args, Debug)]
pub struct BumpFeeUnsignedArgs {
    /// Transaction ID of the pending mempool entry to replace.
    #[arg(long = "txid", short = 'i', value_name = "TXID")]
    pub txid: Option<String>,

    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,

    #[arg(long = "from-address", short = 'a', value_name = "KASPA_ADDRESS")]
    pub from_address: Vec<String>,

    #[arg(long = "use-existing-change-address", short = 'u')]
    pub use_existing_change_address: bool,

    #[arg(long = "max-fee-rate", short = 'm', value_name = "SOMPI_PER_GRAM")]
    pub max_fee_rate: Option<f64>,

    #[arg(long = "fee-rate", short = 'r', value_name = "SOMPI_PER_GRAM")]
    pub fee_rate: Option<f64>,

    #[arg(long = "max-fee", short = 'x', value_name = "SOMPI")]
    pub max_fee: Option<u64>,

    #[command(flatten)]
    pub network: NetworkFlags,
}

/// `get-daemon-version` subcommand arguments. Only
/// `--daemonaddress`; no network flags (the daemon's reported
/// version is network-agnostic).
#[derive(clap::Args, Debug)]
pub struct GetDaemonVersionArgs {
    #[arg(long = "daemonaddress", short = 'd', default_value = DEFAULT_LISTEN, value_name = "HOST:PORT")]
    pub daemon_address: String,
}
