//! Common bounded, cancellable observation lifecycle for Agent and Workspace consumers.
use crate::local_client::cancellation::{Cancellation, OwnedCancellation};
use crate::{
    local_client::{
        LocalClient,
        subscription::{ChangeEvent, ChangeSubscription},
    },
    machine_discovery::Connection,
    machine_picker::Choice,
    machines::ProfileId,
    workbench_client::UiWakeup,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequestId, WorkbenchSnapshotOutcome,
};
use std::{
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const FRESH_FOR: Duration = Duration::from_secs(10);
// Host process titles and optional external Git observations have no committed notification source.
// Only collectors requesting those observations sample; canonical metadata follows invalidations.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
// Failed admission/reads retry independently of successful state subscriptions. This is bounded
// recovery backoff, never a periodic read of a successful canonical checkpoint.
fn retry_after(failures: &mut u8) -> Duration {
    let delay = Duration::from_millis((500_u64 << (*failures).min(6)).min(30_000));
    *failures = failures.saturating_add(1);
    delay
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Observation<T> {
    Loading,
    Ready(Arc<T>),
    Unavailable,
}
#[derive(Clone)]
pub struct Source {
    pub choice: Choice,
    pub connection: Connection,
}
type Reader<T> = fn(&mut LocalClient, Instant, Cancellation<'_>) -> Result<Arc<T>, ()>;
pub(crate) fn request_id() -> Result<WorkbenchRequestId, ()> {
    WorkbenchRequestId::parse(ProfileId::generate().map_err(|_| ())?.as_str()).map_err(|_| ())
}
pub(crate) fn check(deadline: Instant, cancel: Cancellation<'_>) -> Result<(), ()> {
    if Instant::now() >= deadline || cancel.is_cancelled().map_err(|_| ())? {
        Err(())
    } else {
        Ok(())
    }
}
pub fn snapshot(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<Snapshot, ()> {
    check(deadline, cancel)?;
    match client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
            },
            deadline,
            cancel.wake(),
        )
        .map_err(|_| ())?
        .outcome
    {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
        WorkbenchSnapshotOutcome::Denied { .. } | WorkbenchSnapshotOutcome::Unavailable { .. } => {
            Err(())
        }
    }
}
struct Completed<T> {
    at: Instant,
    rows: Result<Arc<T>, ()>,
}
struct Job<T> {
    cancel: Arc<OwnedCancellation>,
    task: Option<JoinHandle<Completed<T>>>,
}
impl<T: Send + Sync + 'static> Job<T> {
    fn start(
        source: Connection,
        deadline: Instant,
        read: Reader<T>,
        wakeup: Option<UiWakeup>,
    ) -> Result<Self, ()> {
        let cancel = Arc::new(OwnedCancellation::new().map_err(|_| ())?);
        let flag = cancel.clone();
        let task = thread::Builder::new()
            .name("orchbee-machine-observation".into())
            .spawn(move || {
                let result = (|| {
                    let mut client = source.reconnect(deadline, flag.borrow()).map_err(|_| ())?;
                    read(&mut client, deadline, flag.borrow())
                })();
                let completed = Completed {
                    at: Instant::now(),
                    rows: result,
                };
                if let Some(wakeup) = wakeup {
                    wakeup.signal();
                }
                completed
            })
            .map_err(|_| ())?;
        Ok(Self {
            cancel,
            task: Some(task),
        })
    }
    fn stop(&self) {
        self.cancel.cancel();
    }
    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}
impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
struct Entry<T> {
    choice: Choice,
    connection: Connection,
    job: Option<Job<T>>,
    observation: Observation<T>,
    observed: Option<Instant>,
    next: Option<Instant>,
    watch: Option<ChangeSubscription>,
    watch_started: bool,
    watch_retry: Option<Instant>,
    watch_failures: u8,
    read_failures: u8,
    waiting_snapshot: bool,
    /// A publication arriving during a read requires another complete read afterwards.
    changed: bool,
}
pub struct Collector<T> {
    entries: Vec<Entry<T>>,
    retired: Vec<Job<T>>,
    cursor: usize,
    sample_external: bool,
    hinted: Vec<Choice>,
    wakeup: Option<UiWakeup>,
}
impl<T: Clone + Send + Sync + 'static> Collector<T> {
    pub fn new(wakeup: UiWakeup, sample_external: bool) -> Self {
        let mut collector = Self::default();
        collector.wakeup = Some(wakeup);
        collector.sample_external = sample_external;
        collector
    }
    pub fn sample_external_sources(&mut self, enabled: bool) {
        self.sample_external = enabled;
    }
    /// Whether this source published a change during the latest UI tick.
    pub fn hinted(&self, choice: &Choice) -> bool {
        self.hinted.contains(choice)
    }
    pub fn clear(&mut self) {
        for mut entry in self.entries.drain(..) {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
            // Dropping the subscription cancels only its logical stream, without a join thread.
        }
    }
    pub fn tick(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        read: Reader<T>,
    ) -> Vec<(Choice, Observation<T>)> {
        self.reconcile(&sources, now);
        self.observe_changes(now);
        let wakeup = self.wakeup.clone();
        self.tick_jobs(now, |source, deadline| {
            Job::start(source, deadline, read, wakeup.clone())
        })
    }
    fn reconcile(&mut self, sources: &[Source], now: Instant) {
        self.retired.retain(|job| !job.finished());
        self.hinted.clear();
        let mut old = std::mem::take(&mut self.entries);
        for source in sources {
            let entry = old
                .iter()
                .position(|e| {
                    e.choice == source.choice && e.connection.same_owner(&source.connection)
                })
                .map(|i| old.remove(i))
                .unwrap_or_else(|| Entry {
                    choice: source.choice.clone(),
                    connection: source.connection.clone(),
                    job: None,
                    observation: Observation::Loading,
                    observed: None,
                    next: Some(now),
                    watch: None,
                    watch_started: false,
                    watch_retry: None,
                    watch_failures: 0,
                    read_failures: 0,
                    waiting_snapshot: false,
                    changed: false,
                });
            self.entries.push(entry);
        }
        for mut entry in old {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
        }
    }
    fn observe_changes(&mut self, now: Instant) {
        for entry in &mut self.entries {
            if !entry.watch_started && entry.watch_retry.is_none_or(|retry| now >= retry) {
                entry.watch_started = true;
                entry.waiting_snapshot = true;
                entry.next = None;
                // Enqueue registration before any domain read. A publication concurrent with
                // capture/read remains in this mailbox until the next UI drain.
                match entry.connection.watch_changes() {
                    Ok(watch) => {
                        entry.watch = Some(watch);
                        entry.watch_retry = None;
                    }
                    Err(_) => {
                        entry.watch_started = false;
                        entry.watch_retry = Some(now + retry_after(&mut entry.watch_failures));
                        entry.waiting_snapshot = true;
                        entry.observation = Observation::Unavailable;
                    }
                }
            }
            while let Some(watch) = &entry.watch {
                match watch.poll() {
                    Ok(Some(ChangeEvent::Published)) => {
                        entry.watch_failures = 0;
                        entry.waiting_snapshot = false;
                        entry.changed = true;
                        self.hinted.push(entry.choice.clone());
                    }
                    result @ (Ok(Some(ChangeEvent::Reset)) | Err(_)) => {
                        // A completed old read must not reinstall pre-gap/pre-reconnect state.
                        if let Some(job) = entry.job.take() {
                            job.stop();
                            self.retired.push(job);
                        }
                        entry.waiting_snapshot = true;
                        entry.changed = false;
                        entry.next = None;
                        entry.observed = None;
                        entry.observation = Observation::Unavailable;
                        if result.is_err() {
                            entry.watch = None;
                            entry.watch_started = false;
                            entry.watch_retry = Some(now + retry_after(&mut entry.watch_failures));
                            break;
                        }
                    }
                    Ok(None) => break,
                }
            }
        }
    }
    fn tick_jobs(
        &mut self,
        now: Instant,
        mut start: impl FnMut(Connection, Instant) -> Result<Job<T>, ()>,
    ) -> Vec<(Choice, Observation<T>)> {
        for entry in &mut self.entries {
            if entry.job.as_ref().is_some_and(Job::finished) {
                let mut job = entry.job.take().expect("finished job");
                entry.observation = match job.task.take().expect("owned job").join() {
                    Ok(Completed { at, rows: Ok(rows) }) => {
                        entry.read_failures = 0;
                        entry.observed = Some(at);
                        entry.next = self.sample_external.then_some(now + SAMPLE_INTERVAL);
                        Observation::Ready(rows)
                    }
                    _ => {
                        entry.observed = None;
                        entry.next = Some(now + retry_after(&mut entry.read_failures));
                        Observation::Unavailable
                    }
                };
            }
        }
        let mut active =
            self.retired.len() + self.entries.iter().filter(|e| e.job.is_some()).count();
        let len = self.entries.len();
        for offset in 0..len {
            let index = (self.cursor + offset) % len;
            let entry = &mut self.entries[index];
            if active >= 4 {
                break;
            }
            if entry.job.is_none()
                && !entry.waiting_snapshot
                && (entry.next.is_some_and(|next| now >= next) || entry.changed)
            {
                entry.changed = false;
                entry.next = None;
                match start(entry.connection.clone(), now + Duration::from_secs(5)) {
                    Ok(job) => {
                        entry.job = Some(job);
                        active += 1;
                    }
                    Err(()) => {
                        entry.observation = Observation::Unavailable;
                        entry.next = Some(now + retry_after(&mut entry.read_failures));
                    }
                }
            }
        }
        if len > 0 {
            self.cursor = (self.cursor + 1) % len;
        }
        self.entries
            .iter()
            .map(|entry| {
                (
                    entry.choice.clone(),
                    if !entry.connection.is_current(now)
                        || (self.sample_external
                            && entry
                                .observed
                                .is_some_and(|at| now.saturating_duration_since(at) > FRESH_FOR))
                    {
                        Observation::Unavailable
                    } else {
                        entry.observation.clone()
                    },
                )
            })
            .collect()
    }
    #[cfg(test)]
    fn tick_with_start(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        start: impl FnMut(Connection, Instant) -> Result<Job<T>, ()>,
    ) -> Vec<(Choice, Observation<T>)> {
        // Scheduling evidence injects completed/bounded jobs; carrier evidence uses tick itself.
        self.reconcile(&sources, now);
        self.tick_jobs(now, start)
    }
}
impl<T> Drop for Collector<T> {
    fn drop(&mut self) {
        for entry in &self.entries {
            if let Some(job) = &entry.job {
                job.cancel.cancel();
            }
        }
        for job in &self.retired {
            job.cancel.cancel();
        }
        // Job::drop joins every writer after the cancellation pass.
    }
}
impl<T> Default for Collector<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            retired: Vec::new(),
            cursor: 0,
            sample_external: false,
            hinted: Vec::new(),
            wakeup: None,
        }
    }
}

#[cfg(test)]
mod tests;
