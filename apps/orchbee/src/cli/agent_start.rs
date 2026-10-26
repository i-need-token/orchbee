//! Agent startup composes a name reservation, one terminal submission and
//! canonical readiness. Every failure retains the independently known effects.

use super::Failure;
use super::output;
use super::{FailureKind, Output, failure_output};
use crate::{
    agent_start::{Report, Request, Verdict},
    cli::caller_context::PanelTarget,
};
use crate::{
    invocation::{AgentStartCommand, OutputFormat},
    localization::Locale,
};
use orchbee_agent_observation_protocol::AgentProviderId;
use serde_json::json;
use std::time::{Duration, Instant};
#[cfg(windows)]
#[path = "agent_start/signals_windows.rs"]
mod signals;
#[cfg(not(windows))]
#[path = "agent_start/signals_unix.rs"]
mod signals;

const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-start";

#[cfg(test)]
mod tests;

pub(crate) fn run(command: &AgentStartCommand, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = crate::agent_runtime::startup::admit() {
        return failure_output(failure, Some(format), locale);
    }
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
    let deadline = started + Duration::from_millis(u64::from(command.timeout_ms));
    if let Some(failure) = integration_preflight(
        request.provider.as_str(),
        deadline.min(Instant::now() + PREFLIGHT_BUDGET),
    ) {
        return project(
            Report::preflight(request, started, failure),
            format,
            locale,
            None,
        );
    }
    let result = signals::run(request, |request, wake| {
        match PanelTarget::prepare(command.panel.as_ref(), |name| std::env::var_os(name)) {
            Ok(prepared) => {
                crate::agent_start::run_local(request, started, deadline, wake, |snapshot| {
                    prepared.resolve(snapshot)
                })
            }
            Err(failure) => Report::preflight(request, started, failure),
        }
    });
    let (mut report, finish) = match result {
        Ok(result) => result,
        Err((request, failure)) => {
            return project(
                Report::preflight(request, started, failure),
                format,
                locale,
                None,
            );
        }
    };
    let finalization = match finish {
        Ok(None) => None,
        Ok(Some(finalization)) => {
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

/// The local inspection never consumes more than this share of the start
/// budget; an inspection it cuts short is inconclusive, not a refusal.
const PREFLIGHT_BUDGET: Duration = Duration::from_secs(10);

/// Refuse before any reservation or input when this machine's managed
/// integration cannot report readiness for the provider: its executable is
/// absent, its hooks are not enabled, or the adapter capsule is not published.
/// An unmanaged provider, a manual-only platform or an inconclusive inspection
/// leaves the decision to canonical readiness. Nothing here enables anything:
/// enabling edits the provider's own configuration.
fn integration_preflight(provider: &str, ceiling: Instant) -> Option<Failure> {
    use crate::agent_integrations::{
        IntegrationId, State,
        model::{Availability, ManagementMode},
    };
    let id = IntegrationId::parse(crate::agent_provider::integration_id(provider)?)?;
    let context = crate::agent_integrations::context_before(None, None, ceiling).ok()?;
    let (items, capsule) = crate::agent_integrations::list(&context, Some(id));
    let item = items.first()?;
    if item.timed_out || item.management_mode != ManagementMode::ManagedLocal {
        return None;
    }
    let reason = match (item.availability, item.state) {
        (Availability::NotFound, _) => "provider-not-found",
        (_, State::Conflict) => "integration-blocked",
        (_, State::NotInstalled | State::Disabled | State::UpdateRequired) => {
            "integration-not-enabled"
        }
        (_, State::Enabled) if !capsule.ready => "integration-not-enabled",
        _ => return None,
    };
    Some(crate::agent_start::failure(
        FailureKind::CheckFailed,
        reason,
    ))
}

fn project(
    mut report: Report,
    format: OutputFormat,
    locale: Locale,
    finalization: Option<super::SourceFinalization>,
) -> Output {
    if let Verdict::Started { session } = &report.verdict {
        let document = json!({"schema":"orchbee.agent.start", "version":1, "result":{
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
