//! Bounded on-demand reads share the sampler thread and its sole uncertain-viewer cleanup owner.
//! Each kind of read keeps one active and one waiting request, so no kind waits behind another
//! kind's queue, and a waiting read takes the sampler before its periodic work continues.
use super::{LocalTerminalService, ProcessObservationTarget, Wake};
use crate::local_terminal_service::viewer_connection::PendingCleanup;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
#[cfg(all(test, windows))]
mod tests;

/// One read of `target`: `parameters` say what to read and `result` receives it.
pub(super) struct Job<P, T> {
    pub(super) target: ProcessObservationTarget,
    pub(super) parameters: P,
    pub(super) deadline: Instant,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) result: mpsc::SyncSender<Option<T>>,
}
struct State<P, T> {
    queued: Option<Job<P, T>>,
    active: Option<Arc<AtomicBool>>,
}
pub(super) struct Requests<P, T>(Mutex<State<P, T>>);
impl<P, T> Default for Requests<P, T> {
    fn default() -> Self {
        Self(Mutex::new(State {
            queued: None,
            active: None,
        }))
    }
}
/// Why a read got no answer: another read of its kind already waits, or nothing could serve it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    Busy,
    Unavailable,
}
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
impl<P, T> Requests<P, T> {
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
    fn enqueue(&self, job: Job<P, T>, stop: &AtomicBool) -> Result<(), Refusal> {
        let mut state = self.0.lock().map_err(|_| Refusal::Unavailable)?;
        if stop.load(Ordering::Acquire) {
            return Err(Refusal::Unavailable);
        }
        if state.queued.is_some() {
            return Err(Refusal::Busy);
        }
        state.queued = Some(job);
        Ok(())
    }
    /// A queued read the sampler has not started, which its next wait must not delay.
    pub(super) fn waiting(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|state| state.queued.is_some() && state.active.is_none())
    }
    fn take(&self, now: Instant, stop: &AtomicBool) -> Option<Job<P, T>> {
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
    fn finish(&self, job: &Job<P, T>) {
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
    /// Queues one read for the sampler thread and waits for its answer within `window`. Dropping
    /// the wait cancels the read, so an abandoned read never attaches.
    pub(super) fn request_on_demand<P, T>(
        &self,
        queue: impl FnOnce(&Wake) -> &Requests<P, T>,
        target: ProcessObservationTarget,
        parameters: P,
        window: Duration,
    ) -> Result<T, Refusal> {
        let observer = &self.process_observer;
        if observer
            .handle
            .lock()
            .map_err(|_| Refusal::Unavailable)?
            .as_ref()
            .is_none_or(|h| h.is_finished())
        {
            return Err(Refusal::Unavailable);
        }
        let deadline = Instant::now() + window;
        let cancellation = CancelOnDrop(Arc::new(AtomicBool::new(false)));
        let (result, receiver) = mpsc::sync_channel(1);
        queue(&observer.wake).enqueue(
            Job {
                target,
                parameters,
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
            .ok_or(Refusal::Unavailable)
    }
}
/// Serves the waiting read in `requests`: settles the retained cleanup first, then samples unless
/// the read was cancelled or expired. An uncertain cleanup from the sample stays retained.
pub(super) fn serve<P, T>(
    service: &LocalTerminalService,
    wake: &Wake,
    requests: &Requests<P, T>,
    sample: impl FnOnce(&Job<P, T>) -> (Option<T>, Option<PendingCleanup>),
) {
    let Some(job) = requests.take(Instant::now(), &wake.stop) else {
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
        let (captured, pending) = sample(&job);
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
    requests.finish(&job);
}
