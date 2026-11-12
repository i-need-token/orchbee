//! Agent startup in one place beside a reference Panel: a new Tab in its Workspace (the default),
//! a new Panel split from it, or that Panel's own prompt. A new Panel runs the Agent as its
//! shell's first command when the start is exactly the catalog command; otherwise one submission
//! is typed at the Panel's confirmed prompt. Without a name the start returns after the early
//! check, and with one it also composes a name reservation and canonical or observed readiness.
//! Every failure retains the independently known effects, including a Panel this invocation
//! created, which stays.

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
    invocation::{AgentStartCommand, AgentStartPlace, OutputFormat},
    localization::Locale,
};
use orchbee_agent_observation_protocol::AgentProviderId;
use orchbee_workbench_protocol::{Snapshot, WorkbenchPanelId};
use serde_json::json;
use std::ffi::OsString;
use std::time::{Duration, Instant};

mod place;
mod report;
use report::{Report, Verdict};

const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-start";
/// The pause before starting again on a new Panel whose shell is not ready yet.
const PROMPT_RETRY: Duration = Duration::from_millis(250);
/// How long a new Panel's prompt gate refusals, each of which typed nothing, are attempted again:
/// a shell that still runs its profile redraws its prompt for seconds. The picker's first-prompt
/// fallback waits as long.
const NEW_SHELL_SETTLE: Duration = Duration::from_secs(30);

#[cfg(test)]
mod tests;

/// Where this start ran: the requested place, the Panel the Agent was started in once known, and
/// whether this invocation created that Panel.
#[derive(Debug)]
struct Placed {
    place: AgentStartPlace,
    panel: Option<WorkbenchPanelId>,
    created: bool,
}

/// The reference Panel, and for `--here` with an explicit Panel, the caller's own Panel, which
/// `--here` may not type into.
struct Reference {
    target: PanelTarget,
    caller: Option<PanelTarget>,
}

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
    let mut placed = Placed {
        place: command.place,
        panel: None,
        created: false,
    };
    if !crate::agent_start::command_valid(&request) {
        return project(
            Report::preflight(
                request,
                started,
                failure(FailureKind::InvalidUsage, "command-too-long"),
            ),
            &placed,
            format,
            locale,
            None,
        );
    }
    let deadline = started + Duration::from_millis(u64::from(command.timeout_ms));
    let mut armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => {
            return project(
                Report::preflight(
                    request,
                    started,
                    failure(FailureKind::InternalFailure, "signal-registration-failed"),
                ),
                &placed,
                format,
                locale,
                None,
            );
        }
    };
    let mut report = match reference(command, |name| std::env::var_os(name)) {
        Ok(reference) => run_local(
            request,
            command,
            (started, deadline),
            &mut armed,
            &reference,
            &mut placed,
        ),
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
    project(report, &placed, format, locale, finalization)
}

/// Name the reference Panel from the arguments and caller context alone. `--here` without an
/// explicit Panel aims at the caller's own Panel, whose shell is running this command, so it is
/// refused here, before any runtime access.
fn reference(
    command: &AgentStartCommand,
    environment: impl Fn(&str) -> Option<OsString>,
) -> Result<Reference, Failure> {
    let target = PanelTarget::prepare(command.panel.as_ref(), &environment)?;
    let caller = match (command.place, &command.panel) {
        (AgentStartPlace::Here, None) => {
            return Err(failure(FailureKind::InvalidUsage, "own-panel"));
        }
        // Without readable caller metadata there is no own Panel to compare with.
        (AgentStartPlace::Here, Some(_)) => {
            PanelTarget::prepare_optional(&environment).ok().flatten()
        }
        _ => None,
    };
    Ok(Reference { target, caller })
}

fn run_local(
    request: Request,
    command: &AgentStartCommand,
    (started, deadline): (Instant, Instant),
    armed: &mut ArmedSignals,
    reference: &Reference,
    placed: &mut Placed,
) -> Report {
    let admitted = admit(deadline, armed.poll_fd()).and_then(|(client, snapshot)| {
        let panel = exact_panel(&reference.target, &snapshot)?;
        Ok((client, snapshot, panel))
    });
    let (mut client, mut snapshot, panel) = match settle(admitted, armed.poll_fd()) {
        Ok(admitted) => admitted,
        Err(error) => return Report::preflight(request, started, error),
    };
    if placed.place == AgentStartPlace::Here {
        let own = reference
            .caller
            .as_ref()
            .and_then(|caller| exact_panel(caller, &snapshot).ok());
        if own.as_ref() == Some(&panel) {
            return Report::preflight(
                request,
                started,
                failure(FailureKind::InvalidUsage, "own-panel"),
            );
        }
        placed.panel = Some(panel.clone());
        return crate::agent_start::run_on_client(
            request,
            started,
            deadline,
            armed.poll_fd(),
            client,
            snapshot,
            panel,
        )
        .into();
    }
    let selector = first_command(command);
    let created = place::create(
        &mut client,
        &mut snapshot,
        deadline,
        armed,
        (&reference.target, placed.place),
        selector,
    );
    drop(client);
    let panel = match created {
        Ok(panel) => panel,
        Err(refusal) => {
            let place::Refusal { failure, created } = *refusal;
            placed.created = created.is_some();
            placed.panel = created;
            return Report::place_failed(request, started, failure, place::operation(placed.place));
        }
    };
    placed.panel = Some(panel.clone());
    placed.created = true;
    let request = if selector.is_some() {
        request.started_as_command_unwatched()
    } else {
        request
    };
    start_in_new_panel(&request, (started, deadline), armed.poll_fd(), &panel)
}

/// The catalog Agent a new Panel's shell runs as its first command. Only a start that is exactly
/// the catalog command, with no arguments and no name, takes this form; every other start is
/// typed once at a confirmed prompt.
fn first_command(
    command: &AgentStartCommand,
) -> Option<orchbee_workbench_protocol::AgentStartSelector> {
    if command.place == AgentStartPlace::Here || command.name.is_some() || !command.args.is_empty()
    {
        return None;
    }
    orchbee_workbench_protocol::AgentStartSelector::parse(&command.provider)
}

/// Start on a Panel this invocation just created. Until its terminal is bound and its shell shows
/// an idle prompt, a start types nothing and is only read again; a submission the prompt gate
/// refused, which typed nothing, is the stale-base refusal and is attempted again under a fresh
/// identity until `NEW_SHELL_SETTLE` has passed since this call. Every attempt shares the original
/// deadline.
fn start_in_new_panel(
    request: &Request,
    (started, deadline): (Instant, Instant),
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    panel: &WorkbenchPanelId,
) -> Report {
    let settle_until = Instant::now() + NEW_SHELL_SETTLE;
    loop {
        let report = match settle(admit(deadline, wake), wake) {
            Ok((client, snapshot)) => crate::agent_start::run_on_client(
                request.clone(),
                started,
                deadline,
                wake,
                client,
                snapshot,
                panel.clone(),
            ),
            Err(error) => return Report::preflight(request.clone(), started, error),
        };
        let gate_refusal = report.submission.is_some();
        let retry = report.awaits_new_shell()
            && (!gate_refusal || Instant::now() < settle_until)
            && deadline.saturating_duration_since(Instant::now()) > PROMPT_RETRY * 8;
        // An interruption ends the wait here; signal finalization reports the cancellation.
        if !retry || crate::agent_runtime::wait_backoff(wake, PROMPT_RETRY) {
            return report.into();
        }
    }
}

/// Admit one Session and read its Snapshot under one step of the original deadline.
fn admit(
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<(crate::local_client::LocalClient, Snapshot), Failure> {
    crate::agent_start::step_deadline(deadline)
        .map_err(report::domain_failure)
        .and_then(|step| super::query::stream_snapshot(step, wake))
}

/// A failure while an interruption is pending reports the cancellation, unless the outcome is
/// unknown.
fn settle<T>(
    result: Result<T, Failure>,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<T, Failure> {
    match result {
        Err(error)
            if error.kind != FailureKind::OutcomeUnknown
                && crate::agent_runtime::wait_backoff(wake, Duration::from_millis(1)) =>
        {
            Err(failure(FailureKind::Cancelled, "cancelled"))
        }
        result => result,
    }
}

/// The exact Panel a prepared target names in `snapshot`.
fn exact_panel(target: &PanelTarget, snapshot: &Snapshot) -> Result<WorkbenchPanelId, Failure> {
    let target = target.resolve(snapshot)?;
    let super::results::Projection::PanelGet(panel) =
        super::results::project(&crate::invocation::QueryCommand::PanelGet(target), snapshot)?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    WorkbenchPanelId::parse(&panel.row.panel_id)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

fn failure(kind: FailureKind, reason: &str) -> Failure {
    Failure::new(kind).details(super::stage_reason_details("agent-start", reason))
}

fn project(
    mut report: Report,
    placed: &Placed,
    format: OutputFormat,
    locale: Locale,
    finalization: Option<super::SourceFinalization>,
) -> Output {
    // Every confirmed start names its Panel; the workflow runs only once that Panel is known.
    if matches!(report.verdict, Verdict::Started { .. }) && placed.panel.is_none() {
        report.replace_failure(Failure::new(FailureKind::InternalFailure));
    }
    let text = CliText::new(locale);
    let name = report.name.as_ref().map_or("", |name| name.as_str());
    let place = placed.place;
    // The workflow names the Panel it confirmed; a canonical Session confirms the Panel it ran in.
    let panel = match &report.verdict {
        Verdict::Observed { panel } | Verdict::Sent { panel } => Some(panel),
        Verdict::Started { .. } => placed.panel.as_ref(),
        Verdict::Failed { .. } => None,
    };
    // Text names the Panel by the handle a person types next; the document keeps its canonical ID.
    let handle = |panel: &WorkbenchPanelId| super::handles::panel(panel.as_str());
    let started = match (&report.verdict, panel) {
        (Verdict::Started { session }, Some(panel)) => Some((
            "session",
            Some(session),
            panel,
            text.cli_agent_started(name, session.as_str(), place, &handle(panel)),
        )),
        (Verdict::Observed { .. }, Some(panel)) => Some((
            "observed",
            None,
            panel,
            text.cli_agent_observed(name, place, &handle(panel)),
        )),
        // A new shell that ran the Agent as its first command had nothing typed into it.
        (Verdict::Sent { .. }, Some(panel)) => Some((
            if report.submission.is_none() {
                "launched"
            } else {
                "sent"
            },
            None,
            panel,
            text.cli_agent_sent(
                crate::agent_provider::product_name(report.provider.as_str()),
                place,
                &handle(panel),
            ),
        )),
        _ => None,
    };
    if let Some((confirmation, session, panel, rendered)) = started {
        // A start without a name ends when its line is typed or its shell is seen, so it promises
        // no ready Agent.
        let mut document = json!({"schema":"orchbee.agent.start", "version":1, "result":{
            "kind":"started", "confirmation":confirmation,
            "interactiveReady":!matches!(confirmation, "sent" | "launched"), "name":&report.name,
            "providerId":&report.provider, "reservationId":&report.reservation,
            "elapsedMs":report.elapsed_ms, "submission":&report.submission,
            "place":place.as_str(), "panelId":panel.as_str(),
        }});
        // Only the canonical Session holds the name; an observed start has none to report.
        if let Some(session) = session {
            document["result"]["sessionId"] = json!(session);
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
        ("place".into(), json!(placed.place.as_str())),
        // A Panel this invocation created stays when a later step fails.
        (
            "createdPanelId".into(),
            json!(
                placed
                    .panel
                    .as_ref()
                    .filter(|_| placed.created)
                    .map(WorkbenchPanelId::as_str)
            ),
        ),
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
