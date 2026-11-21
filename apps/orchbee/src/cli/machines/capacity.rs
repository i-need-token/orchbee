//! What this computer has free, read once a second on a thread of its own from the sampler's start,
//! so a reader never waits on a measurement. The machine hub runs one from its start, and a link
//! helper from its admission when its hub negotiated `capacity`. A reader takes the last reading:
//! memory and the home folder's volume from the first one, the processor share once a second one
//! exists, over the newest and the oldest of the last four, which spans three seconds, and nothing
//! once the sampler has fallen ten seconds behind.

use orchbee_local_runtime::host_capacity::{self, CpuTicks};
use orchbee_machine_protocol::{
    PathText, Time,
    capacity::{Bytes, Capacity, Cpu, Disk},
};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// The pause between two readings.
const PERIOD: Duration = Duration::from_secs(1);
/// The processor readings kept: the share spans the oldest and the newest.
const WINDOW: usize = 4;
/// A reading this old reads as none: the sampler fell behind.
const STALE: Duration = Duration::from_secs(10);

/// One reading.
#[derive(Clone, Debug)]
pub(super) struct Sample {
    /// Each fact this computer could read.
    pub(super) capacity: Capacity,
    /// When it was read, on this computer's clock.
    pub(super) sampled_at: Time,
    taken: Instant,
}

/// The last reading, which the sampler's thread replaces once a second.
pub(super) struct Sampler {
    latest: Mutex<Option<Sample>>,
}

impl Sampler {
    /// Starts reading on a thread of its own and returns at once; `None` when no thread could
    /// start, and then nothing reads this computer.
    pub(super) fn start() -> Option<Arc<Self>> {
        let sampler = Arc::new(Self {
            latest: Mutex::new(None),
        });
        let reader = Arc::clone(&sampler);
        thread::Builder::new()
            .name("orchbee-capacity".into())
            .spawn(move || reader.run())
            .ok()?;
        Some(sampler)
    }

    /// The last reading; `None` before the first one and once the sampler fell behind.
    pub(super) fn latest(&self) -> Option<Sample> {
        self.latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .filter(|sample| sample.taken.elapsed() <= STALE)
    }

    fn run(&self) {
        let home = std::env::home_dir();
        let mut ticks = VecDeque::with_capacity(WINDOW);
        loop {
            let started = Instant::now();
            match host_capacity::cpu_ticks() {
                Some(reading) => {
                    if ticks.len() == WINDOW {
                        ticks.pop_front();
                    }
                    ticks.push_back(reading);
                }
                // A share across a gap would not be the last few seconds'.
                None => ticks.clear(),
            }
            let sample = read(&ticks, home.as_deref());
            *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = sample;
            thread::sleep(PERIOD.saturating_sub(started.elapsed()));
        }
    }
}

/// One reading taken now, without a sampler: memory and the home folder's volume, and no processor
/// share, which needs a second reading.
pub(super) fn once() -> Option<Sample> {
    read(&VecDeque::new(), std::env::home_dir().as_deref())
}

/// One reading of every fact; `None` only when the clock reads before 1970 or past the time range.
fn read(ticks: &VecDeque<CpuTicks>, home: Option<&Path>) -> Option<Sample> {
    let sampled_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .and_then(|milliseconds| Time::new(milliseconds).ok())?;
    let share = match (ticks.front(), ticks.back()) {
        (Some(oldest), Some(newest)) if ticks.len() > 1 => newest.free_percent_since(*oldest),
        _ => None,
    };
    let cpu =
        share.and_then(|free| Cpu::new(u64::from(host_capacity::logical_processors()?), free).ok());
    let memory = host_capacity::memory()
        .and_then(|memory| Bytes::new(memory.total_bytes, memory.free_bytes).ok());
    let disk = home.and_then(|home| {
        let path = PathText::new(home.to_str()?).ok()?;
        let disk = host_capacity::disk(home)?;
        Some(Disk {
            path,
            bytes: Bytes::new(disk.total_bytes, disk.free_bytes).ok()?,
        })
    });
    Some(Sample {
        capacity: Capacity { cpu, memory, disk },
        sampled_at,
        taken: Instant::now(),
    })
}
