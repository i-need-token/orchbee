//! Common bounded, cancellable observation lifecycle for Agent and Workspace consumers.
use crate::{
    local_client::LocalClient, machine_discovery::Connection, machine_picker::Choice,
    machines::ProfileId,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SemanticEventFilter, SemanticEventKind, SemanticEventMatch,
    SemanticEventScope, SemanticEventsAction, SemanticEventsFailure, SemanticEventsOutcome,
    SemanticEventsRequest, SiteId, Snapshot, SnapshotReadIntent, WorkbenchRequestId,
    WorkbenchSnapshotOutcome,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const FRESH_FOR: Duration = Duration::from_secs(10);
/// Periodic reads remain the authority; a change hint only starts the next read early.
const READ_INTERVAL: Duration = Duration::from_secs(2);
const WATCH_RETRY: Duration = Duration::from_secs(10);
const WATCH_WAIT_MILLISECONDS: u16 = 1000;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Observation<T> {
    Loading,
    Ready(Arc<T>),
    Unavailable,
}
#[derive(Clone)]
pub(crate) struct Source {
    pub(crate) choice: Choice,
    pub(crate) connection: Connection,
}
type Reader<T> = fn(&mut LocalClient, Instant, &AtomicU8) -> Result<Arc<T>, ()>;
pub(crate) fn request_id() -> Result<WorkbenchRequestId, ()> {
    WorkbenchRequestId::parse(ProfileId::generate().map_err(|_| ())?.as_str()).map_err(|_| ())
}
pub(crate) fn check(deadline: Instant, cancel: &AtomicU8) -> Result<(), ()> {
    if Instant::now() >= deadline || cancel.load(Ordering::Acquire) != 0 {
        Err(())
    } else {
        Ok(())
    }
}
pub(crate) fn snapshot(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: &AtomicU8,
) -> Result<Snapshot, ()> {
    check(deadline, cancel)?;
    match client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
            },
            deadline,
            Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
        )
        .map_err(|_| ())?
        .outcome
    {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
        _ => Err(()),
    }
}
struct Completed<T> {
    at: Instant,
    rows: Result<Arc<T>, ()>,
}
struct Job<T> {
    cancel: Arc<AtomicU8>,
    task: Option<JoinHandle<Completed<T>>>,
}
impl<T: Send + Sync + 'static> Job<T> {
    fn start(source: Connection, deadline: Instant, read: Reader<T>) -> Result<Self, ()> {
        let cancel = Arc::new(AtomicU8::new(0));
        let flag = cancel.clone();
        let task = thread::Builder::new()
            .name("orchbee-machine-observation".into())
            .spawn(move || {
                let result = (|| {
                    let mut client = source.reconnect(deadline, &flag).map_err(|_| ())?;
                    read(&mut client, deadline, &flag)
                })();
                Completed {
                    at: Instant::now(),
                    rows: result,
                }
            })
            .map_err(|_| ())?;
        Ok(Self {
            cancel,
            task: Some(task),
        })
    }
    fn stop(&self) {
        self.cancel.store(1, Ordering::Release);
    }
    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}
/// Long-polls one source's semantic event journal and counts admitted matching changes. The count
/// is only a refresh hint: coalesced, evicted or missed events still reach the periodic read.
struct Watch {
    cancel: Arc<AtomicU8>,
    changes: Arc<AtomicU64>,
    task: Option<JoinHandle<()>>,
}
impl Watch {
    fn start(source: Connection, kinds: &'static [SemanticEventKind]) -> Result<Self, ()> {
        let cancel = Arc::new(AtomicU8::new(0));
        let changes = Arc::new(AtomicU64::new(0));
        let (flag, counter) = (cancel.clone(), changes.clone());
        let task = thread::Builder::new()
            .name("orchbee-machine-watch".into())
            .spawn(move || {
                let _ended = watch(&source, kinds, &flag, &counter);
            })
            .map_err(|_| ())?;
        Ok(Self {
            cancel,
            changes,
            task: Some(task),
        })
    }
    fn changes(&self) -> u64 {
        self.changes.load(Ordering::Acquire)
    }
    fn stop(&self) {
        self.cancel.store(1, Ordering::Release);
    }
    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        self.stop();
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
fn watch(
    source: &Connection,
    kinds: &'static [SemanticEventKind],
    cancel: &AtomicU8,
    changes: &AtomicU64,
) -> Result<(), ()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut client = source.reconnect(deadline, cancel).map_err(|_| ())?;
    let site_id = snapshot(&mut client, deadline, cancel)?.site_id;
    let subscription_id = request_id()?;
    let filters = kinds
        .iter()
        .map(|kind| SemanticEventFilter::Event {
            event: *kind,
            scope: SemanticEventScope::All {},
            condition: SemanticEventMatch::Any {},
        })
        .collect();
    let opened = events(
        &mut client,
        &site_id,
        SemanticEventsAction::Subscribe {
            subscription_id: subscription_id.clone(),
            filters,
            after: None,
        },
        cancel,
    )?;
    if !matches!(opened, SemanticEventsOutcome::Opened { .. }) {
        return Err(());
    }
    // A change can commit between the consumer's last read and this baseline.
    changes.fetch_add(1, Ordering::AcqRel);
    while cancel.load(Ordering::Acquire) == 0 {
        match events(
            &mut client,
            &site_id,
            SemanticEventsAction::Wait {
                subscription_id: subscription_id.clone(),
                timeout_milliseconds: WATCH_WAIT_MILLISECONDS,
            },
            cancel,
        )? {
            SemanticEventsOutcome::Matched { .. } | SemanticEventsOutcome::Gap { .. } => {
                changes.fetch_add(1, Ordering::AcqRel);
            }
            SemanticEventsOutcome::TimedOut { .. } => {}
            SemanticEventsOutcome::Failed {
                reason: SemanticEventsFailure::Busy,
            } => thread::sleep(Duration::from_millis(20)),
            _ => return Err(()),
        }
    }
    // Connection EOF releases the subscription; no close exchange is owed.
    Ok(())
}
fn events(
    client: &mut LocalClient,
    site_id: &SiteId,
    action: SemanticEventsAction,
    cancel: &AtomicU8,
) -> Result<SemanticEventsOutcome, ()> {
    if cancel.load(Ordering::Acquire) != 0 {
        return Err(());
    }
    client
        .semantic_events(
            SemanticEventsRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
                site_id: site_id.clone(),
                action,
            },
            Instant::now() + Duration::from_secs(3),
            Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
        )
        .map(|result| result.outcome)
        .map_err(|_| ())
}
impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel.store(1, Ordering::Release);
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
struct Entry<T> {
    choice: Choice,
    job: Option<Job<T>>,
    observation: Observation<T>,
    observed: Option<Instant>,
    next: Instant,
    watch: Option<Watch>,
    watch_retry: Instant,
    seen_changes: u64,
    /// A change hint arrived after the latest read started.
    changed: bool,
}
pub(crate) struct Collector<T> {
    entries: Vec<Entry<T>>,
    retired: Vec<Job<T>>,
    retired_watches: Vec<Watch>,
    cursor: usize,
    watch_kinds: &'static [SemanticEventKind],
    hinted: Vec<Choice>,
}
impl<T: Clone + Send + Sync + 'static> Collector<T> {
    /// Reads each source again as soon as it publishes one of `kinds`.
    pub(crate) fn watching(kinds: &'static [SemanticEventKind]) -> Self {
        let mut collector = Self::default();
        collector.watch_kinds = kinds;
        collector
    }
    /// Whether `choice` published a watched change during the latest tick.
    pub(crate) fn hinted(&self, choice: &Choice) -> bool {
        self.hinted.contains(choice)
    }
    pub(crate) fn clear(&mut self) {
        for mut entry in self.entries.drain(..) {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
            if let Some(watch) = entry.watch.take() {
                watch.stop();
                self.retired_watches.push(watch);
            }
        }
    }
    pub(crate) fn tick(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        read: Reader<T>,
    ) -> Vec<(Choice, Observation<T>)> {
        self.tick_with_start(sources, now, |source, deadline| {
            Job::start(source, deadline, read)
        })
    }
    fn tick_with_start(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        mut start: impl FnMut(Connection, Instant) -> Result<Job<T>, ()>,
    ) -> Vec<(Choice, Observation<T>)> {
        self.retired.retain(|job| !job.finished());
        self.retired_watches.retain(|watch| !watch.finished());
        self.hinted.clear();
        let mut old = std::mem::take(&mut self.entries);
        for source in &sources {
            let entry = old
                .iter()
                .position(|e| e.choice == source.choice)
                .map(|i| old.remove(i))
                .unwrap_or_else(|| Entry {
                    choice: source.choice.clone(),
                    job: None,
                    observation: Observation::Loading,
                    observed: None,
                    next: now,
                    watch: None,
                    watch_retry: now,
                    seen_changes: 0,
                    changed: false,
                });
            self.entries.push(entry);
        }
        for mut entry in old {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
            if let Some(watch) = entry.watch.take() {
                watch.stop();
                self.retired_watches.push(watch);
            }
        }
        for entry in &mut self.entries {
            if entry.job.as_ref().is_some_and(Job::finished) {
                let mut job = entry.job.take().expect("finished job");
                entry.observation = match job.task.take().expect("owned job").join() {
                    Ok(Completed { at, rows: Ok(rows) }) => {
                        entry.observed = Some(at);
                        Observation::Ready(rows)
                    }
                    _ => {
                        entry.observed = None;
                        Observation::Unavailable
                    }
                };
                entry.next = now + READ_INTERVAL;
            }
        }
        if !self.watch_kinds.is_empty() {
            for (index, entry) in self.entries.iter_mut().enumerate() {
                if entry.watch.as_ref().is_some_and(Watch::finished) {
                    self.retired_watches.extend(entry.watch.take());
                    entry.watch_retry = now + WATCH_RETRY;
                }
                if entry.watch.is_none() && now >= entry.watch_retry {
                    match Watch::start(sources[index].connection.clone(), self.watch_kinds) {
                        Ok(watch) => {
                            entry.watch = Some(watch);
                            entry.seen_changes = 0;
                        }
                        Err(()) => entry.watch_retry = now + WATCH_RETRY,
                    }
                }
                if let Some(changes) = entry.watch.as_ref().map(Watch::changes)
                    && changes != entry.seen_changes
                {
                    entry.seen_changes = changes;
                    entry.changed = true;
                    self.hinted.push(entry.choice.clone());
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
            if entry.job.is_none() && (now >= entry.next || entry.changed) {
                entry.changed = false;
                match start(
                    sources[index].connection.clone(),
                    now + Duration::from_secs(5),
                ) {
                    Ok(job) => {
                        entry.job = Some(job);
                        active += 1;
                    }
                    Err(()) => {
                        entry.observation = Observation::Unavailable;
                        entry.next = now + READ_INTERVAL;
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
                    if entry
                        .observed
                        .is_some_and(|at| now.saturating_duration_since(at) > FRESH_FOR)
                    {
                        Observation::Unavailable
                    } else {
                        entry.observation.clone()
                    },
                )
            })
            .collect()
    }
}
impl<T> Drop for Collector<T> {
    fn drop(&mut self) {
        for entry in &self.entries {
            if let Some(job) = &entry.job {
                job.cancel.store(1, Ordering::Release);
            }
        }
        for job in &self.retired {
            job.cancel.store(1, Ordering::Release);
        }
        for watch in self.entries.iter().filter_map(|entry| entry.watch.as_ref()) {
            watch.stop();
        }
        for watch in &self.retired_watches {
            watch.stop();
        }
        // Job::drop joins every writer after the cancellation pass.
    }
}
impl<T> Default for Collector<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            retired: Vec::new(),
            retired_watches: Vec::new(),
            cursor: 0,
            watch_kinds: &[],
            hinted: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
