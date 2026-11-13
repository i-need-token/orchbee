//! `--machine <selector> server stop`: the near side confirms the stop under the rule a local
//! `server stop` follows in this build, then asks the machine hub of this data root for
//! `stop-session` and reports the hub's outcome in `orchbee.machine.server-stop`. Nothing travels
//! as a routed command line. Once the request may have been sent, an interrupt ends the wait, not
//! the stop, and the outcome is `unknown`.

use super::{Context, Error, Outcome, failed, mapped, select, stop_status};
use crate::{
    cli::{
        self, Failure, FailureKind, Output,
        console_interrupt::ArmedSignals,
        destructive_request::{RemoteTarget, admit_remote_stop},
        output,
    },
    cli_text::CliText,
    invocation::{OutputFormat, machine_routing::RemoteStop},
    localization::Locale,
    machines::Store,
};
use orchbee_machine_protocol::hub::StopSessionOutcome;
use serde_json::json;
use std::time::Instant;

#[cfg(windows)]
#[path = "stop/hub.rs"]
mod hub;
/// No machine hub runs where it is not built.
#[cfg(not(windows))]
mod hub {
    use super::{super::Refusal, Context, Error, Stopped, mapped};

    pub(super) fn stop(_machine: &str, _context: &mut Context) -> Result<Stopped, Refusal> {
        Err(mapped(Error::SshUnavailable))
    }
}

/// The schema of the result document.
pub(super) const SCHEMA: &str = "orchbee.machine.server-stop";

/// The hub's answer, as far as this command learned it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Stopped {
    pub(super) outcome: StopSessionOutcome,
    /// An interrupt ended the wait once the request may have been sent; the outcome is `unknown`.
    pub(super) interrupted: bool,
}

/// The saved Machine a result names.
pub(super) struct Machine<'a> {
    pub(super) id: &'a str,
    pub(super) label: &'a str,
    pub(super) session: &'a str,
}

pub(crate) fn run(stop: &RemoteStop, locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::SavedMachines
        .require()
        .is_err()
    {
        return cli::unsupported_platform(Some(stop.format), locale);
    }
    let refuse = |error| failed(mapped(error), Outcome::NotSubmitted, stop.format, locale);
    let catalog = match Store::configured().and_then(|store| store.read()) {
        Ok(catalog) => catalog,
        Err(error) => return refuse(Error::Catalog(error)),
    };
    let profile = match select(&catalog, &stop.selector) {
        Ok(profile) => profile,
        Err(error) => return refuse(error),
    };
    // Only an SSH machine is carried.
    if profile.draft.ssh_target().is_none() {
        return refuse(Error::UnsupportedMachine);
    }
    let machine = Machine {
        id: profile.id.as_str(),
        label: &profile.draft.label,
        session: profile.draft.session.as_str(),
    };
    let target = RemoteTarget {
        selector: &stop.selector,
        id: machine.id,
        label: machine.label,
        session: machine.session,
    };
    // A refusal ends the command before the hub is asked anything.
    if let Err(refusal) = admit_remote_stop(stop.confirm, &target, stop.format, locale) {
        return *refusal;
    }
    let Ok(signals) = ArmedSignals::arm_interrupts() else {
        return refuse(Error::Internal);
    };
    let mut context = Context {
        signals,
        deadline: Instant::now(),
    };
    let mut result = match hub::stop(machine.id, &mut context) {
        Ok(stopped) => emit(&machine, stopped, stop.format, locale),
        Err(refusal) => failed(refusal, Outcome::NotSubmitted, stop.format, locale),
    };
    result.finalization = Some(context.signals.into_finalization());
    result
}

/// The result document, or its text line, for the hub's answer; the exit code follows from the
/// outcome alone.
pub(super) fn emit(
    machine: &Machine<'_>,
    stopped: Stopped,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let outcome = stopped.outcome.as_str();
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&json!({
            "schema": SCHEMA,
            "version": 1,
            "result": {
                "machine": {
                    "id": machine.id,
                    "label": machine.label,
                    "session": machine.session,
                },
                "outcome": outcome,
            },
        })),
        OutputFormat::Text => output::encode_text(&CliText::new(locale).cli_machine_stop_result(
            machine.label,
            outcome,
            stopped.interrupted,
        )),
    };
    match encoded {
        Ok(stdout) => Output {
            status: stop_status(stopped.outcome),
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => cli::failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}
