use std::{
    io,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use super::LocalTerminalService;

#[derive(Default)]
struct WakeState {
    pending: bool,
    stop: bool,
}

#[derive(Default)]
struct Wake {
    state: Mutex<WakeState>,
    changed: Condvar,
    available: AtomicBool,
}

#[derive(Default)]
pub(super) struct CloseMaintenance {
    wake: Arc<Wake>,
    handle: Mutex<Option<JoinHandle<()>>>,
    #[cfg(test)]
    after_scan: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    #[cfg(test)]
    after_admission: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    #[cfg(test)]
    after_result: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl CloseMaintenance {
    pub(super) fn available(&self) -> bool {
        self.wake.available.load(Ordering::Acquire)
    }

    pub(super) fn notify(&self) {
        if let Ok(mut state) = self.wake.state.lock() {
            state.pending = true;
            self.wake.changed.notify_one();
        }
    }
}

impl Drop for CloseMaintenance {
    fn drop(&mut self) {
        self.wake.available.store(false, Ordering::Release);
        if let Ok(mut state) = self.wake.state.lock() {
            state.stop = true;
            self.wake.changed.notify_one();
        }
        // Existing Site ownership is process-lifetime. Dropping the handle never joins.
    }
}

struct RunningOwner(Arc<Wake>);
impl Drop for RunningOwner {
    fn drop(&mut self) {
        self.0.available.store(false, Ordering::Release);
    }
}

impl LocalTerminalService {
    /// Stops the maintenance submitter and joins it within the Site owner's deadline.
    #[cfg(windows)]
    pub(crate) fn stop_close_before(&self, deadline: Instant) -> io::Result<()> {
        self.close_maintenance
            .wake
            .available
            .store(false, Ordering::Release);
        {
            let mut state = self
                .close_maintenance
                .wake
                .state
                .lock()
                .map_err(|_| io::Error::other("Close owner poisoned"))?;
            state.stop = true;
            self.close_maintenance.wake.changed.notify_all();
        }
        loop {
            {
                let mut handle = self
                    .close_maintenance
                    .handle
                    .lock()
                    .map_err(|_| io::Error::other("Close handle poisoned"))?;
                if handle.as_ref().is_none_or(|thread| thread.is_finished()) {
                    if let Some(thread) = handle.take() {
                        thread
                            .join()
                            .map_err(|_| io::Error::other("Close owner panicked"))?;
                    }
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[cfg(test)]
    pub(super) fn after_close_admission_for_test(&self, hook: impl FnOnce() + Send + 'static) {
        *self
            .close_maintenance
            .after_admission
            .lock()
            .expect("admission hook") = Some(Box::new(hook));
    }

    #[cfg(test)]
    pub(super) fn after_close_result_for_test(&self, hook: impl FnOnce() + Send + 'static) {
        *self
            .close_maintenance
            .after_result
            .lock()
            .expect("result hook") = Some(Box::new(hook));
    }

    #[cfg(test)]
    pub(super) fn run_close_admission_hook(&self) {
        let hook = self
            .close_maintenance
            .after_admission
            .lock()
            .expect("admission hook")
            .take();
        if let Some(hook) = hook {
            hook();
        }
    }

    #[cfg(test)]
    pub(super) fn run_close_result_hook(&self) {
        let hook = self
            .close_maintenance
            .after_result
            .lock()
            .expect("result hook")
            .take();
        if let Some(hook) = hook {
            hook();
        }
    }
    #[cfg(test)]
    pub(super) fn after_close_scan_for_test(&self, hook: impl FnOnce() + Send + 'static) {
        *self.close_maintenance.after_scan.lock().expect("scan hook") = Some(Box::new(hook));
    }
    pub(crate) fn start_close_maintenance(self: &Arc<Self>) -> io::Result<()> {
        self.start_close_maintenance_with(|task| {
            thread::Builder::new()
                .name("orchbee-close-maintenance".into())
                .spawn(task)
        })
    }

    pub(super) fn start_close_maintenance_with(
        self: &Arc<Self>,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> io::Result<JoinHandle<()>>,
    ) -> io::Result<()> {
        let mut slot = self
            .close_maintenance
            .handle
            .lock()
            .map_err(|_| io::Error::other("Close owner poisoned"))?;
        if slot.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Close owner already started",
            ));
        }
        let weak = Arc::downgrade(self);
        let wake = Arc::clone(&self.close_maintenance.wake);
        wake.available.store(true, Ordering::Release);
        let thread_wake = Arc::clone(&wake);
        match spawn(Box::new(move || run(weak, thread_wake))) {
            Ok(handle) => *slot = Some(handle),
            Err(error) => {
                wake.available.store(false, Ordering::Release);
                return Err(error);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn stop_close_maintenance(&self) {
        self.close_maintenance
            .wake
            .available
            .store(false, Ordering::Release);
        {
            let mut state = self
                .close_maintenance
                .wake
                .state
                .lock()
                .expect("wake owner");
            state.stop = true;
            self.close_maintenance.wake.changed.notify_one();
        }
        let handle = self
            .close_maintenance
            .handle
            .lock()
            .expect("handle owner")
            .take();
        if let Some(handle) = handle {
            handle
                .join()
                .expect("Close owner must settle before test join");
        }
    }
}

fn run(service: Weak<LocalTerminalService>, wake: Arc<Wake>) {
    let _running = RunningOwner(Arc::clone(&wake));
    let mut cursor = None;
    loop {
        {
            let Ok(mut state) = wake.state.lock() else {
                return;
            };
            if state.stop {
                return;
            }
            // Clear BEFORE scanning: concurrent admission cannot lose its notification.
            state.pending = false;
        }
        let Some(service) = service.upgrade() else {
            return;
        };
        #[cfg(windows)]
        let popup_due = service.popup_maintenance_turn();
        let Ok(due) = service.close_maintenance_turn(&mut cursor) else {
            return;
        };
        #[cfg(windows)]
        let due = match (due, popup_due) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        #[cfg(test)]
        {
            let hook = service
                .close_maintenance
                .after_scan
                .lock()
                .expect("scan hook")
                .take();
            if let Some(hook) = hook {
                hook();
            }
        }
        drop(service);
        let Ok(mut state) = wake.state.lock() else {
            return;
        };
        while !state.stop && !state.pending {
            state = if let Some(due) = due {
                let now = Instant::now();
                if due <= now {
                    break;
                }
                let Ok((next, _)) = wake.changed.wait_timeout(state, due.duration_since(now))
                else {
                    return;
                };
                next
            } else {
                let Ok(next) = wake.changed.wait(state) else {
                    return;
                };
                next
            };
        }
    }
}
