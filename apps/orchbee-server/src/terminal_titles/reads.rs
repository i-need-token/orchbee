//! The read pump: one thread that exchanges the title change reads the registry asks for with the
//! active Worker. Each read is admitted to the carrier's idle lane, so it waits behind every
//! queued input, viewer or Agent request, and the pump keeps at most one read outstanding.
use super::{NextRead, TerminalTitles};
use crate::{
    worker_obcn_carrier::{ExchangeWait, WorkerOperationResult},
    worker_registry::WorkerRegistry,
};
use orchbee_capability_node_protocol::{TerminalTitleChangesOutcome, TerminalTitleChangesRequest};
use std::{
    io,
    sync::{Arc, Mutex, Weak},
    thread,
    time::{Duration, Instant},
};

/// How often a pump waiting on an exchange checks for a stop.
const STOP_POLL: Duration = Duration::from_millis(100);

impl TerminalTitles {
    /// Starts the read pump. Only Session composition starts it, once.
    pub(crate) fn start_reads(
        self: &Arc<Self>,
        workers: Weak<Mutex<WorkerRegistry>>,
    ) -> io::Result<()> {
        let mut slot = self
            .reader
            .lock()
            .map_err(|_| io::Error::other("Title reader poisoned"))?;
        if slot.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Title reader already started",
            ));
        }
        let titles = Arc::clone(self);
        *slot = Some(
            thread::Builder::new()
                .name("orchbee-terminal-titles".into())
                .spawn(move || titles.run(&workers))?,
        );
        Ok(())
    }

    /// Stops the read pump and joins it within the Session owner's deadline.
    #[cfg(any(windows, test))]
    pub(crate) fn stop_reads_before(&self, deadline: Instant) -> io::Result<()> {
        self.stop();
        loop {
            {
                let mut slot = self
                    .reader
                    .lock()
                    .map_err(|_| io::Error::other("Title reader poisoned"))?;
                if slot.as_ref().is_none_or(thread::JoinHandle::is_finished) {
                    if let Some(reader) = slot.take() {
                        reader
                            .join()
                            .map_err(|_| io::Error::other("Title reader panicked"))?;
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

    fn run(&self, workers: &Weak<Mutex<WorkerRegistry>>) {
        loop {
            match self.next_read(Instant::now()) {
                NextRead::Stopped => return,
                NextRead::Wait(deadline) => self.wait(deadline),
                NextRead::Read(request) => {
                    let outcome = self.exchange(workers, request);
                    self.apply(&request, outcome, Instant::now());
                }
            }
        }
    }

    /// `None` when the read could not be admitted or answered; the registry retries it later.
    fn exchange(
        &self,
        workers: &Weak<Mutex<WorkerRegistry>>,
        request: TerminalTitleChangesRequest,
    ) -> Option<TerminalTitleChangesOutcome> {
        let handle = {
            let workers = workers.upgrade()?;
            let workers = workers.lock().ok()?;
            workers.request_handle()?
        };
        let pending = handle.admit_title_changes(request).ok()?;
        drop(handle);
        // A retiring carrier completes every queued read, so this wait ends with the carrier.
        loop {
            match pending.recv_deadline(Instant::now() + STOP_POLL) {
                ExchangeWait::Ready(result) => {
                    return match *result {
                        Ok(WorkerOperationResult::TitleChanges(result)) => Some(result.outcome),
                        Ok(_) | Err(_) => None,
                    };
                }
                ExchangeWait::Lost(_) => return None,
                ExchangeWait::Elapsed if self.stopped() => return None,
                ExchangeWait::Elapsed => {}
            }
        }
    }
}
