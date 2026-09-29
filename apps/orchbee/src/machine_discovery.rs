//! Read-only observers of enabled saved machines. The UI alone owns selection.
mod policy;
#[cfg(test)]
mod tests;
mod worker;
use crate::{
    local_client::{LocalClient, LocalClientOrigin, LocalOperationClock},
    machines::{Profile, ProfileId},
    terminal_session::{NativeContextLocation, NativeContextTarget},
};
use orchbee_workbench_protocol::{SiteId, Snapshot, WorkbenchRevision};
use policy::{Phase, Policy};
use std::{
    io,
    sync::atomic::AtomicU8,
    time::{Duration, Instant},
};
use worker::{Event, Update, Worker};

const CONNECT_LIMIT: usize = 4;
const FRESH_FOR: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Overview {
    pub(crate) site: SiteId,
    pub(crate) revision: WorkbenchRevision,
    pub(crate) projects: usize,
    pub(crate) workspaces: usize,
    pub(crate) tabs: usize,
    pub(crate) labels: Vec<String>,
    pub(crate) contexts: std::sync::Arc<[Context]>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Context {
    pub(crate) target: NativeContextTarget,
    pub(crate) project: Option<String>,
    pub(crate) workspace: Option<String>,
}
impl Overview {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        let mut labels = Vec::new();
        for project in &snapshot.navigation.projects {
            if labels.len() < 8 {
                labels.push(display_label(&project.label));
            }
            for workspace in &project.workspaces {
                if labels.len() < 8 {
                    labels.push(display_label(&workspace.label));
                }
            }
        }
        let mut contexts = vec![Context {
            target: NativeContextTarget {
                site_id: snapshot.site_id.clone(),
                location: NativeContextLocation::Standalone(
                    snapshot.navigation.standalone_terminal_context_id.clone(),
                ),
            },
            project: None,
            workspace: None,
        }];
        // The wire boundary has already validated ordering, membership and protocol limits.
        if let Ok(ordered) = snapshot.navigation.ordered_workspaces() {
            contexts.extend(ordered.into_iter().map(|(project, workspace)| Context {
                target: NativeContextTarget {
                    site_id: snapshot.site_id.clone(),
                    location: NativeContextLocation::Workspace {
                        project_id: project.project_id.clone(),
                        workspace_id: workspace.workspace_id.clone(),
                        terminal_context_id: workspace.terminal_context_id.clone(),
                    },
                },
                project: Some(display_label(&project.label)),
                workspace: Some(display_label(&workspace.label)),
            }));
        }
        Self {
            site: snapshot.site_id.clone(),
            revision: snapshot.revision,
            projects: snapshot.navigation.projects.len(),
            workspaces: snapshot
                .navigation
                .projects
                .iter()
                .map(|p| p.workspaces.len())
                .sum(),
            tabs: snapshot
                .terminal_contexts
                .iter()
                .map(|c| c.tabs.len())
                .sum(),
            labels,
            contexts: contexts.into(),
        }
    }
}
pub(crate) fn display_label(value: &str) -> String {
    value.chars().filter(|c| !c.is_control() && !matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')).take(80).collect()
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Observation {
    Waiting,
    Connecting,
    Online(Overview),
    Reconnecting,
    Attention,
    Stale,
}
impl Observation {
    pub(crate) fn key(&self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Connecting => "opening",
            Self::Online(_) => "online",
            Self::Reconnecting => "reconnecting",
            Self::Attention => "attention",
            Self::Stale => "stale",
        }
    }
}
#[derive(Clone)]
pub(crate) struct Connection {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
}
impl Connection {
    pub(crate) fn from_origin(origin: LocalClientOrigin, clock: LocalOperationClock) -> Self {
        Self { origin, clock }
    }
    pub(crate) fn reconnect(
        &self,
        deadline: Instant,
        cancel: &AtomicU8,
    ) -> io::Result<LocalClient> {
        self.origin
            .reconnect_before_cancellable(
                deadline,
                self.clock.clone(),
                Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
            )
            .map_err(crate::machine_tunnel::admission_error)
    }
}
struct Ready {
    overview: Overview,
    connection: Connection,
    observed: Instant,
}
struct Entry {
    profile: Profile,
    generation: u64,
    policy: Policy,
    worker: Option<Worker>,
    ready: Option<Ready>,
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
                .filter(|r| now.saturating_duration_since(r.observed) <= FRESH_FOR)
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
            }
        }
    }
}
pub(crate) fn same_endpoint(a: &Profile, b: &Profile) -> bool {
    a.id == b.id && a.draft.target == b.draft.target && a.draft.session == b.draft.session
}
#[derive(Default)]
pub(crate) struct Discovery {
    entries: Vec<Entry>,
    retired: Vec<Worker>,
    generation: u64,
    next: usize,
}
impl Discovery {
    pub(crate) fn reconcile(&mut self, profiles: &[Profile], now: Instant) {
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
    pub(crate) fn tick(&mut self, now: Instant) {
        self.retired.retain(|w| !w.finished());
        for entry in &mut self.entries {
            if let Some(update) = entry.worker.as_ref().and_then(Worker::take) {
                entry.accept(update);
            }
            if matches!(entry.policy.phase, Phase::Attention | Phase::Reconnecting)
                && let Some(worker) = &entry.worker
            {
                worker.cancel();
            }
            if entry.worker.as_ref().is_some_and(Worker::finished) {
                // A worker may publish between the first drain and the completion check.
                let worker = entry.worker.take().expect("finished observer");
                if let Some(update) = worker.take() {
                    entry.accept(update);
                }
                if matches!(entry.policy.phase, Phase::Connecting | Phase::Online) {
                    entry.ready = None;
                    entry.policy.fail(false, now);
                }
            }
        }
        self.start_due(now, Worker::start);
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
    pub(crate) fn observations(&self, now: Instant) -> Vec<(ProfileId, Observation)> {
        self.entries
            .iter()
            .map(|e| (e.profile.id.clone(), e.observation(now)))
            .collect()
    }
    pub(crate) fn connection(&self, profile: &Profile, now: Instant) -> Option<Connection> {
        self.entries
            .iter()
            .find(|e| {
                same_endpoint(&e.profile, profile)
                    && matches!(e.observation(now), Observation::Online(_))
            })
            .and_then(|e| e.ready.as_ref().map(|r| r.connection.clone()))
    }
    pub(crate) fn failed(&self, profile: &Profile) -> bool {
        self.entries.iter().any(|e| {
            same_endpoint(&e.profile, profile)
                && matches!(e.policy.phase, Phase::Attention | Phase::Reconnecting)
        })
    }
    pub(crate) fn retry(&mut self, id: &ProfileId, now: Instant) {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.profile.id == id) {
            entry.policy.retry(now);
        }
    }
}
impl Drop for Discovery {
    fn drop(&mut self) {
        // Wake all I/O owners before any destructor joins a worker.
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
