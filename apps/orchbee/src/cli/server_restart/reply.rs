//! Restart outcomes render as the closed `orchbee.server.restart` document or its text projection;
//! a Session too old to hand its terminals over answers with the shared capability failure.
use super::{FailedPhase, Refusal, Restart};
use crate::cli::{Failure, FailureKind, Output, failure_output, output, stage_reason_details};
use crate::cli_text::CliText;
use crate::{invocation::OutputFormat, localization::Locale};
use serde_json::json;

/// The closed token of one refusal and the exit status it answers with: nothing is running, which
/// exits 4, and every other refusal exits 3.
fn refusal_token(refusal: Refusal) -> (&'static str, u8) {
    match refusal {
        Refusal::NotRunning => ("not-running", 4),
        Refusal::Busy => ("busy", 3),
        Refusal::BackgroundPrograms => ("background-programs", 3),
        Refusal::Clients => ("clients", 3),
        Refusal::WorkerUnavailable => ("worker-unavailable", 3),
        Refusal::Host => ("host", 3),
        Refusal::Records => ("records", 3),
    }
}

fn phase_token(phase: FailedPhase) -> &'static str {
    match phase {
        FailedPhase::Start => "start",
        FailedPhase::Check => "check",
        FailedPhase::Takeover => "takeover",
        FailedPhase::Open => "open",
    }
}

/// Renders one restart answer. `interrupted` says Ctrl+C ended the wait: an in-progress result
/// becomes `cancelled`, and a definitive result stands.
pub(super) fn render(
    restart: Restart,
    interrupted: bool,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let (outcome, reason, phase, status) = match restart {
        Restart::Restarted => ("restarted", None, None, 0),
        Restart::Refused(refusal) => {
            let (token, status) = refusal_token(refusal);
            ("refused", Some(token), None, status)
        }
        Restart::SessionTooOld => return session_too_old(format, locale),
        Restart::Failed(failed) => ("failed", None, Some(phase_token(failed)), 9),
        Restart::TimedOut if interrupted => ("cancelled", None, None, 130),
        Restart::TimedOut => ("timed-out", None, None, 7),
    };
    let encoded = match format {
        OutputFormat::Json => {
            let mut result = json!({ "outcome": outcome });
            if let Some(reason) = reason {
                result["reason"] = json!(reason);
            }
            if let Some(phase) = phase {
                result["phase"] = json!(phase);
            }
            output::encode_document(&json!({
                "schema": "orchbee.server.restart",
                "version": 1,
                "source": crate::cli_outcome::local_runtime_source(),
                "result": result,
            }))
        }
        OutputFormat::Text => output::encode_text(
            &CliText::new(locale).cli_server_restart_result(outcome, reason, phase),
        ),
    };
    match encoded {
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
    }
}

/// A Session whose Server cannot hand its terminals over is the shared capability refusal, exit 6.
/// The shared incompatible copy describes an admission refusal, so standard error carries the
/// restart's own explanation instead.
fn session_too_old(format: OutputFormat, locale: Locale) -> Output {
    let failure = Failure::new(FailureKind::Incompatible)
        .details(stage_reason_details("capability", "session-too-old"));
    let mut answer = failure_output(failure, Some(format), locale);
    answer.stderr = CliText::new(locale)
        .cli_server_restart_session_too_old()
        .to_owned();
    answer
}
