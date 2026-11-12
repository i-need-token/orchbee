//! Explicit Windows release installation: `install` from the signed release directory the running
//! Client sits in, cohort updates, save-then-update channel changes, and the completion helper a
//! published copy runs; none starts a Session. This entry layer owns the build checks and reads
//! the release environment; the engine below it receives every location, key and command as a
//! value.
use crate::cli_text::CliText;
mod acquire;
mod complete;
mod engine;
mod install;
mod installed;
mod manifest;
#[cfg(windows)]
#[path = "update/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "update/unix.rs"]
mod platform;
mod signature;
mod source_path;
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish},
    failure_output, output,
};
use crate::{invocation::OutputFormat, localization::Locale};
pub(crate) use complete::run as complete_install;
pub(in crate::cli) use installed::{VerifiedRelease, observe as observe_installation};
use orchbee_local_runtime::{
    CohortArtifact, CommandFailure, CommandOutput, EnvironmentKind, UpdateChannel, environment_kind,
};
use serde_json::json;
use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

/// The total deadline of one install or update.
const DEADLINE: Duration = Duration::from_secs(600);

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
    LocalManifestRead,
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
            Self::Download | Self::LocalManifestRead => "download",
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
    /// The diagnostic text key; a local manifest that cannot be read has its own fix.
    fn message(self) -> &'static str {
        match self {
            Self::LocalManifestRead => "local-manifest-read",
            _ => self.code(),
        }
    }
}
struct Context {
    signals: ArmedSignals,
    deadline: Instant,
    curl_checked: bool,
}
impl Context {
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
        "operation": if channel.is_some() {"channel-set"} else {"update"},
        "result":{
            "channel":report.channel.or(channel).map(UpdateChannel::as_str),
            "phase":report.phase, "channelSaved":report.channel_saved, "configPath":report.config_path,
            "releasePath":report.published, "reused":report.reused, "activated":report.activated,
            "releaseVersion":report.version, "buildId":report.build_id, "cleanupPending":report.cleanup_pending,
            "error":error.map(Error::code), "interrupted":interrupted,
            "configurationFailure":report.configuration_failure.as_ref()
                .filter(|_| matches!(error, Some(Error::Configuration)))
                .map(super::configuration::Error::details),
        }
    })
}

/// Whether `install` ran the PATH step, or why it did not.
#[derive(Clone, Copy)]
enum PathStep {
    /// The person passed `--no-path`.
    NotRequested,
    /// This build has no PATH step, so it behaved as if `--no-path` were given.
    Unavailable,
}
impl PathStep {
    fn code(self) -> &'static str {
        match self {
            Self::NotRequested => "not-requested",
            Self::Unavailable => "unavailable",
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
        // A development or test cohort must never write the release installation or its channel.
        if !CohortArtifact::has_release_names() {
            return Err(Error::NonReleaseBuild);
        }
        let environment = |name: &str| std::env::var_os(name);
        let inputs = engine::Inputs {
            channel,
            state: &environment,
            manifest_source: platform::manifest_source(),
            keys: signature::TRUSTED_KEYS,
            install_root: platform::install_root().map_err(|_| Error::InstallLocation),
            probe: install::version_probe,
            activation: install::run_completion,
        };
        engine::run(&inputs, &mut context, &mut report)
    })();
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
                None => {
                    text.push_str(
                        CliText::new(locale)
                            .cli_update_message(settled.error.map_or("complete", Error::message)),
                    );
                    text.push('\n');
                }
            }
            if let Some(line) = client_line(&report) {
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
    // The PATH step reports its own outcome once this build has one.
    let path_step = if no_path {
        PathStep::NotRequested
    } else {
        PathStep::Unavailable
    };
    let mut report = Report::new();
    report.phase = "source";
    let result = (|| {
        context.check()?;
        if environment_kind() == EnvironmentKind::Development {
            return Err(Error::DevelopmentBuild);
        }
        let inputs = engine::InstallInputs {
            directory: own_directory(),
            keys: signature::TRUSTED_KEYS,
            install_root: platform::install_root().map_err(|_| Error::InstallLocation),
            probe: install::version_probe,
            activation: install::run_completion,
            completion: install::Completion {
                path: !no_path && complete::PATH_STEP_BUILT,
                allow_downgrade,
            },
        };
        engine::install(&inputs, &mut context, &mut report)
    })();
    let settled = settle(context, &report, result);
    let text = CliText::new(locale);
    let diagnostic = settled
        .error
        .map(|error| format!("{}\n", text.cli_update_message(error.message())))
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
            lines.push(
                text.cli_update_message(match path_step {
                    PathStep::NotRequested => "path-not-requested",
                    PathStep::Unavailable => "path-unavailable",
                })
                .to_owned(),
            );
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
