//! The Session's published physical terminal titles and the pacing of the Worker change reads that
//! feed them. It keeps the last title the Worker reported for each terminal of the current Worker
//! generation, projected by `project_terminal_title`, and wakes the streams that follow titles. It
//! never touches the Workbench, its revision, Agent reports, unread state or the semantic journal;
//! the Worker exchange belongs to the read pump and Client answers to the panel-agent read.

use orchbee_capability_node_protocol::{
    HostTerminalIdentity, OperationId, TerminalTitleChangesOutcome, TerminalTitleChangesRequest,
    TerminalTitleObservation, WorkerGeneration, WorkerId,
};
use orchbee_server::{SessionChangeSignal, SessionChangeSource};
use orchbee_workbench_protocol::{PresentationTitle, project_terminal_title};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::JoinHandle,
    time::{Duration, Instant},
};
mod reads;

/// At most one read starts per interval. The next page of a read is not throttled.
pub(crate) const READ_INTERVAL: Duration = Duration::from_millis(250);
/// Terminals one Worker generation holds titles for, as many as the Worker itself retains.
const MAXIMUM_TERMINALS: usize = 1024;

/// What the read pump does next.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum NextRead {
    Read(TerminalTitleChangesRequest),
    /// Nothing is due before the deadline, or before a wake when there is none.
    Wait(Option<Instant>),
    Stopped,
}

/// Registry and pump wake. Lock order: this is a leaf; no other lock is taken while it is held,
/// and subscribers are woken after it is released.
#[derive(Debug, Default)]
pub(crate) struct TerminalTitles {
    state: Mutex<State>,
    pump: Condvar,
    subscribers: SessionChangeSource,
    reader: Mutex<Option<JoinHandle<()>>>,
}

impl TerminalTitles {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Wakes `signal` whenever a published title changes.
    pub(crate) fn watch(&self, signal: &Arc<SessionChangeSignal>) {
        self.subscribers.watch(signal);
    }

    /// A newly registered Worker generation replaces the previous one, whose titles become
    /// unavailable, and is read afresh as soon as the pacing allows.
    pub(crate) fn begin(&self, worker_id: WorkerId, generation: NonZeroU64) {
        let changed = self
            .lock()
            .begin(worker_id, WorkerGeneration::new(generation));
        self.pump.notify_all();
        self.publish(changed);
    }

    /// The generation's carrier retired; its titles become unavailable.
    pub(crate) fn retire(&self, generation: NonZeroU64) {
        let changed = self.lock().retire(WorkerGeneration::new(generation));
        self.publish(changed);
    }

    /// A Worker note that its projection changed through `through`.
    pub(crate) fn note(&self, generation: NonZeroU64, through: NonZeroU64) {
        if self.lock().note(WorkerGeneration::new(generation), through) {
            self.pump.notify_all();
        }
    }

    pub(crate) fn next_read(&self, now: Instant) -> NextRead {
        self.lock().next_read(now)
    }

    /// Applies the answer to `request`, or its failure when `outcome` is `None`.
    pub(crate) fn apply(
        &self,
        request: &TerminalTitleChangesRequest,
        outcome: Option<TerminalTitleChangesOutcome>,
        now: Instant,
    ) {
        let changed = self.lock().apply(request, outcome, now);
        self.publish(changed);
    }

    /// Waits for a wake, the deadline or a stop, whichever comes first.
    pub(crate) fn wait(&self, deadline: Option<Instant>) {
        let state = self.lock();
        if state.stopped || state.woken {
            return;
        }
        let state = match deadline {
            Some(deadline) => {
                self.pump
                    .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .0
            }
            None => self
                .pump
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        };
        drop(state);
    }

    /// Only a Windows Session stop joins the pump; elsewhere it lives as long as the process.
    #[cfg(any(windows, test))]
    pub(crate) fn stop(&self) {
        self.lock().stopped = true;
        self.pump.notify_all();
    }

    pub(crate) fn stopped(&self) -> bool {
        self.lock().stopped
    }

    /// The published title of one physical terminal of one Worker generation.
    pub(crate) fn title(
        &self,
        generation: WorkerGeneration,
        identity: HostTerminalIdentity,
    ) -> PresentationTitle {
        self.lock().title(generation, identity)
    }

    fn publish(&self, changed: bool) {
        if changed {
            self.subscribers.notify_titles();
        }
    }
}

#[derive(Debug, Default)]
struct State {
    current: Option<Generation>,
    stopped: bool,
    /// A wake that arrived while the pump was busy, consumed by its next `next_read`.
    woken: bool,
    requests: u128,
}

#[derive(Debug)]
struct Generation {
    worker_id: WorkerId,
    generation: WorkerGeneration,
    titles: BTreeMap<[u8; 16], (HostTerminalIdentity, Option<String>)>,
    /// The `next` of the last applied page; 0 before any.
    cursor: u64,
    /// The next read starts at 0: the generation is new or the Worker reset the cursor.
    fresh: bool,
    /// The newest change a Worker note announced.
    noted: u64,
    /// The last page was incomplete, so the read continues at once.
    continuing: bool,
    started: Option<Instant>,
    in_flight: Option<TerminalTitleChangesRequest>,
}

impl State {
    fn begin(&mut self, worker_id: WorkerId, generation: WorkerGeneration) -> bool {
        let changed = self.retire_current();
        self.current = Some(Generation {
            worker_id,
            generation,
            titles: BTreeMap::new(),
            cursor: 0,
            fresh: true,
            noted: 0,
            continuing: false,
            started: None,
            in_flight: None,
        });
        self.woken = true;
        changed
    }

    fn retire(&mut self, generation: WorkerGeneration) -> bool {
        if self
            .current
            .as_ref()
            .is_some_and(|current| current.generation == generation)
        {
            self.retire_current()
        } else {
            false
        }
    }

    fn retire_current(&mut self) -> bool {
        self.current
            .take()
            .is_some_and(|current| !current.titles.is_empty())
    }

    fn note(&mut self, generation: WorkerGeneration, through: NonZeroU64) -> bool {
        let Some(current) = self
            .current
            .as_mut()
            .filter(|current| current.generation == generation)
        else {
            return false;
        };
        current.noted = current.noted.max(through.get());
        let due = current.due();
        self.woken |= due;
        due
    }

    fn next_read(&mut self, now: Instant) -> NextRead {
        self.woken = false;
        if self.stopped {
            return NextRead::Stopped;
        }
        let Some(current) = self
            .current
            .as_mut()
            .filter(|current| current.in_flight.is_none() && current.due())
        else {
            return NextRead::Wait(None);
        };
        if !current.continuing
            && let Some(allowed) = current.started.map(|started| started + READ_INTERVAL)
            && now < allowed
        {
            return NextRead::Wait(Some(allowed));
        }
        // A pump-local counter suffices: the carrier matches each result to its own request.
        self.requests = self.requests.wrapping_add(1).max(1);
        let Ok(request_id) = OperationId::from_bytes(self.requests.to_be_bytes()) else {
            return NextRead::Wait(None);
        };
        if !current.continuing {
            current.started = Some(now);
        }
        let request = TerminalTitleChangesRequest {
            request_id,
            worker_id: current.worker_id,
            worker_generation: current.generation,
            after: if current.fresh { 0 } else { current.cursor },
        };
        current.in_flight = Some(request);
        NextRead::Read(request)
    }

    fn apply(
        &mut self,
        request: &TerminalTitleChangesRequest,
        outcome: Option<TerminalTitleChangesOutcome>,
        now: Instant,
    ) -> bool {
        let Some(current) = self
            .current
            .as_mut()
            .filter(|current| current.in_flight.as_ref() == Some(request))
        else {
            return false;
        };
        current.in_flight = None;
        match outcome {
            Some(TerminalTitleChangesOutcome::Page(page)) => {
                let mut changed = false;
                for row in page.rows {
                    let key = row.identity.terminal_id().as_bytes();
                    let next = match row.observation {
                        TerminalTitleObservation::Observed { title, .. } => {
                            Some(title.as_deref().and_then(project_terminal_title))
                        }
                        TerminalTitleObservation::Unavailable => None,
                    };
                    changed |= match next {
                        Some(title) => {
                            let entry = (row.identity, title);
                            if current.titles.get(&key) == Some(&entry) {
                                false
                            } else if current.titles.len() >= MAXIMUM_TERMINALS
                                && !current.titles.contains_key(&key)
                            {
                                // Beyond the Worker's own bound; this terminal stays unavailable.
                                false
                            } else {
                                current.titles.insert(key, entry);
                                true
                            }
                        }
                        None => current.titles.remove(&key).is_some(),
                    };
                }
                current.cursor = page.next;
                current.fresh = false;
                current.continuing = !page.complete;
                changed
            }
            Some(TerminalTitleChangesOutcome::Reset) => {
                let changed = !current.titles.is_empty();
                current.titles.clear();
                current.cursor = 0;
                current.fresh = true;
                current.continuing = false;
                changed
            }
            // A Worker refusing its own generation has nothing to read until it notes a change.
            Some(TerminalTitleChangesOutcome::RejectedFencing) => {
                current.fresh = false;
                current.continuing = false;
                current.noted = current.cursor;
                false
            }
            // The read is retried after a full interval, whatever stage it failed at.
            None => {
                current.continuing = false;
                current.started = Some(now);
                false
            }
        }
    }

    fn title(
        &self,
        generation: WorkerGeneration,
        identity: HostTerminalIdentity,
    ) -> PresentationTitle {
        self.current
            .as_ref()
            .filter(|current| current.generation == generation)
            .and_then(|current| current.titles.get(&identity.terminal_id().as_bytes()))
            .filter(|(retained, _)| *retained == identity)
            .map_or(PresentationTitle::Unavailable {}, |(_, title)| {
                PresentationTitle::Observed {
                    title: title.clone(),
                }
            })
    }
}

impl Generation {
    fn due(&self) -> bool {
        self.fresh || self.continuing || self.noted > self.cursor
    }
}

#[cfg(test)]
mod tests;
