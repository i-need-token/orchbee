//! Explicit Windows cohort updates and save-then-update channel changes; never starts a Session.
//! This entry layer owns the release-build check and reads the release environment; the engine
//! below it receives every location, key and command as a value.
use crate::cli_text::CliText;
mod acquire;
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
mod strict_json;
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish},
    failure_output, output,
};
use crate::{invocation::OutputFormat, localization::Locale};
pub(in crate::cli) use installed::{VerifiedRelease, observe as observe_installation};
use orchbee_local_runtime::{CohortArtifact, CommandFailure, CommandOutput, UpdateChannel};
use serde_json::json;
use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Error {
    NonReleaseBuild,
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
            _ => 1,
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
    /// The effective channel once read from the settings file.
    channel: Option<UpdateChannel>,
    channel_saved: bool,
    config_path: Option<PathBuf>,
    published: Option<PathBuf>,
    reused: bool,
    activated: bool,
    cleanup_pending: Option<PathBuf>,
    version: Option<String>,
    build_id: Option<String>,
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
        }
    })
}

/// Persists an optional channel first, then attempts its complete Windows update.
pub(crate) fn run(
    channel: Option<UpdateChannel>,
    timeout_ms: u32,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::ReleaseUpdate) {
        return failure_output(failure, Some(format), locale);
    }
    let signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
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
        deadline: Instant::now() + Duration::from_millis(u64::from(timeout_ms)),
        curl_checked: false,
    };
    let mut report = Report {
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
    };
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
            activation: install::path_activation,
        };
        engine::run(&inputs, &mut context, &mut report)
    })();
    let mut finalization = None;
    let mut error = result.err();
    let interrupted = match context.signals.finish() {
        Ok(Finish::Clear) => false,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
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
        error.map_or(
            if report.cleanup_pending.is_some() {
                1
            } else {
                0
            },
            Error::status,
        )
    };
    let document = document(channel, &report, error, interrupted);
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => {
            let mut text = String::new();
            if report.channel_saved {
                text.push_str(CliText::new(locale).cli_update_message("channel-saved"));
                text.push('\n');
            }
            text.push_str(CliText::new(locale).cli_update_message(error.map_or(
                "complete",
                |error| match error {
                    Error::LocalManifestRead => "local-manifest-read",
                    _ => error.code(),
                },
            )));
            text.push('\n');
            if let Some(path) = &report.published {
                text.push_str(&crate::cli_text::escape::escape(
                    &path.join("orchbee.exe").to_string_lossy(),
                ));
                text.push('\n');
            }
            if let Some(path) = &report.cleanup_pending {
                text.push_str(CliText::new(locale).cli_update_message("cleanup-pending"));
                text.push_str(&crate::cli_text::escape::escape(&path.to_string_lossy()));
                text.push('\n');
            }
            output::encode_text(&text)
        }
    };
    let mut answer = match encoded {
        Ok(stdout) => Output {
            status,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    };
    answer.finalization = finalization;
    answer
}

#[cfg(test)]
mod tests;
