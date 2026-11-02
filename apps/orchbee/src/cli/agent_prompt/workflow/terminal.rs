//! Submit once to a verified physical Agent; preserve delivery when later observation fails.
use super::{
    AgentTarget, ArmedSignals, Completion, Failure, FailureKind, Instant, Locale, Map, Output,
    OutputFormat, READ_BUDGET, ResolvedWait, Sent, Value, WaitDeadline, json, output, query, send,
};
use crate::cli::agent_wait::terminal as terminal_agent_wait;
use crate::cli::query::terminal_agent::failure as error;
use crate::cli::results::terminal_agent::{Matched, TerminalAgentRead};
use crate::cli_text::CliText;
use crate::invocation::Target;
use crate::local_client::LocalClient;
use orchbee_agent_observation_protocol::{AgentLaunchBinding, AgentReportProvider};
use orchbee_workbench_protocol::PanelAgentState;

pub(super) struct Request<'a> {
    pub target: &'a AgentTarget,
    pub binding: AgentLaunchBinding,
    pub text: &'a str,
    pub wait: Option<&'a ResolvedWait>,
    pub start: Instant,
    pub total: WaitDeadline,
}

pub(super) fn span(
    client: &mut LocalClient,
    request: Request<'_>,
    signals: &mut ArmedSignals,
) -> Result<Sent, Failure> {
    let Request {
        target,
        binding,
        text,
        wait,
        start,
        total,
    } = request;
    let deadline = total.step(Instant::now(), READ_BUDGET)?;
    let selected = wait.and_then(|options| options.client.clone());
    if let Some(options) = wait {
        terminal_agent_wait::admit(&options.until)?;
    }
    let (snapshot, before) =
        terminal_agent_wait::read_pinned(client, target, &binding, None, deadline, signals)?;
    let expected = provider(&before)?;
    if let Some(selected) = selected.as_ref() {
        // Selected-Client admission fails before input, even when only later observation needs it.
        crate::cli::agent_view::workflow::terminal_attention::read(
            client, &snapshot, selected, &binding, deadline, signals,
        )?;
    }
    let row = query::named_terminal::terminal_row(&snapshot, &binding)?;
    let terminal = Target::Id(row.terminal_id);
    let (_, sample) = query::read_terminal_processes_cancellable(
        client,
        deadline,
        &snapshot,
        &terminal,
        signals.poll_fd(),
    )?;
    let admitted = sample
        .input_provider()
        .map_err(|reason| error(FailureKind::Rejected, reason))?;
    if admitted != expected {
        return Err(error(FailureKind::Rejected, "agent-provider-mismatch"));
    }
    let (snapshot, before) = terminal_agent_wait::read_pinned(
        client,
        target,
        &binding,
        Some(before.observation.sequence),
        deadline,
        signals,
    )?;
    if provider(&before)? != expected
        || before.observation.changes.iter().any(|change| {
            change.agent.as_deref().is_some_and(|agent| {
                AgentReportProvider::lookup(agent)
                    .is_none_or(|p| p.native_provider_id() != expected)
            })
        })
    {
        return Err(error(FailureKind::NotFound, "agent-identity-changed"));
    }
    let row = query::named_terminal::terminal_row(&snapshot, &binding)?;
    terminal_agent_wait::interrupt(signals)?;
    if Instant::now() >= deadline {
        return Err(error(FailureKind::Timeout, "read-deadline"));
    }
    let receipt = send::submit_prompt_on_session(
        client,
        &snapshot,
        &Target::Id(row.terminal_id),
        text,
        expected == "codex",
        deadline,
    )?;
    let waited = if receipt.accepted() {
        wait.map(|options| {
            terminal_agent_wait::observe(
                client,
                terminal_agent_wait::Options {
                    selected: selected.as_ref(),
                    target,
                    binding: &binding,
                    until: &options.until,
                    after_revision: None,
                    timeout_ms: options.timeout_ms,
                    start,
                    total,
                },
                Some(before),
                signals,
            )
        })
    } else {
        None
    };
    Ok(Sent {
        receipt,
        completion: Completion::Terminal { binding, waited },
    })
}

fn provider(read: &TerminalAgentRead) -> Result<&'static str, Failure> {
    let view = &read.observation.view;
    if view.state == PanelAgentState::Blocked {
        return Err(error(FailureKind::Rejected, "agent-blocked"));
    }
    view.agent
        .as_deref()
        .and_then(AgentReportProvider::lookup)
        .map(AgentReportProvider::native_provider_id)
        .ok_or_else(|| error(FailureKind::Rejected, "agent-provider-unsupported"))
}

pub(super) fn failed_after_send(
    failure: Failure,
    binding: AgentLaunchBinding,
    receipt: Value,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let details = failure.details.as_ref();
    let own_reason = details
        .and_then(|d| d.get("stage"))
        .and_then(Value::as_str)
        .is_some_and(|stage| matches!(stage, "agent-terminal" | "agent-prompt"));
    let reason = if own_reason {
        details
            .and_then(|d| d.get("reason"))
            .cloned()
            .unwrap_or(json!("wait-read-failed"))
    } else {
        json!("wait-read-failed")
    };
    let failure = Failure::new(failure.kind)
        .failure_schema(super::FAILURE_SCHEMA)
        .source(receipt["source"].clone())
        .details(Map::from_iter([
            ("stage".into(), json!("agent-prompt")),
            ("reason".into(), reason),
            (
                "target".into(),
                json!({"kind":"terminal","binding":binding}),
            ),
            ("submission".into(), receipt),
        ]));
    super::prompt_failure(failure, format, locale)
}

pub(super) fn success(
    binding: AgentLaunchBinding,
    receipt: Value,
    matched: Option<Matched>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let document = json!({"schema":"orchbee.agent.prompt","version":1,"source":receipt["source"],
        "result":{"target":{"kind":"terminal","binding":binding},"submission":receipt,
            "wait":matched.as_ref().map(Matched::document)}});
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => output::encode_text(
            &CliText::new(locale)
                .cli_terminal_prompt_submitted(binding.presentation_id.as_str(), matched.is_some()),
        ),
    };
    match encoded {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failed_after_send(
            error(FailureKind::OutputTooLarge, "output-too-large"),
            binding,
            document["result"]["submission"].clone(),
            format,
            locale,
        ),
    }
}

#[cfg(test)]
mod tests;
