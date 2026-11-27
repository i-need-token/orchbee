//! This process's termination signals on macOS and Linux. `SIGTERM` and `SIGINT` end it the way
//! the link's end would, after its children, with exit code `128 + signal`; `SIGHUP` changes
//! nothing, as for the Server. A handler only sets its signal's flag and writes one byte to a
//! socket that one thread reads.

use std::{
    io::{self, Read},
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM},
    flag,
    low_level::pipe,
};

/// The caught signals' flags and the socket their bytes arrive on.
pub(crate) struct Caught {
    bytes: UnixStream,
    terminate: Arc<AtomicBool>,
    interrupt: Arc<AtomicBool>,
}

/// Catches the termination signals; one caught before [`Caught::forward`] waits for it.
pub(crate) fn catch() -> io::Result<Caught> {
    let (bytes, write) = UnixStream::pair()?;
    let terminate = Arc::new(AtomicBool::new(false));
    let interrupt = Arc::new(AtomicBool::new(false));
    // A signal's actions run in the order they were registered, so its flag is set before the
    // reader wakes for its byte.
    flag::register(SIGTERM, Arc::clone(&terminate))?;
    pipe::register(SIGTERM, write.try_clone()?)?;
    flag::register(SIGINT, Arc::clone(&interrupt))?;
    pipe::register(SIGINT, write)?;
    // Caught, a `SIGHUP` no longer takes its default action of ending the process.
    flag::register(SIGHUP, Arc::new(AtomicBool::new(false)))?;
    Ok(Caught {
        bytes,
        terminate,
        interrupt,
    })
}

impl Caught {
    /// Starts the one thread that reads the caught signals and passes the first termination
    /// signal to `ended`.
    pub(crate) fn forward(self, ended: impl FnOnce(i32) + Send + 'static) -> io::Result<()> {
        let Self {
            mut bytes,
            terminate,
            interrupt,
        } = self;
        thread::Builder::new()
            .name("orchbee-files-signals".into())
            .spawn(move || {
                let mut byte = [0; 1];
                loop {
                    match bytes.read(&mut byte) {
                        Ok(0) => return,
                        Ok(_) if terminate.load(Ordering::Acquire) => return ended(SIGTERM),
                        Ok(_) if interrupt.load(Ordering::Acquire) => return ended(SIGINT),
                        Ok(_) => {}
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
