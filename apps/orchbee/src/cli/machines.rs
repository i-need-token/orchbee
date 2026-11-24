//! Saved-machine commands; remote readiness and local publication have separate outcomes.
use crate::cli_text::CliText;
pub(crate) mod askpass;
mod capacity;
pub(crate) mod change;
mod connect;
pub(crate) mod hub;
pub(crate) mod link;
mod list;
pub(crate) mod other_lists;
pub(crate) mod routing;
mod shared_carrier;
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish},
    destructive_request::{RemoteTarget, admit_remove_stop},
    failure_output, output,
};
use crate::{
    invocation::{MachineCommand, OutputFormat},
    localization::Locale,
    machines::{Catalog, Edit, Error as CatalogError, ProfileId, Store},
};
use serde_json::json;
use std::{
    io,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Catalog(CatalogError),
    SshUnavailable,
    SshFailed,
    RemoteProtocol,
    /// The remote runs a system or architecture this build cannot serve.
    RemoteUnsupported,
    /// The remote has no usable Orchbee installation.
    RemoteNotInstalled,
    RemoteOtherEnvironment,
    RemoteNotReady,
    /// The record is disabled.
    DisabledMachine,
    /// The record's connection kind is one this build does not know.
    UnsupportedMachine,
    /// The machine hub of this data root could not be reached or started.
    HubUnavailable,
    /// A machine hub started as administrator serves this data root and refused this standard
    /// process; it reads as `hub-unavailable` with its own message.
    HubElevated,
    HostUnreachable,
    /// Signing in needs an answer and nobody gave one.
    AuthRequired,
    AuthFailed,
    HostUntrusted,
    HostKeyChanged,
    /// One side is too old for the other, or another build's hub serves this data root.
    VersionIncompatible,
    RemoteInstallFailed,
    /// An interrupt ended the command.
    Cancelled,
    /// A `disconnect`, or a reload that changed the record, ended the connection attempt this
    /// command waited for; the hub's reply does not say which.
    AttemptEnded,
    Timeout,
    PhysicalUnknown,
    /// The remote Session did not stop, so the record stays.
    SessionStopFailed,
    /// A verb or option this release parses but does not perform yet.
    NotYetAvailable,
    Internal,
}
impl Error {
    fn code(self) -> &'static str {
        match self {
            Self::Catalog(e) => match e {
                CatalogError::Configuration => "configuration",
                CatalogError::InvalidCatalog => "invalid-catalog",
                CatalogError::Storage => "storage",
                CatalogError::Busy => "busy",
                CatalogError::NotFound => "not-found",
                CatalogError::Ambiguous => "ambiguous-machine",
                CatalogError::Reserved => "reserved-name",
                CatalogError::Capacity => "capacity",
                CatalogError::RevisionExhausted => "revision-exhausted",
            },
            Self::SshUnavailable => "ssh-unavailable",
            Self::SshFailed => "ssh-failed",
            Self::RemoteProtocol => "remote-protocol",
            Self::RemoteUnsupported => "remote-unsupported",
            Self::RemoteNotInstalled => "remote-not-installed",
            Self::RemoteOtherEnvironment => "remote-other-environment",
            Self::RemoteNotReady => "remote-not-ready",
            Self::DisabledMachine => "disabled-machine",
            Self::UnsupportedMachine => "unsupported-machine",
            Self::HubUnavailable | Self::HubElevated => "hub-unavailable",
            Self::HostUnreachable => "host-unreachable",
            Self::AuthRequired => "auth-required",
            Self::AuthFailed => "auth-failed",
            Self::HostUntrusted => "host-untrusted",
            Self::HostKeyChanged => "host-key-changed",
            Self::VersionIncompatible => "version-incompatible",
            Self::RemoteInstallFailed => "remote-install-failed",
            Self::Cancelled | Self::AttemptEnded => "cancelled",
            Self::Timeout => "timeout",
            Self::PhysicalUnknown => "physical-unknown",
            Self::SessionStopFailed => "session-stop-failed",
            Self::NotYetAvailable => "not-yet-available",
            Self::Internal => "internal",
        }
    }
    /// The standard-error message: a `cancelled` the hub gave reads differently from an interrupt,
    /// and a hub started as administrator differently from one that cannot be reached.
    fn message(self) -> &'static str {
        match self {
            Self::AttemptEnded => "attempt-ended",
            Self::HubElevated => "hub-elevated",
            _ => self.code(),
        }
    }
    fn status(self) -> u8 {
        match self {
            Self::Catalog(
                CatalogError::NotFound | CatalogError::Ambiguous | CatalogError::Reserved,
            )
            | Self::DisabledMachine
            | Self::AuthFailed
            | Self::HostUntrusted
            | Self::HostKeyChanged => 3,
            Self::Catalog(
                CatalogError::Busy | CatalogError::Capacity | CatalogError::RevisionExhausted,
            )
            | Self::SshUnavailable
            | Self::RemoteNotInstalled
            | Self::RemoteNotReady
            | Self::UnsupportedMachine
            | Self::HubUnavailable
            | Self::HubElevated
            | Self::HostUnreachable
            | Self::AuthRequired
            | Self::NotYetAvailable => 4,
            Self::SshFailed | Self::RemoteProtocol => 5,
            Self::RemoteUnsupported | Self::RemoteOtherEnvironment | Self::VersionIncompatible => 6,
            Self::Cancelled => 130,
            Self::Timeout | Self::AttemptEnded => 7,
            Self::PhysicalUnknown => 8,
            Self::RemoteInstallFailed | Self::SessionStopFailed => 9,
            _ => 1,
        }
    }
}
struct Context {
    signals: ArmedSignals,
    deadline: Instant,
}
impl Context {
    fn interrupted(&mut self) -> io::Result<bool> {
        self.signals.take().map(|v| v.is_some())
    }
    fn check(&mut self) -> Result<(), Error> {
        if self.interrupted().map_err(|_| Error::Internal)? {
            return Err(Error::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Timeout);
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Remote {
    NotContacted,
    Unchanged,
    PossiblyStarted,
    Ready,
}
impl Remote {
    fn token(self) -> &'static str {
        match self {
            Self::NotContacted => "not-contacted",
            Self::Unchanged => "unchanged",
            Self::PossiblyStarted => "possibly-started",
            Self::Ready => "ready",
        }
    }
}
struct Report {
    catalog: Option<Catalog>,
    id: Option<ProfileId>,
    changed: bool,
    committed: bool,
    remote: Remote,
    /// The remote product version an add admitted; shown, never compared.
    remote_version: Option<String>,
    /// A removed Machine's remembered answers could not all be deleted.
    unforgotten: bool,
    /// `machine list`: this computer, then each saved machine.
    machines: Option<Vec<list::Listed>>,
}
impl Report {
    const fn new() -> Self {
        Self {
            catalog: None,
            id: None,
            changed: false,
            committed: false,
            remote: Remote::NotContacted,
            remote_version: None,
            unforgotten: false,
            machines: None,
        }
    }
}
pub(crate) fn run(command: &MachineCommand, format: OutputFormat, locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::SavedMachines
        .require()
        .is_err()
    {
        return super::unsupported_platform(Some(format), locale);
    }
    // Stopping the remote Session ends its terminals: the Machine is resolved once and confirmed
    // before the hub is asked anything, and the stop and the removal act on that Machine alone.
    let confirmed = match command {
        MachineCommand::Remove {
            machine,
            stop_session: true,
            confirm,
        } => match admit_stop(command, machine, *confirm, format, locale) {
            Ok(confirmed) => Some(confirmed),
            Err(refusal) => return *refusal,
        },
        _ => None,
    };
    let signals = match ArmedSignals::arm_interrupts_except_linux() {
        Ok(s) => s,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut context = Context {
        signals,
        deadline: Instant::now()
            + Duration::from_secs(match command {
                // A hub start, then the hub's own 60-second attempt; a remote stop sets its own.
                MachineCommand::Add(..) | MachineCommand::Connect { .. } => 90,
                MachineCommand::Disconnect(_) => 15,
                _ => 5,
            }),
    };
    let mut report = Report::new();
    let mut error = execute(command, confirmed, &mut context, &mut report, locale).err();
    let mut finalization = None;
    let interrupted = match context.signals.finish() {
        Ok(Finish::Clear) => matches!(error, Some(Error::Cancelled)),
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            if error.is_none() {
                error = Some(Error::Cancelled);
            }
            true
        }
        Err(_) => {
            if error.is_none() {
                error = Some(Error::Internal);
            }
            false
        }
    };
    let mut result = emit(command, &report, error, interrupted, format, locale);
    result.finalization = finalization;
    result
}
/// The saved Machine of `machine remove --stop-session` and the catalog it was read from, resolved
/// once and confirmed as a remote `server stop` confirms it. A catalog that cannot be read or a
/// `selector` that names no Machine refuses the command before anything is stopped.
fn admit_stop(
    command: &MachineCommand,
    selector: &str,
    confirm: bool,
    format: OutputFormat,
    locale: Locale,
) -> Result<(Catalog, ProfileId), Box<Output>> {
    let resolved = Store::configured()
        .and_then(|store| store.read())
        .and_then(|catalog| {
            let profile = catalog.resolve(selector)?.clone();
            Ok((catalog, profile))
        });
    let (catalog, profile) = resolved.map_err(|error| {
        let refusal = Some(Error::Catalog(error));
        Box::new(emit(
            command,
            &Report::new(),
            refusal,
            false,
            format,
            locale,
        ))
    })?;
    let target = RemoteTarget {
        selector,
        id: profile.id.as_str(),
        label: &profile.draft.label,
        session: profile.draft.session.as_str(),
    };
    admit_remove_stop(confirm, &target, format, locale)?;
    Ok((catalog, profile.id))
}
fn execute(
    command: &MachineCommand,
    confirmed: Option<(Catalog, ProfileId)>,
    context: &mut Context,
    report: &mut Report,
    locale: Locale,
) -> Result<(), Error> {
    context.check()?;
    if matches!(command, MachineCommand::NotYetAvailable(_)) {
        return Err(Error::NotYetAvailable);
    }
    let store = Store::configured().map_err(Error::Catalog)?;
    match command {
        MachineCommand::NotYetAvailable(_) => Err(Error::NotYetAvailable),
        MachineCommand::List => list::list(store.read().map_err(Error::Catalog)?, context, report),
        MachineCommand::Add(draft, color) => {
            connect::add(&store, draft, *color, context, report, locale)
        }
        MachineCommand::SetColor { machine, color } => {
            commit_selected(&store, machine, context, report, |id| Edit::SetColor {
                id,
                color: *color,
            })
        }
        MachineCommand::Rename { machine, label } => {
            commit_selected(&store, machine, context, report, |id| Edit::Rename {
                id,
                label: label.clone(),
            })
        }
        MachineCommand::Remove { machine, .. } => {
            match confirmed {
                // The confirmed Machine's remote Session stops first, and only its record goes; a
                // stop that fails keeps the record.
                Some((catalog, id)) => {
                    connect::stop_session(&id, catalog, context, report, locale)?;
                    context.check()?;
                    let removed = store
                        .commit(&Edit::Remove(id.clone()))
                        .map(|(catalog, changed)| (catalog, changed, id));
                    settle(removed, context, report)?;
                }
                None => commit_selected(&store, machine, context, report, Edit::Remove)?,
            }
            report.unforgotten = !forget_removed(report);
            Ok(())
        }
        MachineCommand::Enable { machine, enabled } => {
            commit_selected(&store, machine, context, report, |id| Edit::Enable {
                id,
                enabled: *enabled,
            })
        }
        MachineCommand::Connect { machine, remember } => {
            connect::connect(&store, machine, *remember, context, report, locale)
        }
        MachineCommand::Disconnect(machine) => {
            connect::disconnect(&store, machine, context, report)
        }
    }
}
/// Applies an edit to the machine an `<id|label>` operand names when the catalog is committed.
fn commit_selected(
    store: &Store,
    machine: &str,
    context: &mut Context,
    report: &mut Report,
    edit: impl FnOnce(ProfileId) -> Edit,
) -> Result<(), Error> {
    context.check()?;
    let selected = store.commit_selected(machine, edit);
    settle(selected, context, report)
}
/// Records a commit's outcome for the Machine it changed and tells a running hub.
fn settle(
    commit: Result<(Catalog, bool, ProfileId), CatalogError>,
    context: &mut Context,
    report: &mut Report,
) -> Result<(), Error> {
    let (catalog, changed, id) = commit.map_err(Error::Catalog)?;
    report.id = Some(id);
    report.catalog = Some(catalog);
    report.changed = changed;
    report.committed = changed;
    committed(context, report)
}
/// A removed Machine's remembered answers go with it; false when they could not all be deleted.
/// The removal stands either way: a store that cannot be reached or refuses keeps them under the
/// removed id, where nothing reads them, and the person is told. A platform without a store saved
/// nothing.
fn forget_removed(report: &Report) -> bool {
    use orchbee_workbench_client::application::machine_credentials::Store;
    if !report.committed {
        return true;
    }
    let store = Store::select();
    report
        .id
        .as_ref()
        .and_then(|id| orchbee_machine_protocol::MachineId::new(id.as_str()).ok())
        .is_none_or(|machine| !store.exists() || store.forget(&machine).is_ok())
}
/// A committed edit reaches a running machine hub, which would otherwise carry the record as it
/// was; an unchanged catalog has nothing to tell.
fn committed(context: &mut Context, report: &Report) -> Result<(), Error> {
    if report.committed {
        connect::reload_running(context)?;
    }
    Ok(())
}
fn emit(
    command: &MachineCommand,
    report: &Report,
    error: Option<Error>,
    interrupted: bool,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let status = error.map_or(if interrupted { 130 } else { 0 }, Error::status);
    let document = json!({"schema":"orchbee.machine","version":1,"source":{"kind":"saved-machines"},
        "result":{"operation":command.operation(),
            "profileId":report.id.as_ref().map(ProfileId::as_str),
            "revision":report.catalog.as_ref().map(|c| c.revision),
            "profiles":report.catalog.as_ref().map(|c| c.profiles.iter().map(crate::machines::Profile::json).collect::<Vec<_>>()),
            "changed":report.changed,"committed":report.committed,"remote":report.remote.token(),
            "remoteVersion":report.remote_version,
            "machines":report.machines.as_ref().map(|machines| machines.iter().map(list::Listed::json).collect::<Vec<_>>()),
            "error":error.map(Error::code),"interrupted":interrupted }});
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text if error.is_some() => output::encode_text(""),
        OutputFormat::Text => {
            let mut text = String::new();
            if let MachineCommand::Connect { .. } | MachineCommand::Disconnect(_) = command {
                let message = if matches!(command, MachineCommand::Connect { .. }) {
                    "connected"
                } else {
                    "disconnected"
                };
                text.push_str(CliText::new(locale).cli_machine_message(message));
                if let Some(id) = &report.id {
                    text.push(' ');
                    text.push_str(id.as_str());
                }
                text.push('\n');
                if let Some(version) = &report.remote_version {
                    text.push_str(CliText::new(locale).cli_machine_message("remote-version"));
                    text.push(' ');
                    text.push_str(&crate::cli_text::escape::escape(version));
                    text.push('\n');
                }
            } else if let Some(machines) = &report.machines {
                text.push_str(&list::text(machines, locale));
            } else if report.catalog.is_some() {
                text.push_str(CliText::new(locale).cli_machine_message(if report.changed {
                    "saved"
                } else {
                    "unchanged"
                }));
                if let Some(id) = &report.id {
                    text.push(' ');
                    text.push_str(id.as_str());
                }
                text.push('\n');
                if let Some(version) = &report.remote_version {
                    text.push_str(CliText::new(locale).cli_machine_message("remote-version"));
                    text.push(' ');
                    text.push_str(&crate::cli_text::escape::escape(version));
                    text.push('\n');
                }
            }
            output::encode_text(&text)
        }
    };
    match encoded {
        Ok(stdout) => {
            let mut stderr = String::new();
            if let Some(error) = error {
                stderr.push_str(CliText::new(locale).cli_machine_message(error.message()));
                stderr.push('\n');
                if report.committed {
                    stderr.push_str(CliText::new(locale).cli_machine_message("already-committed"));
                    stderr.push('\n');
                }
                if matches!(report.remote, Remote::PossiblyStarted | Remote::Ready) {
                    stderr.push_str(CliText::new(locale).cli_machine_message("remote-retained"));
                    stderr.push('\n');
                }
            }
            if report.unforgotten {
                stderr.push_str(CliText::new(locale).cli_machine_message("password-kept"));
                stderr.push('\n');
            }
            Output {
                status,
                stdout,
                stderr,
                finalization: None,
            }
        }
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}
#[cfg(test)]
mod tests;
