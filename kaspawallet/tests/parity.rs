//! Cross-implementation parity harness -- offline subset.
//!
//! The full per-subcommand parity matrix the Validator runs spans
//! both an offline (no-daemon, no-kaspad) subset and an online
//! (tn-10 / simnet) subset that requires running daemons and a
//! live kaspad. This file lands the offline subset that ships in
//! CI; the online subset is scaffolded as `#[ignore]`-gated tests
//! so the same module hosts the eventual full matrix without a
//! second cross-cutting reorganization.
//!
//! Three reference binaries participate in the matrix:
//!
//! - **Reference** -- the canonical operator interface this binary
//!   matches byte-for-byte on the documented subcommand surface.
//!   Resolved via `KASPAWALLET_REFERENCE_BIN` env var or the
//!   default workstation path. Source repository:
//!   <https://github.com/kaspanet/kaspad>
//!   (`cmd/kaspawallet`).
//! - **Legacy port** -- the prior in-tree port (one task family
//!   ahead of this one) used as a regression-protection probe.
//!   Resolved via `KASPAWALLET_LEGACY_PORT_BIN` (no baked-in
//!   default -- the legacy-port workspace is workstation-specific).
//!   The legacy port already validated the 17-subcommand surface
//!   against the reference; cross-checking against it guarantees
//!   this binary inherits that validated behaviour instead of
//!   silently diverging.
//! - **Under test** -- the binary produced by this workspace's
//!   `cargo build --bin kaspawallet`. Resolved via
//!   `KASPAWALLET_UNDER_TEST_BIN` or the workspace
//!   `target/{debug,release}/kaspawallet` fallback.
//!
//! Skip semantics: when a binary required by a parity test cannot
//! be resolved, the test prints a one-line skip warning to stderr
//! and exits 0. This keeps the cargo-test surface green on
//! developer workstations that lack one of the reference builds;
//! the Validator runs against a workstation where all three are
//! present.
//!
//! Subcommands exercisable in standalone-binary mode today:
//!
//! - `version` -- both binaries print a one-line version banner;
//!   the version literal is normalised and the framing compared.
//! - `parse` -- both binaries decode a reference unsigned PSTX hex
//!   plus a fixture keyfile and emit a plaintext transcript;
//!   byte-identity holds (raw `cmp`).
//!
//! Subcommands that need the daemon-client wiring (or sign-flow)
//! the standalone CLI cannot exercise without a live daemon plus
//! kaspad are scaffolded below as `#[ignore]`-gated tests. Each
//! carries a one-line reason on the gate; the ignore lifts when
//! the corresponding online fixture is staged (Validator tn-10
//! closure pass).

mod harness;

use std::ffi::{OsStr, OsString};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const DEFAULT_REFERENCE_BIN: &str = "/home/dima/work/kaspa/kaspad/bin/kaspawallet";

const ENV_REFERENCE_BIN: &str = "KASPAWALLET_REFERENCE_BIN";
const ENV_LEGACY_PORT_BIN: &str = "KASPAWALLET_LEGACY_PORT_BIN";
const ENV_UNDER_TEST_BIN: &str = "KASPAWALLET_UNDER_TEST_BIN";

// Workstation env-var contract for daemon-targeted parity rows
// (wallet password file, wallet-core wallet name, legacy-port
// keyfile path, kaspad wRPC / gRPC endpoints) is canonical at
// `harness::paired_daemons`. The non-paired liveness canary
// reuses the password-file constant only.
use harness::paired_daemons::ENV_DAEMON_PASSWORD_FILE;

/// Overall budget for the wallet daemon to bind its gRPC listen
/// address. Covers wallet-open, kaspad-connect, get-block-dag-info,
/// account activation, and the `tokio::spawn` of the tonic server
/// bind. The budget is generous to absorb cold-start latency on a
/// fresh kaspad without making a stuck daemon hang the suite.
const DAEMON_GRPC_LISTEN_TIMEOUT: Duration = Duration::from_secs(30);

/// Budget for a freshly-spawned local kaspad to accept its wRPC
/// listen socket. The wallet daemon's wrpc connect would retry on
/// its own, but waiting here turns a stuck kaspad into a fast,
/// directly-attributed failure instead of an opaque daemon timeout.
const KASPAD_RPC_LISTEN_TIMEOUT: Duration = Duration::from_secs(15);

const WS_SCHEME_PREFIX: &str = "ws://";

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(meta) => meta.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    }
}

fn locate_env_or_default(env_var: &str, default_path: &str) -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var(env_var) {
        let candidate = PathBuf::from(env_path);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    let default = PathBuf::from(default_path);
    if is_executable_file(&default) { Some(default) } else { None }
}

fn locate_reference_binary() -> Option<PathBuf> {
    locate_env_or_default(ENV_REFERENCE_BIN, DEFAULT_REFERENCE_BIN)
}

/// Resolve the legacy-port binary from `KASPAWALLET_LEGACY_PORT_BIN`.
/// No baked-in default: the legacy-port binary lives in a separate
/// workspace whose location is workstation-specific, and source must
/// not anchor on that workstation layout. Callers (legacy-port
/// parity tests) emit the skip-with-warning when the env var is
/// unset or points at a non-executable path.
fn locate_legacy_port_binary() -> Option<PathBuf> {
    let env_path = std::env::var(ENV_LEGACY_PORT_BIN).ok()?;
    let candidate = PathBuf::from(env_path);
    if is_executable_file(&candidate) { Some(candidate) } else { None }
}

fn locate_under_test_binary() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var(ENV_UNDER_TEST_BIN) {
        let candidate = PathBuf::from(env_path);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    let target_root = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| {
        let mut workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        workspace_root.pop();
        workspace_root.push("target");
        workspace_root
    });
    for profile in ["debug", "release"] {
        let mut candidate = target_root.clone();
        candidate.push(profile);
        candidate.push("kaspawallet");
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Resolve (reference, under_test) for a parity test that pairs
/// the binary under test against the canonical reference. Returns
/// `None` when either is missing; the caller emits the skip-with-
/// warning and returns early.
fn resolve_reference_pair(test_name: &str) -> Option<(PathBuf, PathBuf)> {
    let reference = match locate_reference_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- reference binary not found. Set {ENV_REFERENCE_BIN} or place the binary at {DEFAULT_REFERENCE_BIN}."
            );
            return None;
        }
    };
    let under_test = match locate_under_test_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- under-test binary not found. Run `cargo build --bin kaspawallet` first or set {ENV_UNDER_TEST_BIN}."
            );
            return None;
        }
    };
    Some((reference, under_test))
}

/// Resolve (legacy_port, under_test) for a parity test that pairs
/// the binary under test against the in-tree legacy port. Returns
/// `None` with a skip-with-warning when either is missing; the
/// legacy-port resolver has no baked-in default (workstation-
/// specific path), so the typical absence path is missing env var.
fn resolve_legacy_port_pair(test_name: &str) -> Option<(PathBuf, PathBuf)> {
    let legacy_port = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return None;
        }
    };
    let under_test = match locate_under_test_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- under-test binary not found. Run `cargo build --bin kaspawallet` first or set {ENV_UNDER_TEST_BIN}."
            );
            return None;
        }
    };
    Some((legacy_port, under_test))
}

fn run_capture(bin: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new(bin).args(args).output().expect("process spawn");
    let mut combined = output.stdout;
    combined.extend_from_slice(&output.stderr);
    combined
}

/// Normalise a `version` banner to a framing-only form. Both the
/// reference (`kaspawallet version 0.12.22`) and the binary under
/// test (`kaspawallet v1.1.0`) collapse to a common prefix
/// `<binary-name> <NORM>` so the diff exercises the framing without
/// coupling to build metadata.
fn normalize_version_banner(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim_end_matches(['\r', '\n']);
    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    format!("{first_word} <NORM>")
}

#[test]
fn version_framing_parity_with_reference() {
    let Some((reference, under_test)) = resolve_reference_pair("version_framing_parity_with_reference") else {
        return;
    };
    let reference_out = run_capture(&reference, &["version"]);
    let under_test_out = run_capture(&under_test, &["version"]);
    let reference_norm = normalize_version_banner(&reference_out);
    let under_test_norm = normalize_version_banner(&under_test_out);
    assert_eq!(reference_norm, under_test_norm, "version banner framing diverges between reference and under-test binaries");
    assert_eq!(reference_norm, "kaspawallet <NORM>", "framing prefix unexpectedly missing from normalized output");
}

#[test]
fn parse_offline_byte_identity_with_reference() {
    let Some((reference, under_test)) = resolve_reference_pair("parse_offline_byte_identity_with_reference") else {
        return;
    };
    let pstx_path = fixture("go_emitted_pst.hex");
    let pstx_hex = std::fs::read_to_string(&pstx_path).expect("read pstx fixture");
    let pstx_hex = pstx_hex.trim();
    let keys_path = fixture("legacy_go_v1_singlekey.json");
    let keys_arg = keys_path.to_str().expect("ascii path");

    let reference_out = run_capture(&reference, &["parse", "--keys-file", keys_arg, "--transaction", pstx_hex]);
    let under_test_out = run_capture(&under_test, &["parse", "--keys-file", keys_arg, "--transaction", pstx_hex]);
    assert_eq!(
        reference_out,
        under_test_out,
        "parse output diverges between reference and under-test binaries (reference {} bytes vs under-test {} bytes)",
        reference_out.len(),
        under_test_out.len()
    );
}

// ----------------------------------------------------------------
// Scaffolded parity rows for subcommands the standalone CLI cannot
// exercise without a live daemon plus kaspad. Each ignore-gate
// carries the staging requirement; the Validator's tn-10 closure
// pass lifts the gates as fixtures land.
// ----------------------------------------------------------------

/// Byte-identity of `ShowAddresses` between the under-test daemon
/// and the legacy-port daemon when both open wallets derived from
/// the same mnemonic against the same kaspad. Each daemon's storage
/// backend differs (the under-test daemon uses kaspa-wallet-core's
/// named storage; the legacy-port daemon uses a Go-format keyfile),
/// so the fixture provisioning step ships two encrypted
/// representations of the same mnemonic and the test verifies the
/// resulting address lists match.
///
/// Skip-clean prerequisites (any one absent yields an early return):
///
/// - under-test `kaspawallet` binary located (`KASPAWALLET_BIN` or
///   the workspace `target/{debug,release}/kaspawallet`; the daemon
///   runs via the binary's `start-daemon` subcommand);
/// - legacy-port binary located (`KASPAWALLET_LEGACY_PORT_BIN`);
/// - password file path (`KASPAWALLETD_TEST_PASSWORD_FILE`) readable
///   and non-empty;
/// - under-test wallet-core wallet name (`KASPAWALLETD_TEST_WALLET_NAME`);
/// - legacy-port Go-format keyfile path (`KASPAWALLET_LEGACY_PORT_KEYFILE`);
/// - kaspad wRPC endpoint (`KASPA_TN10_ENDPOINT`) for the under-test
///   daemon's `--rpc-server`;
/// - kaspad gRPC endpoint (`KASPAD_GRPC_ENDPOINT`) for the
///   legacy-port daemon's `--rpcserver`.
///
/// The network is testnet-10 by convention: the wRPC endpoint
/// variable carries that name and the paired daemons match it
/// (`--network-id testnet-10` for the under-test daemon, `--testnet`
/// for the legacy-port daemon).
#[tokio::test]
async fn show_addresses_byte_identity() {
    let test_name = "show_addresses_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let under_test_addresses = under_test_client
        .show_addresses(kaspa_wallet_grpc_client::kaspawalletd::ShowAddressesRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: ShowAddresses RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner()
        .address;
    let legacy_port_addresses = legacy_port_client
        .show_addresses(kaspa_wallet_grpc_client::kaspawalletd::ShowAddressesRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: ShowAddresses RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner()
        .address;

    assert_eq!(
        under_test_addresses,
        legacy_port_addresses,
        "parity::{test_name}: address lists diverge -- under-test {} entries, legacy-port {} entries",
        under_test_addresses.len(),
        legacy_port_addresses.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `NewAddress` between the under-test daemon and
/// the legacy-port daemon when both open wallets derived from the
/// same mnemonic against the same kaspad. `NewAddress` mutates
/// persistent state on both daemons -- the under-test daemon
/// advances wallet-core's stored next-receive index, the legacy-port
/// daemon writes the bumped `last_used_external_index` to its
/// Go-format keyfile -- so the paired post-state of one test run
/// becomes the pre-state of the next. The test handles this by
/// asserting paired pre-state via [`ShowAddresses`] (both daemons
/// must agree on the existing address list before either mutates)
/// and asserting paired post-state via the [`NewAddress`] return
/// (both daemons must derive the same address from the same shared
/// mnemonic at the now-bumped index). The two daemons advance in
/// lockstep across runs; drift between runs surfaces as a
/// pre-state divergence panic on the next invocation and the
/// Validator re-provisions the paired fixture.
///
/// Skip-clean prerequisites are the same five-variable set the
/// other paired rows take (the [`harness::paired_daemons`] contract).
/// Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn new_address_byte_identity() {
    let test_name = "new_address_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let under_test_before = under_test_client
        .show_addresses(kaspa_wallet_grpc_client::kaspawalletd::ShowAddressesRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: pre-state ShowAddresses against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner()
        .address;
    let legacy_port_before = legacy_port_client
        .show_addresses(kaspa_wallet_grpc_client::kaspawalletd::ShowAddressesRequest {})
        .await
        .unwrap_or_else(|e| {
            panic!("parity::{test_name}: pre-state ShowAddresses against legacy-port {legacy_port_grpc_endpoint}: {e}")
        })
        .into_inner()
        .address;
    assert_eq!(
        under_test_before,
        legacy_port_before,
        "parity::{test_name}: pre-state address lists diverge -- under-test {} entries, legacy-port {} entries; re-provision the paired fixture",
        under_test_before.len(),
        legacy_port_before.len()
    );

    let under_test_new = under_test_client
        .new_address(kaspa_wallet_grpc_client::kaspawalletd::NewAddressRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: NewAddress RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner()
        .address;
    let legacy_port_new = legacy_port_client
        .new_address(kaspa_wallet_grpc_client::kaspawalletd::NewAddressRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: NewAddress RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner()
        .address;

    assert_eq!(
        under_test_new, legacy_port_new,
        "parity::{test_name}: new addresses diverge -- under-test {under_test_new}, legacy-port {legacy_port_new}"
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `GetBalance` between the under-test daemon and
/// the legacy-port daemon when both open wallets derived from the
/// same mnemonic against the same kaspad. `GetBalance` is read-only
/// (no keyfile or wallet-storage mutation), so the test asserts a
/// single post-RPC parity rather than the paired pre/post pattern
/// the mutating rows take: under-test `GetBalanceResponse` bytes
/// equal legacy-port `GetBalanceResponse` bytes (matching `available`,
/// `pending`, and the `address_balances` vector in declaration
/// order).
///
/// The row's deterministic-UTXO-snapshot prerequisite is satisfied
/// at fixture provisioning time, not at test execution time: both
/// daemons resolve their wallet's UTXO set from the same kaspad
/// endpoint, so the snapshot the wallet sees is whatever kaspad's
/// state at RPC time happens to be. The Validator-workstation
/// provisioning step funds the paired wallet with a known UTXO set
/// against a tn-10 or simnet kaspad whose mempool is at rest so
/// `available`/`pending` are stable between the two consecutive RPC
/// calls; a between-call mempool churn would surface as a noisy
/// pending-amount divergence and is the natural motivation for the
/// fixture's at-rest kaspad selection.
///
/// Skip-clean prerequisites are the same five-variable set the
/// other paired rows take (the [`harness::paired_daemons`] contract).
/// Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn balance_byte_identity() {
    let test_name = "balance_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let under_test_balance = under_test_client
        .get_balance(kaspa_wallet_grpc_client::kaspawalletd::GetBalanceRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: GetBalance RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let legacy_port_balance = legacy_port_client
        .get_balance(kaspa_wallet_grpc_client::kaspawalletd::GetBalanceRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: GetBalance RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();

    assert_eq!(
        under_test_balance.available, legacy_port_balance.available,
        "parity::{test_name}: available diverges -- under-test {} sompi, legacy-port {} sompi",
        under_test_balance.available, legacy_port_balance.available
    );
    assert_eq!(
        under_test_balance.pending, legacy_port_balance.pending,
        "parity::{test_name}: pending diverges -- under-test {} sompi, legacy-port {} sompi",
        under_test_balance.pending, legacy_port_balance.pending
    );
    assert_eq!(
        under_test_balance.address_balances,
        legacy_port_balance.address_balances,
        "parity::{test_name}: address_balances diverge -- under-test {} entries, legacy-port {} entries",
        under_test_balance.address_balances.len(),
        legacy_port_balance.address_balances.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `Send` between the under-test daemon and the
/// legacy-port daemon when both are asked to spend `amount` sompi
/// from the paired wallet's funded single-UTXO set to the same
/// external address against the same kaspad. The request leaves
/// `from` empty (let the daemon walk the wallet's address list),
/// `use_existing_change_address = false`, and `is_send_all = false`;
/// the password is the literal contents of the file at
/// [`harness::paired_daemons::ENV_DAEMON_PASSWORD_FILE`] read by
/// [`harness::paired_daemons::resolve_resources`] (the `password`
/// field on [`PairResources`]).
///
/// Both daemons sign deterministically (Schnorr per RFC 6979) and
/// broadcast the resulting transaction to the same kaspad. The
/// fixture-provisioning step's single-UTXO precondition collapses
/// coin selection to a one-element choice on both daemons, so the
/// signing input is byte-identical on both sides; deterministic
/// signing then yields byte-identical signatures, byte-identical
/// wire bytes, and the same transaction id. The first daemon's
/// `Send` admits the tx into kaspad's mempool; the second daemon's
/// `Send` re-broadcasts the same tx, which kaspad treats as a
/// duplicate-tx no-op (the tx is already mempool-resident), so the
/// `SendResponse` carries the same `tx_ids` and
/// `signed_transactions` payload on both sides.
///
/// The response carries a `Vec<String>` of txids and a
/// `Vec<Vec<u8>>` of signed transactions (one element per output
/// split when the daemon splits a large spend, one element total
/// in the single-UTXO single-output case). The byte-identity
/// assertion compares the vectors element-wise via `Vec::eq`,
/// which the prost-generated `Vec<bytes>` element type satisfies
/// by delegating to `Vec<u8>: PartialEq`.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus
/// [`harness::paired_daemons::ENV_SEND_TO_ADDRESS`] (the recipient
/// address literal) and
/// [`harness::paired_daemons::ENV_SEND_AMOUNT_SOMPI`] (the spend
/// amount). Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn send_byte_identity_single_utxo() {
    let test_name = "send_byte_identity_single_utxo";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(to_address) = harness::paired_daemons::require_env_string(harness::paired_daemons::ENV_SEND_TO_ADDRESS, test_name) else {
        return;
    };
    let Some(amount) = harness::paired_daemons::require_env_u64(harness::paired_daemons::ENV_SEND_AMOUNT_SOMPI, test_name) else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let make_request = || kaspa_wallet_grpc_client::kaspawalletd::SendRequest {
        to_address: to_address.clone(),
        amount,
        password: res.password.clone(),
        from: Vec::new(),
        use_existing_change_address: false,
        is_send_all: false,
        fee_policy: None,
    };

    let under_test_response = under_test_client
        .send(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: Send RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let legacy_port_response = legacy_port_client
        .send(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: Send RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();

    assert_eq!(
        under_test_response.tx_ids,
        legacy_port_response.tx_ids,
        "parity::{test_name}: tx_ids diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.tx_ids.len(),
        legacy_port_response.tx_ids.len()
    );
    assert_eq!(
        under_test_response.signed_transactions,
        legacy_port_response.signed_transactions,
        "parity::{test_name}: signed_transactions diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.signed_transactions.len(),
        legacy_port_response.signed_transactions.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `CreateUnsignedTransactions` between the
/// under-test daemon and the legacy-port daemon when both are asked
/// to produce an unsigned spend of `amount` sompi from the paired
/// wallet's funded single-UTXO set to the same external address
/// against the same kaspad. The request leaves `from` empty (let
/// the daemon walk the wallet's address list),
/// `use_existing_change_address = false`, `is_send_all = false`,
/// and `fee_policy = None` (default daemon policy applies). The
/// fixture-provisioning step's single-UTXO precondition collapses
/// coin selection to a one-element choice on both daemons, which
/// is the "Path-A coin-selected hex cmp" the row's ignore reason
/// names: no coin-selection nondeterminism contaminates the
/// unsigned-bytes byte-identity assertion.
///
/// The response carries a `Vec<Vec<u8>>` of PSTX-encoded unsigned
/// transactions (one per output split when the daemon splits a
/// large spend, one total in the single-UTXO single-output case).
/// The byte-identity assertion compares the vectors element-wise
/// via `Vec::eq`, which the prost-generated `Vec<bytes>` element
/// type satisfies by delegating to `Vec<u8>: PartialEq`.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus
/// [`harness::paired_daemons::ENV_SEND_TO_ADDRESS`] (the recipient
/// address literal) and
/// [`harness::paired_daemons::ENV_SEND_AMOUNT_SOMPI`] (the spend
/// amount). Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn create_unsigned_byte_identity() {
    let test_name = "create_unsigned_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(to_address) = harness::paired_daemons::require_env_string(harness::paired_daemons::ENV_SEND_TO_ADDRESS, test_name) else {
        return;
    };
    let Some(amount) = harness::paired_daemons::require_env_u64(harness::paired_daemons::ENV_SEND_AMOUNT_SOMPI, test_name) else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let make_request = || kaspa_wallet_grpc_client::kaspawalletd::CreateUnsignedTransactionsRequest {
        address: to_address.clone(),
        amount,
        from: Vec::new(),
        use_existing_change_address: false,
        is_send_all: false,
        fee_policy: None,
    };

    let under_test_response = under_test_client
        .create_unsigned_transactions(make_request())
        .await
        .unwrap_or_else(|e| {
            panic!("parity::{test_name}: CreateUnsignedTransactions RPC against under-test {under_test_grpc_endpoint}: {e}")
        })
        .into_inner();
    let legacy_port_response = legacy_port_client
        .create_unsigned_transactions(make_request())
        .await
        .unwrap_or_else(|e| {
            panic!("parity::{test_name}: CreateUnsignedTransactions RPC against legacy-port {legacy_port_grpc_endpoint}: {e}")
        })
        .into_inner();

    assert_eq!(
        under_test_response.unsigned_transactions,
        legacy_port_response.unsigned_transactions,
        "parity::{test_name}: unsigned_transactions diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.unsigned_transactions.len(),
        legacy_port_response.unsigned_transactions.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `Sign` between the under-test daemon and the
/// legacy-port daemon when both sign the same unsigned ECDSA-curve
/// PSTX against a wallet view that resolves to the same ECDSA
/// singlekey keyfile. The fixture file at
/// [`harness::paired_daemons::ENV_UNSIGNED_ECDSA_PSTX_FIXTURE`]
/// carries the unsigned PSTX bytes the fixture-provisioning step
/// emits from the ECDSA singlekey keyfile both daemons share; the
/// test wraps them in a single-element
/// `SignRequest.unsigned_transactions` vector along with the
/// password the paired-daemons topology resolves and dispatches
/// the `Sign` RPC to both daemons in turn.
///
/// The byte-identity contract on `SignResponse.signed_transactions`
/// holds via two composed properties: (i) ECDSA per RFC 6979
/// produces deterministic signatures from identical signing
/// inputs (curve, message digest, private key) -- a property the
/// row name's `byte_identity` half asserts; (ii) Schnorr-validity
/// of any per-input signatures the daemon emits is co-attested
/// by the byte-identity assertion itself -- two independent
/// daemons producing the same signature bytes on the same input
/// cannot diverge on validity (the bytes either both verify
/// against the public key or both do not), so the row name's
/// `schnorr_validity` half is exhausted when the byte-identity
/// assertion passes. A signing-path defect that yielded an invalid
/// signature would manifest either as a byte-divergence (when the
/// defect is asymmetric across daemons) or as a kaspad-mempool
/// admission failure at Validator Phase 9 (when the defect is
/// symmetric across daemons but the signature is structurally
/// rejected on broadcast); the test surface here is the daemon-RPC
/// byte-identity, not the broadcast-time validity, which the
/// Validator's tn-10 happy-path exercises.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus the unsigned
/// ECDSA PSTX fixture file at
/// [`harness::paired_daemons::ENV_UNSIGNED_ECDSA_PSTX_FIXTURE`].
/// Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn sign_ecdsa_byte_identity_and_schnorr_validity() {
    let test_name = "sign_ecdsa_byte_identity_and_schnorr_validity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(unsigned_pstx) =
        harness::paired_daemons::require_fixture_bytes(harness::paired_daemons::ENV_UNSIGNED_ECDSA_PSTX_FIXTURE, test_name)
    else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let make_request = || kaspa_wallet_grpc_client::kaspawalletd::SignRequest {
        unsigned_transactions: vec![unsigned_pstx.clone()],
        password: res.password.clone(),
    };

    let under_test_response = under_test_client
        .sign(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: Sign RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let legacy_port_response = legacy_port_client
        .sign(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: Sign RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();

    assert_eq!(
        under_test_response.signed_transactions,
        legacy_port_response.signed_transactions,
        "parity::{test_name}: signed_transactions diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.signed_transactions.len(),
        legacy_port_response.signed_transactions.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `Broadcast` between the under-test daemon and
/// the legacy-port daemon when both submit the same pre-signed
/// transaction blob against the same kaspad. The pre-signed bytes
/// are loaded from the file at [`harness::paired_daemons::ENV_PRESIGNED_TX_FIXTURE`]
/// and wrapped in a single-element `BroadcastRequest.transactions`
/// vector with `is_domain = false` (the wire-format payload is the
/// consensus transaction encoding the daemon's `deserialize_txs`
/// path consumes when the consensus flag is unset).
///
/// The byte-identity property requires both daemons to receive
/// admit-success from kaspad for the same input transaction.
/// Kaspad's mempool admission of a previously-seen tx hash is the
/// load-bearing precondition: when the second submission resolves
/// to the same tx-id the first submission produced, the two
/// `BroadcastResponse` payloads agree element-wise on `tx_ids`. If
/// the Validator workstation observes a divergence here (one
/// daemon's submit succeeds and the other's surfaces a
/// duplicate-tx status), the fixture provisioning step is the seam
/// to address: either coordinate two kaspads with mirrored UTXO
/// state, or use a mempool configuration that admits duplicates as
/// no-ops returning the existing entry's id.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus the pre-signed
/// transaction fixture at
/// [`harness::paired_daemons::ENV_PRESIGNED_TX_FIXTURE`]. Daemon
/// binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn broadcast_byte_identity() {
    let test_name = "broadcast_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(presigned_tx) =
        harness::paired_daemons::require_fixture_bytes(harness::paired_daemons::ENV_PRESIGNED_TX_FIXTURE, test_name)
    else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let under_test_response = under_test_client
        .broadcast(kaspa_wallet_grpc_client::kaspawalletd::BroadcastRequest {
            is_domain: false,
            transactions: vec![presigned_tx.clone()],
        })
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: Broadcast RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let legacy_port_response = legacy_port_client
        .broadcast(kaspa_wallet_grpc_client::kaspawalletd::BroadcastRequest { is_domain: false, transactions: vec![presigned_tx] })
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: Broadcast RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();

    assert_eq!(
        under_test_response.tx_ids,
        legacy_port_response.tx_ids,
        "parity::{test_name}: tx_ids diverge -- under-test {} entries {:?}, legacy-port {} entries {:?}",
        under_test_response.tx_ids.len(),
        under_test_response.tx_ids,
        legacy_port_response.tx_ids.len(),
        legacy_port_response.tx_ids
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `BroadcastReplacement` between the under-test
/// daemon and the legacy-port daemon when both submit the same
/// pre-signed replacement transaction blob against the same kaspad
/// while a prior conflicting transaction sits in the mempool.
/// The replacement bytes are loaded from the file at
/// [`harness::paired_daemons::ENV_PRESIGNED_REPLACEMENT_TX_FIXTURE`]
/// and wrapped in a single-element `BroadcastRequest.transactions`
/// vector; `is_domain = false` selects the consensus-tx encoding the
/// daemon's `deserialize_txs` consumes.
///
/// The byte-identity property hinges on both daemons receiving the
/// same `tx_ids` response for the same replacement payload. The
/// under-test daemon dispatches via `submit_transaction_replacement`
/// for the first transaction in the vector and `submit_transaction`
/// for any orphan-dependent followups; the legacy-port daemon goes
/// through `broadcast_replacement_inner` which routes identically.
/// On a singleton vector the routing collapses to a single
/// `submit_transaction_replacement` call on each side, so the
/// returned tx-id is the replacement's hash and both responses
/// agree.
///
/// The Validator-workstation fixture step provisions the prior
/// in-mempool transaction (broadcast directly via kaspad or via one
/// of the daemons before this row runs) and then constructs the
/// replacement that conflicts with it. If the prior tx is missing
/// from the mempool, both daemons will surface a "no replaceable
/// transaction" status from kaspad and the row will panic with the
/// matching stderr message attached -- a fail-loud signal that the
/// fixture-provisioning step regressed.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus the pre-signed
/// replacement transaction fixture at
/// [`harness::paired_daemons::ENV_PRESIGNED_REPLACEMENT_TX_FIXTURE`].
/// Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn broadcast_replacement_byte_identity() {
    let test_name = "broadcast_replacement_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(replacement_tx) =
        harness::paired_daemons::require_fixture_bytes(harness::paired_daemons::ENV_PRESIGNED_REPLACEMENT_TX_FIXTURE, test_name)
    else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let under_test_response = under_test_client
        .broadcast_replacement(kaspa_wallet_grpc_client::kaspawalletd::BroadcastRequest {
            is_domain: false,
            transactions: vec![replacement_tx.clone()],
        })
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: BroadcastReplacement RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let legacy_port_response = legacy_port_client
        .broadcast_replacement(kaspa_wallet_grpc_client::kaspawalletd::BroadcastRequest {
            is_domain: false,
            transactions: vec![replacement_tx],
        })
        .await
        .unwrap_or_else(|e| {
            panic!("parity::{test_name}: BroadcastReplacement RPC against legacy-port {legacy_port_grpc_endpoint}: {e}")
        })
        .into_inner();

    assert_eq!(
        under_test_response.tx_ids,
        legacy_port_response.tx_ids,
        "parity::{test_name}: tx_ids diverge -- under-test {} entries {:?}, legacy-port {} entries {:?}",
        under_test_response.tx_ids.len(),
        under_test_response.tx_ids,
        legacy_port_response.tx_ids.len(),
        legacy_port_response.tx_ids
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `BumpFee` between the under-test daemon and the
/// legacy-port daemon when both are asked to replace the same
/// kaspad-mempool-resident parent transaction with a higher-fee
/// version against the same kaspad. The request leaves `from` empty
/// (let the daemon walk the wallet's address list),
/// `use_existing_change_address = false`, and `fee_policy = None`
/// (the daemon picks its default policy, which both daemons
/// implement identically); the password is the literal contents of
/// the file at [`harness::paired_daemons::ENV_DAEMON_PASSWORD_FILE`]
/// read by [`harness::paired_daemons::resolve_resources`] and the
/// parent transaction id is the hex string at
/// [`harness::paired_daemons::ENV_BUMP_FEE_TX_ID`] (a low-fee
/// transaction the fixture-provisioning step admitted into kaspad's
/// mempool from the paired wallet).
///
/// Both daemons resolve the parent transaction from their wallet's
/// view of kaspad's mempool, select the same coin (the parent's
/// inputs) and the same destination (the parent's outputs), and
/// sign the replacement deterministically (Schnorr per RFC 6979).
/// The first daemon's `BumpFee` admits the replacement into
/// kaspad's mempool, evicting the parent; the second daemon's
/// `BumpFee` re-broadcasts the same replacement bytes, which
/// kaspad treats as a duplicate-tx no-op (the replacement is
/// already mempool-resident). The `BumpFeeResponse` carries the
/// same `tx_ids` and `transactions` payload on both sides.
///
/// The response carries a `Vec<String>` of replacement-tx ids and
/// a `Vec<Vec<u8>>` of replacement wire-format bytes (one element
/// per output split, one element total in the typical
/// single-output bump case). The byte-identity assertion compares
/// the vectors element-wise via `Vec::eq`, which the
/// prost-generated `Vec<bytes>` element type satisfies by
/// delegating to `Vec<u8>: PartialEq`.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus
/// [`harness::paired_daemons::ENV_BUMP_FEE_TX_ID`] (the hex parent
/// txid). Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn bump_fee_byte_identity() {
    let test_name = "bump_fee_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(parent_tx_id) = harness::paired_daemons::require_env_string(harness::paired_daemons::ENV_BUMP_FEE_TX_ID, test_name)
    else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let make_request = || kaspa_wallet_grpc_client::kaspawalletd::BumpFeeRequest {
        password: res.password.clone(),
        from: Vec::new(),
        use_existing_change_address: false,
        fee_policy: None,
        tx_id: parent_tx_id.clone(),
    };

    let under_test_response = under_test_client
        .bump_fee(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: BumpFee RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let legacy_port_response = legacy_port_client
        .bump_fee(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: BumpFee RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();

    assert_eq!(
        under_test_response.tx_ids,
        legacy_port_response.tx_ids,
        "parity::{test_name}: tx_ids diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.tx_ids.len(),
        legacy_port_response.tx_ids.len()
    );
    assert_eq!(
        under_test_response.transactions,
        legacy_port_response.transactions,
        "parity::{test_name}: transactions diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.transactions.len(),
        legacy_port_response.transactions.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Byte-identity of `BumpFee` between the under-test daemon and the
/// legacy-port daemon on the *unsigned* CLI flow. The architectural
/// disambiguation: `BumpFee` is a single proto RPC method; the
/// signed-vs-unsigned split lives at the client-CLI layer. The
/// `bump-fee` (signed) CLI calls `BumpFee`, locally signs the
/// response transactions, and broadcasts via `BroadcastReplacement`.
/// The `bump-fee-unsigned` CLI calls the same `BumpFee` with the
/// `password` field left at the empty string and prints the
/// response transactions as hex via `EncodeTransactionsToHex`,
/// with no local-sign step and no broadcast.
///
/// This row exercises the empty-password code path through the
/// daemon's `BumpFee` handler. Both daemons consult kaspad's
/// mempool for the parent transaction named by
/// [`harness::paired_daemons::ENV_BUMP_FEE_TX_ID`], resolve the
/// same coin set and the same destination set deterministically
/// (RFC 6979), and emit byte-identical replacement transaction
/// payloads. The `BumpFeeResponse` carries the same `tx_ids`
/// (`Vec<String>`) and `transactions` (`Vec<Vec<u8>>`) on both
/// sides; the byte-identity assertion compares the vectors
/// element-wise via `Vec::eq`, which the prost-generated
/// `Vec<bytes>` element type satisfies by delegating to
/// `Vec<u8>: PartialEq`.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus
/// [`harness::paired_daemons::ENV_BUMP_FEE_TX_ID`] (the hex parent
/// txid). Daemon binaries are located the same way as in
/// [`show_addresses_byte_identity`].
#[tokio::test]
async fn bump_fee_unsigned_byte_identity() {
    let test_name = "bump_fee_unsigned_byte_identity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(parent_tx_id) = harness::paired_daemons::require_env_string(harness::paired_daemons::ENV_BUMP_FEE_TX_ID, test_name)
    else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let make_request = || kaspa_wallet_grpc_client::kaspawalletd::BumpFeeRequest {
        password: String::new(),
        from: Vec::new(),
        use_existing_change_address: false,
        fee_policy: None,
        tx_id: parent_tx_id.clone(),
    };

    let under_test_response = under_test_client
        .bump_fee(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: BumpFee RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let legacy_port_response = legacy_port_client
        .bump_fee(make_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: BumpFee RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();

    assert_eq!(
        under_test_response.tx_ids,
        legacy_port_response.tx_ids,
        "parity::{test_name}: tx_ids diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.tx_ids.len(),
        legacy_port_response.tx_ids.len()
    );
    assert_eq!(
        under_test_response.transactions,
        legacy_port_response.transactions,
        "parity::{test_name}: transactions diverge -- under-test {} entries, legacy-port {} entries",
        under_test_response.transactions.len(),
        legacy_port_response.transactions.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Cross-binary interop of the unsigned-PSKT flow between the
/// under-test daemon and the legacy-port daemon, exercised in
/// both directions:
///
/// - **Direction A:** the legacy-port daemon emits an unsigned
///   PSKT via `BumpFee` (with `password = ""` per the architect
///   L1 disambiguation that places the signed-vs-unsigned split at
///   the client-CLI layer); the under-test daemon's `Sign` RPC
///   then signs that PSKT against the wallet's password.
/// - **Direction B:** the under-test daemon emits an unsigned PSKT
///   via the same `BumpFee` call; the legacy-port daemon's `Sign`
///   RPC then signs it.
///
/// Two byte-identity contracts compose to bind this row's
/// interop attestation:
///
/// 1. The two `BumpFeeResponse.transactions` payloads agree
///    byte-for-byte across the two daemons -- the same invariant
///    [`bump_fee_unsigned_byte_identity`] asserts at the
///    single-direction surface, restated here so the cross-binary
///    row stands on its own evidence and so a direction-A vs
///    direction-B divergence on the unsigned layer surfaces before
///    the cross-direction signing layer obscures it.
/// 2. The two `SignResponse.signed_transactions` payloads -- one
///    produced by direction A, one by direction B -- agree
///    byte-for-byte. Given contract 1 (the Sign inputs match) and
///    deterministic per-curve signing (RFC 6979 for ECDSA, BIP-340
///    with the same auxiliary-randomness contract for Schnorr),
///    the signed outputs must match too. A divergence here would
///    isolate a per-daemon signing-path defect, even when both
///    daemons see the same unsigned PSKT.
///
/// Both contracts together attest the spec's "cross-binary
/// interop" property: each daemon's unsigned-PSKT output is a
/// faithful input to the other daemon's signing path, and the
/// resulting signed transactions agree byte-for-byte, so an
/// operator-facing pipeline that crosses the daemon boundary
/// observes the same payload at every stage.
///
/// Skip-clean prerequisites: the five-variable
/// [`harness::paired_daemons`] topology set, plus
/// [`harness::paired_daemons::ENV_BUMP_FEE_TX_ID`] (the hex parent
/// txid both daemons bump). Daemon binaries are located the same
/// way as in [`show_addresses_byte_identity`].
#[tokio::test]
async fn bump_fee_unsigned_pskt_cross_binary_interop_both_directions() {
    let test_name = "bump_fee_unsigned_pskt_cross_binary_interop_both_directions";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let legacy_port_bin = match locate_legacy_port_binary() {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- legacy-port binary not found. Set {ENV_LEGACY_PORT_BIN} to its absolute path.");
            return;
        }
    };

    let Some(res) = harness::paired_daemons::resolve_resources(test_name) else {
        return;
    };

    let Some(parent_tx_id) = harness::paired_daemons::require_env_string(harness::paired_daemons::ENV_BUMP_FEE_TX_ID, test_name)
    else {
        return;
    };

    let under_test_args = under_test_daemon_argv(&res.password, &res.wallet_name, &res.wrpc_endpoint, res.under_test_listen);
    let under_test_arg_refs: Vec<&OsStr> = under_test_args.iter().map(AsRef::as_ref).collect();
    let legacy_port_args = legacy_port_daemon_argv(&res.password, &res.legacy_keyfile, &res.grpc_endpoint, res.legacy_port_listen);
    let legacy_port_arg_refs: Vec<&OsStr> = legacy_port_args.iter().map(AsRef::as_ref).collect();
    let (under_test, legacy_port) = harness::paired_daemons::spawn_pair(
        test_name,
        &daemon_bin,
        &legacy_port_bin,
        &under_test_arg_refs,
        &legacy_port_arg_refs,
        res.under_test_listen,
        res.legacy_port_listen,
        DAEMON_GRPC_LISTEN_TIMEOUT,
    );

    let under_test_grpc_endpoint = format!("http://{}", res.under_test_listen);
    let mut under_test_client =
        kaspa_wallet_grpc_client::connect(under_test_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to under-test {under_test_grpc_endpoint}: {e}"));

    let legacy_port_grpc_endpoint = format!("http://{}", res.legacy_port_listen);
    let mut legacy_port_client =
        kaspa_wallet_grpc_client::connect(legacy_port_grpc_endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
            .await
            .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to legacy-port {legacy_port_grpc_endpoint}: {e}"));

    let make_bump_fee_request = || kaspa_wallet_grpc_client::kaspawalletd::BumpFeeRequest {
        password: String::new(),
        from: Vec::new(),
        use_existing_change_address: false,
        fee_policy: None,
        tx_id: parent_tx_id.clone(),
    };

    let unsigned_from_legacy_port = legacy_port_client
        .bump_fee(make_bump_fee_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: BumpFee RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();
    let unsigned_from_under_test = under_test_client
        .bump_fee(make_bump_fee_request())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: BumpFee RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();

    assert_eq!(
        unsigned_from_legacy_port.transactions,
        unsigned_from_under_test.transactions,
        "parity::{test_name}: unsigned BumpFee payloads diverge -- legacy-port {} entries, under-test {} entries",
        unsigned_from_legacy_port.transactions.len(),
        unsigned_from_under_test.transactions.len()
    );

    let signed_direction_a = under_test_client
        .sign(kaspa_wallet_grpc_client::kaspawalletd::SignRequest {
            unsigned_transactions: unsigned_from_legacy_port.transactions.clone(),
            password: res.password.clone(),
        })
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: direction A Sign RPC against under-test {under_test_grpc_endpoint}: {e}"))
        .into_inner();
    let signed_direction_b = legacy_port_client
        .sign(kaspa_wallet_grpc_client::kaspawalletd::SignRequest {
            unsigned_transactions: unsigned_from_under_test.transactions.clone(),
            password: res.password.clone(),
        })
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: direction B Sign RPC against legacy-port {legacy_port_grpc_endpoint}: {e}"))
        .into_inner();

    assert!(!signed_direction_a.signed_transactions.is_empty(), "parity::{test_name}: direction A produced an empty signed payload");
    assert!(!signed_direction_b.signed_transactions.is_empty(), "parity::{test_name}: direction B produced an empty signed payload");
    assert_eq!(
        signed_direction_a.signed_transactions,
        signed_direction_b.signed_transactions,
        "parity::{test_name}: cross-direction signed payloads diverge -- direction A {} entries, direction B {} entries",
        signed_direction_a.signed_transactions.len(),
        signed_direction_b.signed_transactions.len()
    );

    drop(under_test);
    drop(legacy_port);
}

/// Liveness canary on the under-test wallet daemon's gRPC surface.
///
/// Composes the two B10c helpers (`local_kaspad::resolve` for the
/// kaspad endpoint dimension and `daemon_spawn::DaemonSpawn` for
/// the wallet-daemon lifecycle envelope) end-to-end: spawn kaspad,
/// spawn the wallet daemon against it, wait for the daemon's gRPC
/// bind, connect via [`kaspa_wallet_grpc_client`], call
/// [`GetVersion`] and assert the response carries a non-empty
/// version string. Proves the daemon-spawn topology the spec
/// section 7.3 names is wired end-to-end before the 12
/// daemon-requiring parity rows lift onto the same orchestration.
#[tokio::test]
async fn daemon_grpc_liveness_parity() {
    let test_name = "daemon_grpc_liveness_parity";

    let daemon_bin = match harness::daemon_spawn::locate_kaspawallet_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- kaspawallet binary not built. Set KASPAWALLET_BIN or run `cargo build --release -p kaspawallet`."
            );
            return;
        }
    };

    let password = match std::env::var(ENV_DAEMON_PASSWORD_FILE) {
        Ok(path) => match std::fs::read_to_string(&path) {
            Ok(contents) => contents.trim().to_owned(),
            Err(e) => {
                eprintln!("parity::{test_name}: SKIPPED -- read {ENV_DAEMON_PASSWORD_FILE} ({path}): {e}");
                return;
            }
        },
        Err(_) => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- {ENV_DAEMON_PASSWORD_FILE} not set (workstation lacks a provisioned wallet password file)."
            );
            return;
        }
    };

    let kaspad = match harness::local_kaspad::resolve() {
        Ok(Some(k)) => k,
        Ok(None) => return,
        Err(e) => {
            eprintln!("parity::{test_name}: SKIPPED -- kaspad resolution errored: {e}");
            return;
        }
    };

    if let harness::local_kaspad::SimnetOrTn10::LocallySpawned { .. } = &kaspad {
        let kaspad_addr = match parse_ws_endpoint(kaspad.endpoint()) {
            Ok(addr) => addr,
            Err(e) => panic!("parity::{test_name}: parse kaspad endpoint '{}': {e}", kaspad.endpoint()),
        };
        if let Err(e) = harness::daemon_spawn::wait_for_listen(kaspad_addr, KASPAD_RPC_LISTEN_TIMEOUT) {
            panic!("parity::{test_name}: spawned kaspad did not bind {kaspad_addr} within {KASPAD_RPC_LISTEN_TIMEOUT:?}: {e}");
        }
    }

    let daemon_listen =
        harness::daemon_spawn::reserve_ephemeral_loopback_addr().expect("reserve loopback port for daemon gRPC listen");
    let args: [OsString; 6] = [
        OsString::from("--password"),
        OsString::from(&password),
        OsString::from("--listen"),
        OsString::from(daemon_listen.to_string()),
        OsString::from("--rpc-server"),
        OsString::from(kaspad.endpoint()),
    ];
    let arg_refs: Vec<&OsStr> = args.iter().map(AsRef::as_ref).collect();
    let daemon = harness::daemon_spawn::DaemonSpawn::spawn(&daemon_bin, &arg_refs)
        .unwrap_or_else(|e| panic!("parity::{test_name}: spawn wallet daemon: {e}"));

    if let Err(e) = harness::daemon_spawn::wait_for_listen(daemon_listen, DAEMON_GRPC_LISTEN_TIMEOUT) {
        let tail = daemon.stderr_tail(4096).ok().unwrap_or_default();
        let tail_text = String::from_utf8_lossy(&tail);
        panic!(
            "parity::{test_name}: wallet daemon did not bind {daemon_listen} within {DAEMON_GRPC_LISTEN_TIMEOUT:?}: {e}\n--- daemon stderr tail ---\n{tail_text}"
        );
    }

    let endpoint = format!("http://{daemon_listen}");
    let mut client = kaspa_wallet_grpc_client::connect(endpoint.clone(), kaspa_wallet_grpc_client::ClientOptions::default())
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: gRPC connect to {endpoint}: {e}"));
    let response = client
        .get_version(kaspa_wallet_grpc_client::kaspawalletd::GetVersionRequest {})
        .await
        .unwrap_or_else(|e| panic!("parity::{test_name}: GetVersion RPC against {endpoint}: {e}"));
    let version = response.into_inner().version;
    assert!(
        !version.is_empty(),
        "parity::{test_name}: GetVersion returned empty version string -- daemon is reachable but the handler does not populate the response"
    );

    drop(daemon);
}

/// Strip the `ws://` scheme prefix and parse the remainder as a
/// `host:port` socket address. The endpoint string returned by
/// [`harness::local_kaspad::SimnetOrTn10::endpoint`] is normalised
/// to ws://-prefixed form by the helper, so the prefix is expected
/// but tolerated absent.
fn parse_ws_endpoint(endpoint: &str) -> Result<SocketAddr, std::net::AddrParseError> {
    endpoint.strip_prefix(WS_SCHEME_PREFIX).unwrap_or(endpoint).parse::<SocketAddr>()
}

/// Compose the argv vector the under-test daemon takes for a
/// testnet-10 paired-binary parity run. The daemon binds gRPC on
/// `listen`, opens the wallet-core wallet named `wallet_name` with
/// the literal `password` string, and dials kaspad via `wrpc_endpoint`
/// using the Borsh wRPC encoding the daemon's wrpc client expects.
fn under_test_daemon_argv(password: &str, wallet_name: &str, wrpc_endpoint: &str, listen: SocketAddr) -> Vec<OsString> {
    vec![
        OsString::from("--password"),
        OsString::from(password),
        OsString::from("--name"),
        OsString::from(wallet_name),
        OsString::from("--rpc-server"),
        OsString::from(wrpc_endpoint),
        OsString::from("--listen"),
        OsString::from(listen.to_string()),
        OsString::from("--network-id"),
        OsString::from("testnet-10"),
    ]
}

/// Compose the argv vector the legacy-port `kaspawallet` binary
/// takes to drive its `start-daemon` subcommand for the same
/// testnet-10 paired-binary parity run. The binary opens the
/// Go-format keyfile at `keyfile` and dials kaspad's gRPC surface
/// at `grpc_endpoint` (the legacy-port daemon's RPC client expects
/// gRPC, not wRPC; the two endpoints are passed via distinct
/// env variables canonical at `harness::paired_daemons`).
fn legacy_port_daemon_argv(password: &str, keyfile: &str, grpc_endpoint: &str, listen: SocketAddr) -> Vec<OsString> {
    vec![
        OsString::from("start-daemon"),
        OsString::from("--keys-file"),
        OsString::from(keyfile),
        OsString::from("--password"),
        OsString::from(password),
        OsString::from("--rpcserver"),
        OsString::from(grpc_endpoint),
        OsString::from("--listen"),
        OsString::from(listen.to_string()),
        OsString::from("--testnet"),
    ]
}

#[test]
fn legacy_port_rbf_parity_broadcast_replacement() {
    // Help-text framing parity for the `broadcast-replacement`
    // subcommand: both binaries advertise the same daemon-address +
    // signed-transaction flag literals. The live mempool round-trip
    // that compares the daemon's submitted-tx-id list across the
    // two binaries belongs to the daemon-spawn harness landing
    // later in the parity-test family; this row is the offline
    // probe that surfaces a divergence in the CLI surface before
    // the online matrix runs.
    let Some((legacy_port, under_test)) = resolve_legacy_port_pair("legacy_port_rbf_parity_broadcast_replacement") else {
        return;
    };
    let legacy_help = run_capture(&legacy_port, &["broadcast-replacement", "--help"]);
    let under_test_help = run_capture(&under_test, &["broadcast-replacement", "--help"]);
    let legacy_text = String::from_utf8_lossy(&legacy_help);
    let under_test_text = String::from_utf8_lossy(&under_test_help);
    for flag_literal in ["daemonaddress", "transaction", "transaction-file"] {
        assert!(
            legacy_text.contains(flag_literal),
            "legacy-port broadcast-replacement help-text must mention --{flag_literal}: {legacy_text}"
        );
        assert!(
            under_test_text.contains(flag_literal),
            "under-test broadcast-replacement help-text must mention --{flag_literal}: {under_test_text}"
        );
    }
}

#[test]
fn legacy_port_rbf_parity_bump_fee() {
    // Help-text framing parity for the local-signing `bump-fee`
    // subcommand: both binaries advertise the keyfile-resolution
    // and replacement-tx flag set the legacy port shipped (txid,
    // keys-file, password, daemonaddress, fee-rate, max-fee,
    // from-address, use-existing-change-address, show-serialized).
    // The end-to-end mempool replacement that compares the signed
    // replacement-tx-id list belongs to the daemon-spawn harness.
    let Some((legacy_port, under_test)) = resolve_legacy_port_pair("legacy_port_rbf_parity_bump_fee") else {
        return;
    };
    let legacy_help = run_capture(&legacy_port, &["bump-fee", "--help"]);
    let under_test_help = run_capture(&under_test, &["bump-fee", "--help"]);
    let legacy_text = String::from_utf8_lossy(&legacy_help);
    let under_test_text = String::from_utf8_lossy(&under_test_help);
    for flag_literal in [
        "txid",
        "keys-file",
        "password",
        "daemonaddress",
        "fee-rate",
        "max-fee",
        "from-address",
        "use-existing-change-address",
        "show-serialized",
    ] {
        assert!(legacy_text.contains(flag_literal), "legacy-port bump-fee help-text must mention --{flag_literal}: {legacy_text}");
        assert!(
            under_test_text.contains(flag_literal),
            "under-test bump-fee help-text must mention --{flag_literal}: {under_test_text}"
        );
    }
}

#[test]
fn legacy_port_rbf_parity_bump_fee_unsigned() {
    // Help-text framing parity for the `bump-fee-unsigned`
    // subcommand: both binaries advertise the daemon-connect +
    // replacement-tx flag set the unsigned-CLI flow requires
    // (txid, daemonaddress, fee-rate, max-fee, from-address,
    // use-existing-change-address). The unsigned flow does not
    // take a password or a keyfile (the daemon returns the
    // replacement transaction(s) as hex; the operator handles
    // signing and broadcasting separately), so neither binary's
    // help-text should advertise --password or --keys-file in
    // this subcommand's argument set. The end-to-end byte-identity
    // of the daemon's `BumpFee` RPC under the unsigned CLI's
    // empty-password path is covered by the
    // `bump_fee_unsigned_byte_identity` and
    // `bump_fee_unsigned_pskt_cross_binary_interop_both_directions`
    // paired rows above.
    let Some((legacy_port, under_test)) = resolve_legacy_port_pair("legacy_port_rbf_parity_bump_fee_unsigned") else {
        return;
    };
    let legacy_help = run_capture(&legacy_port, &["bump-fee-unsigned", "--help"]);
    let under_test_help = run_capture(&under_test, &["bump-fee-unsigned", "--help"]);
    let legacy_text = String::from_utf8_lossy(&legacy_help);
    let under_test_text = String::from_utf8_lossy(&under_test_help);
    for flag_literal in ["txid", "daemonaddress", "fee-rate", "max-fee", "from-address", "use-existing-change-address"] {
        assert!(
            legacy_text.contains(flag_literal),
            "legacy-port bump-fee-unsigned help-text must mention --{flag_literal}: {legacy_text}"
        );
        assert!(
            under_test_text.contains(flag_literal),
            "under-test bump-fee-unsigned help-text must mention --{flag_literal}: {under_test_text}"
        );
    }
    for absent in ["password", "keys-file"] {
        assert!(
            !legacy_text.contains(absent),
            "legacy-port bump-fee-unsigned help-text must not mention --{absent} (unsigned flow takes no password / keyfile): {legacy_text}"
        );
        assert!(
            !under_test_text.contains(absent),
            "under-test bump-fee-unsigned help-text must not mention --{absent} (unsigned flow takes no password / keyfile): {under_test_text}"
        );
    }
}

#[test]
fn legacy_port_rbf_parity_get_daemon_version() {
    // Help-text framing parity for `get-daemon-version`: both
    // binaries declare only the `--daemonaddress` flag and emit
    // no network-flag block (the daemon's reported version is
    // network-agnostic). The live round-trip that compares the
    // daemon-returned version string belongs to the daemon-spawn
    // harness; this row guards against an accidental flag-surface
    // departure (e.g. the network flags creeping in) before the
    // online matrix runs.
    let Some((legacy_port, under_test)) = resolve_legacy_port_pair("legacy_port_rbf_parity_get_daemon_version") else {
        return;
    };
    let legacy_help = run_capture(&legacy_port, &["get-daemon-version", "--help"]);
    let under_test_help = run_capture(&under_test, &["get-daemon-version", "--help"]);
    let legacy_text = String::from_utf8_lossy(&legacy_help);
    let under_test_text = String::from_utf8_lossy(&under_test_help);
    assert!(
        legacy_text.contains("daemonaddress"),
        "legacy-port get-daemon-version help-text must mention --daemonaddress: {legacy_text}"
    );
    assert!(
        under_test_text.contains("daemonaddress"),
        "under-test get-daemon-version help-text must mention --daemonaddress: {under_test_text}"
    );
    for absent in ["testnet", "simnet", "devnet"] {
        assert!(
            !legacy_text.contains(absent),
            "legacy-port get-daemon-version help-text must not mention --{absent} (network-agnostic surface): {legacy_text}"
        );
        assert!(
            !under_test_text.contains(absent),
            "under-test get-daemon-version help-text must not mention --{absent} (network-agnostic surface): {under_test_text}"
        );
    }
}

#[test]
fn get_daemon_version_help_framing_parity_with_reference() {
    // The `get-daemon-version` subcommand has no client-side
    // observable surface without a running daemon. The help-text
    // framing (subcommand name + the `--daemonaddress` flag
    // presence) is the closest deterministic parity probe
    // available offline.
    let Some((reference, under_test)) = resolve_reference_pair("get_daemon_version_help_framing_parity_with_reference") else {
        return;
    };
    let reference_help = run_capture(&reference, &["get-daemon-version", "--help"]);
    let under_test_help = run_capture(&under_test, &["get-daemon-version", "--help"]);
    let reference_text = String::from_utf8_lossy(&reference_help);
    let under_test_text = String::from_utf8_lossy(&under_test_help);
    assert!(reference_text.contains("daemonaddress"), "reference help-text must mention --daemonaddress: {reference_text}");
    assert!(under_test_text.contains("daemonaddress"), "under-test help-text must mention --daemonaddress: {under_test_text}");
}

// ----------------------------------------------------------------
// Locator smoke tests. These run unconditionally and guard the
// harness wiring itself: if the binary-resolution logic regresses
// (e.g. workspace layout shifts, env-var typo), these surface the
// break before a downstream parity row mis-skips silently.
// ----------------------------------------------------------------

#[test]
fn locator_under_test_resolves_or_skips_cleanly() {
    match locate_under_test_binary() {
        Some(path) => {
            assert!(is_executable_file(&path), "resolved under-test path is not executable: {}", path.display());
        }
        None => {
            eprintln!("locator_under_test: under-test binary not built -- skip is the expected outcome on a clean checkout");
        }
    }
}

#[test]
fn locator_reference_resolves_or_skips_cleanly() {
    match locate_reference_binary() {
        Some(path) => {
            assert!(is_executable_file(&path), "resolved reference path is not executable: {}", path.display());
        }
        None => {
            eprintln!("locator_reference: reference binary absent on this workstation -- skip is the expected outcome");
        }
    }
}

#[test]
fn locator_legacy_port_resolves_or_skips_cleanly() {
    match locate_legacy_port_binary() {
        Some(path) => {
            assert!(is_executable_file(&path), "resolved legacy-port path is not executable: {}", path.display());
        }
        None => {
            eprintln!("locator_legacy_port: legacy-port binary absent on this workstation -- skip is the expected outcome");
        }
    }
}
