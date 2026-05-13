//! Spawn the wallet daemon as a child process, wait for its
//! gRPC listen address to come up, capture stderr to a tempfile,
//! and reap the child on `Drop`.
//!
//! Since the wallet daemon is consolidated into the `kaspawallet`
//! binary's `start-daemon` subcommand (no separate daemon binary
//! ships), this harness locates `kaspawallet` and injects the
//! `start-daemon` subcommand as the first argv element. Callers
//! pass the daemon-side argv (password file, network id,
//! rpc-server URL, TLS / auth flags) verbatim; the harness owns
//! the lifecycle envelope: where the binary lives, which port to
//! bind, how long to wait for the bind, where the stderr lands,
//! and how the child is killed and reaped at scope exit.

use std::ffi::OsStr;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Environment variable that names the `kaspawallet` binary path
/// on workstations where the default workspace `target/` lookup
/// does not apply (containers, prebuilt images, cross-workstation
/// validator hosts).
const ENV_KASPAWALLET_BIN: &str = "KASPAWALLET_BIN";

/// File name of the consolidated wallet binary in the workspace's
/// target profile directory. The `start-daemon` subcommand is the
/// daemon entry point.
const KASPAWALLET_BIN_NAME: &str = "kaspawallet";

/// Subcommand the consolidated binary exposes for running the
/// wallet daemon in-process. Injected as the first argv element
/// by [`DaemonSpawn::spawn`] so callers can keep passing the
/// daemon-side argv verbatim.
const START_DAEMON_SUBCOMMAND: &str = "start-daemon";

/// Polling interval between TCP-connect attempts when waiting for
/// the daemon's gRPC server to bind. Short enough that a fast bind
/// is detected promptly; long enough that the polling loop does
/// not burn CPU when the daemon is genuinely slow to start.
const WAIT_FOR_LISTEN_INTERVAL: Duration = Duration::from_millis(50);

/// Resolve the `kaspawallet` binary path.
///
/// Lookup order:
///
/// 1. `KASPAWALLET_BIN` environment variable.
/// 2. Workspace `target/{debug,release}/kaspawallet` (debug first
///    to favour the developer workflow over CI residue).
///
/// Returns `None` when no candidate is found; the caller emits a
/// skip-with-warning so the parity test surface stays green on a
/// workstation that has not built the binary yet.
pub fn locate_kaspawallet_binary() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var(ENV_KASPAWALLET_BIN) {
        let candidate = PathBuf::from(env_path);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    let target_root = workspace_target_dir();
    for profile in ["debug", "release"] {
        let mut candidate = target_root.clone();
        candidate.push(profile);
        candidate.push(KASPAWALLET_BIN_NAME);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Reserve a free loopback `SocketAddr` the daemon can be told to
/// bind via `--listen`. The kernel picks the port; the
/// caller drops the temporary listener and races the daemon to
/// reclaim it. A daemon that loses the race surfaces the collision
/// loudly on startup, which is the desired failure mode rather
/// than the harness silently masking it.
pub fn reserve_ephemeral_loopback_addr() -> io::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);
    Ok(addr)
}

/// Poll `addr` until a TCP connection succeeds or the deadline
/// elapses. The retry interval is fixed at
/// `WAIT_FOR_LISTEN_INTERVAL`; callers pick the overall timeout
/// based on the expected startup posture (a few hundred
/// milliseconds locally, a few seconds on a cold-cache CI host).
pub fn wait_for_listen(addr: SocketAddr, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect_timeout(&addr, WAIT_FOR_LISTEN_INTERVAL).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("address {addr} did not start accepting connections within {timeout:?}"),
            ));
        }
        std::thread::sleep(WAIT_FOR_LISTEN_INTERVAL);
    }
}

/// A spawned wallet-daemon process and the harness-owned resources
/// it depends on (a tempfile for stderr capture, the recorded
/// listen address). Dropping the value sends `SIGKILL` to the child
/// via [`Child::kill`] and reaps it via [`Child::wait`].
///
/// The harness opts for `SIGKILL` over a graceful `SIGTERM` /
/// timeout / `SIGKILL` ladder. Tests do not need the daemon to
/// flush state, and the unconditional kill avoids pulling a Unix-
/// signal crate into the dev-dependency surface for marginal
/// benefit.
#[derive(Debug)]
pub struct DaemonSpawn {
    child: Option<Child>,
    stderr_path: PathBuf,
    // Tempfile retained on the spawn handle so the file is unlinked
    // only when the spawn drops, not when the constructor returns.
    _stderr_handle: tempfile::TempPath,
}

impl DaemonSpawn {
    /// Spawn the wallet daemon via `binary start-daemon <daemon_args>`.
    /// Stdout is discarded; stderr is redirected to a tempfile retained
    /// for the spawn's lifetime. The caller composes the daemon-side
    /// argv (password file, listen address, rpc-server URL, etc.);
    /// the harness prepends the `start-daemon` subcommand so the
    /// consolidated `kaspawallet` binary routes into the daemon entry
    /// point in-process.
    pub fn spawn(binary: &Path, daemon_args: &[&OsStr]) -> io::Result<Self> {
        let tempfile = tempfile::NamedTempFile::new()?;
        let stderr_path = tempfile.path().to_owned();
        let stderr_file = tempfile.reopen()?;
        let tempfile_handle = tempfile.into_temp_path();
        let mut cmd = Command::new(binary);
        cmd.arg(START_DAEMON_SUBCOMMAND);
        cmd.args(daemon_args);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::from(stderr_file));
        let child = cmd.spawn()?;
        Ok(Self { child: Some(child), stderr_path, _stderr_handle: tempfile_handle })
    }

    /// Process id of the spawned daemon. `None` once the harness
    /// has dropped the [`Child`] handle internally.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    /// Read the last `max_bytes` of captured stderr. Returns the
    /// entire file when its size is below `max_bytes`. Useful in
    /// panic messages on harness-reported test failures so the
    /// reader sees what the daemon printed before it died.
    pub fn stderr_tail(&self, max_bytes: usize) -> io::Result<Vec<u8>> {
        let bytes = std::fs::read(&self.stderr_path)?;
        let start = bytes.len().saturating_sub(max_bytes);
        Ok(bytes[start..].to_vec())
    }
}

impl Drop for DaemonSpawn {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Test whether `path` names a present, executable regular file.
/// On Unix the executable check inspects the mode bits; on other
/// platforms it falls back to the `.exe` extension as a coarse
/// proxy. Mirrors the parity binary's locator helper to keep the
/// two modules' resolution semantics aligned without forcing a
/// shared-utility refactor in this batch.
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

/// Workspace `target/` root. Honours `CARGO_TARGET_DIR` when set,
/// otherwise walks one level up from the `kaspawallet` crate manifest
/// to the workspace root.
fn workspace_target_dir() -> PathBuf {
    if let Some(env_root) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(env_root);
    }
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.push("target");
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn reserve_ephemeral_loopback_addr_returns_loopback_port() {
        let addr = reserve_ephemeral_loopback_addr().expect("kernel ephemeral-port reservation");
        assert!(addr.ip().is_loopback(), "expected loopback ip, got {addr}");
        assert!(addr.port() > 0, "expected non-zero ephemeral port, got {}", addr.port());
    }

    #[test]
    fn reserve_ephemeral_loopback_addr_returns_distinct_ports_on_repeat_calls() {
        // Not a uniqueness guarantee from the kernel -- the port pool
        // could reuse -- but reservations within the same process and
        // wall-clock window typically differ. The assertion guards a
        // gross regression (e.g. a constant-port helper) without
        // claiming an ironclad invariant.
        let a = reserve_ephemeral_loopback_addr().expect("first reservation");
        let b = reserve_ephemeral_loopback_addr().expect("second reservation");
        assert_ne!(a.port(), b.port(), "two reservations should typically differ: {a} vs {b}");
    }

    #[test]
    fn wait_for_listen_succeeds_for_bound_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener bind");
        let addr = listener.local_addr().expect("listener addr");
        let res = wait_for_listen(addr, Duration::from_millis(500));
        assert!(res.is_ok(), "wait_for_listen should succeed against a bound listener: {res:?}");
        drop(listener);
    }

    #[test]
    fn wait_for_listen_times_out_for_unbound_port() {
        let addr = reserve_ephemeral_loopback_addr().expect("port reservation");
        let res = wait_for_listen(addr, Duration::from_millis(150));
        let err = res.expect_err("wait_for_listen should time out on an unbound port");
        assert_eq!(err.kind(), io::ErrorKind::TimedOut, "expected TimedOut, got {err:?}");
    }

    #[test]
    fn locate_kaspawallet_binary_resolves_or_skips_cleanly() {
        match locate_kaspawallet_binary() {
            Some(path) => {
                assert!(is_executable_file(&path), "resolved kaspawallet path is not executable: {}", path.display());
            }
            None => {
                eprintln!(
                    "locate_kaspawallet_binary: kaspawallet not built on this workstation -- skip is the expected outcome on a clean checkout; set {ENV_KASPAWALLET_BIN} or run `cargo build --release -p kaspawallet`"
                );
            }
        }
    }

    /// Exercise the spawn -> Drop reap cycle against a long-running
    /// stand-in process. The harness is designed for the wallet
    /// daemon, but a daemon spawn requires kaspad reachability the
    /// harness intentionally does not own; using `/bin/sleep` lets
    /// the lifecycle code path run end-to-end without a kaspad
    /// dependency. The kaspad-coordinated lift onto the real daemon
    /// lands in a sibling helper module.
    #[test]
    fn spawn_lifecycle_drop_reaps_stand_in_child() {
        let sleep_bin = Path::new("/bin/sleep");
        if !sleep_bin.is_file() {
            eprintln!("spawn_lifecycle_drop_reaps_stand_in_child: /bin/sleep not present -- skipping (non-Linux workstation)");
            return;
        }
        let args: [OsString; 1] = [OsString::from("60")];
        let arg_refs: Vec<&OsStr> = args.iter().map(AsRef::as_ref).collect();
        let pid = {
            let spawn = DaemonSpawn::spawn(sleep_bin, &arg_refs).expect("spawn /bin/sleep 60");
            let pid = spawn.pid().expect("pid available immediately after spawn");
            assert!(pid > 0, "expected positive pid, got {pid}");
            pid
            // spawn drops here -- SIGKILL + reap.
        };
        // Linux-specific: confirm the kernel has released the pid.
        // On other platforms the std lib's Child::wait reap is
        // authoritative and we trust it without /proc cross-check.
        #[cfg(target_os = "linux")]
        {
            // Give the kernel a beat to remove the /proc entry after
            // SIGKILL + wait return.
            let deadline = Instant::now() + Duration::from_millis(500);
            while Path::new(&format!("/proc/{pid}")).exists() {
                if Instant::now() >= deadline {
                    panic!("pid {pid} still present in /proc after Drop reaped the child");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    #[test]
    fn spawn_records_pid_and_stderr_path() {
        let sleep_bin = Path::new("/bin/sleep");
        if !sleep_bin.is_file() {
            eprintln!("spawn_records_pid_and_stderr_path: /bin/sleep not present -- skipping");
            return;
        }
        let args: [OsString; 1] = [OsString::from("5")];
        let arg_refs: Vec<&OsStr> = args.iter().map(AsRef::as_ref).collect();
        let spawn = DaemonSpawn::spawn(sleep_bin, &arg_refs).expect("spawn /bin/sleep 5");
        assert!(spawn.pid().is_some(), "pid should be Some immediately after spawn");
        let tail = spawn.stderr_tail(1024).expect("read stderr capture");
        // sleep writes nothing to stderr on the happy path.
        assert!(tail.is_empty(), "expected empty stderr tail for /bin/sleep 5, got {} bytes", tail.len());
    }
}
