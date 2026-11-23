//! The Server's move turns on Windows (`proposals/platform/host-handover.md`, H3 design). While a
//! Worker that adopted its Hosts serves, a thread sends it a move turn every 2 seconds; each terminal
//! it moved is rebound, and the binding record names the new Host before the next turn, which tells
//! the Worker to drop the old copy. The turns end at `Done`, at a turn the Worker declines or cannot
//! answer, and when the Session stops, after which they never write the record.

use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use orchbee_capability_node_protocol::{
    OperationId, TerminalMoveTurnOutcome, TerminalMoveTurnRequest,
};

use super::bindings::Writer;
use crate::{
    local_terminal_service::LocalTerminalService,
    terminal_coordinator::WorkerPlacement,
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
    },
};

/// The pause between two turns, and between two attempts to record a rebinding.
const SPACING: Duration = Duration::from_secs(2);

/// The thread that sends one Worker generation its move turns.
pub(in crate::local_worker_service::managed) struct Turns {
    wake: mpsc::Sender<()>,
    /// Set before the Session's final capture; read under the binding writer's lock, so no turn
    /// writes the record after that capture.
    stopped: Arc<AtomicBool>,
}

impl Turns {
    /// Starts the turns of the Worker `handle` names. A thread that cannot start leaves every
    /// terminal where it runs.
    pub(super) fn start(
        handle: WorkerRequestHandle,
        terminals: Arc<LocalTerminalService>,
        writer: Arc<Mutex<Writer>>,
        worker: orchbee_workbench_protocol::WorkerId,
    ) -> Option<Self> {
        let placement = WorkerPlacement::of(&handle)?;
        let (wake, wait) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let turns = Turn {
            handle,
            placement,
            terminals,
            writer,
            worker,
            stopped: Arc::clone(&stopped),
        };
        thread::Builder::new()
            .name("move-turns".into())
            .spawn(move || turns.run(&wait))
            .ok()?;
        Some(Self { wake, stopped })
    }

    /// Ends the turns: a turn in flight finishes, and none writes the binding record after this.
    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = self.wake.send(());
    }
}

impl Drop for Turns {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Turn {
    handle: WorkerRequestHandle,
    placement: WorkerPlacement,
    terminals: Arc<LocalTerminalService>,
    writer: Arc<Mutex<Writer>>,
    worker: orchbee_workbench_protocol::WorkerId,
    stopped: Arc<AtomicBool>,
}

impl Turn {
    fn run(&self, wait: &mpsc::Receiver<()>) {
        // A binding the adoption found moved is recorded before the first turn too.
        let mut unrecorded = true;
        loop {
            match wait.recv_timeout(SPACING) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if self.stopped.load(Ordering::SeqCst) {
                return;
            }
            // The Worker drops a moved terminal's old copy at the next turn, so the record names
            // its new Host first.
            if unrecorded {
                match self.record() {
                    Some(true) => unrecorded = false,
                    Some(false) => continue,
                    None => return,
                }
            }
            let Ok(request) = self.request() else {
                return;
            };
            match self
                .handle
                .request(WorkerOperationRequest::MoveTurn(request))
            {
                Ok(WorkerOperationResult::MoveTurn(result)) => match result.outcome {
                    TerminalMoveTurnOutcome::Moved {
                        terminal_id,
                        from,
                        to,
                    } => {
                        // A binding closed meanwhile keeps nothing to rebind; the turn still
                        // counts, and the Worker drops the old copy at the next.
                        if self
                            .terminals
                            .rebind_moved(self.placement, terminal_id, from, to)
                            .is_err()
                        {
                            return;
                        }
                        unrecorded = true;
                    }
                    TerminalMoveTurnOutcome::Waiting => {}
                    TerminalMoveTurnOutcome::Done => return,
                },
                Err(WorkerRequestError::Backpressure) => {}
                // A decline, a lost wire, a replaced generation or an answer of another kind.
                Ok(_) | Err(_) => return,
            }
        }
    }

    /// Writes the binding record: whether it was written, or `None` once the turns stopped.
    fn record(&self) -> Option<bool> {
        let mut writer = self.writer.lock().ok()?;
        if self.stopped.load(Ordering::SeqCst) {
            return None;
        }
        Some(writer.write_exact(&self.terminals, &self.worker).is_ok())
    }

    fn request(&self) -> io::Result<TerminalMoveTurnRequest> {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
        Ok(TerminalMoveTurnRequest {
            request_id: OperationId::from_bytes(bytes).map_err(io::Error::other)?,
            worker_id: self.placement.worker_id,
            worker_generation: self.placement.generation,
        })
    }
}
