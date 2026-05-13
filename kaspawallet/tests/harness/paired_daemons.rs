//! Paired-daemon orchestration for cross-binary parity rows that
//! exercise the under-test wallet daemon against the legacy-port
//! `kaspawallet start-daemon` against the same kaspad.
//!
//! Each row pairs two `DaemonSpawn` lifecycles around the same gRPC
//! contract: it spawns two daemons on distinct loopback ports, waits
//! for both to bind, issues the row-specific RPCs via two
//! `kaspa_wallet_grpc_client` connections, and asserts byte-identity
//! of the response payloads. The duplication between such rows is
//! the env-var resolution (five workstation variables guard a clean
//! skip when fixtures are absent) plus the spawn + listen-wait +
//! stderr-tail-on-failure envelope; this module factors out that
//! boilerplate so a parity row body retains only its row-specific
//! RPC sequence + assertion.
//!
//! Connect setup stays inline at the call site so the harness does
//! not need to express the tonic `InterceptedService<Channel,
//! AuthInterceptor>` generics. The two `kaspa_wallet_grpc_client::
//! connect` calls are eight lines per row -- not load-bearing
//! deduplication, and inlining keeps the helper free of a
//! tonic-types coupling that otherwise leaks the wallet-grpc-client
//! crate's transport stack into every test-harness consumer.

use std::ffi::OsStr;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use crate::harness::daemon_spawn::{self, DaemonSpawn};

/// Environment variable naming a file whose contents are the wallet
/// password the daemons open their stores with. Both daemons take
/// the literal password string via `--password`; the file
/// indirection keeps the password out of the test runner's own
/// env-var dump (where a shared workstation's `env` listing would
/// otherwise leak it). The literal contents still appear on the
/// spawned daemon's argv -- a `ps` snapshot on the test host reveals
/// them -- so the indirection is a soft mitigation, not a secrecy
/// guarantee.
pub const ENV_DAEMON_PASSWORD_FILE: &str = "KASPAWALLETD_TEST_PASSWORD_FILE";

/// Environment variable naming the wallet-core wallet the under-test
/// daemon opens via `--name`. The Validator-workstation provisioning
/// step creates this wallet from a mnemonic shared with the
/// legacy-port keyfile so the two daemons derive identical address
/// lists.
pub const ENV_UNDER_TEST_WALLET_NAME: &str = "KASPAWALLETD_TEST_WALLET_NAME";

/// Environment variable naming the Go-format keyfile the legacy-port
/// daemon opens via `--keys-file`. Provisioned alongside
/// [`ENV_UNDER_TEST_WALLET_NAME`] over the same mnemonic.
pub const ENV_LEGACY_PORT_KEYFILE: &str = "KASPAWALLET_LEGACY_PORT_KEYFILE";

/// Environment variable naming the kaspad wRPC endpoint the under-test
/// daemon's `--rpc-server` consumes (Borsh-encoded JSON-RPC).
pub const ENV_KASPAD_WRPC_ENDPOINT: &str = "KASPA_TN10_ENDPOINT";

/// Environment variable naming the kaspad gRPC `host:port` the
/// legacy-port daemon's `--rpcserver` consumes. The legacy-port
/// daemon dials kaspad's gRPC surface, whereas the under-test daemon
/// dials kaspad's wRPC Borsh surface; the two protocols listen on
/// different ports, so the test resolves them via distinct variables.
pub const ENV_KASPAD_GRPC_ENDPOINT: &str = "KASPAD_GRPC_ENDPOINT";

/// Environment variable naming a file whose raw bytes are the
/// pre-signed transaction the `broadcast_byte_identity` paired row
/// submits to both daemons. The bytes are the wire-format payload
/// the `BroadcastRequest.transactions` field carries (proto
/// `repeated bytes`); a single-element vector is built at the call
/// site.
pub const ENV_PRESIGNED_TX_FIXTURE: &str = "KASPAWALLETD_TEST_PRESIGNED_TX";

/// Environment variable naming a file whose raw bytes are the
/// pre-signed replacement transaction the
/// `broadcast_replacement_byte_identity` paired row submits to both
/// daemons via the `BroadcastReplacement` RPC. Distinct from
/// [`ENV_PRESIGNED_TX_FIXTURE`] because the replacement-tx
/// semantics require a transaction that conflicts with an existing
/// mempool entry (the fee-bumped replacement spends an input that
/// the prior in-mempool transaction also spends), not a fresh-spend
/// transaction.
pub const ENV_PRESIGNED_REPLACEMENT_TX_FIXTURE: &str = "KASPAWALLETD_TEST_PRESIGNED_REPLACEMENT_TX";

/// Environment variable naming a kaspa address literal both daemons
/// take as the `to_address` / `address` argument to fresh-spend RPCs
/// (`Send`, `CreateUnsignedTransactions`). The address must be valid
/// on the network the paired kaspad serves and must NOT be one of
/// the wallet's own addresses -- self-spend would muddy the
/// byte-identity comparison via the change-detection branch.
pub const ENV_SEND_TO_ADDRESS: &str = "KASPAWALLETD_TEST_SEND_TO_ADDRESS";

/// Environment variable naming a u64 sompi amount both daemons take
/// as the `amount` argument to fresh-spend RPCs. The amount must be
/// payable from the paired wallet's funded UTXO set after the
/// fee-rate-imposed minimum-fee residue. The fixture-provisioning
/// step ensures the wallet's UTXO set is single-UTXO so coin
/// selection is deterministic (single-element selection on both
/// daemons) and the resulting unsigned-tx bytes are byte-identical.
pub const ENV_SEND_AMOUNT_SOMPI: &str = "KASPAWALLETD_TEST_SEND_AMOUNT_SOMPI";

/// Environment variable naming the hex transaction id of a kaspad
/// mempool-resident transaction the `bump_fee_byte_identity` paired
/// row asks both daemons to bump-fee. The fixture-provisioning step
/// submits a low-fee parent transaction from the paired wallet to
/// kaspad and exports its txid through this variable; both daemons
/// then build a replacement transaction off the same parent. The
/// replacement signing is deterministic (Schnorr per RFC 6979), so
/// the resulting `BumpFeeResponse.transactions` bytes are byte-
/// identical on both sides as long as both daemons see the same
/// parent in their respective views of kaspad's mempool when the
/// RPC is issued (the second daemon's `BumpFee` re-broadcasts the
/// same replacement, which kaspad accepts as a duplicate-tx no-op
/// just like the `send_byte_identity_single_utxo` row).
pub const ENV_BUMP_FEE_TX_ID: &str = "KASPAWALLETD_TEST_BUMP_FEE_TX_ID";

/// Environment variable naming a file whose raw bytes are an
/// unsigned PSTX whose inputs require ECDSA-curve signatures (the
/// fixture-provisioning step produces it from the ECDSA singlekey
/// keyfile both paired daemons share). The
/// `sign_ecdsa_byte_identity_and_schnorr_validity` paired row sends
/// the file's bytes as the single element of
/// `SignRequest.unsigned_transactions` to both daemons. ECDSA
/// signing is deterministic per RFC 6979, so when both daemons see
/// the same wallet view and the same unsigned input, their
/// `SignResponse.signed_transactions` payloads agree byte-for-byte
/// -- the byte-identity attestation simultaneously witnesses
/// signature validity, because two daemons producing the same
/// signature bytes on the same input either both produce a valid
/// signature or both produce an invalid one (no signing-path
/// nondeterminism can hide a defect on one side).
pub const ENV_UNSIGNED_ECDSA_PSTX_FIXTURE: &str = "KASPAWALLETD_TEST_UNSIGNED_ECDSA_PSTX";

/// Resolved per-test resources shared by both daemons in a paired
/// parity row. Field order follows the resolution order in
/// [`resolve_resources`] so a reader can trace the skip-clean
/// prerequisite probes against the struct layout.
#[derive(Debug)]
pub struct PairResources {
    /// Loopback `SocketAddr` the under-test daemon binds via
    /// `--listen`. Distinct from [`Self::legacy_port_listen`].
    pub under_test_listen: SocketAddr,
    /// Loopback `SocketAddr` the legacy-port daemon binds via
    /// `--listen`. Distinct from [`Self::under_test_listen`].
    pub legacy_port_listen: SocketAddr,
    /// Literal wallet password string (the contents of the file at
    /// [`ENV_DAEMON_PASSWORD_FILE`] with trailing whitespace
    /// stripped). Passed to both daemons via their respective
    /// `--password` flags.
    pub password: String,
    /// Wallet-core wallet name the under-test daemon opens via
    /// `--name`. Sourced from [`ENV_UNDER_TEST_WALLET_NAME`].
    pub wallet_name: String,
    /// Go-format keyfile path the legacy-port daemon opens via
    /// `--keys-file`. Sourced from [`ENV_LEGACY_PORT_KEYFILE`].
    pub legacy_keyfile: String,
    /// kaspad wRPC endpoint URL or `host:port` the under-test
    /// daemon's `--rpc-server` consumes. Sourced from
    /// [`ENV_KASPAD_WRPC_ENDPOINT`].
    pub wrpc_endpoint: String,
    /// kaspad gRPC `host:port` the legacy-port daemon's
    /// `--rpcserver` consumes. Sourced from [`ENV_KASPAD_GRPC_ENDPOINT`].
    pub grpc_endpoint: String,
}

/// Resolve the per-test resources a paired parity row needs.
///
/// Probes the five workstation env vars + reserves two ephemeral
/// loopback addresses. On any prerequisite gap, emits a one-line
/// stderr warning naming exactly the absent variable and returns
/// `None`; the caller returns early so the test surface stays green
/// on a workstation that has not been provisioned.
///
/// `test_name` is the bare identifier printed in skip warnings (no
/// `parity::` prefix -- this module is parity-test private; the
/// crate prefix is implied).
pub fn resolve_resources(test_name: &str) -> Option<PairResources> {
    let password = match read_trimmed_file_env(ENV_DAEMON_PASSWORD_FILE) {
        Ok(Some(p)) => p,
        Ok(None) => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- {ENV_DAEMON_PASSWORD_FILE} not set (workstation lacks a provisioned wallet password file)."
            );
            return None;
        }
        Err(e) => {
            eprintln!("parity::{test_name}: SKIPPED -- {e}");
            return None;
        }
    };

    let wallet_name = match nonempty_env(ENV_UNDER_TEST_WALLET_NAME) {
        Some(v) => v,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- {ENV_UNDER_TEST_WALLET_NAME} not set (workstation lacks a provisioned wallet-core wallet)."
            );
            return None;
        }
    };

    let legacy_keyfile = match nonempty_env(ENV_LEGACY_PORT_KEYFILE) {
        Some(v) => v,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- {ENV_LEGACY_PORT_KEYFILE} not set (workstation lacks a provisioned legacy-port keyfile)."
            );
            return None;
        }
    };

    let wrpc_endpoint = match nonempty_env(ENV_KASPAD_WRPC_ENDPOINT) {
        Some(v) => v,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- {ENV_KASPAD_WRPC_ENDPOINT} not set (no kaspad wRPC endpoint for the under-test daemon)."
            );
            return None;
        }
    };

    let grpc_endpoint = match nonempty_env(ENV_KASPAD_GRPC_ENDPOINT) {
        Some(v) => v,
        None => {
            eprintln!(
                "parity::{test_name}: SKIPPED -- {ENV_KASPAD_GRPC_ENDPOINT} not set (no kaspad gRPC endpoint for the legacy-port daemon)."
            );
            return None;
        }
    };

    let under_test_listen = daemon_spawn::reserve_ephemeral_loopback_addr().expect("reserve loopback port for under-test gRPC listen");
    let legacy_port_listen =
        daemon_spawn::reserve_ephemeral_loopback_addr().expect("reserve loopback port for legacy-port gRPC listen");

    Some(PairResources { under_test_listen, legacy_port_listen, password, wallet_name, legacy_keyfile, wrpc_endpoint, grpc_endpoint })
}

/// Spawn both daemons against their pre-composed argv vectors and
/// wait for both to bind their respective loopback listen addresses.
/// On listen-wait failure, panics with the offending listen address
/// and a tail of the failing daemon's stderr so the reader can
/// distinguish a stuck under-test daemon from a stuck legacy-port
/// daemon at a glance.
///
/// `listen_timeout` is the per-daemon budget; the function spends
/// up to `2 * listen_timeout` total in the worst case (sequential
/// listen-wait on both daemons).
pub fn spawn_pair(
    test_name: &str,
    daemon_bin: &Path,
    legacy_port_bin: &Path,
    under_test_args: &[&OsStr],
    legacy_port_args: &[&OsStr],
    under_test_listen: SocketAddr,
    legacy_port_listen: SocketAddr,
    listen_timeout: Duration,
) -> (DaemonSpawn, DaemonSpawn) {
    let under_test = DaemonSpawn::spawn(daemon_bin, under_test_args)
        .unwrap_or_else(|e| panic!("parity::{test_name}: spawn under-test wallet daemon: {e}"));

    let legacy_port = DaemonSpawn::spawn(legacy_port_bin, legacy_port_args)
        .unwrap_or_else(|e| panic!("parity::{test_name}: spawn legacy-port wallet daemon: {e}"));

    if let Err(e) = daemon_spawn::wait_for_listen(under_test_listen, listen_timeout) {
        let tail = under_test.stderr_tail(STDERR_TAIL_BYTES).ok().unwrap_or_default();
        let tail_text = String::from_utf8_lossy(&tail);
        panic!(
            "parity::{test_name}: under-test wallet daemon did not bind {under_test_listen} within {listen_timeout:?}: {e}\n--- under-test stderr tail ---\n{tail_text}"
        );
    }

    if let Err(e) = daemon_spawn::wait_for_listen(legacy_port_listen, listen_timeout) {
        let tail = legacy_port.stderr_tail(STDERR_TAIL_BYTES).ok().unwrap_or_default();
        let tail_text = String::from_utf8_lossy(&tail);
        panic!(
            "parity::{test_name}: legacy-port wallet daemon did not bind {legacy_port_listen} within {listen_timeout:?}: {e}\n--- legacy-port stderr tail ---\n{tail_text}"
        );
    }

    (under_test, legacy_port)
}

/// Tail size for stderr panic-message capture. Generous enough for a
/// few hundred kilobytes of structured log output; bounded so a
/// runaway daemon's stderr does not turn the panic message into a
/// multi-megabyte wall of text.
const STDERR_TAIL_BYTES: usize = 4096;

/// Read a fixture file whose path is named by environment variable
/// `env`. Emits a one-line skip warning when the env var is
/// unset/empty or the file is unreadable, and returns `None`. On
/// success returns the file contents as raw bytes. The skip-clean
/// envelope mirrors the five topology probes inside
/// [`resolve_resources`]: an absent per-row fixture takes the row
/// off the active test surface on this workstation rather than
/// panicking.
pub fn require_fixture_bytes(env: &str, test_name: &str) -> Option<Vec<u8>> {
    let path = match nonempty_env(env) {
        Some(p) => p,
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- {env} not set (no fixture provisioned for this row).");
            return None;
        }
    };
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            eprintln!("parity::{test_name}: SKIPPED -- read {env} ({path}): {e}");
            None
        }
    }
}

/// Read an environment variable's literal value, emitting a skip
/// warning when the variable is unset or empty. Mirrors
/// [`require_fixture_bytes`] for string-typed per-row prerequisites
/// (an address literal, a hex transaction id).
pub fn require_env_string(env: &str, test_name: &str) -> Option<String> {
    match nonempty_env(env) {
        Some(v) => Some(v),
        None => {
            eprintln!("parity::{test_name}: SKIPPED -- {env} not set (no value provisioned for this row).");
            None
        }
    }
}

/// Read an environment variable's value and parse it as a u64.
/// Emits a skip warning when the variable is unset/empty or the
/// parse fails.
pub fn require_env_u64(env: &str, test_name: &str) -> Option<u64> {
    let raw = require_env_string(env, test_name)?;
    match raw.parse::<u64>() {
        Ok(n) => Some(n),
        Err(e) => {
            eprintln!("parity::{test_name}: SKIPPED -- {env}='{raw}' is not a valid u64: {e}");
            None
        }
    }
}

fn nonempty_env(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

fn read_trimmed_file_env(name: &str) -> Result<Option<String>, String> {
    let path = match std::env::var(name) {
        Ok(p) if !p.is_empty() => p,
        _ => return Ok(None),
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => Ok(Some(contents.trim().to_owned())),
        Err(e) => Err(format!("read {name} ({path}): {e}")),
    }
}
