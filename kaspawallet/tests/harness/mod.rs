//! Test harness primitives shared across the parity integration
//! binary's daemon-requiring rows.
//!
//! The harness owns three orthogonal concerns the parity tests need
//! a clean seam against:
//!
//! - **Process lifecycle.** A spawned wallet daemon is killed and
//!   reaped on `Drop` so a panicking test does not leak a zombie
//!   into the rest of the suite.
//! - **Port reservation.** Two daemons running in the same
//!   `cargo nextest` parallel pass each bind a distinct loopback
//!   port; the harness asks the kernel for an ephemeral port pair
//!   rather than hard-coding values.
//! - **Stderr capture.** The daemon's diagnostic output is written
//!   to a tempfile retained for the spawn's lifetime; a tail of the
//!   capture is available for panic-time diagnostic.
//!
//! `daemon_spawn` ships the building blocks the parity rows that
//! need a live wallet daemon consume (launched via the `kaspawallet
//! start-daemon` subcommand). Coordination against a
//! live `kaspad` (simnet locally, or a remote testnet endpoint) is
//! the `local_kaspad` concern. `paired_daemons` layers on top of
//! `daemon_spawn` for rows that pair the under-test wallet daemon
//! with the legacy-port `kaspawallet start-daemon` against the same
//! kaspad: env-var resolution, port reservation, two spawns + two
//! listen-waits with stderr-tail panics. Each module stays
//! decoupled from the others' load-bearing types so a test can
//! consume only the layer it needs.

pub mod daemon_spawn;
pub mod local_kaspad;
pub mod paired_daemons;
