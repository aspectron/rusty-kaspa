use cfg_if::cfg_if;
use dashmap::DashMap;
use std::{collections::VecDeque, sync::Arc};

pub use kaspa_utils_macros::sample;

pub struct Sampler {
    pub name: &'static str,
    pub samples: VecDeque<f64>,
}

impl Sampler {
    const DEFAULT_CAPACITY: usize = 128;

    pub fn new(name: &'static str, capacity: usize) -> Self {
        Self { name, samples: VecDeque::with_capacity(capacity) }
    }
}

impl std::fmt::Display for Sampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.name)?;
        for v in self.samples.iter() {
            write!(f, " {:.2}", v)?;
        }
        Ok(())
    }
}

pub struct Inner {
    pub samplers: DashMap<u64, Sampler, SamplerIdHasher>,
    pub capacity: Option<usize>,
}

#[derive(Clone)]
pub struct Profiler {
    inner: Arc<Inner>,
}

static mut PROFILER: Option<Profiler> = None;

impl Profiler {
    pub fn init(capacity: Option<usize>) -> Profiler {
        let hasher = SamplerIdHasher::default();
        let profiler = Profiler { inner: Arc::new(Inner { samplers: DashMap::with_hasher(hasher), capacity }) };

        unsafe {
            if PROFILER.is_some() {
                panic!("Multiple instances of Profiler are not allowed");
            } else {
                PROFILER = Some(profiler.clone());
            }
        }

        profiler
    }

    pub fn check() {
        cfg_if! {
            if #[cfg(not(feature = "profile"))] {
                panic!("Profiler feature is not enabled (must be enabled during compilation)");
            }
        }
    }

    pub fn get() -> &'static Profiler {
        unsafe { PROFILER.as_ref().unwrap() }
    }

    pub fn print(&self, clear: bool) {
        for mut sampler in self.inner.samplers.iter_mut() {
            println!("{}", *sampler);
            if clear {
                sampler.samples.clear();
            }
        }
    }

    pub fn report(&self, clear: bool) -> Vec<String> {
        self.inner
            .samplers
            .iter_mut()
            .map(|mut sampler| {
                let string = format!("{}", *sampler);
                if clear {
                    sampler.samples.clear();
                }
                string
            })
            .collect::<Vec<_>>()
    }

    pub fn clear(&self) {
        self.inner.samplers.iter_mut().for_each(|mut sampler| {
            sampler.samples.clear();
        });
    }

    /// Store time delta in a sampler, create a sampler if not present
    #[inline]
    pub fn store(&self, name: &'static str, sampler_id: u64, delta: f64) {
        if let Some(mut sampler) = self.inner.samplers.get_mut(&sampler_id) {
            sampler.samples.push_back(delta);
            if self.inner.capacity.is_some_and(|cap| sampler.samples.len() > cap) {
                sampler.samples.pop_front();
            }
        } else {
            let mut sampler = Sampler::new(name, self.inner.capacity.unwrap_or(Sampler::DEFAULT_CAPACITY));
            sampler.samples.push_back(delta);
            self.inner.samplers.insert(sampler_id, sampler);
        }
    }
}

cfg_if! {
    if #[cfg(feature = "profile")] {

        use std::time::Instant;

        #[inline]
        pub fn start_sampling() -> Instant {
            Instant::now()
        }

        #[inline]
        pub fn stop_sampling(name: &'static str, sampler_id: u64, start: Instant) {
            let delta = Instant::now().duration_since(start);
            Profiler::get().store(name, sampler_id, delta.as_secs_f64() * 1000.0);
        }

    } else {

        #[inline(always)]
        pub fn start_sampling() {
        }

        #[inline(always)]
        pub fn stop_sampling(_name: &'static str, _sampler_id: u64, _start: ()) {
        }

    }
}

#[derive(Clone, Default)]
pub struct SamplerIdHasher {
    state: u64,
}

impl std::hash::Hasher for SamplerIdHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.state = u64::from_le_bytes(bytes.try_into().expect("SamplerIdHasher expects a u64 key"));
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

impl std::hash::BuildHasher for SamplerIdHasher {
    type Hasher = SamplerIdHasher;
    fn build_hasher(&self) -> SamplerIdHasher {
        SamplerIdHasher { state: 0 }
    }
}
