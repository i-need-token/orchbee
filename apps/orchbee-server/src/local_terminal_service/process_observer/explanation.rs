//! Bounded on-demand reads share the sampler thread and its sole uncertain-viewer cleanup owner.
use super::{LocalTerminalService, ProcessObservationTarget, ScreenDetector, Wake};
use crate::local_terminal_service::viewer_connection::CapturedAgentExplanation;
use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_workbench_protocol::PanelAgentFailure as Failure;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const READ_WINDOW: Duration = Duration::from_secs(4);
#[cfg(test)]
mod tests;

struct Job {
    target: ProcessObservationTarget,
    provider: AgentReportProvider,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    result: mpsc::SyncSender<Option<CapturedAgentExplanation>>,
}
#[derive(Default)]
struct State {
    queued: Option<Job>,
    active: Option<Arc<AtomicBool>>,
}
#[derive(Default)]
pub(super) struct Requests(Mutex<State>);
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
impl Requests {
    pub(super) fn cancel(&self) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.queued.take();
        if let Some(active) = &state.active {
            active.store(true, Ordering::Release);
        }
    }
    fn enqueue(&self, job: Job, stop: &AtomicBool) -> Result<(), Failure> {
        let mut state = self.0.lock().map_err(|_| Failure::Unavailable)?;
        if stop.load(Ordering::Acquire) {
            return Err(Failure::Unavailable);
        }
        if state.queued.is_some() {
            return Err(Failure::ResourceLimit);
        }
        state.queued = Some(job);
        Ok(())
    }
    fn take(&self, now: Instant, stop: &AtomicBool) -> Option<Job> {
        let mut state = self.0.lock().ok()?;
        if state.active.is_some() {
            return None;
        }
        let job = state.queued.take()?;
        if stop.load(Ordering::Acquire)
            || job.cancelled.load(Ordering::Acquire)
            || now >= job.deadline
        {
            return None;
        }
        state.active = Some(Arc::clone(&job.cancelled));
        Some(job)
    }
    fn finish(&self, job: &Job) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .active
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, &job.cancelled))
        {
            state.active = None;
        }
    }
}
impl LocalTerminalService {
    pub(in crate::local_terminal_service) fn request_agent_explanation(
        &self,
        target: ProcessObservationTarget,
        provider: AgentReportProvider,
    ) -> Result<CapturedAgentExplanation, Failure> {
        let observer = &self.process_observer;
        if observer
            .handle
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .as_ref()
            .is_none_or(|h| h.is_finished())
        {
            return Err(Failure::Unavailable);
        }
        let deadline = Instant::now() + READ_WINDOW;
        let cancellation = CancelOnDrop(Arc::new(AtomicBool::new(false)));
        let (result, receiver) = mpsc::sync_channel(1);
        observer.wake.explanations.enqueue(
            Job {
                target,
                provider,
                deadline,
                cancelled: Arc::clone(&cancellation.0),
                result,
            },
            &observer.wake.stop,
        )?;
        if let Ok(_guard) = observer.wake.wait.lock() {
            observer.wake.changed.notify_all();
        }
        receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .ok()
            .flatten()
            .ok_or(Failure::Unavailable)
    }
}
pub(super) fn serve(service: &LocalTerminalService, wake: &Wake, detector: &ScreenDetector) {
    let Some(job) = wake.explanations.take(Instant::now(), &wake.stop) else {
        return;
    };
    let captured = (|| {
        let pending = wake.pending.lock().ok()?.take();
        if let Some(pending) = pending
            && !pending.settle(service)
        {
            *wake
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pending);
            return None;
        }
        if job.cancelled.load(Ordering::Acquire) || Instant::now() >= job.deadline {
            return None;
        }
        let (captured, pending) =
            service.sample_agent_explanation(&job.target, &job.cancelled, detector, job.provider);
        if let Some(pending) = pending {
            *wake
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pending);
            return None;
        }
        if job.cancelled.load(Ordering::Acquire) || Instant::now() >= job.deadline {
            None
        } else {
            captured
        }
    })();
    let _ = job.result.send(captured);
    wake.explanations.finish(&job);
}
