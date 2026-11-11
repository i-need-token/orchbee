//! Agent startup types one terminal submission; without a name it returns after the early check,
//! and with one it also composes a name reservation and canonical or observed readiness. Every
//! failure retains the independently known effects.

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
        .and_then(|(client, snapshot)| {
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

fn project(
    mut report: Report,
    format: OutputFormat,
    locale: Locale,
    finalization: Option<super::SourceFinalization>,
) -> Output {
    let text = CliText::new(locale);
    let name = report.name.as_ref().map_or("", |name| name.as_str());
    let started = match &report.verdict {
        Verdict::Started { session } => Some((
            "session",
            Some(session),
            None,
            text.cli_agent_started(name, session.as_str()),
        )),
        Verdict::Observed { panel } => Some((
            "observed",
            None,
            None,
            text.cli_agent_observed(name, panel.as_str()),
        )),
        Verdict::Sent { panel } => Some((
            "sent",
            None,
            Some(panel),
            text.cli_agent_sent(
                crate::agent_provider::product_name(report.provider.as_str()),
                panel.as_str(),
            ),
        )),
        Verdict::Failed { .. } => None,
    };
    if let Some((confirmation, session, panel, rendered)) = started {
        // A start without a name ends when its line is typed, so it promises no ready Agent.
        let mut document = json!({"schema":"orchbee.agent.start", "version":1, "result":{
            "kind":"started", "confirmation":confirmation,
            "interactiveReady":confirmation != "sent", "name":&report.name, "providerId":&report.provider,
            "reservationId":&report.reservation, "elapsedMs":report.elapsed_ms, "submission":&report.submission,
        }});
        // Only the canonical Session holds the name; an observed start has none to report.
        if let Some(session) = session {
            document["result"]["sessionId"] = json!(session);
        }
        if let Some(panel) = panel {
            document["result"]["panelId"] = json!(panel.as_str());
        }
        let bytes = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => output::encode_text(&rendered),
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
    let prompt_state = failure
        .details
        .as_ref()
        .and_then(|details| details.get(report::PROMPT_STATE))
        .cloned();
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
    if let (Some(details), Some(state)) = (failure.details.as_mut(), prompt_state) {
        details.insert(report::PROMPT_STATE.into(), state);
    }
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
