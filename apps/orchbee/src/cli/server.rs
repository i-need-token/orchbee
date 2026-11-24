//! `server start`: headless connect-or-activate for the selected local Session.
//! On Windows the selection is the current SID's default or exact named
//! scope; on Linux it is the implicit per-euid scope. The command shares the
//! TUI's private activation and admission path — a transient admission probe
//! is the only client it creates — and reports the branch it actually took:
//! `started` iff this invocation spawned the Server, `already-running`
//! otherwise. Windows may cold-recover a completely registered generation
//! only after its exact owners are proved gone and its locks and endpoints are
//! clear. Other interrupted, live, incomplete or uncertain state refuses
//! recovery-required without repair or deletion; a registered process proved
//! still running is named as `blockingProcess`. A Server the system refused to
//! create answers `server-not-started` with the system's answer.

use crate::cli_text::CliText;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use std::time::{Duration, Instant};

use serde_json::{Map, json};

use super::{Failure, FailureKind, Output, failure_output, output};
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use super::{stage_details, stage_reason_details};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use crate::local_activation::{ActivationCause, ActivationFailure, StartupFailure};

/// One absolute budget covering cohort validation, rendezvous, locking,
/// spawn, and readiness admission — the same 20 seconds the TUI uses.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
const START_BUDGET: Duration = Duration::from_secs(20);

pub(crate) fn start(format: OutputFormat, locale: Locale) -> Output {
    match run() {
        // Every `server start` failure is answered by the local runtime
        // authority, so the failure document carries the same source the
        // success envelope does; it also selects the localized start
        // diagnostics rather than the shared query wording.
        Err(failure) => failure_output(
            failure.source(crate::cli_outcome::local_runtime_source()),
            Some(format),
            locale,
        ),
        Ok(started) => {
            let warning = restore_warning(started.restore_notice, locale);
            let result = json!({
                "outcome": started.outcome,
                "restoreNotice": started.restore_notice.map(|notice| match notice {
                    orchbee_local_runtime::CheckpointRestoreNotice::Recovered => "recovered",
                    orchbee_local_runtime::CheckpointRestoreNotice::Empty => "empty",
                }),
                "scope": started.scope,
                "runtimeBase": Map::from_iter([started.runtime_base]),
                "negotiatedCapabilities": started.negotiated_capabilities,
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
                        "schema": "orchbee.server.start",
                        "version": 1,
                        "source": crate::cli_outcome::local_runtime_source(),
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
                        CliText::new(locale).cli_server_started()
                    } else {
                        CliText::new(locale).cli_server_already_running()
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

/// The stderr line a start that restored saved Workspaces adds, or nothing.
fn restore_warning(
    notice: Option<orchbee_local_runtime::CheckpointRestoreNotice>,
    locale: Locale,
) -> String {
    notice
        .map(|notice| match notice {
            orchbee_local_runtime::CheckpointRestoreNotice::Recovered => {
                crate::client_view::Notice::CheckpointRecovered
            }
            orchbee_local_runtime::CheckpointRestoreNotice::Empty => {
                crate::client_view::Notice::CheckpointEmpty
            }
        })
        .map(|notice| format!("{}\n", locale.shell_notice(Some(notice))))
        .unwrap_or_default()
}

/// The outcome a successful attempt reports: the admitted probe is dropped
/// before output, and only the branch fact and rendered paths remain.
struct StartOutcome {
    restore_notice: Option<orchbee_local_runtime::CheckpointRestoreNotice>,
    outcome: &'static str,
    spawned: bool,
    scope: String,
    /// The single-member `runtimeBase` detail (`path` or `pathBytesHex`).
    runtime_base: (String, serde_json::Value),
    /// The capabilities the admitted probe's `client-server` handshake listed, in bytewise order.
    negotiated_capabilities: Vec<String>,
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn run() -> Result<StartOutcome, Failure> {
    Err(
        Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
            "platform",
            "unsupported-platform",
        )),
    )
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn run() -> Result<StartOutcome, Failure> {
    let deadline = Instant::now() + START_BUDGET;
    crate::local_activation::activate_local_session(deadline)
        .map(|started| StartOutcome {
            negotiated_capabilities: started
                .client
                .server_capabilities()
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect(),
            restore_notice: started.restore_notice,
            outcome: if started.spawned {
                "started"
            } else {
                "already-running"
            },
            spawned: started.spawned,
            scope: started.scope,
            runtime_base: crate::native_path::detail(&started.runtime_base),
        })
        .map_err(classify)
}

/// The first phase of a creating command that declares activation: the selected local Session
/// starts exactly as `server start` starts it, under the same budget, before the command's own
/// deadline begins. The answer is the stderr text: `server start`'s started line and restore
/// notice when this invocation started the Session, otherwise nothing. A command a machine runs
/// for another machine's route never starts its Session; the route already made it ready.
pub(super) fn activate_first(locale: Locale) -> Result<String, Failure> {
    if routed() {
        return Ok(String::new());
    }
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        let started =
            crate::local_activation::activate_local_session(Instant::now() + START_BUDGET)
                .map_err(classify)?;
        Ok(if started.spawned {
            format!(
                "{}{}",
                CliText::new(locale).cli_server_started(),
                restore_warning(started.restore_notice, locale)
            )
        } else {
            String::new()
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _unused = locale;
        Err(
            Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
                "platform",
                "unsupported-platform",
            )),
        )
    }
}

/// The private marker the machine link sets on the command it runs for a routed request.
pub(crate) const ROUTED_COMMAND: &str = "ORCHBEE_ROUTED_COMMAND";

/// This command runs for another machine's route.
pub(crate) fn routed() -> bool {
    std::env::var_os(ROUTED_COMMAND).is_some_and(|value| value == "1")
}

/// Activates like `server start` but keeps the admitted connection for `server connect`.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(super) fn activate_for_relay(
    deadline: std::time::Instant,
) -> Result<crate::local_activation::StartedSession, Failure> {
    crate::local_activation::activate_local_session(deadline).map_err(classify)
}

/// Project the shared activation failure onto the CLI's typed vocabulary.
/// The raw cause — never a string — selects the arm; the verdict only
/// arbitrates between a typed cause and a bare lifecycle refusal.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn classify(failure: ActivationFailure) -> Failure {
    match failure.cause {
        #[cfg(any(windows, target_os = "macos"))]
        Some(ActivationCause::Configuration(error)) => {
            super::configuration::Error::from(error).failure()
        }
        Some(ActivationCause::Cohort(error)) => classify_cohort(&error),
        Some(ActivationCause::Rendezvous(error, scope_directory)) => naming_refusal(
            super::query::classify_rendezvous(error, &scope_directory),
            failure.failure,
        ),
        Some(ActivationCause::Connect(error)) => {
            if failure.failure == StartupFailure::TimedOut {
                timeout()
            } else {
                super::query::classify_connect(error)
            }
        }
        Some(ActivationCause::RuntimeLocation(error)) => {
            if error.kind() == std::io::ErrorKind::TimedOut {
                timeout()
            } else {
                naming_refusal(
                    Failure::new(FailureKind::CheckFailed).details(stage_details("runtime-base")),
                    failure.failure,
                )
            }
        }
        // Lock/marker/probe I/O: a TimedOut kind means the shared budget
        // expired; anything else is an internal runtime fault.
        Some(ActivationCause::RuntimeIo(error)) => {
            if error.kind() == std::io::ErrorKind::TimedOut {
                timeout()
            } else {
                runtime_io(failure.failure)
            }
        }
        // Lock acquisition carries the rendezvous error; only its wrapped
        // System io kind can mean the shared budget expired.
        Some(ActivationCause::LockFailure(error)) => {
            if let orchbee_local_runtime::RendezvousError::System { source, .. } = &error
                && source.kind() == std::io::ErrorKind::TimedOut
            {
                timeout()
            } else {
                runtime_io(failure.failure)
            }
        }
        // A record failure around the spawn and a post-spawn liveness failure
        // keep the existing wedge verdict: the marker may already be committed
        // — refused, never a clean timeout claim.
        Some(ActivationCause::SpawnIo) => recovery_required(),
        None => match failure.failure {
            StartupFailure::TimedOut => timeout(),
            StartupFailure::SessionStopping => super::query::stopping(),
            StartupFailure::RecoveryRequired => recovery_required(),
            StartupFailure::RecoveryBlocked(blocker) => recovery_blocked(blocker),
            StartupFailure::ServerNotStarted(spawn) => server_not_started(spawn),
            StartupFailure::SocketPathTooLong => socket_budget(),
            _ => Failure::new(FailureKind::InternalFailure),
        },
    }
}

/// A refused folder or file names the item at fault and how to fix it on the check failure its
/// step reports, as the TUI words it.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn naming_refusal(failed: Failure, verdict: StartupFailure) -> Failure {
    match verdict {
        StartupFailure::Refused(refusal) if failed.kind == FailureKind::CheckFailed => {
            failed.admission_refusal(refusal)
        }
        _ => failed,
    }
}

/// Lock, marker, record and probe I/O that met a refused item fails the check of the Session's
/// runtime folder, naming that item; any other is an internal runtime fault.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn runtime_io(verdict: StartupFailure) -> Failure {
    match verdict {
        StartupFailure::Refused(refusal) => Failure::new(FailureKind::CheckFailed)
            .details(stage_details("runtime-directory"))
            .admission_refusal(refusal),
        _ => Failure::new(FailureKind::InternalFailure).details(stage_details("activation")),
    }
}

/// The `recovery-required` refusal naming the registered process that still runs, worded as the
/// TUI words it.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn recovery_blocked(blocker: orchbee_local_runtime::RecoveryBlocker) -> Failure {
    let mut details = stage_reason_details("activation", "recovery-required");
    details.insert(
        "blockingProcess".to_owned(),
        super::blocking_process::member(blocker),
    );
    Failure::new(FailureKind::Rejected)
        .details(details)
        .startup_display(StartupFailure::RecoveryBlocked(blocker))
}

/// A Server the system refused to create: its own reason with what the system answered, worded as
/// the TUI words it. It keeps the activation refusal's kind and exit code.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn server_not_started(spawn: crate::local_activation::ServerSpawnFailure) -> Failure {
    let mut details = stage_reason_details("activation", "server-not-started");
    details.insert(
        "ioKind".to_owned(),
        json!(super::io_kind::IoKind::of(spawn.kind).token()),
    );
    details.insert("osError".to_owned(), json!(spawn.os_error));
    Failure::new(FailureKind::Rejected)
        .details(details)
        .startup_display(StartupFailure::ServerNotStarted(spawn))
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn classify_cohort(error: &orchbee_local_runtime::CohortError) -> Failure {
    let kind = match error.kind() {
        std::io::ErrorKind::InvalidData => FailureKind::Incompatible,
        std::io::ErrorKind::NotFound => FailureKind::ServiceUnavailable,
        std::io::ErrorKind::PermissionDenied => FailureKind::Denied,
        std::io::ErrorKind::TimedOut => FailureKind::Timeout,
        _ => FailureKind::InternalFailure,
    };
    Failure::new(kind)
        .details(stage_details("cohort"))
        .io_diagnostic(error)
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn timeout() -> Failure {
    Failure::new(FailureKind::Timeout).details(stage_details("activation"))
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn recovery_required() -> Failure {
    Failure::new(FailureKind::Rejected)
        .details(stage_reason_details("activation", "recovery-required"))
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn socket_budget() -> Failure {
    Failure::new(FailureKind::ServiceUnavailable).details(stage_reason_details(
        "runtime-directory",
        "address-too-long",
    ))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos", windows)))]
mod tests;
