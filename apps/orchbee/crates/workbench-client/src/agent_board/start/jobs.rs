//! The empty Board's private background jobs: the Client-PATH catalog lookup
//! and one Agent startup workflow per started Panel.
//!
//! The composition owns these jobs beside the Board and drives them against
//! the start state. Each job settles its own outcome into that state, and the
//! state never holds a thread or transport handle. Session teardown cancels and
//! joins every job within the workflow's existing bounded I/O and release
//! contract.

use std::{
    ffi::OsString,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::WorkbenchPanelId;

use super::{CatalogAdmission, CatalogBegin, Drain, Phase, State};
use crate::{
    agent_start::{Failure, Progress, Report, Request, ReservationState, StartFailure, Verdict},
    background_thread::BackgroundThread,
    local_client::{LocalClientOrigin, LocalOperationClock},
    workbench_state::AgentStartRefusal,
};
use orchbee_client_server_protocol::SendPromptState;

const START_DEADLINE: Duration = Duration::from_secs(300);
const CATALOG_DEADLINE: Duration = Duration::from_secs(6);

mod client_path;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct Jobs {
    origin: Option<LocalClientOrigin>,
    catalog: Option<CatalogTask>,
    retiring_catalogs: Vec<CatalogTask>,
    /// At most one workflow per Panel.
    start: Vec<StartTask>,
}

impl Jobs {
    pub(crate) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    /// Applies the state's latest decisions to the owned jobs: a retired catalog
    /// lookup is cancelled and an explicit cancellation wakes its workflow.
    pub(crate) fn reconcile(&mut self, state: &State) {
        if !state.catalog_loading() {
            self.retire_active_catalog();
        }
        for task in &mut self.start {
            if state.cancelling(task.generation) {
                task.cancel();
            }
        }
    }

    /// One drain: reconcile, admit the catalog when the Board allows it, then
    /// settle catalog updates, each workflow's progress, and its completion.
    pub(crate) fn drain(&mut self, state: &mut State, admission: CatalogAdmission) -> Drain {
        self.reconcile(state);
        let admitted = match admission {
            CatalogAdmission::Open(path) => self.ensure_catalog(state, path),
            CatalogAdmission::Closed => false,
        };
        self.reap_catalogs();
        let mut drained = Drain {
            changed: self.drain_catalog(state),
            started: false,
        };
        self.start.retain(|task| {
            if let Some(progress) = task
                .progress
                .try_lock()
                .ok()
                .and_then(|mut progress| progress.take())
            {
                drained.changed |= state.settle_start_progress(
                    task.generation,
                    match progress {
                        Progress::Observing => Phase::Starting,
                        Progress::WaitingInput => Phase::WaitingInput,
                    },
                );
            }
            match task.take() {
                Ok(completion) => {
                    let phase = report_phase(&completion.report);
                    let applied =
                        state.apply_completion(completion.generation, &completion.panel, phase);
                    drained.changed |= applied;
                    drained.started |= applied && phase == Phase::Started;
                    false
                }
                Err(TryRecvError::Disconnected) => {
                    drained.changed |= state.settle_start_lost(task.generation);
                    false
                }
                Err(TryRecvError::Empty) => true,
            }
        });
        drained.changed |= admitted;
        drained
    }

    /// Accepts one start in `state` and runs its workflow on the current origin. Only a workflow
    /// still running in the same Panel refuses it.
    pub(crate) fn start(
        &mut self,
        state: &mut State,
        episode: u64,
        panel: WorkbenchPanelId,
        clock: LocalOperationClock,
    ) -> bool {
        if self.start.iter().any(|task| task.panel == panel) {
            return false;
        }
        let Some(origin) = self.origin.clone() else {
            return false;
        };
        let Some(deadline) = Instant::now().checked_add(START_DEADLINE) else {
            return false;
        };
        let Some(operation) = state.begin_start(episode, panel, deadline) else {
            return false;
        };
        #[cfg(test)]
        if operation.without_task {
            return true;
        }
        let request = Request::new(operation.name, operation.provider, Vec::new());
        match StartTask::spawn(
            operation.generation,
            operation.panel,
            origin,
            clock,
            request,
            deadline,
        ) {
            Ok(task) => self.start.push(task),
            Err(_) => state.settle_start_spawn_failed(operation.generation),
        }
        true
    }

    /// Session teardown: wake and join every workflow, then cancel and join every lookup.
    pub(crate) fn close(&mut self) {
        for task in &mut self.start {
            task.cancel();
        }
        for mut task in self.start.drain(..) {
            let _ = task.task.join();
        }
        self.retire_active_catalog();
        for mut task in self.retiring_catalogs.drain(..) {
            task.cancel();
            let _ = task.task.join();
        }
    }

    /// Process exit: wakes every workflow and cancels every lookup without waiting. An in-flight
    /// start still cancels and releases its reservation on its own thread.
    pub(crate) fn request_stop(&mut self) {
        for task in &mut self.start {
            task.cancel();
        }
        self.retire_active_catalog();
        for task in &mut self.retiring_catalogs {
            task.cancel();
        }
    }

    /// Process exit: observes every workflow and lookup against the shared deadline.
    pub(crate) fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.request_stop();
        self.start
            .iter_mut()
            .map(|task| task.task.join_before(deadline))
            .chain(
                self.retiring_catalogs
                    .iter_mut()
                    .map(|task| task.task.join_before(deadline)),
            )
            .fold(Ok(()), std::io::Result::and)
    }

    fn ensure_catalog(&mut self, state: &mut State, path: Option<OsString>) -> bool {
        self.reap_catalogs();
        match state.begin_catalog(!self.retiring_catalogs.is_empty()) {
            CatalogBegin::Unchanged => false,
            CatalogBegin::Settled => true,
            CatalogBegin::Load(revision) => {
                match CatalogTask::spawn(revision, path) {
                    Ok(task) => self.catalog = Some(task),
                    Err(_) => state.settle_catalog_finished(false),
                }
                true
            }
        }
    }

    fn drain_catalog(&mut self, state: &mut State) -> bool {
        let mut changed = false;
        // One candidate per canonical provider plus one final verdict is the
        // complete bounded mailbox. Input never waits for a lookup or a join.
        for _ in 0..=crate::agent_provider::CANONICAL_IDS.len() {
            let Some(task) = self.catalog.as_ref() else {
                break;
            };
            let expected = task.revision;
            match task.take() {
                Ok((revision, _)) if revision != expected => continue,
                Ok((_, CatalogUpdate::Found(provider))) => {
                    changed |= state.settle_catalog_found(provider);
                }
                Ok((_, CatalogUpdate::Finished(complete))) => {
                    self.retire_active_catalog();
                    state.settle_catalog_finished(complete);
                    changed = true;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    self.retire_active_catalog();
                    state.settle_catalog_finished(false);
                    changed = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if changed {
            state.touch_catalog();
        }
        changed
    }

    fn retire_active_catalog(&mut self) {
        if let Some(mut task) = self.catalog.take() {
            task.cancel();
            self.retiring_catalogs.push(task);
        }
    }

    fn reap_catalogs(&mut self) {
        self.retiring_catalogs.retain(|task| !task.finished());
    }

    #[cfg(test)]
    pub(crate) fn queue_completion_t(&mut self, state: &State, report: Report) -> bool {
        let Some(operation) = state.presented() else {
            return false;
        };
        let (sender, outcomes) = mpsc::sync_channel(1);
        if sender
            .send(Completion {
                generation: operation.generation,
                panel: operation.panel.clone(),
                report,
            })
            .is_err()
        {
            return false;
        }
        drop(sender);
        let Ok((_reader, wake)) = LocalStream::pair() else {
            return false;
        };
        self.start.push(StartTask {
            generation: operation.generation,
            panel: operation.panel.clone(),
            progress: Arc::new(Mutex::new(None)),
            outcomes,
            wake,
            task: BackgroundThread::ended(),
            cancelled: false,
        });
        true
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        // Only close and stop_before wait; unfinished jobs are detached here.
        self.request_stop();
    }
}

fn report_phase(report: &Report) -> Phase {
    match &report.verdict {
        Verdict::Started { .. } | Verdict::Observed => Phase::Started,
        Verdict::Failed { failure, .. }
            if failure.timed_out()
                && report.delivery == crate::agent_start::DeliveryDisposition::Applied =>
        {
            Phase::Unconfirmed
        }
        // Observation showed the Panel back at its shell prompt: no Agent holds it.
        Verdict::Failed {
            failure: Failure::Start(StartFailure::AgentExited | StartFailure::LaunchFailed),
            ..
        } => Phase::Failed,
        Verdict::Failed { failure, .. }
            if failure.unknown()
                || matches!(
                    report.reservation_state,
                    ReservationState::Pending | ReservationState::Unknown
                )
                || report.delivery.possibly_applied() =>
        {
            Phase::Unknown
        }
        Verdict::Failed { failure, .. } if failure.cancelled() => Phase::Cancelled,
        Verdict::Failed { failure, .. } => refusal(failure).map_or(Phase::Failed, Phase::Refused),
    }
}

/// The prompt refusals that typed nothing. The Host's `busy` and `not-shell` read like the
/// workflow's own idle-shell refusal: the shell is not at an empty prompt.
fn refusal(failure: &Failure) -> Option<AgentStartRefusal> {
    let Failure::Start(failure) = failure else {
        return None;
    };
    Some(match failure {
        StartFailure::PromptUnconfirmed(SendPromptState::Unmarked) => AgentStartRefusal::Unmarked,
        StartFailure::TerminalNotIdle | StartFailure::PromptUnconfirmed(_) => {
            AgentStartRefusal::NotAtPrompt
        }
        StartFailure::SessionTooOld => AgentStartRefusal::SessionTooOld,
        StartFailure::TerminalTooOld => AgentStartRefusal::TerminalTooOld,
        _ => return None,
    })
}

enum CatalogUpdate {
    Found(&'static str),
    Finished(bool),
}

struct CatalogTask {
    revision: u64,
    outcomes: Receiver<(u64, CatalogUpdate)>,
    cancelled: Arc<AtomicBool>,
    task: BackgroundThread,
}

impl CatalogTask {
    fn spawn(revision: u64, path: Option<OsString>) -> std::io::Result<Self> {
        let deadline = Instant::now()
            .checked_add(CATALOG_DEADLINE)
            .ok_or_else(|| std::io::Error::other("Agent catalog deadline overflow"))?;
        let (sender, outcomes) = mpsc::sync_channel(crate::agent_provider::CANONICAL_IDS.len() + 1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = Arc::clone(&cancelled);
        let task = BackgroundThread::spawn("orchbee-agent-start-catalog", move || {
            let complete = client_path::client_path_candidates(
                path.as_deref(),
                task_cancelled.as_ref(),
                deadline,
                |provider| {
                    let _ = sender.send((revision, CatalogUpdate::Found(provider)));
                },
            );
            let _ = sender.send((revision, CatalogUpdate::Finished(complete)));
        })?;
        Ok(Self {
            revision,
            outcomes,
            cancelled,
            task,
        })
    }

    fn take(&self) -> Result<(u64, CatalogUpdate), TryRecvError> {
        self.outcomes.try_recv()
    }

    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn finished(&self) -> bool {
        self.task.is_finished()
    }
}

impl Drop for CatalogTask {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct Completion {
    generation: u64,
    panel: WorkbenchPanelId,
    report: Report,
}

struct StartTask {
    generation: u64,
    panel: WorkbenchPanelId,
    progress: Arc<Mutex<Option<Progress>>>,
    outcomes: Receiver<Completion>,
    wake: LocalStream,
    task: BackgroundThread,
    cancelled: bool,
}

impl StartTask {
    fn spawn(
        generation: u64,
        panel: WorkbenchPanelId,
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        request: Request,
        deadline: Instant,
    ) -> std::io::Result<Self> {
        let (sender, outcomes) = mpsc::sync_channel(1);
        let (reader, wake) = LocalStream::pair()?;
        let task_panel = panel.clone();
        let completion_panel = panel.clone();
        let progress = Arc::new(Mutex::new(None));
        let task_progress = Arc::clone(&progress);
        let task = BackgroundThread::spawn("orchbee-agent-start", move || {
            let started = Instant::now();
            let report = crate::agent_start::run_on_origin(
                request,
                started,
                deadline,
                Some(reader.as_local_io()),
                &origin,
                clock,
                task_panel,
                &|value| {
                    if let Ok(mut progress) = task_progress.lock() {
                        *progress = Some(value);
                    }
                },
            );
            let _ = sender.send(Completion {
                generation,
                panel: completion_panel,
                report,
            });
        })?;
        Ok(Self {
            generation,
            panel,
            progress,
            outcomes,
            wake,
            task,
            cancelled: false,
        })
    }

    fn take(&self) -> Result<Completion, TryRecvError> {
        self.outcomes.try_recv()
    }

    fn cancel(&mut self) {
        if self.cancelled {
            return;
        }
        self.cancelled = true;
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
    }
}

impl Drop for StartTask {
    fn drop(&mut self) {
        self.cancel();
    }
}
