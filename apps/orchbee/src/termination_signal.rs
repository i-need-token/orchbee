use std::io;

#[cfg(unix)]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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
