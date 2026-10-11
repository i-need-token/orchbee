//! One cancellable read/close job, with submission identity owned before the thread receives it.
use super::state::{NativeSessionState, OverlayJob, OverlayResult};
use crate::local_client::{
    LocalClientOrigin, LocalOperationClock,
    workbench::panel_close::{WorkbenchPanelCloseRequest, WorkbenchPanelCloseSubmission},
};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{
    PluginAction, PluginOutcome, PluginRequest, ProtocolVersion, WorkbenchAction, WorkbenchCommand,
    WorkbenchRequestId,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Lane {
    sender: Option<SyncSender<OverlayJob>>,
    outcomes: Receiver<(OverlayJob, OverlayResult)>,
    wake: LocalStream,
    stopping: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}
impl Lane {
    fn spawn(origin: LocalClientOrigin, clock: LocalOperationClock) -> std::io::Result<Self> {
        let (sender, commands) = mpsc::sync_channel::<OverlayJob>(1);
        let (output, outcomes) = mpsc::sync_channel(1);
        let (reader, wake) = LocalStream::pair()?;
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let task = thread::Builder::new()
            .name("orchbee-plugin-overlays".into())
            .spawn(move || {
                while let Ok(job) = commands.recv() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = execute(&origin, &clock, &job, &reader, &stop);
                    if output.send((job, result)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            outcomes,
            wake,
            stopping,
            task: Some(task),
        })
    }
    fn interrupt(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.sender.take();
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        self.interrupt();
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

pub(super) struct Driver {
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    lane: Option<Lane>,
    inflight: Option<OverlayJob>,
    next: Option<Instant>,
}
impl Driver {
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        Self {
            clock,
            origin: None,
            lane: None,
            inflight: None,
            next: None,
        }
    }
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.origin = Some(origin);
    }
    pub(super) fn close(&mut self) {
        self.lane.take();
        self.origin = None;
        self.inflight = None;
        self.next = None;
    }
    pub(super) fn tick(&mut self, state: &mut NativeSessionState) -> bool {
        let now = self.clock.now();
        let mut changed = false;
        if let Some((job, result)) = self
            .lane
            .as_ref()
            .and_then(|lane| lane.outcomes.try_recv().ok())
            && self
                .inflight
                .as_ref()
                .is_some_and(|pending| pending.id == job.id)
        {
            changed = matches!(result, OverlayResult::CloseOwned { .. });
            state.finish_overlay_job(&job, result);
            self.inflight = None;
            self.next = now.checked_add(Duration::from_secs(1));
        }
        if self
            .lane
            .as_ref()
            .is_some_and(|lane| lane.task.as_ref().is_none_or(JoinHandle::is_finished))
        {
            self.lane.take();
            if let Some(job) = self.inflight.take() {
                state.finish_overlay_job(
                    &job,
                    OverlayResult::CloseOwned {
                        operation_id: job.id.clone(),
                    },
                );
                changed = true;
            }
            self.next = now.checked_add(Duration::from_secs(1));
        }
        if let Some(job) = &self.inflight {
            if now >= job.deadline
                && let Some(lane) = &mut self.lane
            {
                lane.interrupt();
            }
            return changed;
        }
        if self.next.is_some_and(|next| now < next) {
            return changed;
        }
        let Some(origin) = self.origin.clone() else {
            return changed;
        };
        let Some(deadline) = now.checked_add(Duration::from_secs(10)) else {
            return changed;
        };
        let Ok(id) = request_id() else {
            return changed;
        };
        let Some(job) = state.begin_overlay_job(id, deadline) else {
            return changed;
        };
        if self.lane.is_none() {
            match Lane::spawn(origin, self.clock.clone()) {
                Ok(lane) => self.lane = Some(lane),
                Err(_) => {
                    state.finish_overlay_job(&job, OverlayResult::Waiting);
                    self.next = now.checked_add(Duration::from_secs(1));
                    return changed;
                }
            }
        }
        if self
            .lane
            .as_ref()
            .and_then(|lane| lane.sender.as_ref())
            .is_some_and(|sender| sender.try_send(job.clone()).is_ok())
        {
            self.inflight = Some(job);
        } else {
            state.finish_overlay_job(&job, OverlayResult::Waiting);
            self.next = now.checked_add(Duration::from_secs(1));
        }
        changed
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.close();
    }
}

fn execute(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    job: &OverlayJob,
    wake: &LocalStream,
    stop: &AtomicBool,
) -> OverlayResult {
    let before = || stop.load(Ordering::Acquire) || clock.now() >= job.deadline;
    let Ok(mut client) =
        origin.reconnect_before_cancellable(job.deadline, clock.clone(), Some(wake.as_local_io()))
    else {
        return OverlayResult::Waiting;
    };
    let mut read = |action| {
        if before() {
            return None;
        }
        client
            .plugin(
                PluginRequest {
                    protocol_version: ProtocolVersion,
                    request_id: request_id().ok()?,
                    site_id: job.site.clone(),
                    action,
                },
                job.deadline,
                Some(wake.as_local_io()),
            )
            .ok()
            .map(|result| result.outcome)
    };
    let Some(PluginOutcome::PaneFound { pane, .. }) = read(PluginAction::PaneGet {
        panel_id: job.member.panel_id.clone(),
    }) else {
        return OverlayResult::Waiting;
    };
    if pane.terminal_context_id != job.context
        || pane.tab_id != job.tab
        || pane.presentation_id != job.member.presentation_id
        || Some(&pane.binding) != job.member.binding.as_ref()
    {
        return OverlayResult::Waiting;
    }
    let Some(PluginOutcome::PaneObserved {
        revision,
        settled: true,
        ..
    }) = read(PluginAction::PaneObserve { pane })
    else {
        return OverlayResult::Waiting;
    };
    if before() {
        return OverlayResult::Waiting;
    }
    let Ok(request) = WorkbenchPanelCloseRequest::try_from(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: job.id.clone(),
        site_id: job.site.clone(),
        expected_revision: revision,
        action: WorkbenchAction::PanelClose {
            terminal_context_id: job.context.clone(),
            tab_id: job.tab.clone(),
            panel_id: job.member.panel_id.clone(),
        },
    }) else {
        return OverlayResult::Waiting;
    };
    match client.submit_workbench_panel_close_cancellable(
        request,
        job.deadline,
        Some(wake.as_local_io()),
    ) {
        WorkbenchPanelCloseSubmission::Accepted(receipt) => OverlayResult::CloseOwned {
            operation_id: receipt.close_request_id,
        },
        WorkbenchPanelCloseSubmission::OutcomeUnknown { request_id, .. } => {
            OverlayResult::CloseOwned {
                operation_id: request_id,
            }
        }
        WorkbenchPanelCloseSubmission::DefinitelyNotSubmitted(_)
        | WorkbenchPanelCloseSubmission::DefinitePrecommit(_) => OverlayResult::Waiting,
    }
}

fn request_id() -> Result<WorkbenchRequestId, ()> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ())?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes))).map_err(|_| ())
}

#[cfg(test)]
mod tests;
