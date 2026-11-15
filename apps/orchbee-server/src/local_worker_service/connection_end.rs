//! Serves an admitted Worker's wire on its own thread and writes one `worker-connection-ended`
//! record when it ends: the carrier's exit and failure, the request that ended it, and what the
//! Server observed of the Worker process when it holds a handle to that process.

use std::{
    io,
    panic::{self, AssertUnwindSafe},
    time::Duration,
};

use orchbee_local_ipc::LocalStream as UnixStream;

use crate::{
    fault_records::{self, WorkerConnectionEnd, WorkerProcessState},
    worker_obcn_carrier::{WorkerWireEnd, WorkerWireExit},
    worker_registry::RegisteredWorkerWireOwner,
};

/// How long a connection that ended on its own waits for the Worker process to finish exiting, so
/// the record can carry its exit code: a Worker that crashes closes its connection before the
/// system reports its exit.
const WORKER_EXIT_WAIT: Duration = Duration::from_secs(2);

/// The Worker process as this Server can observe it.
pub(super) enum WorkerProcess {
    /// The Server holds no handle to the Worker: a POSIX Session lets its Worker go after admission,
    /// and the private endpoint form does not start one.
    NotHeld,
    /// A handle of this connection's own, or `None` when it could not be opened.
    #[cfg(windows)]
    Held(Option<winsafe::guard::CloseHandleGuard<winsafe::HPROCESS>>),
}

impl WorkerProcess {
    /// Opens a handle to the Worker process `worker`. The caller still holds the handle it started
    /// that Worker with, so the identifier cannot name another process.
    pub(super) fn open(worker: Option<u32>) -> Self {
        #[cfg(windows)]
        if let Some(worker) = worker {
            use winsafe::{HPROCESS, co};
            return Self::Held(
                HPROCESS::OpenProcess(
                    co::PROCESS::SYNCHRONIZE | co::PROCESS::QUERY_LIMITED_INFORMATION,
                    false,
                    worker,
                )
                .ok(),
            );
        }
        #[cfg(not(windows))]
        let _ = worker;
        Self::NotHeld
    }

    /// Whether the process exited within `wait`, and its exit code when it did.
    fn observe(&self, wait: Duration) -> WorkerProcessState {
        match self {
            Self::NotHeld => {
                let _ = wait;
                WorkerProcessState::NotHeld
            }
            #[cfg(windows)]
            Self::Held(None) => WorkerProcessState::Unknown,
            #[cfg(windows)]
            Self::Held(Some(process)) => {
                use winsafe::co;
                let milliseconds = u32::try_from(wait.as_millis()).unwrap_or(u32::MAX);
                match process.WaitForSingleObject(Some(milliseconds)) {
                    Ok(co::WAIT::OBJECT_0) => process
                        .GetExitCodeProcess()
                        .map_or(WorkerProcessState::Unknown, WorkerProcessState::Exited),
                    Ok(co::WAIT::TIMEOUT) => WorkerProcessState::Running,
                    _ => WorkerProcessState::Unknown,
                }
            }
        }
    }
}

/// Serves the wire to its end on the calling thread and records that end. A panic in the wire
/// records `panicked` and then continues unwinding, so the thread's owner still sees the panic.
pub(super) fn serve(owner: RegisteredWorkerWireOwner, stream: UnixStream, process: WorkerProcess) {
    match panic::catch_unwind(AssertUnwindSafe(|| owner.run_to_end(stream))) {
        Ok(end) => {
            // The Server ended this wire itself; the Worker's exit belongs to the stop that follows.
            let wait = if matches!(end.exit, WorkerWireExit::Shutdown) {
                Duration::ZERO
            } else {
                WORKER_EXIT_WAIT
            };
            fault_records::worker_connection_ended(&ended(&end, process.observe(wait)));
        }
        Err(payload) => {
            fault_records::worker_connection_ended(&WorkerConnectionEnd {
                exit: "panicked",
                failure: "none",
                error: "none",
                request: None,
                after_shutdown: false,
                process: process.observe(Duration::ZERO),
            });
            panic::resume_unwind(payload);
        }
    }
}

/// Records a wire whose thread could not start; its unstarted owner already retired the generation.
pub(super) fn not_started(error: io::ErrorKind, process: &WorkerProcess) {
    fault_records::worker_connection_ended(&WorkerConnectionEnd {
        exit: "not-started",
        failure: "spawn-thread",
        error: fault_records::io_token(error),
        request: None,
        after_shutdown: false,
        process: process.observe(Duration::ZERO),
    });
}

fn ended(end: &WorkerWireEnd, process: WorkerProcessState) -> WorkerConnectionEnd {
    WorkerConnectionEnd {
        exit: end.exit_token(),
        failure: end.failure_token(),
        error: end.error_token(),
        request: end.request(),
        after_shutdown: end.after_shutdown(),
        process,
    }
}
