//! The Agent start's private background jobs: one Agent startup workflow per
//! started Panel.
//!
//! The composition owns these jobs beside the Board and drives them against
//! the start state. Each job settles its own outcome into that state, and the
//! state never holds a thread or transport handle. Session teardown cancels and
//! joins every job within the workflow's existing bounded I/O and release
//! contract.

use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

use orchbee_agent_observation_protocol::AgentProviderId;
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::WorkbenchPanelId;

use super::{Drain, Phase, State};
use crate::{
    agent_start::{Failure, Progress, Report, Request, ReservationState, StartFailure, Verdict},
    background_thread::BackgroundThread,
    local_client::{LocalClientOrigin, LocalOperationClock},
    workbench_state::AgentStartRefusal,
};
use orchbee_client_server_protocol::SendPromptState;

/// Bounds a start in an existing Panel: admission, the one send and the early check. Nothing
/// waits for the Agent to become ready.
const START_DEADLINE: Duration = Duration::from_secs(20);

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct Jobs {
    origin: Option<LocalClientOrigin>,
    /// At most one workflow per Panel.
    start: Vec<StartTask>,
}

impl Jobs {
    pub(crate) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    /// One drain: settle each workflow's progress and its completion.
    pub(crate) fn drain(&mut self, state: &mut State) -> Drain {
        let mut drained = Drain::default();
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
        drained
    }

    /// Accepts one start of `provider` in `panel` and runs its workflow on the current origin.
    /// Only a workflow still running in the same Panel refuses it. A start moves to a new Tab when
    /// its prompt refuses it.
    pub(crate) fn start(
        &mut self,
        state: &mut State,
        provider: &'static str,
        place: Place,
        clock: LocalOperationClock,
    ) -> bool {
        let Place::Panel(panel) = place;
        if self.start.iter().any(|task| task.panel == panel) {
            return false;
        }
        let Some(origin) = self.origin.clone() else {
            return false;
        };
        let Some(deadline) = Instant::now().checked_add(START_DEADLINE) else {
            return false;
        };
        let Some(operation) = state.begin_start(provider, panel, true, deadline) else {
            return false;
        };
        #[cfg(test)]
        if operation.without_task {
            return true;
        }
        match StartTask::spawn(
            operation.generation,
            operation.panel,
            origin,
            clock,
            operation.provider,
            deadline,
        ) {
            Ok(task) => self.start.push(task),
            Err(_) => state.settle_start_spawn_failed(operation.generation),
        }
        true
    }

    /// Session teardown: wake and join every workflow.
    pub(crate) fn close(&mut self) {
        for task in &mut self.start {
            task.cancel();
        }
        for mut task in self.start.drain(..) {
            let _ = task.task.join();
        }
    }

    /// Process exit: wakes every workflow without waiting. An in-flight start still cancels and
    /// releases its reservation on its own thread.
    pub(crate) fn request_stop(&mut self) {
        for task in &mut self.start {
            task.cancel();
        }
    }

    /// Process exit: observes every workflow against the shared deadline.
    pub(crate) fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.request_stop();
        self.start
            .iter_mut()
            .map(|task| task.task.join_before(deadline))
            .fold(Ok(()), std::io::Result::and)
    }

    #[cfg(test)]
    pub(crate) fn queue_completion_t(&mut self, state: &State, report: Report) -> bool {
        let Some((generation, panel)) = state.latest() else {
            return false;
        };
        let (sender, outcomes) = mpsc::sync_channel(1);
        if sender
            .send(Completion {
                generation,
                panel: panel.clone(),
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
            generation,
            panel: panel.clone(),
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
    let product = || {
        crate::agent_provider::choice(report.provider.as_str())
            .map_or("Agent", |choice| choice.product)
    };
    match &report.verdict {
        Verdict::Started { .. } | Verdict::Observed { .. } | Verdict::Sent { .. } => Phase::Started,
        // Observation showed the Panel back at its shell prompt: no Agent holds it.
        Verdict::Failed {
            failure: Failure::Start(StartFailure::AgentExited),
            ..
        } => Phase::Exited(product()),
        Verdict::Failed {
            failure: Failure::Start(StartFailure::CommandNotFound),
            ..
        } => Phase::NotFound(product()),
        Verdict::Failed {
            failure: Failure::Start(StartFailure::LaunchFailed),
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

/// Where one accepted start runs: an existing Panel, where the command is typed. A start in a new
/// Panel runs no job here: the Server delivers and watches it.
pub(crate) enum Place {
    Panel(WorkbenchPanelId),
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
        provider: AgentProviderId,
        deadline: Instant,
    ) -> std::io::Result<Self> {
        let (sender, outcomes) = mpsc::sync_channel(1);
        let (reader, wake) = LocalStream::pair()?;
        let completion_panel = panel.clone();
        let progress = Arc::new(Mutex::new(None));
        let task_progress = Arc::clone(&progress);
        let task_panel = panel.clone();
        let task = BackgroundThread::spawn("orchbee-agent-start", move || {
            let report = crate::agent_start::run_on_origin(
                Request::new(None, provider, Vec::new()),
                Instant::now(),
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
