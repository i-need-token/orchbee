//! Owns host termination observation through scoped Unix signal registrations or the Windows
//! control-interrupt adapter. It signals the application's shutdown path without restoring
//! terminal modes or terminating owned resources itself.

use std::io;

#[cfg(unix)]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(all(any(test, feature = "test-support"), unix))]
use std::cell::Cell;

// Test-only seam: arms the next `install` call ON THIS THREAD to fail
// before any signal registration runs, so callers' refusal-before-effect
// ordering is observable without a real registration failure. Thread-local
// so a parallel test's own `install` can never steal the armed fault.
#[cfg(all(any(test, feature = "test-support"), unix))]
thread_local! {
    static INSTALL_FAULT: Cell<bool> = const { Cell::new(false) };
}

#[cfg(all(any(test, feature = "test-support"), unix))]
pub fn arm_install_fault() {
    INSTALL_FAULT.with(|armed| armed.set(true));
}

#[cfg(unix)]
use signal_hook::{
    SigId,
    consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    flag,
    low_level::unregister,
};

pub trait TerminationSource {
    fn requested(&self) -> bool;
}

pub struct TerminationSignal {
    #[cfg(windows)]
    interrupt: crate::windows_signal::ControlInterrupt,
    #[cfg(unix)]
    registrations: Vec<SigId>,
    #[cfg(unix)]
    signal: Arc<AtomicUsize>,
}

impl TerminationSignal {
    pub fn install() -> io::Result<Self> {
        #[cfg(all(any(test, feature = "test-support"), unix))]
        if INSTALL_FAULT.with(|armed| armed.replace(false)) {
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

        #[cfg(windows)]
        {
            Ok(Self {
                interrupt: crate::windows_signal::ControlInterrupt::arm()?,
            })
        }
        #[cfg(not(any(unix, windows)))]
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

        #[cfg(windows)]
        {
            self.interrupt.requested()
        }
        #[cfg(not(any(unix, windows)))]
        {
            false
        }
    }
}

impl orchbee_agent_integration_writer::TerminationSource for TerminationSignal {
    fn requested(&self) -> bool {
        #[cfg(unix)]
        {
            self.signal.load(Ordering::Relaxed) != 0
        }

        #[cfg(windows)]
        {
            self.interrupt.requested()
        }
        #[cfg(not(any(unix, windows)))]
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

#[cfg(all(test, unix))]
mod tests;
