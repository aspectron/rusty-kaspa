//! Subcommand dispatchers.
//!
//! Three subcommand families share this module:
//!
//! - **Daemon-client subcommands** (`balance`, `show-addresses`,
//!   `new-address`, `get-daemon-version`) open a fresh gRPC
//!   connection to the wallet daemon at `--daemonaddress`, issue
//!   one RPC, print the response to stdout, and return an
//!   [`ExitCode`]. The dispatcher never retains wallet secrets;
//!   daemon-side state owns the signing material.
//! - **Offline subcommands** (`parse`, `dump-unencrypted-data`,
//!   `sign`) read the operator's encrypted keyfile directly and
//!   never dial the daemon. Used for inspecting hex-encoded
//!   partially-signed transactions, recovering the unencrypted
//!   mnemonic + extended public keys on a trusted host, and
//!   producing signatures locally with the keyfile's signing
//!   material (Schnorr or ECDSA, dispatched per the keyfile's
//!   `ecdsa` flag).
//! - **Daemon-relay tx-shape subcommands** (`broadcast`,
//!   `broadcast-replacement`, `create-unsigned-transaction`,
//!   `bump-fee-unsigned`) construct a transaction-shape RPC
//!   (Broadcast / BroadcastReplacement / CreateUnsignedTransactions
//!   / BumpFee with empty password), forward to the daemon, and
//!   render the response. No keyfile is read and no local signing
//!   happens: the daemon either submits already-signed bytes or
//!   returns unsigned PSI bytes for downstream offline signing.
//! - **Online subcommands** (`send`, `bump-fee`) combine a daemon
//!   round-trip with local signing: the daemon constructs the
//!   unsigned transaction(s), the dispatcher decrypts the
//!   operator's keyfile and signs the returned PSTX bytes with the
//!   same curve-dispatched helper [`sign_partially_signed_transactions`]
//!   the offline `sign` subcommand uses, then streams the signed
//!   bytes back to the daemon in [`BROADCAST_CHUNK_SIZE`]-bounded
//!   chunks. Multisig keyfiles where the operator does not hold
//!   every cosigner's mnemonic are refused outright -- the
//!   threshold cannot be reached without external coordination.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use kaspa_bip32::{DerivationPath, ExtendedPrivateKey, ExtendedPublicKey, Language, Mnemonic, Prefix as Bip32Prefix, SecretKey};
use kaspa_consensus_core::network::NetworkType;
use kaspa_wallet_grpc_client::{
    ClientOptions, connect,
    kaspawalletd::{
        BroadcastRequest, BumpFeeRequest, CreateUnsignedTransactionsRequest, FeePolicy, GetBalanceRequest, GetVersionRequest,
        NewAddressRequest, ShowAddressesRequest, fee_policy,
    },
};
use zeroize::Zeroizing;

use crate::cli::args::{
    BalanceArgs, BroadcastArgs, BumpFeeArgs, BumpFeeUnsignedArgs, CreateUnsignedTransactionArgs, DumpUnencryptedDataArgs,
    GetDaemonVersionArgs, NewAddressArgs, ParseArgs, SendArgs, ShowAddressesArgs, SignArgs,
};
use crate::cli::network::NetworkFlags;
use crate::keyfile;
use crate::keysource::require_existing_keyfile;
use crate::parse::{ParseInput, parse};
use crate::serialization::{deserialize_partially_signed_transaction, serialize_partially_signed_transaction};
use crate::sign::{is_pst_fully_signed, sign_pst_ecdsa_with_mnemonic, sign_pst_schnorr_with_mnemonic};
use crate::transactions_hex::{decode_transactions_from_hex, encode_transactions_to_hex};

/// BIP-43 purpose component for single-signer wallets.
const SINGLE_SIGNER_PURPOSE: u32 = 44;

/// BIP-43-style purpose component for multisig wallets.
const MULTISIG_PURPOSE: u32 = 45;

/// Kaspa SLIP-0044 coin type.
const COIN_TYPE: u32 = 111111;

/// Sompi-per-kaspa multiplier.
pub const SOMPI_PER_KASPA: u64 = 100_000_000;

/// Format an amount in sompi as 8-decimal KAS, fixed 19-char
/// width, space-padded for zero amounts.
pub fn format_kas(amount_sompi: u64) -> String {
    if amount_sompi == 0 {
        return " ".repeat(19);
    }
    let kas = amount_sompi as f64 / SOMPI_PER_KASPA as f64;
    format!("{kas:19.8}")
}

pub(crate) fn fail(msg: impl AsRef<str>) -> ExitCode {
    let _ = writeln!(io::stderr(), "{}", msg.as_ref());
    ExitCode::from(1)
}

pub(crate) fn build_runtime() -> Result<tokio::runtime::Runtime, ExitCode> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| fail(format!("failed to build tokio runtime: {e}")))
}

pub(crate) fn endpoint_url(daemon_address: &str) -> String {
    if daemon_address.starts_with("http://") || daemon_address.starts_with("https://") {
        daemon_address.to_owned()
    } else {
        format!("http://{daemon_address}")
    }
}

pub fn run_balance(args: BalanceArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.get_balance(GetBalanceRequest {}).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("GetBalance failed: {s}")),
        };
        let pending_suffix = if !args.verbose && resp.pending > 0 { " (pending)" } else { "" };
        if args.verbose {
            println!("Address                                                                       Available             Pending");
            println!("-----------------------------------------------------------------------------------------------------------");
            for entry in &resp.address_balances {
                println!("{} {} {}", entry.address, format_kas(entry.available), format_kas(entry.pending));
            }
            println!("-----------------------------------------------------------------------------------------------------------");
            print!("                                                 ");
        }
        println!("Total balance, KAS {} {}{}", format_kas(resp.available), format_kas(resp.pending), pending_suffix);
        ExitCode::SUCCESS
    })
}

pub fn run_show_addresses(args: ShowAddressesArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.show_addresses(ShowAddressesRequest {}).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("ShowAddresses failed: {s}")),
        };
        println!("Addresses ({}):", resp.address.len());
        for addr in &resp.address {
            println!("{addr}");
        }
        println!(
            "\nNote: the above are only addresses that were manually created by the 'new-address' command. \
If you want to see a list of all addresses, including change addresses, that have a positive balance, use the command 'balance -v'"
        );
        ExitCode::SUCCESS
    })
}

pub fn run_new_address(args: NewAddressArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.new_address(NewAddressRequest {}).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("NewAddress failed: {s}")),
        };
        println!("New address:\n{}", resp.address);
        ExitCode::SUCCESS
    })
}

pub fn run_get_daemon_version(args: GetDaemonVersionArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.get_version(GetVersionRequest {}).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("GetVersion failed: {s}")),
        };
        println!("{}", resp.version);
        ExitCode::SUCCESS
    })
}

// ---- offline subcommands ----------------------------------------

pub(crate) fn merge_network(top: &NetworkFlags, sub: &NetworkFlags) -> NetworkFlags {
    let mut merged = top.clone();
    merged.combine(sub);
    merged
}

pub fn run_parse(args: ParseArgs, top: &NetworkFlags) -> ExitCode {
    let network = merge_network(top, &args.network);

    // Resolve the keyfile path: operator-supplied `--keys-file`
    // override if present, otherwise the platform-aware default
    // (`<app-dir>/<network>/keys.json`). The keyfile is always
    // read; failure at the resolved path exits 1 with a
    // structured error.
    let override_path = args.keys_file.as_deref().map(Path::new);
    let keysfile_path = match require_existing_keyfile(override_path, network.network_name()) {
        Ok(p) => p,
        Err(err) => return fail(format!("{err}")),
    };
    let keysfile = match keyfile::read_from_path(&keysfile_path) {
        Ok(kf) => kf,
        Err(err) => return fail(format!("{err}")),
    };

    let input = ParseInput {
        transaction: args.transaction.as_deref(),
        transaction_file: args.transaction_file.as_deref(),
        verbose: args.verbose,
        network: &network,
        keysfile: Some(&keysfile),
    };

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    match parse(&input, &mut handle) {
        Ok(_) => {
            let _ = handle.flush();
            ExitCode::SUCCESS
        }
        Err(err) => fail(format!("{err}")),
    }
}

/// Read a password from a file, after enforcing owner-only
/// permissions on Unix. Trailing newline / carriage-return bytes
/// are stripped; the remaining content is wrapped in a zeroizing
/// container so the secret is wiped on drop.
pub(crate) fn read_password_file(path: &Path) -> Result<Zeroizing<String>, ExitCode> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path).map_err(|e| fail(format!("stat --password-file {}: {e}", path.display())))?;
        let mode = metadata.permissions().mode();
        if mode & 0o077 != 0 {
            return Err(fail(format!(
                "--password-file {} is group/world readable (mode {:#o}); restrict to owner-only (e.g. chmod 600)",
                path.display(),
                mode & 0o777,
            )));
        }
    }
    let raw = fs::read_to_string(path).map_err(|e| fail(format!("read --password-file {}: {e}", path.display())))?;
    Ok(Zeroizing::new(raw.trim_end_matches(['\r', '\n']).to_owned()))
}

/// Resolve a password supplied as either a literal string or a
/// path-to-file. The file form is preferred when both are
/// present (and a warning is printed). Returns `None` when
/// neither flag is set so callers can prompt interactively.
fn resolve_password_flag(password: &Option<String>, password_file: &Option<PathBuf>) -> Result<Option<Zeroizing<String>>, ExitCode> {
    if let Some(path) = password_file.as_deref() {
        if password.as_deref().map(str::is_empty) == Some(false) {
            let _ = writeln!(io::stderr(), "warning: --password and --password-file both set; using --password-file");
        }
        return Ok(Some(read_password_file(path)?));
    }
    if let Some(p) = password.as_ref()
        && !p.is_empty()
    {
        return Ok(Some(Zeroizing::new(p.clone())));
    }
    Ok(None)
}

/// Resolve the operator's keyfile password.
///
/// Order of preference:
/// 1. `--password-file` if set (file content, trimmed).
/// 2. `--password` on the command line, if non-empty.
/// 3. An interactive no-echo prompt when stdin is a terminal.
/// 4. Otherwise a hard error, since reading the secret blindly
///    from a redirected stream would expose it to whatever
///    process produced the input.
pub(crate) fn require_password(
    password: &Option<String>,
    password_file: &Option<PathBuf>,
    subcommand: &str,
) -> Result<Zeroizing<String>, ExitCode> {
    if let Some(pw) = resolve_password_flag(password, password_file)? {
        return Ok(pw);
    }
    if !io::stdin().is_terminal() {
        return Err(fail(format!(
            "'{subcommand}' requires --password or --password-file (or run with stdin attached to a terminal)",
        )));
    }
    prompt_password_no_echo("Enter password for the key file: ")
}

/// Print the prompt to stderr and read a single line from the
/// controlling terminal with echo disabled. Returns an
/// already-zeroizing wrapper so the secret is wiped from memory
/// when the caller drops it.
fn prompt_password_no_echo(prompt: &str) -> Result<Zeroizing<String>, ExitCode> {
    eprint!("{prompt}");
    let _ = io::stderr().flush();
    match rpassword::read_password() {
        Ok(pw) => Ok(Zeroizing::new(pw)),
        Err(err) => {
            // rpassword echoes nothing on Ctrl-D, so emit a newline
            // before the error so the next shell prompt does not
            // sit on the same line as the password prompt.
            eprintln!();
            Err(fail(format!("read password: {err}")))
        }
    }
}

/// Read and confirm a fresh password. Used by the wallet-creation
/// flow where the operator is choosing the keyfile's encryption
/// secret for the first time; a typo would lock the keyfile, so
/// the prompt is repeated and the two reads must match exactly.
pub(crate) fn require_password_with_confirmation(
    password: &Option<String>,
    password_file: &Option<PathBuf>,
    subcommand: &str,
) -> Result<Zeroizing<String>, ExitCode> {
    if let Some(pw) = resolve_password_flag(password, password_file)? {
        return Ok(pw);
    }
    if !io::stdin().is_terminal() {
        return Err(fail(format!(
            "'{subcommand}' requires --password or --password-file (or run with stdin attached to a terminal)",
        )));
    }
    let first = prompt_password_no_echo("Enter password for the key file: ")?;
    let second = prompt_password_no_echo("Confirm password: ")?;
    if first.as_bytes() != second.as_bytes() {
        return Err(fail("passwords do not match"));
    }
    Ok(first)
}

/// Map the network to the bip32 extended-pubkey prefix the
/// keyfile format expects: `kpub` for mainnet, `ktub` for every
/// test network.
pub(crate) fn xpub_prefix(network: &NetworkFlags) -> Bip32Prefix {
    match network_type(network) {
        NetworkType::Mainnet => Bip32Prefix::KPUB,
        NetworkType::Testnet | NetworkType::Simnet | NetworkType::Devnet => Bip32Prefix::KTUB,
    }
}

pub(crate) fn network_type(network: &NetworkFlags) -> NetworkType {
    if network.simnet {
        NetworkType::Simnet
    } else if network.devnet {
        NetworkType::Devnet
    } else if network.testnet {
        NetworkType::Testnet
    } else {
        NetworkType::Mainnet
    }
}

/// Derive the cosigner-level extended public key from a BIP-39
/// mnemonic phrase, encoded with the requested kaspa
/// extended-pubkey prefix (`kpub` for mainnet, `ktub` for any
/// test network). Single-signer wallets use BIP-44 purpose 44;
/// multisig wallets use purpose 45 per the kaspa keyfile format.
/// The coin type is SLIP-0044 entry 111111.
pub(crate) fn master_xpub_from_mnemonic(mnemonic_phrase: &str, is_multisig: bool, prefix: Bip32Prefix) -> Result<String, String> {
    let mnemonic = Mnemonic::new(mnemonic_phrase, Language::English).map_err(|e| format!("invalid mnemonic: {e}"))?;
    let seed = mnemonic.to_seed("");
    let master = ExtendedPrivateKey::<SecretKey>::new(seed.as_bytes()).map_err(|e| format!("master xpriv derivation: {e}"))?;
    let purpose = if is_multisig { MULTISIG_PURPOSE } else { SINGLE_SIGNER_PURPOSE };
    let path = DerivationPath::from_str(&format!("m/{purpose}'/{COIN_TYPE}'/0'")).map_err(|e| format!("derivation path: {e}"))?;
    let cosigner = master.derive_path(&path).map_err(|e| format!("cosigner derivation: {e}"))?;
    let xpub: ExtendedPublicKey<secp256k1::PublicKey> = (&cosigner).into();
    Ok(xpub.to_string(Some(prefix)))
}

pub fn run_dump_unencrypted_data(args: DumpUnencryptedDataArgs, top: &NetworkFlags) -> ExitCode {
    if !args.yes {
        return fail("'dump-unencrypted-data' requires --yes to confirm");
    }
    let password = match require_password(&args.password, &args.password_file,"dump-unencrypted-data") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let network = merge_network(top, &args.network);
    let override_path = args.keys_file.as_deref().map(Path::new);
    let keysfile_path = match require_existing_keyfile(override_path, network.network_name()) {
        Ok(p) => p,
        Err(err) => return fail(format!("{err}")),
    };
    let kf = match keyfile::read_from_path(&keysfile_path) {
        Ok(kf) => kf,
        Err(err) => return fail(format!("{err}")),
    };
    let mnemonics = match keyfile::decrypt::decrypt_mnemonics(&kf, password.as_bytes()) {
        Ok(m) => m,
        Err(e) => return fail(format!("keyfile decryption failed: {e}")),
    };
    let is_multisig = kf.extended_public_keys.len() > 1;
    let prefix = xpub_prefix(&network);
    let mut mnemonic_xpubs: Vec<String> = Vec::with_capacity(mnemonics.len());
    for (i, m) in mnemonics.iter().enumerate() {
        println!("Mnemonic #{}:\n{m}\n", i + 1);
        match master_xpub_from_mnemonic(m, is_multisig, prefix) {
            Ok(x) => mnemonic_xpubs.push(x),
            Err(e) => return fail(format!("xpub derivation: {e}")),
        }
    }
    let mut i = 1;
    for xpub in &kf.extended_public_keys {
        if mnemonic_xpubs.iter().any(|own| own == xpub) {
            continue;
        }
        println!("Extended Public key #{i}:\n{xpub}\n");
        i += 1;
    }
    println!("Minimum number of signatures: {}", kf.minimum_signatures);
    ExitCode::SUCCESS
}

/// Resolve hex-encoded transaction(s) from either the `--transaction`
/// literal or `--transaction-file` path. Exactly one of the two must
/// be supplied; supplying both is rejected so the operator's intent
/// stays unambiguous. Leading and trailing whitespace is stripped from
/// both sources so a file written with a trailing newline (the
/// canonical shell-redirect form `... > tx.hex`) is accepted verbatim
/// without a hex-decoder "Odd number of digits" stumble on the final
/// `\n`.
fn resolve_transaction_hex(transaction: Option<&str>, transaction_file: Option<&str>, subcommand: &str) -> Result<String, ExitCode> {
    match (transaction, transaction_file) {
        (None, None) => Err(fail(format!("'{subcommand}' requires --transaction or --transaction-file"))),
        (Some(_), Some(_)) => Err(fail(format!("'{subcommand}': --transaction and --transaction-file are mutually exclusive"))),
        (Some(literal), None) => Ok(literal.trim().to_owned()),
        (None, Some(path)) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|err| fail(format!("'{subcommand}': read --transaction-file '{path}': {err}")))?;
            Ok(raw.trim().to_owned())
        }
    }
}

/// Sign every partially-signed transaction in `serialized_inputs`
/// once per mnemonic. Returns the freshly-serialized post-sign
/// bytes plus a flag that is `true` when every transaction has
/// reached its `minimum_signatures` threshold. The signing curve
/// dispatches off the keyfile's `ecdsa` flag: ECDSA keyfiles use
/// `sign_pst_ecdsa_with_mnemonic`, every other keyfile uses
/// `sign_pst_schnorr_with_mnemonic`.
pub(crate) fn sign_partially_signed_transactions(
    kf: &keyfile::KeysFile,
    mnemonics: &[String],
    serialized_inputs: &[Vec<u8>],
) -> Result<(Vec<Vec<u8>>, bool), String> {
    let mut updated: Vec<Vec<u8>> = Vec::with_capacity(serialized_inputs.len());
    let mut all_fully_signed = true;
    for bytes in serialized_inputs {
        let mut pst = deserialize_partially_signed_transaction(bytes).map_err(|e| format!("PSTX deserialization failed: {e}"))?;
        for mnemonic in mnemonics {
            let res = if kf.ecdsa {
                sign_pst_ecdsa_with_mnemonic(&mut pst, mnemonic, "")
            } else {
                sign_pst_schnorr_with_mnemonic(&mut pst, mnemonic, "")
            };
            res.map_err(|e| format!("sign failed: {e}"))?;
        }
        if !is_pst_fully_signed(&pst) {
            all_fully_signed = false;
        }
        let out = serialize_partially_signed_transaction(&pst).map_err(|e| format!("PSTX serialization failed: {e}"))?;
        updated.push(out);
    }
    Ok((updated, all_fully_signed))
}

pub fn run_sign(args: SignArgs, top: &NetworkFlags) -> ExitCode {
    let tx_hex = match resolve_transaction_hex(args.transaction.as_deref(), args.transaction_file.as_deref(), "sign") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let password = match require_password(&args.password, &args.password_file,"sign") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let network = merge_network(top, &args.network);
    let override_path = args.keys_file.as_deref().map(Path::new);
    let keysfile_path = match require_existing_keyfile(override_path, network.network_name()) {
        Ok(p) => p,
        Err(err) => return fail(format!("{err}")),
    };
    let kf = match keyfile::read_from_path(&keysfile_path) {
        Ok(kf) => kf,
        Err(err) => return fail(format!("{err}")),
    };
    let mnemonics = match keyfile::decrypt::decrypt_mnemonics(&kf, password.as_bytes()) {
        Ok(m) => m,
        Err(e) => return fail(format!("keyfile decryption failed: {e}")),
    };
    let partially_signed = match decode_transactions_from_hex(&tx_hex) {
        Ok(t) => t,
        Err(e) => return fail(format!("'sign': invalid hex: {e}")),
    };
    let (updated, all_fully_signed) = match sign_partially_signed_transactions(&kf, &mnemonics, &partially_signed) {
        Ok(out) => out,
        Err(e) => return fail(e),
    };
    if all_fully_signed {
        eprintln!("The transaction is signed and ready to broadcast");
    } else {
        eprintln!("Successfully signed transaction");
    }
    println!("{}", encode_transactions_to_hex(&updated));
    ExitCode::SUCCESS
}

// ---- daemon-relay tx-shape subcommands --------------------------

/// Decimal-KAS string -> sompi `u64`. Empty / whitespace-only
/// input is rejected (matches the legacy CLI semantics, where
/// `KasToSompi("")` failed with `strconv.ParseFloat: parsing "":
/// invalid syntax`).
fn parse_kaspa_amount_to_sompi(amount: &str) -> Result<u64, String> {
    let trimmed = amount.trim();
    if trimmed.is_empty() {
        return Err("send amount must not be empty".to_owned());
    }
    let kas: f64 = trimmed.parse().map_err(|err| format!("invalid send amount '{trimmed}': {err}"))?;
    if !kas.is_finite() {
        return Err(format!("invalid send amount '{trimmed}': not finite"));
    }
    if kas < 0.0 {
        return Err(format!("invalid send amount '{trimmed}': negative"));
    }
    Ok((kas * SOMPI_PER_KASPA as f64) as u64)
}

/// Build the proto fee-policy oneof from the three operator
/// flags. Precedence matches the legacy CLI's if/elseif chain:
/// `--fee-rate` (exact rate) wins over `--max-fee-rate` (cap on
/// rate) wins over `--max-fee` (cap on absolute fee). If none of
/// the three is set, the daemon falls back to its built-in
/// estimator.
fn build_fee_policy(fee_rate: Option<f64>, max_fee_rate: Option<f64>, max_fee: Option<u64>) -> Option<FeePolicy> {
    let inner = if let Some(rate) = fee_rate.filter(|r| *r > 0.0) {
        fee_policy::FeePolicy::ExactFeeRate(rate)
    } else if let Some(max) = max_fee_rate.filter(|r| *r > 0.0) {
        fee_policy::FeePolicy::MaxFeeRate(max)
    } else if let Some(cap) = max_fee.filter(|c| *c > 0) {
        fee_policy::FeePolicy::MaxFee(cap)
    } else {
        return None;
    };
    Some(FeePolicy { fee_policy: Some(inner) })
}

/// Either-or hex source: at least one of `--transaction` /
/// `--transaction-file` is required, both at once is rejected.
/// File contents are stripped of leading and trailing whitespace
/// so a file written with a trailing newline is accepted
/// verbatim.
fn read_tx_hex(transaction: Option<&str>, transaction_file: Option<&str>) -> Result<String, String> {
    match (transaction, transaction_file) {
        (None, None) => Err("Either --transaction or --transaction-file is required".to_owned()),
        (Some(_), Some(_)) => Err("Both --transaction and --transaction-file cannot be passed at the same time".to_owned()),
        (Some(hex), None) => Ok(hex.trim().to_owned()),
        (None, Some(path)) => {
            let raw = std::fs::read_to_string(path).map_err(|err| format!("Could not read hex from {path}: {err}"))?;
            Ok(raw.trim().to_owned())
        }
    }
}

fn print_broadcast_success(tx_ids: &[String]) {
    println!("Transactions were sent successfully");
    println!("Transaction ID(s): ");
    for tx_id in tx_ids {
        println!("\t{tx_id}");
    }
}

pub fn run_broadcast(args: BroadcastArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let hex = match read_tx_hex(args.transaction.as_deref(), args.transaction_file.as_deref()) {
            Ok(h) => h,
            Err(msg) => return fail(msg),
        };
        let transactions = match decode_transactions_from_hex(&hex) {
            Ok(t) => t,
            Err(err) => return fail(format!("decode transactions: {err}")),
        };
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.broadcast(BroadcastRequest { is_domain: false, transactions }).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("Broadcast failed: {s}")),
        };
        print_broadcast_success(&resp.tx_ids);
        ExitCode::SUCCESS
    })
}

pub fn run_broadcast_replacement(args: BroadcastArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let hex = match read_tx_hex(args.transaction.as_deref(), args.transaction_file.as_deref()) {
            Ok(h) => h,
            Err(msg) => return fail(msg),
        };
        let transactions = match decode_transactions_from_hex(&hex) {
            Ok(t) => t,
            Err(err) => return fail(format!("decode transactions: {err}")),
        };
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.broadcast_replacement(BroadcastRequest { is_domain: false, transactions }).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("BroadcastReplacement failed: {s}")),
        };
        print_broadcast_success(&resp.tx_ids);
        ExitCode::SUCCESS
    })
}

pub fn run_create_unsigned_transaction(args: CreateUnsignedTransactionArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let amount_sompi = if args.send_all {
            0
        } else {
            match args.send_amount.as_deref() {
                Some(s) => match parse_kaspa_amount_to_sompi(s) {
                    Ok(v) => v,
                    Err(msg) => return fail(msg),
                },
                None => return fail("Either --send-amount or --send-all is required"),
            }
        };
        let fee_policy = build_fee_policy(args.fee_rate, args.max_fee_rate, args.max_fee);
        let request = CreateUnsignedTransactionsRequest {
            address: args.to_address,
            amount: amount_sompi,
            from: args.from_address,
            use_existing_change_address: args.use_existing_change_address,
            is_send_all: args.send_all,
            fee_policy,
        };
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.create_unsigned_transactions(request).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("CreateUnsignedTransactions failed: {s}")),
        };
        let _ = writeln!(io::stderr(), "Created unsigned transaction");
        println!("{}", encode_transactions_to_hex(&resp.unsigned_transactions));
        ExitCode::SUCCESS
    })
}

// ---- online subcommands (daemon RPC + local signing) -----------

/// Maximum number of transactions per Broadcast /
/// BroadcastReplacement RPC. Bounds the wire-message size below
/// the gRPC max-message limit when an operator sweeps a large
/// UTXO set.
pub const BROADCAST_CHUNK_SIZE: usize = 100;

/// Refuse a `send` / `bump-fee` operation when the operator's
/// keyfile holds more cosigner public keys than encrypted
/// mnemonics: at least one cosigner's signing material is absent
/// and the threshold cannot be reached locally without external
/// coordination, which neither subcommand supports.
pub(crate) fn check_keyfile_holds_every_signer(kf: &keyfile::KeysFile, subcommand: &str) -> Result<(), String> {
    if kf.extended_public_keys.len() > kf.encrypted_mnemonics.len() {
        return Err(format!("Cannot use '{subcommand}' command for multisig wallet without all of the keys"));
    }
    Ok(())
}

/// Operator-visible header line for one chunk of the chunked-
/// broadcast progress block. The width-pinned `{:.2}` percentage
/// matches the legacy CLI's `%.2f` formatter byte-for-byte.
fn format_chunk_progress_header(chunk_len: usize, broadcasted_so_far: usize, total: usize) -> String {
    let pct = 100.0_f64 * broadcasted_so_far as f64 / total as f64;
    format!("Broadcasted {chunk_len} transaction(s) (broadcasted {pct:.2}% of the transactions so far)")
}

fn print_broadcast_chunk_progress(chunk_len: usize, broadcasted_so_far: usize, total: usize, tx_ids: &[String]) {
    println!("{}", format_chunk_progress_header(chunk_len, broadcasted_so_far, total));
    println!("Broadcasted Transaction ID(s): ");
    for tx_id in tx_ids {
        println!("\t{tx_id}");
    }
}

/// Operator-visible `--show-serialized` block. Each transaction
/// renders as `\t<lowercase-hex>\n` followed by a blank line,
/// matching the legacy CLI shape so the output can be sliced
/// apart and round-tripped through `parse` or resent via
/// `broadcast`.
fn format_show_serialized_block(signed_transactions: &[Vec<u8>]) -> String {
    let mut out = String::from("Serialized Transaction(s) (can be parsed via the `parse` command or resent via `broadcast`): \n");
    for signed_tx in signed_transactions {
        out.push('\t');
        out.push_str(&hex::encode(signed_tx));
        out.push('\n');
        out.push('\n');
    }
    out
}

fn print_show_serialized(signed_transactions: &[Vec<u8>]) {
    print!("{}", format_show_serialized_block(signed_transactions));
}

pub fn run_send(args: SendArgs, top: &NetworkFlags) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        // Amount selection: `--send-all` short-circuits the
        // decimal-amount parse; otherwise `--send-amount` is
        // required and parsed into sompi. Matches the legacy
        // CLI's "skip parse when is_send_all" contract: when
        // both flags happen to be supplied the all-funds path
        // wins, identical to the reference binary.
        let send_amount_sompi: u64 = if args.send_all {
            0
        } else {
            match args.send_amount.as_deref() {
                Some(s) => match parse_kaspa_amount_to_sompi(s) {
                    Ok(v) => v,
                    Err(msg) => return fail(msg),
                },
                None => return fail("'send' requires either --send-amount or --send-all"),
            }
        };

        let password = match require_password(&args.password, &args.password_file,"send") {
            Ok(p) => p,
            Err(e) => return e,
        };

        let network = merge_network(top, &args.network);
        let override_path = args.keys_file.as_deref().map(Path::new);
        let keysfile_path = match require_existing_keyfile(override_path, network.network_name()) {
            Ok(p) => p,
            Err(err) => return fail(format!("{err}")),
        };
        let kf = match keyfile::read_from_path(&keysfile_path) {
            Ok(kf) => kf,
            Err(err) => return fail(format!("{err}")),
        };
        if let Err(msg) = check_keyfile_holds_every_signer(&kf, "send") {
            return fail(msg);
        }

        let fee_policy = build_fee_policy(args.fee_rate, args.max_fee_rate, args.max_fee);
        let request = CreateUnsignedTransactionsRequest {
            address: args.to_address,
            amount: send_amount_sompi,
            from: args.from_address,
            use_existing_change_address: args.use_existing_change_address,
            is_send_all: args.send_all,
            fee_policy,
        };

        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.create_unsigned_transactions(request).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("CreateUnsignedTransactions failed: {s}")),
        };

        let mnemonics = match keyfile::decrypt::decrypt_mnemonics(&kf, password.as_bytes()) {
            Ok(m) => m,
            Err(e) => return fail(format!("keyfile decryption failed: {e}")),
        };
        let (signed_transactions, _all_fully_signed) =
            match sign_partially_signed_transactions(&kf, &mnemonics, &resp.unsigned_transactions) {
                Ok(out) => out,
                Err(e) => return fail(e),
            };

        println!("Broadcasting {} transaction(s)", signed_transactions.len());
        let total = signed_transactions.len();
        let mut broadcasted_so_far = 0usize;
        for chunk in signed_transactions.chunks(BROADCAST_CHUNK_SIZE) {
            let request = BroadcastRequest { is_domain: false, transactions: chunk.to_vec() };
            let resp = match client.broadcast(request).await {
                Ok(r) => r.into_inner(),
                Err(s) => return fail(format!("Broadcast failed: {s}")),
            };
            broadcasted_so_far += chunk.len();
            print_broadcast_chunk_progress(chunk.len(), broadcasted_so_far, total, &resp.tx_ids);
        }
        if args.show_serialized {
            print_show_serialized(&signed_transactions);
        }
        ExitCode::SUCCESS
    })
}

pub fn run_bump_fee(args: BumpFeeArgs, top: &NetworkFlags) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let tx_id = match args.txid.as_deref() {
            Some(t) if !t.is_empty() => t.to_owned(),
            _ => return fail("'bump-fee' requires --txid"),
        };

        let password = match require_password(&args.password, &args.password_file,"bump-fee") {
            Ok(p) => p,
            Err(e) => return e,
        };

        let network = merge_network(top, &args.network);
        let override_path = args.keys_file.as_deref().map(Path::new);
        let keysfile_path = match require_existing_keyfile(override_path, network.network_name()) {
            Ok(p) => p,
            Err(err) => return fail(format!("{err}")),
        };
        let kf = match keyfile::read_from_path(&keysfile_path) {
            Ok(kf) => kf,
            Err(err) => return fail(format!("{err}")),
        };
        if let Err(msg) = check_keyfile_holds_every_signer(&kf, "bump-fee") {
            return fail(msg);
        }

        let fee_policy = build_fee_policy(args.fee_rate, args.max_fee_rate, args.max_fee);
        // Empty password on the wire: BumpFee's `password` field
        // selects the daemon's server-side signing path when
        // populated; passing the empty string returns unsigned PSI
        // bytes for the client to sign locally -- the contract
        // `bump-fee` requires, matching the legacy CLI's behaviour.
        let request = BumpFeeRequest {
            password: String::new(),
            from: args.from_address,
            use_existing_change_address: args.use_existing_change_address,
            fee_policy,
            tx_id,
        };

        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.bump_fee(request).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("BumpFee failed: {s}")),
        };

        let mnemonics = match keyfile::decrypt::decrypt_mnemonics(&kf, password.as_bytes()) {
            Ok(m) => m,
            Err(e) => return fail(format!("keyfile decryption failed: {e}")),
        };
        let (signed_transactions, _all_fully_signed) = match sign_partially_signed_transactions(&kf, &mnemonics, &resp.transactions) {
            Ok(out) => out,
            Err(e) => return fail(e),
        };

        println!("Broadcasting {} transaction(s)", signed_transactions.len());
        let total = signed_transactions.len();
        let mut broadcasted_so_far = 0usize;
        for chunk in signed_transactions.chunks(BROADCAST_CHUNK_SIZE) {
            let request = BroadcastRequest { is_domain: false, transactions: chunk.to_vec() };
            let resp = match client.broadcast_replacement(request).await {
                Ok(r) => r.into_inner(),
                Err(s) => return fail(format!("BroadcastReplacement failed: {s}")),
            };
            broadcasted_so_far += chunk.len();
            print_broadcast_chunk_progress(chunk.len(), broadcasted_so_far, total, &resp.tx_ids);
        }
        if args.show_serialized {
            print_show_serialized(&signed_transactions);
        }
        ExitCode::SUCCESS
    })
}

pub fn run_bump_fee_unsigned(args: BumpFeeUnsignedArgs) -> ExitCode {
    let runtime = match build_runtime() {
        Ok(r) => r,
        Err(e) => return e,
    };
    runtime.block_on(async move {
        let tx_id = match args.txid {
            Some(t) if !t.is_empty() => t,
            _ => return fail("--txid is required"),
        };
        let fee_policy = build_fee_policy(args.fee_rate, args.max_fee_rate, args.max_fee);
        // Empty password selects the daemon's unsigned-fee-bump
        // pathway: BumpFee returns the unsigned PSI bytes in
        // `transactions` and leaves `tx_ids` empty for the
        // operator to sign and broadcast offline.
        let request = BumpFeeRequest {
            password: String::new(),
            from: args.from_address,
            use_existing_change_address: args.use_existing_change_address,
            fee_policy,
            tx_id,
        };
        let mut client = match connect(endpoint_url(&args.daemon_address), ClientOptions::default()).await {
            Ok(c) => c,
            Err(e) => return fail(format!("dial daemon '{}': {e}", args.daemon_address)),
        };
        let resp = match client.bump_fee(request).await {
            Ok(r) => r.into_inner(),
            Err(s) => return fail(format!("BumpFee failed: {s}")),
        };
        let _ = writeln!(io::stderr(), "Created unsigned transaction");
        println!("{}", encode_transactions_to_hex(&resp.transactions));
        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kaspa_amount_to_sompi_integer_kaspa() {
        assert_eq!(parse_kaspa_amount_to_sompi("1").unwrap(), SOMPI_PER_KASPA);
        assert_eq!(parse_kaspa_amount_to_sompi("42").unwrap(), 42 * SOMPI_PER_KASPA);
    }

    #[test]
    fn parse_kaspa_amount_to_sompi_decimal_kaspa() {
        assert_eq!(parse_kaspa_amount_to_sompi("0.5").unwrap(), SOMPI_PER_KASPA / 2);
        assert_eq!(parse_kaspa_amount_to_sompi("2.5").unwrap(), 250_000_000);
    }

    #[test]
    fn parse_kaspa_amount_to_sompi_strips_whitespace() {
        assert_eq!(parse_kaspa_amount_to_sompi("  3  ").unwrap(), 3 * SOMPI_PER_KASPA);
    }

    #[test]
    fn parse_kaspa_amount_to_sompi_rejects_empty() {
        let err = parse_kaspa_amount_to_sompi("").unwrap_err();
        assert!(err.contains("must not be empty"));
        let err = parse_kaspa_amount_to_sompi("   ").unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[test]
    fn parse_kaspa_amount_to_sompi_rejects_negative() {
        let err = parse_kaspa_amount_to_sompi("-1").unwrap_err();
        assert!(err.contains("negative"));
    }

    #[test]
    fn parse_kaspa_amount_to_sompi_rejects_garbage() {
        assert!(parse_kaspa_amount_to_sompi("not-a-number").is_err());
        assert!(parse_kaspa_amount_to_sompi("1.2.3").is_err());
    }

    #[test]
    fn build_fee_policy_returns_none_when_all_unset() {
        assert!(build_fee_policy(None, None, None).is_none());
        assert!(build_fee_policy(Some(0.0), Some(0.0), Some(0)).is_none());
    }

    #[test]
    fn build_fee_policy_exact_rate_wins_over_max_rate_and_max_fee() {
        let pol = build_fee_policy(Some(2.5), Some(10.0), Some(50_000)).expect("some");
        match pol.fee_policy.expect("inner") {
            fee_policy::FeePolicy::ExactFeeRate(r) => assert_eq!(r, 2.5),
            other => panic!("expected ExactFeeRate, got {other:?}"),
        }
    }

    #[test]
    fn build_fee_policy_max_rate_wins_over_max_fee() {
        let pol = build_fee_policy(None, Some(10.0), Some(50_000)).expect("some");
        match pol.fee_policy.expect("inner") {
            fee_policy::FeePolicy::MaxFeeRate(r) => assert_eq!(r, 10.0),
            other => panic!("expected MaxFeeRate, got {other:?}"),
        }
    }

    #[test]
    fn build_fee_policy_max_fee_alone() {
        let pol = build_fee_policy(None, None, Some(50_000)).expect("some");
        match pol.fee_policy.expect("inner") {
            fee_policy::FeePolicy::MaxFee(c) => assert_eq!(c, 50_000),
            other => panic!("expected MaxFee, got {other:?}"),
        }
    }

    #[test]
    fn read_tx_hex_requires_one_source() {
        let err = read_tx_hex(None, None).unwrap_err();
        assert_eq!(err, "Either --transaction or --transaction-file is required");
    }

    #[test]
    fn read_tx_hex_rejects_both_sources() {
        let err = read_tx_hex(Some("aa"), Some("/tmp/x")).unwrap_err();
        assert_eq!(err, "Both --transaction and --transaction-file cannot be passed at the same time");
    }

    #[test]
    fn read_tx_hex_inline_argument_is_trimmed() {
        let got = read_tx_hex(Some("  deadbeef\n"), None).unwrap();
        assert_eq!(got, "deadbeef");
    }

    #[test]
    fn read_tx_hex_reads_and_trims_file_contents() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tx.hex");
        std::fs::write(&path, "  cafebabe_baadf00d\n").expect("write");
        let got = read_tx_hex(None, Some(path.to_str().unwrap())).unwrap();
        assert_eq!(got, "cafebabe_baadf00d");
    }

    #[test]
    fn read_tx_hex_reports_missing_file_path() {
        let err = read_tx_hex(None, Some("/no/such/dispatcher_test_path.hex")).unwrap_err();
        assert!(err.starts_with("Could not read hex from /no/such/dispatcher_test_path.hex: "));
    }

    #[test]
    fn resolve_transaction_hex_inline_argument_is_trimmed() {
        let got = resolve_transaction_hex(Some("  deadbeef\n"), None, "sign").unwrap();
        assert_eq!(got, "deadbeef");
    }

    #[test]
    fn resolve_transaction_hex_reads_and_trims_file_contents() {
        // Mirrors the canonical operator workflow:
        //   kaspawallet create-unsigned-transaction ... > unsigned.hex
        //   kaspawallet sign --transaction-file unsigned.hex
        // The shell redirect appends `\n` to the file; the resolver
        // must strip it before the hex decode runs, otherwise the
        // decode trips on an odd-length input.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tx.hex");
        std::fs::write(&path, "  cafebabe_baadf00d\n").expect("write");
        let got = resolve_transaction_hex(None, Some(path.to_str().unwrap()), "sign").unwrap();
        assert_eq!(got, "cafebabe_baadf00d");
    }

    #[test]
    fn resolve_transaction_hex_file_with_trailing_newline_decodes_clean() {
        // Hex-decoder reality check: the trimmed contents must be a
        // valid even-length hex string after the trim. Pre-fix this
        // call would have returned `"abcd\n"` and the downstream
        // hex decode would have failed with "Odd number of digits".
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tx.hex");
        std::fs::write(&path, "abcd\n").expect("write");
        let got = resolve_transaction_hex(None, Some(path.to_str().unwrap()), "sign").unwrap();
        assert_eq!(got, "abcd");
        // Round-trip through the hex decoder to prove the symptom is
        // gone (the Validator's repro fixture).
        let _decoded = hex::decode(&got).expect("trimmed hex must decode cleanly");
    }

    // ---- send + bump-fee online subcommands -----------------

    use crate::keyfile::{EncryptedMnemonic, KeysFile};

    fn synthetic_keyfile(num_pubkeys: usize, num_mnemonics: usize) -> KeysFile {
        KeysFile {
            version: 1,
            num_threads: 8,
            encrypted_mnemonics: vec![EncryptedMnemonic { cipher: vec![0u8; 8], salt: vec![0u8; 16] }; num_mnemonics],
            extended_public_keys: vec!["kpub-dummy".to_owned(); num_pubkeys],
            minimum_signatures: 1,
            cosigner_index: 0,
            last_used_external_index: 0,
            last_used_internal_index: 0,
            ecdsa: false,
        }
    }

    #[test]
    fn check_keyfile_holds_every_signer_rejects_incomplete_multisig() {
        let kf = synthetic_keyfile(3, 2);
        let err = check_keyfile_holds_every_signer(&kf, "send").unwrap_err();
        assert_eq!(err, "Cannot use 'send' command for multisig wallet without all of the keys");
    }

    #[test]
    fn check_keyfile_holds_every_signer_uses_subcommand_name_in_error() {
        let kf = synthetic_keyfile(3, 2);
        let err = check_keyfile_holds_every_signer(&kf, "bump-fee").unwrap_err();
        assert_eq!(err, "Cannot use 'bump-fee' command for multisig wallet without all of the keys");
    }

    #[test]
    fn check_keyfile_holds_every_signer_accepts_complete_multisig() {
        // Equal counts: every cosigner's mnemonic is present, so
        // the threshold can be reached locally without external
        // coordination -- operation proceeds.
        let kf = synthetic_keyfile(3, 3);
        check_keyfile_holds_every_signer(&kf, "send").unwrap();
    }

    #[test]
    fn check_keyfile_holds_every_signer_accepts_singlekey() {
        let kf = synthetic_keyfile(1, 1);
        check_keyfile_holds_every_signer(&kf, "send").unwrap();
    }

    #[test]
    fn broadcast_chunk_size_matches_legacy_cap() {
        assert_eq!(BROADCAST_CHUNK_SIZE, 100);
    }

    #[test]
    fn format_chunk_progress_header_single_chunk_shape() {
        // 100 of 100 = 100.00%, mirroring the legacy `%.2f` shape.
        let got = format_chunk_progress_header(100, 100, 100);
        assert_eq!(got, "Broadcasted 100 transaction(s) (broadcasted 100.00% of the transactions so far)");
    }

    #[test]
    fn format_chunk_progress_header_mid_stream_shape() {
        // First of two 100-tx chunks: chunk=100 / sent=100 / total=200.
        let got = format_chunk_progress_header(100, 100, 200);
        assert_eq!(got, "Broadcasted 100 transaction(s) (broadcasted 50.00% of the transactions so far)");
    }

    #[test]
    fn format_chunk_progress_header_final_short_chunk_shape() {
        // Three-chunk stream {100, 100, 50}: final chunk lands sent=250 / total=250.
        let got = format_chunk_progress_header(50, 250, 250);
        assert_eq!(got, "Broadcasted 50 transaction(s) (broadcasted 100.00% of the transactions so far)");
    }

    #[test]
    fn chunks_iterator_partitions_signed_transactions_correctly() {
        // The chunked-broadcast loop relies on `.chunks(BROADCAST_CHUNK_SIZE)`
        // to partition the signed-transaction slice. Verify that
        // partition behaviour at the three load-bearing boundaries:
        // 100-multiple (one full chunk), strict 100-multiple of two,
        // and a non-multiple producing a trailing short chunk.
        let one = vec![Vec::<u8>::new(); 100];
        assert_eq!(one.chunks(BROADCAST_CHUNK_SIZE).map(|c| c.len()).collect::<Vec<_>>(), vec![100]);

        let two = vec![Vec::<u8>::new(); 200];
        assert_eq!(two.chunks(BROADCAST_CHUNK_SIZE).map(|c| c.len()).collect::<Vec<_>>(), vec![100, 100]);

        let two_and_a_bit = vec![Vec::<u8>::new(); 250];
        assert_eq!(two_and_a_bit.chunks(BROADCAST_CHUNK_SIZE).map(|c| c.len()).collect::<Vec<_>>(), vec![100, 100, 50]);

        let empty: Vec<Vec<u8>> = vec![];
        assert_eq!(empty.chunks(BROADCAST_CHUNK_SIZE).count(), 0);
    }

    #[test]
    fn format_show_serialized_block_single_transaction_shape() {
        let txs = vec![vec![0xde, 0xad, 0xbe, 0xef]];
        let got = format_show_serialized_block(&txs);
        assert_eq!(
            got,
            "Serialized Transaction(s) (can be parsed via the `parse` command or resent via `broadcast`): \n\
             \tdeadbeef\n\n"
        );
    }

    #[test]
    fn format_show_serialized_block_multiple_transactions_separated_by_blank_lines() {
        let txs = vec![vec![0xaa, 0xbb], vec![0xcc, 0xdd]];
        let got = format_show_serialized_block(&txs);
        assert_eq!(
            got,
            "Serialized Transaction(s) (can be parsed via the `parse` command or resent via `broadcast`): \n\
             \taabb\n\n\
             \tccdd\n\n"
        );
    }

    #[test]
    fn format_show_serialized_block_empty_input_emits_header_only() {
        let got = format_show_serialized_block(&[]);
        assert_eq!(got, "Serialized Transaction(s) (can be parsed via the `parse` command or resent via `broadcast`): \n");
    }
}
