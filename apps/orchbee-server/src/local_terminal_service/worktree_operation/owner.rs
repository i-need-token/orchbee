use super::LocalTerminalService;
use std::{
    io,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[derive(Default)]
struct Wake {
    pending: Mutex<bool>,
    changed: Condvar,
    stop: AtomicBool,
    available: AtomicBool,
}
#[derive(Default)]
pub(in super::super) struct WorktreeMaintenance {
    wake: Arc<Wake>,
    handle: Mutex<Option<JoinHandle<()>>>,
}
impl WorktreeMaintenance {
    pub(super) fn available(&self) -> bool {
        self.wake.available.load(Ordering::Acquire) && !self.wake.stop.load(Ordering::Acquire)
    }
    pub(super) fn notify(&self) {
        if let Ok(mut pending) = self.wake.pending.lock() {
            *pending = true;
            self.wake.changed.notify_all();
        }
    }
    pub(super) fn ensure(&self, service: &Arc<LocalTerminalService>) -> io::Result<()> {
        let mut handle = self
            .handle
            .lock()
            .map_err(|_| io::Error::other("Worktree owner poisoned"))?;
        if self.wake.stop.load(Ordering::Acquire) {
            return Err(io::ErrorKind::NotConnected.into());
        }
        if handle.is_some() {
            return if self.available() {
                Ok(())
            } else {
                Err(io::ErrorKind::NotConnected.into())
            };
        }
        let service = Arc::downgrade(service);
        let wake = Arc::clone(&self.wake);
        self.wake.available.store(true, Ordering::Release);
        match thread::Builder::new()
            .name("orchbee-worktree-operations".into())
            .spawn(move || run(service, wake))
        {
            Ok(thread) => {
                *handle = Some(thread);
                Ok(())
            }
            Err(error) => {
                self.wake.available.store(false, Ordering::Release);
                Err(error)
            }
        }
    }
    #[cfg(any(windows, test))]
    pub(super) fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        self.wake.available.store(false, Ordering::Release);
        self.wake.stop.store(true, Ordering::Release);
        self.notify();
        loop {
            {
                let mut handle = self
                    .handle
                    .lock()
                    .map_err(|_| io::Error::other("Worktree handle poisoned"))?;
                if handle.as_ref().is_none_or(|thread| thread.is_finished()) {
                    if let Some(thread) = handle.take() {
                        thread
                            .join()
                            .map_err(|_| io::Error::other("Worktree owner panicked"))?;
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
impl Drop for WorktreeMaintenance {
    fn drop(&mut self) {
        self.wake.available.store(false, Ordering::Release);
        self.wake.stop.store(true, Ordering::Release);
        self.notify();
        // Explicit Site shutdown owns joining; Drop only signals cancellation.
    }
}
struct Running(Arc<Wake>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.available.store(false, Ordering::Release);
    }
}
fn run(service: Weak<LocalTerminalService>, wake: Arc<Wake>) {
    let _running = Running(Arc::clone(&wake));
    loop {
        let Ok(mut pending) = wake.pending.lock() else {
            return;
        };
        while !*pending && !wake.stop.load(Ordering::Acquire) {
            let Ok(next) = wake.changed.wait(pending) else {
                return;
            };
            pending = next;
        }
        *pending = false;
        drop(pending);
        let Some(service) = service.upgrade() else {
            return;
        };
        // Claim and publish a queued request even if stop won before its first turn.
        service.worktree_turn(&wake.stop);
        if wake.stop.load(Ordering::Acquire) {
            return;
        }
    }
}
