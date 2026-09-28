//! Windows Site-owned periodic process observations with one retained viewer cleanup responsibility.
use super::{LocalTerminalService, viewer_session::PendingCleanup};
use crate::agent_screen::ScreenDetector;
use crate::{site_coordinator::ProcessObservationTarget, worker_obcn_carrier::WorkerRequestHandle};
use std::{
    io,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const INTERVAL: Duration = Duration::from_millis(250);
const TURN_BUDGET: Duration = Duration::from_secs(1);
const TARGETS_PER_TURN: usize = 8;
type Cursor = (String, String);
#[derive(Default)]
struct Wake {
    stop: AtomicBool,
    wait: Mutex<()>,
    changed: Condvar,
    pending: Mutex<Option<PendingCleanup>>,
}
#[derive(Default)]
pub(super) struct ProcessObserver {
    wake: Arc<Wake>,
    handle: Mutex<Option<JoinHandle<()>>>,
}
impl ProcessObserver {
    fn cancel(&self) {
        self.wake.stop.store(true, Ordering::Release);
        if let Ok(_guard) = self.wake.wait.lock() {
            self.wake.changed.notify_all();
        }
    }
    fn start(&self, service: &Arc<LocalTerminalService>) -> io::Result<()> {
        let mut handle = self
            .handle
            .lock()
            .map_err(|_| io::Error::other("Process observer owner poisoned"))?;
        if self.wake.stop.load(Ordering::Acquire) {
            return Err(io::ErrorKind::NotConnected.into());
        }
        if handle.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let detector = ScreenDetector::bundled().map_err(io::Error::other)?;
        let service = Arc::downgrade(service);
        let wake = Arc::clone(&self.wake);
        *handle = Some(
            thread::Builder::new()
                .name("orchbee-process-observer".into())
                .spawn(move || run(service, wake, detector))?,
        );
        Ok(())
    }
    fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        self.cancel();
        loop {
            {
                let mut handle = self
                    .handle
                    .lock()
                    .map_err(|_| io::Error::other("Process observer handle poisoned"))?;
                if handle.as_ref().is_none_or(|h| h.is_finished()) {
                    if let Some(handle) = handle.take() {
                        handle
                            .join()
                            .map_err(|_| io::Error::other("Process observer panicked"))?;
                    }
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for ProcessObserver {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl LocalTerminalService {
    #[cfg(test)]
    pub(super) fn process_observer_turn_for_test(&self, cursor: &mut Option<Cursor>) {
        turn(
            self,
            &self.process_observer.wake,
            cursor,
            &ScreenDetector::bundled().unwrap(),
        );
    }
    #[cfg(test)]
    pub(super) fn process_observer_pending_for_test(&self) -> bool {
        self.process_observer.wake.pending.lock().unwrap().is_some()
    }
    /// Starts the sole periodic sampler; a stopped or already running owner cannot restart.
    pub(crate) fn start_process_observer(self: &Arc<Self>) -> io::Result<()> {
        self.process_observer.start(self)
    }
    /// Stops new sampling and joins admitted work; unresolved viewer cleanup transfers to Site shutdown.
    pub(crate) fn stop_process_observer_before(&self, deadline: Instant) -> io::Result<()> {
        self.process_observer.stop_before(deadline)
    }
    /// Only the managed Site owner calls this after matching Worker/Host stop plus actual Worker exit.
    pub(crate) fn finish_process_observer_after_worker_exit(
        &self,
        worker: &WorkerRequestHandle,
    ) -> io::Result<()> {
        if !self.process_observer.wake.stop.load(Ordering::Acquire)
            || self
                .process_observer
                .handle
                .lock()
                .map_err(|_| io::Error::other("Process observer handle poisoned"))?
                .is_some()
        {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let mut pending = self
            .process_observer
            .wake
            .pending
            .lock()
            .map_err(|_| io::Error::other("Process observer cleanup poisoned"))?;
        if pending.as_ref().is_some_and(|p| !p.matches_worker(worker)) {
            return Err(io::Error::other(
                "Process observer cleanup belongs to another Worker",
            ));
        }
        *pending = None;
        Ok(())
    }
}
fn select_turn(targets: &[ProcessObservationTarget], cursor: Option<&Cursor>) -> Vec<usize> {
    if targets.is_empty() {
        return vec![];
    }
    let start = cursor
        .and_then(|cursor| targets.iter().position(|t| t.key() > *cursor))
        .unwrap_or(0);
    (0..TARGETS_PER_TURN.min(targets.len()))
        .map(|n| (start + n) % targets.len())
        .collect()
}
fn turn(
    service: &LocalTerminalService,
    wake: &Wake,
    cursor: &mut Option<Cursor>,
    detector: &ScreenDetector,
) {
    let deadline = Instant::now() + TURN_BUDGET;
    let pending = match wake.pending.lock() {
        Ok(mut p) => p.take(),
        Err(_) => return,
    };
    if let Some(pending) = pending
        && !pending.settle(service)
    {
        *wake
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pending);
        return;
    }
    if wake.stop.load(Ordering::Acquire) {
        return;
    }
    let targets = {
        let Ok(mut coordinator) = service.coordinator.lock() else {
            return;
        };
        let Ok(targets) = coordinator.process_observation_targets() else {
            return;
        };
        targets
    };
    if targets.is_empty() {
        *cursor = None;
        return;
    }
    for index in select_turn(&targets, cursor.as_ref()) {
        if wake.stop.load(Ordering::Acquire) || Instant::now() >= deadline {
            break;
        }
        let target = &targets[index];
        *cursor = Some(target.key());
        if let Some(pending) = service.sample_process_target(target, &wake.stop, detector) {
            *wake
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pending);
            break;
        }
    }
}
fn run(service: Weak<LocalTerminalService>, wake: Arc<Wake>, detector: ScreenDetector) {
    let mut cursor = None;
    loop {
        if wake.stop.load(Ordering::Acquire) {
            return;
        }
        let Some(service) = service.upgrade() else {
            return;
        };
        turn(&service, &wake, &mut cursor, &detector);
        drop(service);
        let Ok(guard) = wake.wait.lock() else {
            return;
        };
        if wake.stop.load(Ordering::Acquire) {
            return;
        }
        if wake.changed.wait_timeout(guard, INTERVAL).is_err() {
            return;
        }
    }
}
#[cfg(test)]
mod tests;
