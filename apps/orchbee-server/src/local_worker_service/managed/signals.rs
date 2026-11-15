//! The managed POSIX Server's signals. `SIGTERM` and `SIGINT`, which a logout, a shutdown or a
//! person ending the process sends, begin the Session's normal stop; `SIGHUP`, which a closed
//! terminal sends, changes nothing. The Server catches them instead of blocking them: every
//! process it starts, and through the Worker the Host and each terminal process, inherits a
//! blocked mask but begins a caught signal at its default action. Each caught signal writes one
//! byte to a socket that one thread reads, so a signal caught before that thread starts waits.
use crate::session_shutdown::SessionShutdown;
use orchbee_local_client_protocol::StopId;
use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM},
    flag,
    low_level::pipe,
};
use std::{
    io::{self, Read},
    os::unix::net::UnixStream,
    sync::{Arc, atomic::AtomicBool},
    thread,
};

/// The read side of the stop signals' socket.
pub(super) struct Caught(UnixStream);

/// Catches the Server's signals; one caught before [`Caught::forward`] waits for it.
pub(super) fn catch() -> io::Result<Caught> {
    let (read, write) = UnixStream::pair()?;
    for signal in [SIGTERM, SIGINT] {
        pipe::register(signal, write.try_clone()?)?;
    }
    // Caught, a `SIGHUP` no longer takes its default action of ending the process.
    flag::register(SIGHUP, Arc::new(AtomicBool::new(false)))?;
    Ok(Caught(read))
}

impl Caught {
    /// Starts the one thread that reads the caught signals. Each `SIGTERM` or `SIGINT` begins the
    /// stop under `own`, or queues its next attempt once one failed.
    pub(super) fn forward(self, shutdown: Arc<SessionShutdown>, own: StopId) -> io::Result<()> {
        let Self(mut caught) = self;
        thread::Builder::new()
            .name("orchbee-server-signals".into())
            .spawn(move || {
                let mut byte = [0; 1];
                loop {
                    match caught.read(&mut byte) {
                        Ok(0) => return,
                        Ok(_) => super::stop::begin_own(&shutdown, own),
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                        // The registrations keep the write side open, so a read fails only for a
                        // socket that would fail again at once.
                        Err(_) => return,
                    }
                }
            })?;
        Ok(())
    }
}
