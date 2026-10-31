//! Standard-library measurement loop for the frame-pipeline benchmarks. `cargo bench` passes
//! `--bench` and receives calibrated per-iteration statistics; any other invocation, including the
//! repository test gate's `cargo test --all-targets`, runs every selected case once as a smoke check.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

/// Whether a case pays a one-time cost or a cost repeated on every frame of a live pane.
#[derive(Clone, Copy)]
pub enum Phase {
    Init,
    Steady,
}

impl Phase {
    const fn label(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Steady => "steady",
        }
    }
}

/// One measured operation: a stable name, its phase, and the input bytes one iteration consumes.
pub struct Case<'a> {
    pub name: &'a str,
    pub phase: Phase,
    pub bytes: usize,
}

struct Settings {
    samples: usize,
    sample: Duration,
    warmup: Duration,
}

enum Mode {
    Smoke { cases: usize },
    Measure(Settings),
    List,
}

pub struct Harness {
    mode: Mode,
    filters: Vec<String>,
}

impl Harness {
    /// Reads `--bench`, `--list`, `--samples <n>`, `--sample-ms <n>`, `--warmup-ms <n>`, and
    /// substring filters. Other flags are ignored so a test-runner argument cannot fail the gate.
    pub fn from_args() -> Result<Self, String> {
        let mut bench = false;
        let mut list = false;
        let mut settings = Settings {
            samples: 50,
            sample: Duration::from_millis(2),
            warmup: Duration::from_millis(300),
        };
        let mut filters = Vec::new();
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--bench" => bench = true,
                "--list" => list = true,
                "--samples" => settings.samples = count(arguments.next(), &argument)?,
                "--sample-ms" => {
                    settings.sample = Duration::from_millis(count(arguments.next(), &argument)?);
                }
                "--warmup-ms" => {
                    settings.warmup = Duration::from_millis(count(arguments.next(), &argument)?);
                }
                flag if flag.starts_with('-') => {}
                filter => filters.push(filter.to_owned()),
            }
        }
        let mode = if list {
            Mode::List
        } else if bench {
            if settings.samples < 5 || settings.sample.is_zero() {
                return Err("--samples must be at least 5 and --sample-ms at least 1".to_owned());
            }
            Mode::Measure(settings)
        } else {
            Mode::Smoke { cases: 0 }
        };
        Ok(Self { mode, filters })
    }

    /// Prints the build and host facts that every reported result depends on.
    pub fn start(&self) {
        if let Mode::Measure(settings) = &self.mode {
            let parallelism = std::thread::available_parallelism().map_or(0, usize::from);
            // Every call is bracketed by two clock reads; this is their cost per call.
            let reads = 100_000_u32;
            let started = Instant::now();
            for _ in 0..reads {
                black_box(Instant::now());
            }
            let clock_ns = started.elapsed().as_secs_f64() * 1e9 / f64::from(reads);
            println!(
                "frame_pipeline os={} arch={} parallelism={parallelism} debug_assertions={} \
                 samples={} sample_ms={} warmup_ms={} clock_read_ns={clock_ns:.1}",
                std::env::consts::OS,
                std::env::consts::ARCH,
                cfg!(debug_assertions),
                settings.samples,
                settings.sample.as_millis(),
                settings.warmup.as_millis(),
            );
            if cfg!(debug_assertions) {
                println!("warning: debug assertions are enabled; results do not describe release");
            }
            println!(
                "{:<30} {:<6} {:>7} {:>10} {:>10} {:>10} {:>10} {:>7} {:>9} {:>9}",
                "case",
                "phase",
                "iters",
                "first_us",
                "min_us",
                "median_us",
                "p90_us",
                "mad_pct",
                "bytes",
                "MiB/s"
            );
        }
    }

    pub fn finish(&self) {
        if let Mode::Smoke { cases } = self.mode {
            println!("frame_pipeline smoke: {cases} cases ran once");
        }
    }

    fn selected(&self, name: &str) -> bool {
        self.filters.is_empty()
            || self
                .filters
                .iter()
                .any(|filter| name.contains(filter.as_str()))
    }

    /// Times each call of `routine` in calibrated batches. State the routine carries across calls
    /// belongs to the measured operation, so steady-state cases advance a live projection here.
    pub fn run<R>(&mut self, case: &Case<'_>, mut routine: impl FnMut() -> R) {
        self.run_with_input(case, || (), |()| routine());
    }

    /// Times `routine` on inputs built by `setup` outside the timed region. Each call is timed
    /// separately and its output is dropped after its clock stops, so the release of produced
    /// state is not attributed to the call and a batch never accumulates retained frames: batched
    /// retention made heap commit and page faults part of the result.
    pub fn run_with_input<I, R>(
        &mut self,
        case: &Case<'_>,
        mut setup: impl FnMut() -> I,
        mut routine: impl FnMut(I) -> R,
    ) {
        if !self.selected(case.name) {
            return;
        }
        let settings = match &mut self.mode {
            Mode::List => {
                println!("{} ({})", case.name, case.phase.label());
                return;
            }
            Mode::Smoke { cases } => {
                black_box(routine(setup()));
                *cases += 1;
                return;
            }
            Mode::Measure(settings) => settings,
        };
        let mut batch = |iterations: usize| -> Duration {
            let mut elapsed = Duration::ZERO;
            for _ in 0..iterations {
                let input = setup();
                let started = Instant::now();
                let output = routine(black_box(input));
                elapsed += started.elapsed();
                drop(black_box(output));
            }
            elapsed
        };
        let first = batch(1);
        let mut iterations = 1_usize;
        let warmup_started = Instant::now();
        loop {
            let elapsed = batch(iterations);
            if elapsed < settings.sample {
                iterations = iterations.saturating_mul(2);
            } else if warmup_started.elapsed() >= settings.warmup {
                break;
            }
        }
        let mut per_iteration: Vec<f64> = (0..settings.samples)
            .map(|_| batch(iterations).as_secs_f64() * 1e9 / iterations as f64)
            .collect();
        per_iteration.sort_by(f64::total_cmp);
        let median = percentile(&per_iteration, 0.5);
        let mut deviations: Vec<f64> = per_iteration
            .iter()
            .map(|value| (value - median).abs())
            .collect();
        deviations.sort_by(f64::total_cmp);
        let mad = percentile(&deviations, 0.5);
        let throughput = case.bytes as f64 / (median / 1e9) / (1024.0 * 1024.0);
        println!(
            "{:<30} {:<6} {:>7} {:>10.3} {:>10.3} {:>10.3} {:>10.3} {:>7.1} {:>9} {:>9.1}",
            case.name,
            case.phase.label(),
            iterations,
            first.as_secs_f64() * 1e6,
            per_iteration.first().copied().unwrap_or(f64::NAN) / 1e3,
            median / 1e3,
            percentile(&per_iteration, 0.9) / 1e3,
            mad / median * 100.0,
            case.bytes,
            throughput,
        );
    }
}

fn count<T: std::str::FromStr>(value: Option<String>, flag: &str) -> Result<T, String> {
    value
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("{flag} requires a non-negative integer"))
}

/// Nearest-rank percentile of an ascending, non-empty sample.
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let rank = (fraction * sorted.len() as f64).ceil() as usize;
    sorted
        .get(rank.clamp(1, sorted.len()) - 1)
        .copied()
        .unwrap_or(f64::NAN)
}
