//! Agent startup composes a name reservation, one terminal submission and
//! canonical readiness. Every failure retains the independently known effects.

#[cfg(any(not(windows), test))]
use super::Failure;
#[cfg(windows)]
use super::output;
use super::{FailureKind, Output, failure_output};
#[cfg(windows)]
use crate::{
    agent_start::{Report, Request, Verdict},
    cli::{
        caller_context::PanelTarget,
        source::{ArmedSignals, Finish, SignalWatch},
    },
};
use crate::{
    invocation::{AgentStartCommand, OutputFormat},
    localization::Locale,
};
#[cfg(windows)]
use orchbee_agent_observation_protocol::AgentProviderId;
#[cfg(windows)]
use serde_json::json;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-start";

#[cfg(all(test, windows))]
mod tests;

pub(crate) fn run(command: &AgentStartCommand, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        run_windows(command, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
                "platform",
                "unsupported-platform",
            )),
            Some(format),
            locale,
        )
    }
}

#[cfg(windows)]
fn run_windows(command: &AgentStartCommand, format: OutputFormat, locale: Locale) -> Output {
    let started = Instant::now();
    let provider = AgentProviderId::parse(&command.provider)
        .expect("agent start parser admits only canonical providers");
    let request = Request::new(command.name.clone(), provider, command.args.clone());
    if !crate::agent_start::command_valid(&request) {
        return project(
            Report::preflight(
                request,
                started,
                crate::agent_start::failure(FailureKind::InvalidUsage, "command-too-long"),
            ),
            format,
            locale,
            None,
        );
    }
    let armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => {
            return project(
                Report::preflight(
                    request,
                    started,
                    crate::agent_start::failure(
                        FailureKind::InternalFailure,
                        "signal-registration-failed",
                    ),
                ),
                format,
                locale,
                None,
            );
        }
    };
    let deadline = started + Duration::from_millis(u64::from(command.timeout_ms));
    let mut report = match PanelTarget::prepare(command.panel.as_ref(), |name| {
        std::env::var_os(name)
    }) {
        Ok(prepared) => {
            crate::agent_start::run_local(request, started, deadline, armed.poll_fd(), |snapshot| {
                prepared.resolve(snapshot)
            })
        }
        Err(failure) => Report::preflight(request, started, failure),
    };
    let finalization = match armed.finish() {
        Ok(Finish::Clear) => None,
        Ok(Finish::Cancelled { finalization, .. }) => {
            if !report.completed() && report.failure_kind() != Some(FailureKind::OutcomeUnknown) {
                report.replace_failure(crate::agent_start::failure(
                    FailureKind::Cancelled,
                    "cancelled",
                ));
            }
            Some(finalization)
        }
        Err(_) => {
            if !report.completed() && report.failure_kind() != Some(FailureKind::OutcomeUnknown) {
                report.replace_failure(crate::agent_start::failure(
                    FailureKind::InternalFailure,
                    "signal-finalization-failed",
                ));
            }
            None
        }
    };
    // The CLI's elapsed fact includes its process-wide signal arbitration,
    // matching the former direct owner. Native surfaces have no such adapter
    // boundary and keep the workflow's own settlement time.
    report.elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    project(report, format, locale, finalization)
}

#[cfg(windows)]
fn project(
    mut report: Report,
    format: OutputFormat,
    locale: Locale,
    finalization: Option<super::SourceFinalization>,
) -> Output {
    if let Verdict::Started { session } = &report.verdict {
        let document = json!({"schema":"orchbee.agent.start", "version":2, "result":{
            "kind":"started", "interactiveReady":true, "name":&report.name, "providerId":&report.provider,
            "sessionId":session, "reservationId":&report.reservation, "elapsedMs":report.elapsed_ms, "submission":&report.submission,
        }});
        let bytes = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => output::encode_text(
                &locale.cli_agent_started(report.name.as_str(), session.as_str()),
            ),
        };
        if let Ok(stdout) = bytes {
            return Output {
                status: 0,
                stdout,
                stderr: String::new(),
                finalization,
            };
        }
        report.replace_failure(crate::agent_start::failure(
            FailureKind::OutputTooLarge,
            "output-too-large",
        ));
    }
    let Verdict::Failed {
        mut failure,
        reason,
        cause,
    } = report.verdict
    else {
        unreachable!("started result returned above")
    };
    failure.details = Some(serde_json::Map::from_iter([
        ("stage".into(), json!("agent-start")),
        ("reason".into(), json!(reason)),
        ("name".into(), json!(report.name)),
        ("providerId".into(), json!(report.provider)),
        ("reservationId".into(), json!(report.reservation)),
        (
            "reservationState".into(),
            json!(report.reservation_state.as_str()),
        ),
        (
            "lastObservedReservation".into(),
            json!(report.last_observed),
        ),
        ("submission".into(), json!(report.submission)),
        ("elapsedMs".into(), json!(report.elapsed_ms)),
        ("cause".into(), json!(cause)),
    ]));
    failure.source = None;
    failure.version = 1;
    failure.failure_schema = Some(FAILURE_SCHEMA);
    let submission_status = (reason == "input-not-accepted")
        .then_some(report.submission_status)
        .flatten();
    let mut answer = failure_output(failure, Some(format), locale);
    if let Some(status) = submission_status {
        answer.status = status;
    }
    answer.finalization = finalization;
    answer
}
