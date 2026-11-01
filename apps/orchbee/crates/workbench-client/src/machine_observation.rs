//! Common bounded, cancellable observation lifecycle for Agent and Workspace consumers.
pub(crate) mod outcome;
use crate::background_thread::BackgroundThread;
use crate::local_client::LocalClient;
use crate::local_client::cancellation::{Cancellation, OwnedCancellation};
use crate::local_client::subscription::{ChangeEvent, ChangeSubscription};
use crate::machine_discovery::Connection;
use crate::machines::{Choice, ProfileId};
use crate::ui_wakeup::UiWakeup;
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequestId, WorkbenchSnapshotOutcome,
};
pub use outcome::Observation;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
// A sampled observation stays valid this long after its read completed, through failed refreshes,
// reset subscriptions and carrier blips; only sustained loss withdraws it.
const FRESH_FOR: Duration = Duration::from_secs(10);
// Host process titles and optional external Git observations have no committed notification source.
// Only collectors requesting those observations sample; canonical metadata follows invalidations.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
// A sampled read shares the carrier with the terminals. Its cooldown stretches to this many times
// the read, so Workspaces reporting Git normally take at most a quarter of the carrier. The cap lets
// the next five-second read complete before the last checkpoint ages out.
const SAMPLE_DUTY: u32 = 3;
const MAXIMUM_SAMPLE_COOLDOWN: Duration = Duration::from_secs(4);
// Failed admission/reads retry independently of successful state subscriptions. This is bounded
// recovery backoff, never a periodic read of a successful canonical checkpoint.
fn retry_after(failures: &mut u8) -> Duration {
    let delay = Duration::from_millis((500_u64 << (*failures).min(6)).min(30_000));
    *failures = failures.saturating_add(1);
    delay
}
#[derive(Clone)]
pub struct Source {
    pub choice: Choice,
    pub connection: Connection,
}
type Reader<T> = fn(&mut LocalClient, Instant, Cancellation<'_>) -> Result<Arc<T>, ()>;
/// One read that a consumer asks for ahead of the sampling cooldown.
pub type ExpeditedRead<T> =
    Box<dyn FnOnce(&mut LocalClient, Instant, Cancellation<'_>) -> Result<Arc<T>, ()> + Send>;
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
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
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
    task: BackgroundThread<Completed<T>>,
}
impl<T: Send + Sync + 'static> Job<T> {
    fn start(
        source: Connection,
        deadline: Instant,
        read: impl FnOnce(&mut LocalClient, Instant, Cancellation<'_>) -> Result<Arc<T>, ()>
        + Send
        + 'static,
        wakeup: Option<UiWakeup>,
    ) -> Result<Self, ()> {
        let cancel = Arc::new(OwnedCancellation::new().map_err(|_| ())?);
        let flag = cancel.clone();
        let task = BackgroundThread::spawn("orchbee-machine-observation", move || {
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
        Ok(Self { cancel, task })
    }
    fn stop(&self) {
        self.cancel.cancel();
    }
    fn finished(&self) -> bool {
        self.task.is_finished()
    }
}
impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        // Only Collector::stop_before waits; an unfinished reader is detached here.
        self.cancel.cancel();
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
    /// The running job refreshes sampled facts, so its completion restarts their cooldown.
    sampling: bool,
    started: Instant,
    /// Runs ahead of the cooldown once no read is running; a sampled read that is due replaces it.
    expedited: Option<ExpeditedRead<T>>,
}
impl<T> Entry<T> {
    /// A failed refresh is no canonical deletion: a sampling source keeps its last Ready until
    /// that checkpoint ages out, which keeps the presented rows from flashing away when a commit
    /// lands inside a multi-request read.
    fn withdraw(&mut self, sampling: bool) {
        if !sampling || !matches!(self.observation, Observation::Ready(_)) {
            self.observed = None;
            self.observation = Observation::Unavailable;
        }
    }
    fn presented(&self, sampling: bool, now: Instant) -> Observation<T>
    where
        T: Clone,
    {
        let withdrawn = if sampling && matches!(self.observation, Observation::Ready(_)) {
            self.observed
                .is_some_and(|at| now.saturating_duration_since(at) > FRESH_FOR)
        } else {
            !self.connection.is_current(now)
        };
        if withdrawn {
            Observation::Unavailable
        } else {
            self.observation.clone()
        }
    }
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
    /// Starts `read` for this source as soon as its running read ends, leaving the sampling
    /// cooldown armed. A sampled read that is due covers it instead.
    pub fn expedite(&mut self, choice: &Choice, read: ExpeditedRead<T>) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| &entry.choice == choice)
        {
            entry.expedited = Some(read);
        }
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
        self.tick_jobs(now, false, |source, deadline, _| {
            Job::start(source, deadline, read, wakeup.clone())
        })
    }
    /// Answers invalidations with `canonical` alone. Only `sampled` refreshes the external facts
    /// and owns their cooldown, so a commit never adds an expensive external read.
    pub fn tick_split(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        canonical: Reader<T>,
        sampled: Reader<T>,
    ) -> Vec<(Choice, Observation<T>)> {
        self.reconcile(&sources, now);
        self.observe_changes(now);
        let wakeup = self.wakeup.clone();
        self.tick_jobs(now, true, |source, deadline, sampling| {
            let read = if sampling { sampled } else { canonical };
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
                    sampling: false,
                    started: now,
                    expedited: None,
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
        let sampling = self.sample_external;
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
                        entry.withdraw(sampling);
                    }
                }
            }
            while let Some(watch) = &entry.watch {
                match watch.poll() {
                    Ok(Some(ChangeEvent::Published)) => {
                        entry.watch_failures = 0;
                        // A fresh baseline reads everything, sampled facts included.
                        if entry.waiting_snapshot {
                            entry.next = Some(now);
                        }
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
                        entry.withdraw(sampling);
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
        split: bool,
        mut start: impl FnMut(Connection, Instant, bool) -> Result<Job<T>, ()>,
    ) -> Vec<(Choice, Observation<T>)> {
        for entry in &mut self.entries {
            if entry.job.as_ref().is_some_and(Job::finished) {
                let mut job = entry.job.take().expect("finished job");
                match job.task.join().expect("owned job") {
                    Ok(Completed { at, rows: Ok(rows) }) => {
                        entry.read_failures = 0;
                        entry.observed = Some(at);
                        if entry.sampling {
                            let took = at.saturating_duration_since(entry.started);
                            let cooldown = (took * SAMPLE_DUTY)
                                .clamp(SAMPLE_INTERVAL, MAXIMUM_SAMPLE_COOLDOWN);
                            entry.next = self.sample_external.then_some(now + cooldown);
                        }
                        entry.observation = Observation::Ready(rows);
                    }
                    _ => {
                        entry.next = Some(now + retry_after(&mut entry.read_failures));
                        entry.withdraw(self.sample_external);
                    }
                }
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
            let due = entry.next.is_some_and(|next| now >= next);
            if entry.job.is_none()
                && !entry.waiting_snapshot
                && (due || entry.changed || entry.expedited.is_some())
            {
                entry.changed = false;
                let expedited = entry.expedited.take().filter(|_| !due);
                // Only a sampled read consumes the cooldown; a split canonical read and an
                // expedited one leave it armed.
                entry.sampling = due || !split && expedited.is_none();
                if entry.sampling {
                    entry.next = None;
                }
                entry.started = now;
                let deadline = now + Duration::from_secs(5);
                let started = match expedited {
                    Some(read) => Job::start(
                        entry.connection.clone(),
                        deadline,
                        read,
                        self.wakeup.clone(),
                    ),
                    None => start(entry.connection.clone(), deadline, entry.sampling),
                };
                match started {
                    Ok(job) => {
                        entry.job = Some(job);
                        active += 1;
                    }
                    Err(()) => {
                        entry.withdraw(self.sample_external);
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
                    entry.presented(self.sample_external, now),
                )
            })
            .collect()
    }
    #[cfg(test)]
    fn tick_with_start(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        mut start: impl FnMut(Connection, Instant) -> Result<Job<T>, ()>,
    ) -> Vec<(Choice, Observation<T>)> {
        // Scheduling evidence injects completed/bounded jobs; carrier evidence uses tick itself.
        self.reconcile(&sources, now);
        self.tick_jobs(now, false, |source, deadline, _| start(source, deadline))
    }
    #[cfg(test)]
    fn tick_split_with_start(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        start: impl FnMut(Connection, Instant, bool) -> Result<Job<T>, ()>,
    ) -> Vec<(Choice, Observation<T>)> {
        self.reconcile(&sources, now);
        self.tick_jobs(now, true, start)
    }
}
impl<T> Collector<T> {
    /// Process exit: cancels every reader without waiting.
    pub fn request_stop(&mut self) {
        for job in self
            .entries
            .iter()
            .filter_map(|entry| entry.job.as_ref())
            .chain(&self.retired)
        {
            job.cancel.cancel();
        }
    }

    /// Process exit: observes every reader thread against the shared deadline.
    ///
    /// # Errors
    /// TimedOut when a reader is still running at the deadline; it is detached on drop.
    pub fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.request_stop();
        self.entries
            .iter_mut()
            .filter_map(|entry| entry.job.as_mut())
            .chain(&mut self.retired)
            .map(|job| job.task.join_before(deadline))
            .fold(Ok(()), std::io::Result::and)
    }
}
impl<T> Drop for Collector<T> {
    fn drop(&mut self) {
        // Only stop_before waits; unfinished readers are detached here.
        self.request_stop();
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
