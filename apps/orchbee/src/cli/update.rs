//! Explicit Windows cohort updates and save-then-update channel changes; never starts a Session.
use crate::cli_text::CliText;
mod acquire;
mod install;
mod installed;
mod manifest;
#[cfg(windows)]
#[path = "update/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "update/unix.rs"]
mod platform;
mod source_path;
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
    channel_saved: bool,
    config_path: Option<PathBuf>,
    published: Option<PathBuf>,
    reused: bool,
    activated: bool,
    cleanup_pending: Option<PathBuf>,
    version: Option<String>,
    build_id: Option<String>,
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
        channel_saved: false,
        config_path: None,
        published: None,
        reused: false,
        activated: false,
        cleanup_pending: None,
        version: None,
        build_id: None,
    };
    let mut selected = channel;
    let result = (|| {
        context.check()?;
        // A development or test cohort must never write the release installation or its channel.
        if !CohortArtifact::has_release_names() {
            return Err(Error::NonReleaseBuild);
        }
        if let Some(channel) = channel {
            report.config_path =
                Some(platform::save_channel(channel).map_err(|_| Error::Configuration)?);
            report.channel_saved = true;
        }
        let selected_channel = platform::selected_channel().map_err(|_| Error::Configuration)?;
        selected = Some(selected_channel);
        report.phase = "source";
        let source = acquire::Source::configured(selected_channel)?;
        report.phase = "manifest";
        let bytes = source.manifest(&mut context)?;
        let release =
            manifest::Release::parse(&bytes, selected_channel).ok_or(Error::ManifestInvalid)?;
        report.version = Some(release.version.clone());
        report.build_id = Some(release.build_id.clone());
        let root = install::root()?;
        report.phase = "download";
        let path = install::publish(&root, &source, &release, &mut context, &mut report)?;
        report.phase = "activate";
        install::activate(&root, &path, &mut context)?;
        report.activated = true;
        report.phase = "complete";
        Ok::<(), Error>(())
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
    let document = json!({
        "schema":"orchbee.update", "version":1, "source":{"kind":"local-installation"},
        "operation": if channel.is_some() {"channel-set"} else {"update"},
        "result":{
            "channel":selected.map(UpdateChannel::as_str),
            "phase":report.phase, "channelSaved":report.channel_saved, "configPath":report.config_path,
            "releasePath":report.published, "reused":report.reused, "activated":report.activated,
            "releaseVersion":report.version, "buildId":report.build_id, "cleanupPending":report.cleanup_pending,
            "error":error.map(Error::code), "interrupted":interrupted,
        }
    });
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
