//! The managed POSIX Server's signals. `SIGTERM` and `SIGINT`, which a logout, a shutdown or a
//! person ending the process sends, begin the Session's normal stop; `SIGHUP`, which a closed
//! terminal sends, changes nothing. They are blocked before any thread starts, so every Server
//! thread inherits the blocked mask and one thread alone waits for them. A child such as the Worker
//! starts with an empty mask again, since the standard library resets it for every process it
//! creates. A signal inherited as ignored stays ignored on macOS, which discards it before any
//! waiter sees it; Linux keeps a blocked signal pending whatever its action.
use crate::session_shutdown::SessionShutdown;
use nix::sys::signal::{SigSet, Signal};
use orchbee_local_client_protocol::StopId;
use std::{io, sync::Arc, thread};

/// The stop signals, blocked on the thread that blocked them and on every thread it starts later.
pub(super) struct Blocked(SigSet);

/// Blocks the Server's signals; the caller has started no thread yet.
pub(super) fn block() -> io::Result<Blocked> {
    let mut signals = SigSet::empty();
    for signal in [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP] {
        signals.add(signal);
    }
    signals.thread_block()?;
    Ok(Blocked(signals))
}

impl Blocked {
    /// Starts the one thread that waits for the blocked signals. Each `SIGTERM` or `SIGINT` begins
    /// the stop under `own`, or queues its next attempt once one failed; a signal that arrived
    /// earlier is waiting for it.
    pub(super) fn forward(self, shutdown: Arc<SessionShutdown>, own: StopId) -> io::Result<()> {
        thread::Builder::new()
            .name("orchbee-server-signals".into())
            .spawn(move || {
                // `sigwait` fails only for a set it cannot wait on, which would fail again at once.
                while let Ok(signal) = self.0.wait() {
                    if matches!(signal, Signal::SIGTERM | Signal::SIGINT) {
                        super::stop::begin_own(&shutdown, own);
                    }
                }
            })?;
        Ok(())
    }
}
