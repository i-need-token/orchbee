//! Explicit Windows release installation: `install` from the signed release directory the running
//! Client sits in, cohort updates, save-then-update channel changes, and the completion helper a
//! published copy runs; none starts a Session. This entry layer owns the build checks and reads
//! the release environment; the engine below it receives every location, key and command as a
//! value.
use crate::cli_text::CliText;
mod acquire;
mod archive;
mod complete;
mod engine;
mod feed;
mod install;
mod installed;
mod manifest;
mod path_step;
mod path_value;
#[cfg(windows)]
#[path = "update/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "update/unix.rs"]
mod platform;
mod signature;
mod source_path;
mod uninstall;
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish},
    failure_output, output,
};
use crate::{invocation::OutputFormat, localization::Locale};
pub(crate) use complete::run as complete_install;
pub(in crate::cli) use installed::{VerifiedRelease, observe as observe_installation};
use orchbee_local_runtime::{
    AdmissionRefusal, CohortArtifact, CommandFailure, CommandOutput, EnvironmentKind,
    RefusalReason, UpdateChannel, environment_kind,
};
use serde_json::json;
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub(crate) use uninstall::{installer as installer_uninstall, run as uninstall};

/// The deadline of one install or update. It stands still while the archive transfers under its
/// own limit (`acquire::transfer_seconds`).
const DEADLINE: Duration = Duration::from_secs(600);
/// Whether `update` admits a loopback feed address, fixed at compile time: only a test build does,
/// so a release or development build never reads plain HTTP.
const LOOPBACK: bool = matches!(environment_kind(), EnvironmentKind::Test);

#[derive(Clone, Copy)]
enum Error {
    NonReleaseBuild,
    DevelopmentBuild,
    NotReleaseDirectory,
    Configuration,
    SourceUnconfigured,
    SourceInvalid,
    SigningKeysUnavailable,
    SignatureUntrusted,
    SignatureInvalid,
    ManifestInvalid,
    Download,
    /// A local feed, or a manifest of a local release directory, could not be read.
    LocalSourceRead,
    DownloadUnavailable,
    Checksum,
    Storage,
    InstallLocation,
    Busy,
    CohortInvalid,
    VersionConflict,
    DowngradeRefused,
    OtherArchitecture,
    SmokeFailed,
    /// The completing Client could not place `bin\<stem>.exe`.
    PathLauncher,
    /// The completing Client could not edit the user `Path`.
    PathVariable,
    Activation,
    ActivationUnknown,
    Cancelled,
    Timeout,
    PhysicalUnknown,
    Internal,
}
impl Error {
    fn code(self) -> &'static str {
        match self {
            Self::NonReleaseBuild => "non-release-build",
            Self::DevelopmentBuild => "development-build",
            Self::NotReleaseDirectory => "not-release-directory",
            Self::Configuration => "configuration",
            Self::SourceUnconfigured => "source-unconfigured",
            Self::SourceInvalid => "source-invalid",
            Self::SigningKeysUnavailable => "signing-keys-unavailable",
            Self::SignatureUntrusted => "signature-untrusted",
            Self::SignatureInvalid => "signature-invalid",
            Self::ManifestInvalid => "manifest-invalid",
            Self::Download | Self::LocalSourceRead => "download",
            Self::DownloadUnavailable => "download-unavailable",
            Self::Checksum => "checksum",
            Self::Storage => "storage",
            Self::InstallLocation => "install-location",
            Self::Busy => "busy",
            Self::CohortInvalid => "cohort-invalid",
            Self::VersionConflict => "version-conflict",
            Self::DowngradeRefused => "downgrade-refused",
            Self::OtherArchitecture => "other-architecture",
            Self::SmokeFailed => "smoke-failed",
            Self::PathLauncher => "path-launcher",
            Self::PathVariable => "path-variable",
            Self::Activation => "activation",
            Self::ActivationUnknown => "activation-unknown",
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::PhysicalUnknown => "physical-unknown",
            Self::Internal => "internal",
        }
    }
    fn status(self) -> u8 {
        match self {
            Self::Cancelled => 130,
            Self::Timeout => 7,
            Self::PhysicalUnknown | Self::ActivationUnknown => 8,
            Self::VersionConflict | Self::DowngradeRefused | Self::OtherArchitecture => 3,
            Self::DevelopmentBuild | Self::NotReleaseDirectory => 4,
            _ => 1,
        }
    }
    /// The diagnostic text key; a local feed that cannot be read has its own fix.
    fn message(self) -> &'static str {
        match self {
            Self::LocalSourceRead => "local-feed-read",
            _ => self.code(),
        }
    }
}
struct Context {
    signals: ArmedSignals,
    deadline: Instant,
    curl_checked: bool,
    /// The first line a failed completion helper wrote to standard error, which names its cause.
    completion_diagnostic: Option<String>,
    /// Where and why the first `storage` failure happened.
    storage_cause: Option<StorageCause>,
}
impl Context {
    /// Keeps the cause of the run's first storage failure, at `path` unless the error names the
    /// refused object itself, and answers [`Error::Storage`].
    fn storage(&mut self, path: &Path, error: &io::Error) -> Error {
        self.storage_cause
            .get_or_insert_with(|| StorageCause::new(path, error));
        Error::Storage
    }
    fn interrupted(&mut self) -> io::Result<bool> {
        self.signals.take().map(|s| s.is_some())
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
    fn command_result(&self, result: &CommandOutput) -> Result<(), Error> {
        match result.failure {
            None => Ok(()),
            Some(CommandFailure::Cancelled) => Err(Error::Cancelled),
            Some(CommandFailure::Timeout) => Err(Error::Timeout),
            Some(CommandFailure::PhysicalUnknown) => Err(Error::PhysicalUnknown),
            Some(_) => Err(Error::Download),
        }
    }
}
/// The object a store step refused or failed on, and why, so that a `storage` failure says more
/// than that it happened. Only standard error carries it; the documents keep their members.
struct StorageCause {
    path: PathBuf,
    reason: StorageReason,
}
enum StorageReason {
    /// Private admission refused the object.
    Refused(RefusalReason),
    /// The system refused the operation with this error code.
    System(i32),
    /// Neither says more.
    Unknown,
}
impl StorageCause {
    fn new(path: &Path, error: &io::Error) -> Self {
        match AdmissionRefusal::of(error) {
            Some(refusal) => Self {
                path: refusal.path.clone(),
                reason: StorageReason::Refused(refusal.reason),
            },
            None => Self {
                path: path.to_owned(),
                reason: error
                    .raw_os_error()
                    .map_or(StorageReason::Unknown, StorageReason::System),
            },
        }
    }
    /// One localized line: the path, then what was wrong with it.
    fn line(&self, text: CliText) -> String {
        let (key, code) = match self.reason {
            StorageReason::Refused(RefusalReason::OtherOwner) => ("storage-owner", None),
            StorageReason::Refused(
                RefusalReason::OthersCanRead
                | RefusalReason::OthersCanChange
                | RefusalReason::AccessControlList,
            ) => ("storage-access", None),
            StorageReason::Refused(RefusalReason::SymbolicLink) => ("storage-link", None),
            StorageReason::System(code) => ("storage-system", Some(code)),
            StorageReason::Refused(_) | StorageReason::Unknown => ("storage-unknown", None),
        };
        text.cli_storage_cause(
            &crate::native_path::display(&self.path.to_string_lossy()),
            key,
            code,
        )
    }
}

struct Report {
    phase: &'static str,
    /// The effective channel once read from the settings file, or the channel an installed
    /// manifest names.
    channel: Option<UpdateChannel>,
    channel_saved: bool,
    config_path: Option<PathBuf>,
    published: Option<PathBuf>,
    reused: bool,
    activated: bool,
    cleanup_pending: Option<PathBuf>,
    version: Option<String>,
    build_id: Option<String>,
    /// The version the pointer named active before this run, once read.
    active_before: Option<String>,
    /// The update found the channel's release already active, or a lower one on the feed, and
    /// changed nothing.
    up_to_date: bool,
    /// Why the settings file could not be saved or read, once that stopped the run.
    configuration_failure: Option<super::configuration::Error>,
}
impl Report {
    fn new() -> Self {
        Self {
            phase: "configuration",
            channel: None,
            channel_saved: false,
            config_path: None,
            published: None,
            reused: false,
            activated: false,
            cleanup_pending: None,
            version: None,
            build_id: None,
            active_before: None,
            up_to_date: false,
            configuration_failure: None,
        }
    }
    /// Records why the settings file stopped the run and returns the run's error.
    fn configuration(&mut self, failure: super::configuration::Error) -> Error {
        self.configuration_failure = Some(failure);
        Error::Configuration
    }
}

/// The `orchbee.update` document for one run that requested `channel`, if any.
fn document(
    channel: Option<UpdateChannel>,
    report: &Report,
    error: Option<Error>,
    interrupted: bool,
) -> serde_json::Value {
    json!({
        "schema":"orchbee.update", "version":1, "source":{"kind":"local-installation"},
        "result":{
            "operation": if channel.is_some() {"channel-set"} else {"update"},
            "channel":report.channel.or(channel).map(UpdateChannel::as_str),
            "phase":report.phase, "channelSaved":report.channel_saved, "configPath":report.config_path,
            "releasePath":report.published, "reused":report.reused, "activated":report.activated,
            "upToDate":report.up_to_date,
            "releaseVersion":report.version, "buildId":report.build_id, "cleanupPending":report.cleanup_pending,
            "error":error.map(Error::code), "interrupted":interrupted,
            "configurationFailure":report.configuration_failure.as_ref()
                .filter(|_| matches!(error, Some(Error::Configuration)))
                .map(super::configuration::Error::details),
        }
    })
}

/// What `install` did to the user `Path`, or why it did nothing.
#[derive(Clone, Copy)]
enum PathStep {
    /// The person passed `--no-path`.
    NotRequested,
    /// This build has no PATH step, so it behaved as if `--no-path` were given.
    Unavailable,
    /// The run ended before the PATH step completed.
    NotReached,
    /// `<store root>\bin` was added to the user `Path`.
    Added,
    /// The user `Path` already named `<store root>\bin` and was left unchanged.
    Present,
    /// Entries naming other folders inside the store root were removed, and the user `Path` names
    /// `<store root>\bin`.
    Replaced,
}
impl PathStep {
    fn code(self) -> &'static str {
        match self {
            Self::NotRequested => "not-requested",
            Self::Unavailable => "unavailable",
            Self::NotReached => "not-reached",
            Self::Added => "added",
            Self::Present => "present",
            Self::Replaced => "replaced",
        }
    }

    /// The outcome of a completed PATH step, classified by the edit planned from the user `Path` as
    /// it was before the completion ran. A value that could not be read beforehand classifies as
    /// added, since the completion succeeded and the `Path` now names `bin`.
    fn completed(plan: Option<&path_value::Edit>) -> Self {
        match plan {
            Some(path_value::Edit::Unchanged) => Self::Present,
            Some(path_value::Edit::Write { removed: true, .. }) => Self::Replaced,
            _ => Self::Added,
        }
    }
}

/// The `orchbee.install` document for one run.
fn install_document(
    report: &Report,
    path_step: PathStep,
    error: Option<Error>,
    interrupted: bool,
) -> serde_json::Value {
    json!({
        "schema":"orchbee.install", "version":1,
        "result":{
            "phase":report.phase, "channel":report.channel.map(UpdateChannel::as_str),
            "releaseVersion":report.version, "buildId":report.build_id,
            "releasePath":report.published, "reused":report.reused, "activated":report.activated,
            "activeBefore":report.active_before, "pathStep":path_step.code(),
            "cleanupPending":report.cleanup_pending,
            "error":error.map(Error::code), "interrupted":interrupted,
        }
    })
}

/// The text for a run the settings file stopped: the configuration failure message for the same
/// closed details the JSON form carries.
fn configuration_text(
    details: &serde_json::Map<String, serde_json::Value>,
    locale: Locale,
) -> String {
    CliText::new(locale).cli_failure(
        &Failure::new(FailureKind::CheckFailed)
            .failure_schema("orchbee.cli.failure.configuration")
            .details(details.clone()),
    )
}

/// One run's terminal facts once its signals are settled.
struct Settled {
    status: u8,
    error: Option<Error>,
    interrupted: bool,
    finalization: Option<super::SourceFinalization>,
}

/// Arms the console interrupt and builds the run's context, or answers the failure.
fn start(format: OutputFormat, locale: Locale) -> Result<Context, Box<Output>> {
    super::execution::admit(super::execution::Capability::ReleaseUpdate)
        .map_err(|failure| Box::new(failure_output(failure, Some(format), locale)))?;
    let signals = ArmedSignals::arm_interrupts().map_err(|_| {
        Box::new(failure_output(
            Failure::new(FailureKind::InternalFailure),
            Some(format),
            locale,
        ))
    })?;
    Ok(Context {
        signals,
        deadline: Instant::now() + DEADLINE,
        curl_checked: false,
        completion_diagnostic: None,
        storage_cause: None,
    })
}

/// Settles the console interrupt and derives the exit status: interruption first, then the
/// error, then a stage left for recovery.
fn settle(context: Context, report: &Report, result: Result<(), Error>) -> Settled {
    let mut finalization = None;
    let mut error = result.err();
    let interrupted = match context.signals.finish() {
        Ok(Finish::Clear) => false,
        Ok(Finish::Cancelled { finalization: owed }) => {
            finalization = Some(owed);
            true
        }
        Err(_) => {
            error = Some(Error::Internal);
            false
        }
    };
    let status = if interrupted {
        130
    } else {
        error.map_or(u8::from(report.cleanup_pending.is_some()), Error::status)
    };
    Settled {
        status,
        error,
        interrupted,
        finalization,
    }
}

/// The published Client's path as people read and paste it.
fn client_line(report: &Report) -> Option<String> {
    report.published.as_ref().map(|path| {
        crate::native_path::display(
            &path
                .join(CohortArtifact::Client.file_name())
                .to_string_lossy(),
        )
    })
}

/// The release store root this build installs into and updates: the `storeRoot` the remote link
/// helper reports.
#[cfg(windows)]
pub(in crate::cli) fn store_root() -> io::Result<PathBuf> {
    platform::install_root()
}

/// Persists an optional channel first, then attempts its complete Windows update.
pub(crate) fn run(
    channel: Option<UpdateChannel>,
    timeout_ms: u32,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut context = match start(format, locale) {
        Ok(context) => context,
        Err(output) => return *output,
    };
    context.deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    let mut report = Report::new();
    let result = (|| {
        context.check()?;
        // A development cohort never installs or saves a channel. A test cohort updates as its
        // `install` does: with the test key its build compiled, into its own store by default.
        if environment_kind() == EnvironmentKind::Development {
            return Err(Error::NonReleaseBuild);
        }
        let environment = |name: &str| std::env::var_os(name);
        let inputs = engine::Inputs {
            channel,
            state: &environment,
            feed_source: platform::feed_source(),
            keys: signature::TRUSTED_KEYS,
            install_root: platform::install_root().map_err(|_| Error::InstallLocation),
            loopback: LOOPBACK,
            probe: install::version_probe,
            activation: install::run_completion,
            unpack: &archive::system_unpack,
        };
        engine::run(&inputs, &mut context, &mut report)
    })();
    let completion_diagnostic = context.completion_diagnostic.take();
    let storage_cause = context.storage_cause.take();
    let settled = settle(context, &report, result);
    let document = document(channel, &report, settled.error, settled.interrupted);
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => {
            let mut text = String::new();
            if report.channel_saved {
                text.push_str(CliText::new(locale).cli_update_message("channel-saved"));
                text.push('\n');
            }
            match document["result"]["configurationFailure"].as_object() {
                // The typed reason and its fix come from the same details the JSON form carries.
                Some(failure) => text.push_str(&configuration_text(failure, locale)),
                // One line naming the version that stays active.
                None if settled.error.is_none() && report.up_to_date => {
                    text.push_str(CliText::new(locale).cli_update_message("up-to-date"));
                    text.push_str(report.active_before.as_deref().unwrap_or_default());
                    text.push('\n');
                }
                None if settled.error.is_none() => {
                    text.push_str(CliText::new(locale).cli_update_message("updated-to"));
                    text.push_str(report.version.as_deref().unwrap_or_default());
                    text.push('\n');
                    text.push_str(CliText::new(locale).cli_update_message("complete"));
                    text.push('\n');
                }
                None => {
                    text.push_str(
                        CliText::new(locale)
                            .cli_update_message(settled.error.map_or("complete", Error::message)),
                    );
                    text.push('\n');
                    if let Some(cause) = completion_diagnostic.filter(|_| settled.error.is_some()) {
                        text.push_str(&cause);
                        text.push('\n');
                    }
                    if let Some(cause) = storage_cause
                        .as_ref()
                        .filter(|_| matches!(settled.error, Some(Error::Storage)))
                    {
                        text.push_str(&cause.line(CliText::new(locale)));
                        text.push('\n');
                    }
                }
            }
            if let Some(line) = client_line(&report).filter(|_| !report.up_to_date) {
                text.push_str(&line);
                text.push('\n');
            }
            if let Some(path) = &report.cleanup_pending {
                text.push_str(CliText::new(locale).cli_update_message("cleanup-pending"));
                text.push_str(&crate::native_path::display(&path.to_string_lossy()));
                text.push('\n');
            }
            output::encode_text(&text)
        }
    };
    answer(encoded, String::new(), settled, format, locale)
}

/// The signed release directory this executable sits in.
fn own_directory() -> Result<PathBuf, Error> {
    let executable = std::env::current_exe().map_err(|_| Error::Internal)?;
    executable
        .parent()
        .map(PathBuf::from)
        .ok_or(Error::NotReleaseDirectory)
}

/// Installs the signed release directory this executable sits in and makes it the active version.
/// A release build uses the release store root and keys; a test build its own root and the test
/// key; a development build refuses.
pub(crate) fn install(
    no_path: bool,
    allow_downgrade: bool,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut context = match start(format, locale) {
        Ok(context) => context,
        Err(output) => return *output,
    };
    let requested = !no_path && complete::PATH_STEP_BUILT;
    let mut path_step = if no_path {
        PathStep::NotRequested
    } else if requested {
        PathStep::NotReached
    } else {
        PathStep::Unavailable
    };
    let install_root = platform::install_root().map_err(|_| Error::InstallLocation);
    let mut path_plan = None;
    let mut report = Report::new();
    report.phase = "source";
    let result = (|| {
        context.check()?;
        if environment_kind() == EnvironmentKind::Development {
            return Err(Error::DevelopmentBuild);
        }
        if requested && let Ok(root) = &install_root {
            path_plan = platform::planned_path_edit(root);
        }
        let inputs = engine::InstallInputs {
            directory: own_directory(),
            keys: signature::TRUSTED_KEYS,
            install_root: install_root.clone(),
            probe: install::version_probe,
            activation: install::run_completion,
            completion: install::Completion {
                path: requested,
                allow_downgrade,
            },
        };
        engine::install(&inputs, &mut context, &mut report)
    })();
    if requested && result.is_ok() {
        path_step = PathStep::completed(path_plan.as_ref());
    }
    let completion_diagnostic = context.completion_diagnostic.take();
    let storage_cause = context.storage_cause.take();
    let settled = settle(context, &report, result);
    let text = CliText::new(locale);
    let diagnostic = settled
        .error
        .map(|error| {
            let mut lines = format!("{}\n", text.cli_update_message(error.message()));
            if let Some(cause) = &completion_diagnostic {
                lines.push_str(cause);
                lines.push('\n');
            }
            if let Some(cause) = storage_cause
                .as_ref()
                .filter(|_| matches!(error, Error::Storage))
            {
                lines.push_str(&cause.line(text));
                lines.push('\n');
            }
            lines
        })
        .unwrap_or_default();
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&install_document(
            &report,
            path_step,
            settled.error,
            settled.interrupted,
        )),
        // A failure writes nothing to standard output under text.
        OutputFormat::Text if settled.error.is_some() || settled.interrupted => {
            output::encode_text("")
        }
        OutputFormat::Text => {
            let mut lines = vec![format!(
                "{}{}",
                text.cli_update_message(if report.reused {
                    "install-reused"
                } else {
                    "install-complete"
                }),
                report.version.as_deref().unwrap_or_default()
            )];
            lines.extend(client_line(&report));
            let bin = install_root
                .as_ref()
                .ok()
                .and_then(|root| root.to_str())
                .map(|root| crate::native_path::display(&path_value::bin(root)))
                .unwrap_or_default();
            lines.push(match path_step {
                PathStep::NotRequested => text.cli_update_message("path-not-requested").to_owned(),
                PathStep::Unavailable => text.cli_update_message("path-unavailable").to_owned(),
                PathStep::NotReached => text.cli_update_message("path-not-reached").to_owned(),
                PathStep::Added => format!("{}{bin}", text.cli_update_message("path-added")),
                PathStep::Present => format!("{}{bin}", text.cli_update_message("path-present")),
                PathStep::Replaced => format!("{}{bin}", text.cli_update_message("path-replaced")),
            });
            // The change is not broadcast, so programs started earlier keep the old environment.
            if matches!(path_step, PathStep::Added | PathStep::Replaced) {
                lines.push(text.cli_update_message("path-new-terminal").to_owned());
            }
            if let Some(path) = &report.cleanup_pending {
                lines.push(format!(
                    "{}{}",
                    text.cli_update_message("cleanup-pending"),
                    crate::native_path::display(&path.to_string_lossy())
                ));
            }
            output::encode_text(&(lines.join("\n") + "\n"))
        }
    };
    answer(encoded, diagnostic, settled, format, locale)
}

fn answer(
    encoded: Result<Vec<u8>, impl Sized>,
    stderr: String,
    settled: Settled,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut answer = match encoded {
        Ok(stdout) => Output {
            status: settled.status,
            stdout,
            stderr,
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    };
    answer.finalization = settled.finalization;
    answer
}

#[cfg(test)]
mod tests;
