//! Agent startup composes a name reservation, one terminal submission and
//! canonical readiness. Every failure retains the independently known effects.

use super::Failure;
use super::output;
use super::{FailureKind, Output, failure_output};
use crate::cli_text::CliText;
use crate::{
    agent_start::Request,
    cli::{
        caller_context::PanelTarget,
        console_interrupt::{ArmedSignals, Finish},
    },
};
use crate::{
    invocation::{AgentStartCommand, OutputFormat},
    localization::Locale,
};
use orchbee_agent_observation_protocol::AgentProviderId;
use serde_json::json;
use std::time::{Duration, Instant};

mod report;
use report::{Report, Verdict};

const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-start";

#[cfg(test)]
mod tests;

pub(crate) fn run(command: &AgentStartCommand, format: OutputFormat, locale: Locale) -> Output {
    if crate::agent_runtime::startup::admit().is_err() {
        return failure_output(
            report::domain_failure(crate::agent_start::Failure::Unsupported),
            Some(format),
            locale,
        );
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
                failure(FailureKind::InvalidUsage, "command-too-long"),
            ),
            format,
            locale,
            None,
        );
    }
    let deadline = started + Duration::from_millis(u64::from(command.timeout_ms));
    let armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => {
            return project(
                Report::preflight(
                    request,
                    started,
                    failure(FailureKind::InternalFailure, "signal-registration-failed"),
                ),
                format,
                locale,
                None,
            );
        }
    };
    let mut report =
        match PanelTarget::prepare(command.panel.as_ref(), |name| std::env::var_os(name)) {
            Ok(prepared) => run_local(request, started, deadline, armed.poll_fd(), prepared),
            Err(failure) => Report::preflight(request, started, failure),
        };
    let finalization = match armed.finish() {
        Ok(Finish::Clear) => None,
        Ok(Finish::Cancelled { finalization }) => {
            if !report.completed() && report.failure_kind() != Some(FailureKind::OutcomeUnknown) {
                report.replace_failure(failure(FailureKind::Cancelled, "cancelled"));
            }
            Some(finalization)
        }
        Err(_) => {
            if !report.completed() && report.failure_kind() != Some(FailureKind::OutcomeUnknown) {
                report.replace_failure(failure(
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

fn run_local(
    request: Request,
    started: Instant,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    prepared: PanelTarget,
) -> Report {
    let admitted = crate::agent_start::step_deadline(deadline)
        .map_err(report::domain_failure)
        .and_then(|step| super::query::stream_snapshot(step, wake))
        .and_then(|(mut client, snapshot)| {
            if let Some(failure) = integration_preflight(
                &mut client,
                &snapshot.session_id,
                request.provider.as_str(),
                deadline,
                wake,
            ) {
                return Err(failure);
            }
            let target = prepared.resolve(&snapshot)?;
            let super::results::Projection::PanelGet(panel) = super::results::project(
                &crate::invocation::QueryCommand::PanelGet(target),
                &snapshot,
            )?
            else {
                return Err(Failure::new(FailureKind::InternalFailure));
            };
            let panel = orchbee_workbench_protocol::WorkbenchPanelId::parse(&panel.row.panel_id)
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
            Ok((client, snapshot, panel))
        });
    let admitted = match admitted {
        Err(error)
            if error.kind != FailureKind::OutcomeUnknown
                && crate::agent_runtime::wait_backoff(wake, Duration::from_millis(1)) =>
        {
            Err(failure(FailureKind::Cancelled, "cancelled"))
        }
        result => result,
    };
    match admitted {
        Ok((client, snapshot, panel)) => crate::agent_start::run_on_client(
            request, started, deadline, wake, client, snapshot, panel,
        )
        .into(),
        Err(error) => Report::preflight(request, started, error),
    }
}

fn failure(kind: FailureKind, reason: &str) -> Failure {
    Failure::new(kind).details(super::stage_reason_details("agent-start", reason))
}

/// The Worker inspection never consumes more than this share of the start
/// budget; an inspection it cuts short is inconclusive, not a refusal.
const PREFLIGHT_BUDGET: Duration = Duration::from_secs(10);

/// Refuse before any reservation or input when the Session's Worker cannot report readiness
/// of its managed integration for the provider: its executable is absent, its hooks are not
/// enabled, or the adapter capsule is not published. An unmanaged provider, a manual-only
/// platform or an inconclusive, interrupted or failed inspection leaves the decision to canonical
/// readiness. Nothing here enables anything: enabling edits the provider's own configuration.
fn integration_preflight(
    client: &mut crate::local_client::LocalClient,
    session_id: &orchbee_workbench_protocol::SessionId,
    provider: &str,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Option<Failure> {
    use crate::agent_integrations::{
        IntegrationId, IntegrationOperation, IntegrationReport, IntegrationRequest, State,
        model::{Availability, ManagementMode},
    };
    use crate::local_client::agent_integration::AgentIntegrationJobEnd;
    let id = IntegrationId::parse(crate::agent_provider::integration_id(provider)?)?;
    let budget = deadline
        .saturating_duration_since(Instant::now())
        .min(PREFLIGHT_BUDGET);
    let request = IntegrationRequest {
        operation: IntegrationOperation::List { provider: Some(id) },
        adapter_source: None,
        budget_milliseconds: Some(
            u32::try_from(budget.as_millis())
                .ok()
                .filter(|ms| *ms > 0)?,
        ),
    };
    let end = client.agent_integration_job(
        session_id,
        request,
        deadline.min(Instant::now() + budget + PREFLIGHT_SETTLEMENT),
        || super::query::request_id().ok(),
        |pause| crate::agent_runtime::wait_backoff(wake, pause),
    );
    let AgentIntegrationJobEnd::Completed {
        report,
        cancel_registered: false,
    } = end
    else {
        return None;
    };
    let IntegrationReport::List { items, capsule } = *report else {
        return None;
    };
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
    Some(failure(FailureKind::CheckFailed, reason))
}

/// Beyond its own budget the preflight waits only this long for the job's final report.
const PREFLIGHT_SETTLEMENT: Duration = Duration::from_secs(1);

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
                &CliText::new(locale).cli_agent_started(report.name.as_str(), session.as_str()),
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
        report.replace_failure(failure(FailureKind::OutputTooLarge, "output-too-large"));
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
