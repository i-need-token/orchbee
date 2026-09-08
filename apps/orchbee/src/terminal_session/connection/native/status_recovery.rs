use super::{NativeExecutor, NativeWorkerExit, WorkerExit};
use crate::{
    local_client::{LocalClientOrigin, LocalClientShutdown, LocalOperationClock},
    terminal_session::{mailbox::CommandMailbox, state::CarrierFailure},
};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Instant,
};

enum ShutdownState {
    Waiting,
    Connected(LocalClientShutdown),
    Closed,
}

#[derive(Clone)]
pub(in crate::terminal_session) struct RecoveryShutdown(Arc<Mutex<ShutdownState>>);

impl RecoveryShutdown {
    pub(in crate::terminal_session) fn new() -> Self {
        Self(Arc::new(Mutex::new(ShutdownState::Waiting)))
    }
    fn install(&self, handle: LocalClientShutdown) -> io::Result<()> {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if matches!(*state, ShutdownState::Waiting) {
            *state = ShutdownState::Connected(handle);
            Ok(())
        } else {
            handle.close()?;
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "status recovery was retired",
            ))
        }
    }
    pub(in crate::terminal_session) fn close(&self) {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if let ShutdownState::Connected(handle) =
            std::mem::replace(&mut *state, ShutdownState::Closed)
        {
            let _closed = handle.close();
        }
    }
}

pub(in crate::terminal_session) fn run_status_recovery(
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    mailbox: Arc<CommandMailbox>,
    shutdown: RecoveryShutdown,
    deadline: Instant,
    membership: bool,
) -> NativeWorkerExit {
    let mut exit = WorkerExit::new(Arc::clone(&mailbox));
    let client = match origin.reconnect_before(deadline, clock) {
        Ok(client) => client,
        Err(_) => {
            exit.failure = CarrierFailure::Transport;
            return NativeWorkerExit::new(None, None, None, None);
        }
    };
    if client
        .shutdown_handle()
        .and_then(|handle| shutdown.install(handle))
        .is_err()
    {
        exit.failure = CarrierFailure::Transport;
        return NativeWorkerExit::new(None, None, None, None);
    }
    let mut executor = NativeExecutor::new(client, mailbox);
    executor.status_only = !membership;
    executor.membership_recovery = membership;
    if let Err(failure) = executor.run_ready_queue() {
        exit.failure = failure;
    }
    NativeWorkerExit::new(
        executor.pending_create.take(),
        executor.pending_move.take(),
        executor.pending_close.take(),
        executor.pending_rename.take(),
    )
    .with_unavailable_poll(executor.unavailable_poll.take())
}
