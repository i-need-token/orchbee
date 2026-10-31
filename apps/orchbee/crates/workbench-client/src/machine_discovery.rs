//! Read-only observers of enabled saved machines. The UI alone owns selection.
pub(crate) mod overview;
mod policy;
pub use overview::{Context, Observation, Overview, display_label};
#[cfg(test)]
mod tests;
mod worker;
use crate::local_client::cancellation::Cancellation;
use crate::local_client::{LocalClient, LocalClientOrigin, LocalOperationClock};
use crate::machines::{Profile, ProfileId};
use orchbee_workbench_protocol::{SessionId, WorkbenchRevision};
use policy::{Phase, Policy};
use std::{io, time::Instant};
use worker::{Event, Update, Worker};

const CONNECT_LIMIT: usize = 4;

#[derive(Clone)]
pub struct Connection {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
}
impl Connection {
    /// Uses the carrier's shared application-progress deadline, including pending probe expiry.
    pub(crate) fn is_current(&self, now: Instant) -> bool {
        self.origin.is_current(now)
    }
    /// Distinguishes replacement carriers even when a saved profile's logical choice is unchanged.
    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        self.origin.shares_connection(&other.origin)
    }
    pub(crate) fn watch_changes(
        &self,
    ) -> io::Result<crate::local_client::subscription::ChangeSubscription> {
        self.origin.watch_changes()
    }
    pub fn from_origin(origin: LocalClientOrigin, clock: LocalOperationClock) -> Self {
        Self { origin, clock }
    }
    pub fn reconnect(
        &self,
        deadline: Instant,
        cancel: Cancellation<'_>,
    ) -> io::Result<LocalClient> {
        self.origin
            .lane_before_cancellable(deadline, self.clock.clone(), cancel.wake())
            .map_err(crate::machine_tunnel::admission_error)
    }
}
struct Ready {
    overview: Overview,
    connection: Connection,
}
struct Entry {
    profile: Profile,
    generation: u64,
    policy: Policy,
    worker: Option<Worker>,
    ready: Option<Ready>,
    retiring: bool,
}
impl Entry {
    fn observation(&self, now: Instant) -> Observation {
        match self.policy.phase {
            Phase::Waiting => Observation::Waiting,
            Phase::Connecting => Observation::Connecting,
            Phase::Reconnecting => Observation::Reconnecting,
            Phase::Attention => Observation::Attention,
            Phase::Online => self
                .ready
                .as_ref()
                .filter(|r| r.connection.is_current(now))
                .map_or(Observation::Stale, |r| {
                    Observation::Online(r.overview.clone())
                }),
        }
    }
    fn accept(&mut self, update: Update) {
        if update.generation != self.generation {
            return;
        }
        match update.event {
            Event::Healthy {
                ready,
                online_since,
            } => {
                self.policy.healthy(online_since);
                self.ready = Some(ready);
            }
            Event::Failed { permanent, at } => {
                self.ready = None;
                self.policy.fail(permanent, at);
                self.retiring = true;
            }
            Event::Reset => {
                self.ready = None;
                self.policy.recovering();
            }
        }
    }
}
pub fn same_endpoint(a: &Profile, b: &Profile) -> bool {
    a.id == b.id && a.draft.target == b.draft.target && a.draft.session == b.draft.session
}
#[derive(Default)]
pub struct Discovery {
    entries: Vec<Entry>,
    retired: Vec<Worker>,
    generation: u64,
    next: usize,
}
impl Discovery {
    pub fn reconcile(&mut self, profiles: &[Profile], now: Instant) {
        let mut old = std::mem::take(&mut self.entries);
        for profile in profiles.iter().filter(|p| p.enabled) {
            if let Some(index) = old.iter().position(|e| same_endpoint(&e.profile, profile)) {
                let mut entry = old.remove(index);
                entry.profile = profile.clone();
                self.entries.push(entry);
            } else if let Some(generation) = self.generation.checked_add(1) {
                self.generation = generation;
                self.entries.push(Entry {
                    profile: profile.clone(),
                    generation,
                    policy: Policy::new(now),
                    worker: None,
                    ready: None,
                    retiring: false,
                });
            }
        }
        for mut entry in old {
            entry.ready = None;
            if let Some(worker) = entry.worker.take() {
                worker.cancel();
                self.retired.push(worker);
            }
        }
    }
    pub fn tick(&mut self, now: Instant) {
        self.retired.retain(|w| !w.finished());
        for entry in &mut self.entries {
            if let Some(update) = entry.worker.as_ref().and_then(Worker::take) {
                entry.accept(update);
            }
            if entry.worker.as_ref().is_some_and(Worker::finished) {
                // A worker may publish between the first drain and the completion check.
                let worker = entry.worker.take().expect("finished observer");
                if let Some(update) = worker.take() {
                    entry.accept(update);
                }
                if matches!(
                    entry.policy.phase,
                    Phase::Connecting | Phase::Online | Phase::Reconnecting
                ) && !worker.cancelled()
                {
                    entry.ready = None;
                    entry.policy.fail(false, now);
                }
                entry.retiring = false;
            }
        }
        self.start_due(now, Worker::start);
    }
    /// The manager acknowledges after both collectors have retired cached source/watch owners.
    /// Until then a failed worker keeps its last carrier/tunnel guard on the background thread.
    pub fn finish_retirement(&mut self) {
        for entry in &self.entries {
            if entry.retiring
                && let Some(worker) = &entry.worker
            {
                worker.cancel();
            }
        }
    }
    fn start_due(
        &mut self,
        now: Instant,
        mut start: impl FnMut(Profile, u64) -> io::Result<Worker>,
    ) {
        let mut used = self.retired.len()
            + self
                .entries
                .iter()
                .filter(|e| e.worker.is_some() && e.policy.phase != Phase::Online)
                .count();
        let count = self.entries.len();
        let cursor = self.next;
        for offset in 0..count {
            if used >= CONNECT_LIMIT {
                break;
            }
            let index = (cursor + offset) % count;
            let entry = &mut self.entries[index];
            if entry.worker.is_none() && entry.policy.due(now) {
                self.next = (index + 1) % count;
                entry.policy.begin();
                match start(entry.profile.clone(), entry.generation) {
                    Ok(worker) => {
                        entry.worker = Some(worker);
                        used += 1;
                    }
                    Err(_) => entry.policy.fail(false, now),
                }
            }
        }
    }
    pub fn observations(&self, now: Instant) -> Vec<(ProfileId, Observation)> {
        self.entries
            .iter()
            .map(|e| (e.profile.id.clone(), e.observation(now)))
            .collect()
    }
    pub fn connection(&self, profile: &Profile, now: Instant) -> Option<Connection> {
        self.entries
            .iter()
            .find(|e| {
                same_endpoint(&e.profile, profile)
                    && matches!(e.observation(now), Observation::Online(_))
            })
            .and_then(|e| e.ready.as_ref().map(|r| r.connection.clone()))
    }
    pub fn failed(&self, profile: &Profile) -> bool {
        self.entries.iter().any(|e| {
            same_endpoint(&e.profile, profile)
                && matches!(e.policy.phase, Phase::Attention | Phase::Reconnecting)
        })
    }
    pub fn retry(&mut self, id: &ProfileId, now: Instant) {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.profile.id == id) {
            entry.policy.retry(now);
        }
    }
}
impl Discovery {
    /// Process exit: observes every observer thread against the shared deadline after all of
    /// them were cancelled.
    ///
    /// # Errors
    /// TimedOut when an observer is still running at the deadline; it is detached on drop.
    pub fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.request_stop();
        self.entries
            .iter_mut()
            .filter_map(|entry| entry.worker.as_mut())
            .chain(&mut self.retired)
            .map(|worker| worker.stop_before(deadline))
            .fold(Ok(()), io::Result::and)
    }

    /// Wakes all I/O owners and cancels every observer without waiting.
    pub fn request_stop(&mut self) {
        for entry in &mut self.entries {
            entry.ready = None;
        }
        for worker in self
            .entries
            .iter()
            .filter_map(|e| e.worker.as_ref())
            .chain(&self.retired)
        {
            worker.cancel();
        }
    }
}
impl Drop for Discovery {
    fn drop(&mut self) {
        // Only stop_before waits; unfinished observers are detached here.
        self.request_stop();
    }
}

/// Field-for-field `Overview` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct OverviewFixture {
    pub session: SessionId,
    pub revision: WorkbenchRevision,
    pub projects: usize,
    pub workspaces: usize,
    pub tabs: usize,
    pub labels: Vec<String>,
    pub contexts: std::sync::Arc<[Context]>,
}

#[cfg(any(test, feature = "test-support"))]
impl From<OverviewFixture> for Overview {
    fn from(fixture: OverviewFixture) -> Self {
        let OverviewFixture {
            session,
            revision,
            projects,
            workspaces,
            tabs,
            labels,
            contexts,
        } = fixture;
        Self {
            session,
            revision,
            projects,
            workspaces,
            tabs,
            labels,
            contexts,
        }
    }
}
