//! The machine hub `orchbee --machine-hub-v1`: the one process per data root through which
//! interfaces reach saved machines over `machine-hub`, and its diagnostic entries
//! `orchbee --machine-hub-state-v1` and `orchbee --machine-hub-hold-v1`, which reach it through the
//! workbench Client's one client owner. `docs/machines.md#machine-hub` owns the lifetime, the frozen
//! names, each Machine's carrying, the channels it relays, how it stops a remote Session, the bounds
//! and the races kept here; the contract in
//! `schemas/machine` owns every byte.
#[cfg(windows)]
mod carrier;
#[cfg(windows)]
mod catalog;
#[cfg(windows)]
mod environment;
#[cfg(windows)]
mod hold;
#[cfg(windows)]
mod lifetime;
#[cfg(windows)]
mod machine;
#[cfg(windows)]
mod peer;
#[cfg(windows)]
mod printer;
#[cfg(windows)]
mod relay;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod state;
#[cfg(windows)]
mod stop;

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

/// Attaches with `presence` and `watch`, starting the hub when none runs, and prints each
/// attachment and event until standard input ends.
pub(crate) fn hold(locale: Locale) -> Output {
    #[cfg(windows)]
    {
        hold::run(locale)
    }
    #[cfg(not(windows))]
    {
        crate::cli::unsupported_platform(None, locale)
    }
}
