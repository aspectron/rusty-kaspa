## `kaspa-utils`
---

### Profiling Instrumentation Prototype

`src/profiler.rs` and `macros/src/profiler.rs` includes a prototype for basic profiling instrumentation.

It maintains a global instance of the `Profiler` struct that keeps a HashMap (`DashMap`) of `Sampler` structs, hashed by `u64` keys.  Each `Sampler` contains a name and a `VecDequeue` queue of `f64` samples.

Right now this simply accumulates samples into the vec, bound by the capacity specified in the `Profiler::new(<capacity>)` constructor.

A number of other methods can be used to process the data to identify outliers etc. for example “deviation from last” or “deviation from an average” etc.

Each `Sampler` can be modified based on the required sampling methodology and perform its own processing.

`Profiler::report(clear : bool)` takes the `clear` argument and if it is `true`, it clears the Sampler Vec after formatting the printable strings.

The instrumentation is done using a `sample!{}` macro as follows:
```rust
pub async fn async_is_nearly_synced(&self) -> bool {
    sample! {"async_is_nearly_synced" , {
        self.clone().spawn_blocking(|c| c.is_nearly_synced()).await
    }}
}
```
Usage: `sample!{ "static sample name str", { … code block for sampling … }};`

NOTE: `sample!{}` and `sample!()` (macro brackets) can be used interchangeably.

The `sample!` macro takes 2 arguments, the name of the sampler and the code block. The code block can be anything. The name *must be  `&'static str`*.

The macro (at compile time) generates a `u64` hash of the given name and uses this as the sampler identifier. So the hashing doesn’t occur during runtime.

Since the macro generates a `sampler_id : u64` value from the supplied sampler name, the `DashMap` uses a custom (fake) `SamplerIdHasher` hasher which just copies the macro-supplied `u64` id value without actually hashing it.

The macro then wraps the code block, captures its return value into a variable, and adds `start_sampling()` and `stop_sampling()` invocations around that code, following which it returns the original result.

The macro expansion from the above example looks like this:
```
let sampler_7203018407332621605 = kaspa_utils::profiler::start_sampling();
let result_7203018407332621605 = { self.clone().spawn_blocking(|c| c.is_nearly_synced()).await };
kaspa_utils::profiler::stop_sampling("async_is_nearly_synced", 7203018407332621605u64, sampler_7203018407332621605);
result_7203018407332621605
```

To use the profiler, the crate must include `kaspa-utils.workspace = true` as a dependency and include the macro via the `use kaspa_utils::profiler::*` directive.

`kaspa-utils` crate has a feature called `profiler`. If this feature is enabled, the functions relay the arguments to the global instance of the profiler. If the `profiler` feature is disabled, the functions are substituted with inline blanks, which should be optimized away by the compiler.

The macro can include a check for the `profile` feature being enabled, but that would require the `profile` feature to be propagated throughout all the crates, which will make the setup very cumbersome. (it is not possible to check if a feature is enabled in a foreign crate)

Another thing that can be done is the use of the environment variables during the build using the `env!()` macro. 

Using blank function stubs is the simplest solution.

The `profile` feature has also been added to the `kaspad` binary. Enabling it propagates it to the `kaspa-utils` crate. It is currently enabled in the `default` feature set and should eventually be removed. The `profile` feature can be enabled during the build (or run) by specifying `--feature profile` argument.  I.e. `cargo run --bin kaspad --release --feature profile -- …`

`kaspad` should enable this based on the `--profiling` flag (right now there is a temporary stub in the `kaspad/src/main.rs` creating the `Profiler` instance).  If the feature is not enabled, `Profiler` will never be accessed from the macro instrumentation, so it doesn't need to be instantiated.

Currently, the sampling delta measurements are stored as milliseconds represented by `f64` formatted as `{:.2}`.  The sampling is taken using `Instant::now().duration_since(start).as_millis() * 1000.0`

The `report()` output currently piggy-backs on top of the monitor task that outputs processing stats. The `report()` function can do additional processing on the sampler. For example, printing only filtered outliers or updating sample averages, etc.

This method of sampling is meant for measuring time-consuming tasks to identify potential execution stalls. It will perform fine on synchronous tasks, but in the async execution pipeline, it can only be used to identify execution latency pattern changes as async functions have too much variation in their execution time.

