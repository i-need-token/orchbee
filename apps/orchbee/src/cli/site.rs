//! `site start`: headless connect-or-activate for the selected local Site.
//! On Windows the selection is the current SID's default or exact named
//! scope; on Linux it is the implicit per-euid scope. The command shares the
//! TUI's private activation and admission path — a transient admission probe
//! is the only client it creates — and reports the branch it actually took:
//! `started` iff this invocation spawned the Server, `already-running`
//! otherwise. Windows may cold-recover a completely registered generation
//! only after its exact owners are proved gone and its locks and endpoints are
//! clear. Other interrupted, live, incomplete or uncertain state refuses
//! recovery-required without repair or deletion.

#[cfg(any(target_os = "linux", windows))]
use std::time::{Duration, Instant};

use serde_json::{Map, json};

use super::{Failure, FailureKind, Output, failure_output, output};
#[cfg(any(target_os = "linux", windows))]
use super::{stage_details, stage_reason_details};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

#[cfg(any(target_os = "linux", windows))]
use crate::local_activation::{ActivationCause, ActivationFailure, StartupFailure};

/// One absolute budget covering cohort validation, rendezvous, locking,
/// spawn, and readiness admission — the same 20 seconds the TUI uses.
#[cfg(any(target_os = "linux", windows))]
const START_BUDGET: Duration = Duration::from_secs(20);

pub(crate) fn start(format: OutputFormat, locale: Locale) -> Output {
    match run() {
        // Every `site start` failure is answered by the local runtime
        // authority, so the failure document carries the same source the
        // success envelope does; it also selects the localized start
        // diagnostics rather than the shared query wording.
        Err(failure) => failure_output(
            failure.source(super::config::local_runtime_source()),
            Some(format),
            locale,
        ),
        Ok(started) => {
            #[cfg(windows)]
            let warning = started
                .restore_notice
                .map(|notice| match notice {
                    orchbee_local_site_runtime::WindowsCheckpointRestoreNotice::Recovered => {
                        crate::client_view::Notice::CheckpointRecovered
                    }
                    orchbee_local_site_runtime::WindowsCheckpointRestoreNotice::Empty => {
                        crate::client_view::Notice::CheckpointEmpty
                    }
                })
                .map(|notice| format!("{}\n", locale.shell_notice(Some(notice))))
                .unwrap_or_default();
            #[cfg(not(windows))]
            let warning = String::new();
            let result = json!({
                "outcome": started.outcome,
                "scope": started.scope,
                "runtimeBase": Map::from_iter([started.runtime_base]),
            });
            // A preparation overflow answers in the caller's format, never
            // with a second stdout document.
            let too_large = || {
                failure_output(
                    Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                        "limitBytes".to_owned(),
                        json!(output::LIMIT),
                    )])),
                    Some(format),
                    locale,
                )
            };
            match format {
                OutputFormat::Json => {
                    let document = json!({
                        "schema": "orchbee.site.start",
                        "version": 1,
                        "source": super::config::local_runtime_source(),
                        "result": result,
                    });
                    match output::encode_document(&document) {
                        Ok(stdout) => Output {
                            status: 0,
                            stdout,
                            stderr: warning,
                            finalization: None,
                        },
                        Err(output::TooLarge) => too_large(),
                    }
                }
                OutputFormat::Text => {
                    let line = if started.spawned {
                        locale.cli_site_started()
                    } else {
                        locale.cli_site_already_running()
                    };
                    match output::encode_text(line) {
                        Ok(stdout) => Output {
                            status: 0,
                            stdout,
                            stderr: warning,
                            finalization: None,
                        },
                        Err(output::TooLarge) => too_large(),
                    }
                }
            }
        }
    }
}

/// The outcome a successful attempt reports: the admitted probe is dropped
/// before output, and only the branch fact and rendered paths remain.
struct StartOutcome {
    #[cfg(windows)]
    restore_notice: Option<orchbee_local_site_runtime::WindowsCheckpointRestoreNotice>,
    outcome: &'static str,
    spawned: bool,
    scope: String,
    /// The single-member `runtimeBase` detail (`path` or `pathBytesHex`).
    runtime_base: (String, serde_json::Value),
}

#[cfg(not(any(target_os = "linux", windows)))]
fn run() -> Result<StartOutcome, Failure> {
    Err(
        Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
            "platform",
            "unsupported-platform",
        )),
    )
}

#[cfg(any(target_os = "linux", windows))]
fn run() -> Result<StartOutcome, Failure> {
    let deadline = Instant::now() + START_BUDGET;
    crate::local_activation::activate_local_site(deadline)
        .map(|started| StartOutcome {
            #[cfg(windows)]
            restore_notice: started.restore_notice,
            outcome: if started.spawned {
                "started"
            } else {
                "already-running"
            },
            spawned: started.spawned,
            scope: started.scope,
            runtime_base: super::config::path_detail(&started.runtime_base),
        })
        .map_err(classify)
}

/// Project the shared activation failure onto the CLI's typed vocabulary.
/// The raw cause — never a string — selects the arm; the verdict only
/// arbitrates between a typed cause and a bare lifecycle refusal.
#[cfg(any(target_os = "linux", windows))]
fn classify(failure: ActivationFailure) -> Failure {
    match failure.cause {
        #[cfg(windows)]
        Some(ActivationCause::Configuration(error)) => {
            super::configuration::Error::from(error).failure()
        }
        Some(ActivationCause::Cohort(error)) => classify_cohort(&error),
        Some(ActivationCause::Rendezvous(error, scope_directory)) => {
            super::query::classify_rendezvous(error, &scope_directory)
        }
        Some(ActivationCause::Connect(error)) => {
            if failure.failure == StartupFailure::TimedOut {
                timeout()
            } else {
                super::query::classify_connect(error)
            }
        }
        // Lock/marker/probe I/O: a TimedOut kind means the shared budget
        // expired; anything else is an internal runtime fault.
        Some(ActivationCause::RuntimeIo(error)) => {
            if error.kind() == std::io::ErrorKind::TimedOut {
                timeout()
            } else {
                Failure::new(FailureKind::InternalFailure).details(stage_details("activation"))
            }
        }
        // Lock acquisition carries the rendezvous error; only its wrapped
        // System io kind can mean the shared budget expired.
        Some(ActivationCause::LockFailure(error)) => {
            if let orchbee_local_site_runtime::RendezvousError::System { source, .. } = &error
                && source.kind() == std::io::ErrorKind::TimedOut
            {
                timeout()
            } else {
                Failure::new(FailureKind::InternalFailure).details(stage_details("activation"))
            }
        }
        // Spawn and post-spawn liveness failures keep the existing wedge
        // verdict: the marker may already be committed — refused, never a
        // clean timeout claim.
        Some(ActivationCause::SpawnIo) => recovery_required(),
        None => match failure.failure {
            StartupFailure::TimedOut => timeout(),
            StartupFailure::RecoveryRequired => recovery_required(),
            StartupFailure::SocketPathTooLong => socket_budget(),
            _ => Failure::new(FailureKind::InternalFailure),
        },
    }
}

#[cfg(any(target_os = "linux", windows))]
fn classify_cohort(error: &std::io::Error) -> Failure {
    let kind = match error.kind() {
        std::io::ErrorKind::InvalidData => FailureKind::Incompatible,
        std::io::ErrorKind::NotFound => FailureKind::ServiceUnavailable,
        std::io::ErrorKind::PermissionDenied => FailureKind::Denied,
        std::io::ErrorKind::TimedOut => FailureKind::Timeout,
        _ => FailureKind::InternalFailure,
    };
    Failure::new(kind).details(stage_details("cohort"))
}

#[cfg(any(target_os = "linux", windows))]
fn timeout() -> Failure {
    Failure::new(FailureKind::Timeout).details(stage_details("activation"))
}

#[cfg(any(target_os = "linux", windows))]
fn recovery_required() -> Failure {
    Failure::new(FailureKind::Rejected)
        .details(stage_reason_details("activation", "recovery-required"))
}

#[cfg(any(target_os = "linux", windows))]
fn socket_budget() -> Failure {
    Failure::new(FailureKind::ServiceUnavailable).details(stage_reason_details(
        "runtime-directory",
        "address-too-long",
    ))
}
