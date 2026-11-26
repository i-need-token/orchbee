//! The machine hub `orchbee --machine-hub`: the one process per data root through which
//! interfaces reach saved machines over `machine-hub`, and its diagnostic entries
//! `orchbee --machine-hub-state` and `orchbee --machine-hub-hold`, which reach it through the
//! workbench Client's one client owner. `docs/machines.md#machine-hub` owns the lifetime, the frozen
//! names, each Machine's carrying, the channels it relays, how it stops a remote Session, the bounds
//! and the races kept here; the contract in
//! `schemas/machine` owns every byte.
mod askpass;
mod capacities;
mod capacity_answers;
mod carrier;
mod catalog;
mod draft_ledger;
mod environment;
mod hold;
mod lifetime;
mod machine;
mod peer;
mod printer;
mod relay;
mod server;
mod state;
mod stop;

use crate::{cli::Output, localization::Locale, tui_capabilities::Capability};

/// Serves the hub until it closes. The status is 0 after closing or when another hub holds the
/// data root's lock, and 1 when the hub cannot serve; it writes nothing. A host without saved
/// machines answers unsupported before it touches the data root.
pub(crate) fn serve(locale: Locale) -> Output {
    if Capability::SavedMachines.require().is_err() {
        return crate::cli::unsupported_platform(None, locale);
    }
    Output {
        status: server::serve(),
        stdout: Vec::new(),
        stderr: String::new(),
        finalization: None,
    }
}

/// Attaches to the hub, starting it when none runs, and prints its state.
pub(crate) fn print_state(locale: Locale) -> Output {
    if Capability::SavedMachines.require().is_err() {
        return crate::cli::unsupported_platform(None, locale);
    }
    printer::run(locale)
}

/// Attaches with `presence` and `watch`, starting the hub when none runs, and prints each
/// attachment and event until standard input ends.
pub(crate) fn hold(locale: Locale) -> Output {
    if Capability::SavedMachines.require().is_err() {
        return crate::cli::unsupported_platform(None, locale);
    }
    hold::run(locale)
}
