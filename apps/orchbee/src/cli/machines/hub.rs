//! The machine hub `orchbee --machine-hub-v1`: the one process per data root through which
//! interfaces reach saved machines over `machine-hub`, and its state printer
//! `orchbee --machine-hub-state-v1`. `docs/machines.md#machine-hub` owns the lifetime, the frozen
//! names, the bounds and the races kept here; the contract in `schemas/machine` owns every byte.
#[cfg(windows)]
mod attach;
#[cfg(windows)]
mod catalog;
#[cfg(windows)]
mod lifetime;
#[cfg(windows)]
mod peer;
#[cfg(windows)]
mod printer;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod sessions;
#[cfg(windows)]
mod state;

#[cfg(windows)]
pub(crate) use printer::StateFailure;

use crate::{cli::Output, localization::Locale};

/// Serves the hub until it closes. The status is 0 after closing or when another hub holds the
/// data root's lock, and 1 when the hub cannot serve; it writes nothing. Other platforms answer
/// unsupported before they touch the data root.
pub(crate) fn serve(locale: Locale) -> Output {
    #[cfg(windows)]
    {
        let _ = locale; // The hub writes no text.
        Output {
            status: server::serve(),
            stdout: Vec::new(),
            stderr: String::new(),
            finalization: None,
        }
    }
    #[cfg(not(windows))]
    {
        crate::cli::unsupported_platform(None, locale)
    }
}

/// Attaches to the hub, starting it when none runs, and prints its state.
pub(crate) fn print_state(locale: Locale) -> Output {
    #[cfg(windows)]
    {
        printer::run(locale)
    }
    #[cfg(not(windows))]
    {
        crate::cli::unsupported_platform(None, locale)
    }
}
