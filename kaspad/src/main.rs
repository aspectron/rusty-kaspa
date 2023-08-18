extern crate kaspa_consensus;
extern crate kaspa_core;
extern crate kaspa_hashes;

use args::{Args, Defaults};
use kaspa_core::kaspad_env::version;
use kaspa_core::signals::Signals;
#[allow(unused_imports)]
use kaspa_core::{info, trace};
use std::sync::Arc;

mod args;
mod context;
mod daemon;
mod runtime;
mod utils;

use crate::context::Context; // per-instance context
use crate::daemon::Kaspad;
use crate::runtime::Runtime; // global runtime handling // kaspad daemon itself

pub fn main() {
    #[cfg(feature = "heap")]
    let _profiler = dhat::Profiler::builder().file_name("kaspad-heap.json").build();

    let args = Args::parse(&Defaults::default());
    let runtime = Runtime::new_with_args(&args);
    let context = Context::new_with_args(&args);

    // Print package name and version
    info!("{} v{}", env!("CARGO_PKG_NAME"), version());

    let kaspad = Kaspad::new(&runtime, context, args);

    // Bind the SIGTERM signal handler to the core
    Arc::new(Signals::new(kaspad.core())).init();

    kaspad.run();

    info!("Kaspad has stopped");
}
