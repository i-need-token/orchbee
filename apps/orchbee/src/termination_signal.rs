use std::io;

#[cfg(unix)]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(all(test, unix))]
use std::sync::atomic::AtomicBool;

/// Test-only seam: arms the next `install` call to fail before any signal
/// registration runs, so callers' refusal-before-effect ordering is
/// observable without a real registration failure.
#[cfg(all(test, unix))]
static INSTALL_FAULT: AtomicBool = AtomicBool::new(false);

#[cfg(all(test, unix))]
pub(crate) fn arm_install_fault() {
    INSTALL_FAULT.store(true, Ordering::Relaxed);
}

#[cfg(unix)]
use signal_hook::{
    SigId,
    consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    flag,
    low_level::unregister,
};

pub(crate) trait TerminationSource {
    fn requested(&self) -> bool;
}

pub(crate) struct TerminationSignal {
    #[cfg(unix)]
    registrations: Vec<SigId>,
    #[cfg(unix)]
    signal: Arc<AtomicUsize>,
}

impl TerminationSignal {
    pub(crate) fn install() -> io::Result<Self> {
        #[cfg(all(test, unix))]
        if INSTALL_FAULT.swap(false, Ordering::Relaxed) {
            return Err(io::Error::other("termination-signal install fault"));
        }

        #[cfg(unix)]
        {
            let mut monitor = Self {
                registrations: Vec::with_capacity(4),
                signal: Arc::new(AtomicUsize::new(0)),
            };
            for signal in [SIGHUP, SIGINT, SIGQUIT, SIGTERM] {
                let registration =
                    flag::register_usize(signal, Arc::clone(&monitor.signal), signal as usize)?;
                monitor.registrations.push(registration);
            }
            Ok(monitor)
        }

        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }
}

impl TerminationSource for TerminationSignal {
    fn requested(&self) -> bool {
        #[cfg(unix)]
        {
            self.signal.load(Ordering::Relaxed) != 0
        }

        #[cfg(not(unix))]
        {
            false
        }
    }
}

#[cfg(unix)]
impl Drop for TerminationSignal {
    fn drop(&mut self) {
        for registration in self.registrations.drain(..) {
            unregister(registration);
        }
    }
}

#[cfg(test)]
mod tests;
