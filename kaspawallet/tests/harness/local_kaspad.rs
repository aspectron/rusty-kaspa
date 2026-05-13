//! Resolve a `kaspad` RPC endpoint the wallet daemon connects to.
//!
//! The wallet daemon's `--rpc-server` flag (`wallet/daemon/src/args.rs:55`)
//! takes a wRPC URL the daemon hands to the wRPC client at startup
//! (`wallet/daemon/src/lib.rs:60`). Daemon-targeted parity tests therefore
//! need a reachable kaspad before the wallet daemon can pass its own
//! startup. This module sources that kaspad from one of two places:
//!
//! - `KASPAD_RPC_BIN` -- an executable path that, when set, names a local
//!   kaspad binary the harness spawns on a kernel-allocated loopback port.
//!   The spawn handle rides on the returned value so dropping the value
//!   reaps the child. Lifecycle is owned by the harness.
//!
//! - `KASPA_TN10_ENDPOINT` -- a `host:port` (with optional `ws://` prefix)
//!   that, when set, names an externally-managed kaspad the harness only
//!   probes for TCP reachability. No spawn, no reap.
//!
//! When neither variable is set, the resolver emits a skip-with-warning on
//! stderr and returns `None`; callers in subsequent batches treat that as
//! "skip-clean" so the parity test surface stays green on a workstation
//! that has neither a local kaspad build nor a remote testnet endpoint.
//!
//! The helper does not call [`wait_for_listen`] against the kaspad it
//! spawns -- waiting on the kaspad bind is a consumer concern with its
//! own timeout posture (a cold kaspad startup can run into seconds). The
//! caller composes `resolve` + `wait_for_listen` when readiness matters.

use std::ffi::{OsStr, OsString};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use super::daemon_spawn::{DaemonSpawn, reserve_ephemeral_loopback_addr};

/// Environment variable naming a local kaspad binary the harness may
/// spawn. Takes precedence over the remote-endpoint variable so a
/// developer with a local build is not forced to probe a public
/// testnet for every parity-row run.
const ENV_KASPAD_RPC_BIN: &str = "KASPAD_RPC_BIN";

/// Environment variable naming a reachable kaspad endpoint the harness
/// only probes (TCP-connect) without spawning anything. Accepts either
/// a `host:port` literal or a `ws://host:port` URL; the scheme prefix
/// is stripped before the TCP-probe.
const ENV_KASPA_TN10_ENDPOINT: &str = "KASPA_TN10_ENDPOINT";

/// `ws://` prefix recognised on the `KASPA_TN10_ENDPOINT` value. The
/// wRPC client accepts `ws://`-prefixed URLs (and bare host:port); the
/// helper accepts both forms and normalises the probe target to the
/// host:port portion.
const WS_SCHEME_PREFIX: &str = "ws://";

/// Per-attempt timeout for the `KASPA_TN10_ENDPOINT` TCP probe. The
/// probe is a single connect attempt -- a remote endpoint that is not
/// reachable within this budget is reported as an error, not silently
/// re-tried, so a misconfigured environment surfaces loudly.
const TN10_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// Endpoint of the kaspad RPC the wallet daemon connects to. Either a
/// locally-spawned kaspad (with its lifecycle handle held by the
/// harness) or a reference to an externally-managed remote.
#[derive(Debug)]
pub enum SimnetOrTn10 {
    /// Locally-spawned kaspad. The [`DaemonSpawn`] handle reaps the
    /// child on `Drop`. The endpoint is the wRPC URL the wallet
    /// daemon's `--rpc-server` flag consumes.
    LocallySpawned { spawn: DaemonSpawn, endpoint: String },

    /// External kaspad reachable at the endpoint. No process to reap.
    Remote { endpoint: String },
}

impl SimnetOrTn10 {
    /// The wRPC URL the wallet daemon's `--rpc-server` flag consumes.
    pub fn endpoint(&self) -> &str {
        match self {
            SimnetOrTn10::LocallySpawned { endpoint, .. } => endpoint,
            SimnetOrTn10::Remote { endpoint } => endpoint,
        }
    }
}

/// Resolve a kaspad endpoint from environment configuration.
///
/// Returns `Ok(None)` when neither variable is set; the caller should
/// treat that as a clean skip. Returns `Err` when a variable is set
/// but the underlying check fails (binary missing or non-executable,
/// remote endpoint unparseable or unreachable). Returns
/// `Ok(Some(SimnetOrTn10::*))` on the happy paths.
pub fn resolve() -> io::Result<Option<SimnetOrTn10>> {
    resolve_from(std::env::var_os(ENV_KASPAD_RPC_BIN), std::env::var_os(ENV_KASPA_TN10_ENDPOINT))
}

/// Variant of [`resolve`] that takes the env-var values as explicit
/// inputs. Tests use this form to avoid the process-global env-var
/// state that would otherwise race across `cargo nextest` parallel
/// workers.
pub(crate) fn resolve_from(
    kaspad_rpc_bin: Option<OsString>,
    kaspa_tn10_endpoint: Option<OsString>,
) -> io::Result<Option<SimnetOrTn10>> {
    if let Some(bin) = kaspad_rpc_bin {
        return spawn_local(Path::new(&bin)).map(Some);
    }
    if let Some(endpoint) = kaspa_tn10_endpoint {
        return probe_remote(&endpoint).map(Some);
    }
    eprintln!(
        "local_kaspad::resolve: neither {ENV_KASPAD_RPC_BIN} nor {ENV_KASPA_TN10_ENDPOINT} set -- daemon-targeted parity rows will skip clean"
    );
    Ok(None)
}

/// Spawn a local kaspad on a kernel-allocated loopback port and return
/// the spawn handle plus the wRPC URL the wallet daemon consumes.
///
/// Kaspad CLI surface verified at `kaspad/src/args.rs`:
/// - line 360: `--simnet` -- the simulation network (no peers required).
/// - line 249: `--rpclisten-borsh=IP:PORT` -- the wRPC Borsh listen
///   socket, which matches the wallet daemon's wRPC client connect
///   semantics at `wallet/daemon/src/lib.rs:60-76`.
fn spawn_local(bin: &Path) -> io::Result<SimnetOrTn10> {
    if !is_executable_file(bin) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{ENV_KASPAD_RPC_BIN} -> {} is not an executable file", bin.display()),
        ));
    }
    let listen_addr = reserve_ephemeral_loopback_addr()?;
    let args: [OsString; 2] =
        [OsString::from("--simnet"), OsString::from(format!("--rpclisten-borsh=127.0.0.1:{}", listen_addr.port()))];
    let arg_refs: Vec<&OsStr> = args.iter().map(AsRef::as_ref).collect();
    let spawn = DaemonSpawn::spawn(bin, &arg_refs)?;
    Ok(SimnetOrTn10::LocallySpawned { spawn, endpoint: format!("{WS_SCHEME_PREFIX}127.0.0.1:{}", listen_addr.port()) })
}

/// Parse the `KASPA_TN10_ENDPOINT` value and TCP-probe the resulting
/// host:port. The probe is one connect attempt with
/// [`TN10_PROBE_TIMEOUT`] as the deadline; the helper does not retry.
fn probe_remote(raw: &OsStr) -> io::Result<SimnetOrTn10> {
    let raw_str = raw
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{ENV_KASPA_TN10_ENDPOINT} value is not valid UTF-8")))?;
    let host_port = raw_str.strip_prefix(WS_SCHEME_PREFIX).unwrap_or(raw_str);
    let socket_addr = host_port.parse::<SocketAddr>().map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{ENV_KASPA_TN10_ENDPOINT} -> {raw_str} not parseable as host:port: {e}"))
    })?;
    TcpStream::connect_timeout(&socket_addr, TN10_PROBE_TIMEOUT).map_err(|e| {
        io::Error::new(e.kind(), format!("{ENV_KASPA_TN10_ENDPOINT} -> {raw_str} unreachable within {TN10_PROBE_TIMEOUT:?}: {e}"))
    })?;
    let endpoint = if raw_str.starts_with(WS_SCHEME_PREFIX) { raw_str.to_owned() } else { format!("{WS_SCHEME_PREFIX}{raw_str}") };
    Ok(SimnetOrTn10::Remote { endpoint })
}

/// Test whether `path` names a present, executable regular file. Same
/// semantics as the sibling helper in [`super::daemon_spawn`]; kept
/// local rather than re-exported to avoid widening that module's
/// public surface for a single internal consumer.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn resolve_from_returns_none_when_neither_env_set() {
        let outcome = resolve_from(None, None).expect("resolve_from with no env should not error");
        assert!(outcome.is_none(), "expected None for unset-env outcome");
    }

    /// Sanity-track the thin env-reading public wrapper. The outcome
    /// depends on the runtime env so the assertion only verifies the
    /// call returns without panicking; the substantive resolution
    /// matrix is covered by the `resolve_from_*` tests above.
    #[test]
    fn resolve_reads_process_env_without_panic() {
        let _ = resolve();
    }

    #[test]
    fn resolve_from_local_errs_when_bin_is_not_executable() {
        let bogus = OsString::from("/does/not/exist/kaspad-bogus");
        let res = resolve_from(Some(bogus), None);
        let err = res.expect_err("non-executable KASPAD_RPC_BIN should be an error");
        assert_eq!(err.kind(), io::ErrorKind::NotFound, "expected NotFound, got {err:?}");
    }

    /// Drive the local-spawn path against `/bin/sleep`. Sleep rejects
    /// the kaspad CLI flags and exits immediately with non-zero; the
    /// `Child::wait` reap inside `DaemonSpawn::drop` still completes
    /// cleanly. The assertion verifies the resolution-dispatch path
    /// picks the local branch, the spawn lifecycle envelope holds, and
    /// the constructed endpoint string carries the `ws://` scheme the
    /// wallet daemon's wRPC client expects.
    #[test]
    fn resolve_from_local_returns_locally_spawned_against_stand_in() {
        let sleep_bin = Path::new("/bin/sleep");
        if !sleep_bin.is_file() {
            eprintln!("resolve_from_local_returns_locally_spawned_against_stand_in: /bin/sleep not present -- skipping");
            return;
        }
        let outcome =
            resolve_from(Some(OsString::from(sleep_bin)), None).expect("resolve_from with KASPAD_RPC_BIN=/bin/sleep should not error");
        let kaspad = outcome.expect("expected Some(SimnetOrTn10::LocallySpawned)");
        match kaspad {
            SimnetOrTn10::LocallySpawned { spawn, endpoint } => {
                assert!(spawn.pid().is_some(), "spawn handle should expose a pid immediately after construction");
                assert!(endpoint.starts_with(WS_SCHEME_PREFIX), "expected ws:// scheme prefix on endpoint, got {endpoint}");
                let host_port = endpoint.strip_prefix(WS_SCHEME_PREFIX).expect("prefix present");
                let parsed: SocketAddr = host_port.parse().expect("endpoint host:port must parse as SocketAddr");
                assert!(parsed.ip().is_loopback(), "expected loopback ip, got {parsed}");
                assert!(parsed.port() > 0, "expected non-zero ephemeral port, got {}", parsed.port());
                // Drop runs here: SIGKILL + reap. `/bin/sleep` may have
                // already exited from the bad arg list; the kernel
                // ESRCH on kill is swallowed by Drop, wait reaps the
                // zombie either way.
            }
            SimnetOrTn10::Remote { .. } => panic!("expected LocallySpawned, got Remote"),
        }
    }

    #[test]
    fn resolve_from_remote_returns_ok_for_reachable_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("stand-in listener bind");
        let addr = listener.local_addr().expect("listener addr");
        let raw = OsString::from(format!("{addr}"));
        let outcome = resolve_from(None, Some(raw)).expect("reachable endpoint should resolve");
        let kaspad = outcome.expect("expected Some(SimnetOrTn10::Remote)");
        match kaspad {
            SimnetOrTn10::Remote { endpoint } => {
                let expected = format!("{WS_SCHEME_PREFIX}{addr}");
                assert_eq!(endpoint, expected, "endpoint should be normalised to ws://-prefixed form");
            }
            SimnetOrTn10::LocallySpawned { .. } => panic!("expected Remote, got LocallySpawned"),
        }
        drop(listener);
    }

    #[test]
    fn resolve_from_remote_preserves_ws_scheme_when_present() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("stand-in listener bind");
        let addr = listener.local_addr().expect("listener addr");
        let raw_str = format!("{WS_SCHEME_PREFIX}{addr}");
        let outcome = resolve_from(None, Some(OsString::from(&raw_str))).expect("reachable ws:// endpoint should resolve");
        let kaspad = outcome.expect("expected Some(SimnetOrTn10::Remote)");
        assert_eq!(kaspad.endpoint(), raw_str, "ws://-prefixed input should round-trip verbatim");
        drop(listener);
    }

    #[test]
    fn resolve_from_remote_errs_for_unparseable_endpoint() {
        let raw = OsString::from("not-a-host-port");
        let err = resolve_from(None, Some(raw)).expect_err("unparseable endpoint should be an error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "expected InvalidInput, got {err:?}");
    }

    #[test]
    fn resolve_from_remote_errs_for_unreachable_endpoint() {
        // Reserve an ephemeral port via a TcpListener, drop the listener
        // to release the bind. The kernel may reuse the port quickly
        // but the connect attempt within the 500ms budget races against
        // any reuse, so the typical outcome is a refused connection.
        let listener = TcpListener::bind("127.0.0.1:0").expect("stand-in listener bind");
        let addr = listener.local_addr().expect("listener addr");
        drop(listener);
        let raw = OsString::from(format!("{addr}"));
        let res = resolve_from(None, Some(raw));
        assert!(res.is_err(), "expected probe failure for unbound port, got {res:?}");
    }

    #[test]
    fn resolve_from_prefers_local_when_both_env_vars_set() {
        let sleep_bin = Path::new("/bin/sleep");
        if !sleep_bin.is_file() {
            eprintln!("resolve_from_prefers_local_when_both_env_vars_set: /bin/sleep not present -- skipping");
            return;
        }
        // KASPA_TN10_ENDPOINT intentionally points at a parse-failing
        // value -- if precedence were reversed, the test would fail
        // with InvalidInput; under correct precedence, the local
        // branch is taken and the remote value is never inspected.
        let outcome = resolve_from(Some(OsString::from(sleep_bin)), Some(OsString::from("not-a-host-port")))
            .expect("local precedence should bypass the remote parse");
        assert!(matches!(outcome, Some(SimnetOrTn10::LocallySpawned { .. })), "expected LocallySpawned under env-precedence rule");
    }
}
