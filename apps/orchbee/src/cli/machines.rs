//! Saved-machine commands; remote readiness and local publication have separate outcomes.
use crate::cli_text::CliText;
pub(crate) mod link;
pub(crate) mod routing;
#[cfg(windows)]
mod shared_carrier;
mod ssh;
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish},
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
    Cancelled,
    Timeout,
    PhysicalUnknown,
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
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::PhysicalUnknown => "physical-unknown",
            Self::NotYetAvailable => "not-yet-available",
            Self::Internal => "internal",
        }
    }
    fn status(self) -> u8 {
        match self {
            Self::Catalog(CatalogError::NotFound | CatalogError::Ambiguous) => 3,
            Self::Catalog(
                CatalogError::Busy | CatalogError::Capacity | CatalogError::RevisionExhausted,
            )
            | Self::SshUnavailable
            | Self::RemoteNotInstalled
            | Self::RemoteNotReady
            | Self::NotYetAvailable => 4,
            Self::SshFailed | Self::RemoteProtocol => 5,
            Self::RemoteUnsupported | Self::RemoteOtherEnvironment => 6,
            Self::Cancelled => 130,
            Self::Timeout => 7,
            Self::PhysicalUnknown => 8,
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
}
pub(crate) fn run(command: &MachineCommand, format: OutputFormat, locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::SavedMachines
        .require()
        .is_err()
    {
        return super::unsupported_platform(Some(format), locale);
    }
    let signals = match ArmedSignals::arm_interrupts() {
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
            + Duration::from_secs(if matches!(command, MachineCommand::Add(_)) {
                60
            } else {
                5
            }),
    };
    let mut report = Report {
        catalog: None,
        id: None,
        changed: false,
        committed: false,
        remote: Remote::NotContacted,
        remote_version: None,
    };
    let mut error = execute(command, &mut context, &mut report).err();
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
fn execute(
    command: &MachineCommand,
    context: &mut Context,
    report: &mut Report,
) -> Result<(), Error> {
    context.check()?;
    if matches!(command, MachineCommand::NotYetAvailable(_)) {
        return Err(Error::NotYetAvailable);
    }
    let store = Store::configured().map_err(Error::Catalog)?;
    let edit = match command {
        MachineCommand::NotYetAvailable(_) => return Err(Error::NotYetAvailable),
        MachineCommand::List => {
            report.catalog = Some(store.read().map_err(Error::Catalog)?);
            return context.check();
        }
        MachineCommand::Add(draft) => {
            let id = ProfileId::generate().map_err(Error::Catalog)?;
            let edit = Edit::Add {
                id: id.clone(),
                draft: draft.clone(),
            };
            store
                .read()
                .map_err(Error::Catalog)?
                .edit(&edit)
                .map_err(Error::Catalog)?;
            report.id = Some(id);
            report.remote_version = Some(ssh::prepare(draft, context, &mut report.remote)?);
            edit
        }
        MachineCommand::Rename { machine, label } => {
            return commit_selected(&store, machine, context, report, |id| Edit::Rename {
                id,
                label: label.clone(),
            });
        }
        MachineCommand::Remove(machine) => {
            return commit_selected(&store, machine, context, report, Edit::Remove);
        }
        MachineCommand::Enable { machine, enabled } => {
            return commit_selected(&store, machine, context, report, |id| Edit::Enable {
                id,
                enabled: *enabled,
            });
        }
    };
    context.check()?;
    let (catalog, changed) = store.commit(&edit).map_err(Error::Catalog)?;
    report.catalog = Some(catalog);
    report.changed = changed;
    report.committed = changed;
    Ok(())
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
    let (catalog, changed, id) = store
        .commit_selected(machine, edit)
        .map_err(Error::Catalog)?;
    report.id = Some(id);
    report.catalog = Some(catalog);
    report.changed = changed;
    report.committed = changed;
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
            "error":error.map(Error::code),"interrupted":interrupted }});
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text if error.is_some() => output::encode_text(""),
        OutputFormat::Text => {
            let mut text = String::new();
            if let Some(catalog) = &report.catalog {
                if matches!(command, MachineCommand::List) {
                    if catalog.profiles.is_empty() {
                        text.push_str(CliText::new(locale).cli_machine_message("empty"));
                        text.push('\n');
                    }
                    for profile in &catalog.profiles {
                        text.push_str(&format!(
                            "{}\t{}\t{}\t{}\t{}\n",
                            profile.id.as_str(),
                            crate::cli_text::escape::escape(&profile.draft.label),
                            crate::cli_text::escape::escape(
                                profile.draft.ssh_target().unwrap_or_else(
                                    || CliText::new(locale).cli_machine_message("unsupported")
                                )
                            ),
                            profile.draft.session.as_str(),
                            CliText::new(locale).cli_machine_message(if profile.enabled {
                                "enabled"
                            } else {
                                "disabled"
                            }),
                        ));
                    }
                } else {
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
            }
            output::encode_text(&text)
        }
    };
    match encoded {
        Ok(stdout) => {
            let mut stderr = String::new();
            if let Some(error) = error {
                stderr.push_str(CliText::new(locale).cli_machine_message(error.code()));
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
