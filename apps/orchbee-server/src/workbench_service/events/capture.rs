//! A connection owns one physical read and joins it before releasing observation authority.
use crate::{
    local_terminal_service::{
        LocalTerminalService,
        semantic_events::{OutputRead, OutputSample},
    },
    session_coordinator::ProcessObservationTarget,
};
use orchbee_workbench_protocol::{
    SemanticEventsFailure as Failure, SemanticPane, WorkbenchRevision,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};

struct Permit(Arc<AtomicBool>);
impl Permit {
    fn acquire(slot: &Arc<AtomicBool>) -> Option<Self> {
        slot.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self(Arc::clone(slot)))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
struct Running {
    probe: usize,
    _permit: Permit,
    stopped: Arc<AtomicBool>,
    completion: mpsc::Receiver<Result<OutputSample, Failure>>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub(super) struct Capture {
    running: Option<Running>,
    slot: Arc<AtomicBool>,
}
impl Capture {
    pub(super) fn new(slot: Arc<AtomicBool>) -> Self {
        Self {
            running: None,
            slot,
        }
    }
    pub(super) fn cancel(&mut self) {
        // Running::drop cancels and joins before releasing the shared capture permit.
        self.running = None;
    }
    pub(super) fn pending_for(&self, probe: usize) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| running.probe == probe)
    }
    pub(super) fn busy_for_another(&self, probe: usize) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| running.probe != probe)
    }
    pub(super) fn sample(
        &mut self,
        probe: usize,
        service: Arc<LocalTerminalService>,
        target: &ProcessObservationTarget,
        pane: &SemanticPane,
        revision: WorkbenchRevision,
        read: OutputRead,
    ) -> Result<Option<OutputSample>, Failure> {
        if let Some(running) = self.running.as_ref() {
            if running.probe != probe {
                return Ok(None);
            }
            let result = match running.completion.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return Ok(None),
                Err(mpsc::TryRecvError::Disconnected) => Err(Failure::Unavailable),
            };
            self.running = None;
            return match result {
                // Safe, side-effect-free backpressure/version races establish no false match.
                Err(Failure::Busy | Failure::SourceChanged) => Ok(None),
                result => result.map(Some),
            };
        }
        let target = target.clone();
        let pane = pane.clone();
        match self.start(probe, move |stop| {
            service.capture_event_output(&target, pane, revision, &read, stop)
        }) {
            Ok(()) | Err(Failure::Busy) => Ok(None),
            Err(error) => Err(error),
        }
    }
    pub(super) fn start(
        &mut self,
        probe: usize,
        read: impl FnOnce(&AtomicBool) -> Result<OutputSample, Failure> + Send + 'static,
    ) -> Result<(), Failure> {
        // Sibling subscriptions keep separate cursors and cancellation owners, but cannot
        // create one Host reader per stream. The permit is held until its owner joins the job.
        let permit = Permit::acquire(&self.slot).ok_or(Failure::Busy)?;
        let stopped = Arc::new(AtomicBool::new(false));
        let actor_stop = Arc::clone(&stopped);
        let (sender, completion) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("orchbee-event-observer".into())
            .spawn(move || {
                let _ = sender.send(read(&actor_stop));
            })
            .map_err(|_| Failure::Unavailable)?;
        self.running = Some(Running {
            probe,
            _permit: permit,
            stopped,
            completion,
            thread: Some(thread),
        });
        Ok(())
    }
}
