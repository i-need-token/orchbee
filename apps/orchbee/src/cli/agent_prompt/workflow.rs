//! Shared prompt delivery and subsequent sampled or journal-backed observation.
use crate::cli_text::CliText;
mod terminal;
use super::{AgentTarget, Locale, Output, OutputFormat, ResolvedWait, failure_output};
use crate::cli::command_interrupt::{ArmedSignals, Finish};
use crate::cli::wait_deadline::WaitDeadline;
use crate::cli::{Failure, FailureKind, output, query, send};
use crate::cli_outcome::WaitDisplay;
use crate::invocation::AgentWaitCondition;
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentObservationFreshness, AgentSemanticStatus, AgentSessionDetail,
};
use serde_json::{Map, Value, json};
use std::time::{Duration, Instant};

const READ_BUDGET: Duration = Duration::from_secs(5);
const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-prompt";
struct Sent {
    receipt: send::Receipt,
    completion: Completion,
}
enum Completion {
    Session {
        session_id: String,
        waited: Option<Result<Matched, Failure>>,
    },
    Terminal {
        binding: orchbee_agent_observation_protocol::AgentLaunchBinding,
        handle: Option<String>,
        // Boxed: a physical match carries its whole terminal read.
        waited: Option<Box<Result<crate::cli::results::terminal_agent::Matched, Failure>>>,
    },
}
impl Completion {
    fn waiting(&self) -> bool {
        match self {
            Self::Session { waited, .. } => waited.is_some(),
            Self::Terminal { waited, .. } => waited.is_some(),
        }
    }
}
struct Matched {
    observation: query::AgentObservationRead,
    conditions: Vec<AgentWaitCondition>,
    observations: u64,
    elapsed_ms: u64,
    timeout_ms: Option<u32>,
    client: Option<Box<crate::cli_outcome::AgentClientViewDoc>>,
    matched_change: Option<(
        orchbee_agent_observation_protocol::ObapRequestId,
        orchbee_agent_observation_protocol::AgentChange,
    )>,
}

pub(super) fn run(
    target: &AgentTarget,
    text: &str,
    wait: Option<&ResolvedWait>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if wait.is_some_and(|wait| wait.client.is_some())
        && let Err(failure) =
            crate::cli::execution::admit(crate::cli::execution::Capability::AgentProjection)
    {
        return failure_output(failure, Some(format), locale);
    }
    let start = Instant::now();
    let timeout_ms = wait.map_or(Some(5000), |wait| wait.timeout_ms);
    let total = match WaitDeadline::new(start, timeout_ms) {
        Ok(total) => total,
        Err(failure) => return failure_output(failure, Some(format), locale),
    };
    let mut armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    // span owns and releases the client before finish or any receipt projection.
    let attempt = span(target, text, wait, start, total, &mut armed);
    let (cancelled, fault, finalization) = match armed.finish() {
        Ok(Finish::Clear) => (false, false, None),
        Ok(Finish::Cancelled { finalization, .. }) => (true, false, Some(finalization)),
        Err(_) => (false, true, None),
    };
    let until = wait.map_or(&[][..], |wait| wait.until.as_slice());
    let mut output = settle(attempt, cancelled, fault, until, format, locale);
    output.finalization = finalization;
    output
}

fn settle(
    attempt: Result<Sent, Failure>,
    cancelled: bool,
    fault: bool,
    until: &[AgentWaitCondition],
    format: OutputFormat,
    locale: Locale,
) -> Output {
    match attempt {
        Err(failure) if failure.kind == FailureKind::OutcomeUnknown => {
            failure_output(failure, Some(format), locale)
        }
        Err(_) if cancelled => prompt_failure(
            prompt_error(FailureKind::Cancelled, "cancelled"),
            format,
            locale,
        ),
        Err(_) if fault => prompt_failure(
            prompt_error(FailureKind::InternalFailure, "signal-finalization-failed"),
            format,
            locale,
        ),
        Err(failure) => prompt_failure(failure, format, locale),
        Ok(sent) if !sent.receipt.accepted() => send::receipt_output(sent.receipt, format, locale),
        Ok(sent) => {
            let receipt = sent.receipt.document();
            let interrupted = if fault {
                Some(prompt_error(
                    FailureKind::InternalFailure,
                    "signal-finalization-failed",
                ))
            } else if cancelled && sent.completion.waiting() {
                Some(prompt_error(FailureKind::Cancelled, "cancelled"))
            } else {
                None
            };
            match sent.completion {
                Completion::Session { session_id, waited } => {
                    let outcome = interrupted.map_or_else(|| waited.transpose(), Err);
                    match outcome {
                        Ok(matched) => success(&session_id, receipt, matched, format, locale),
                        Err(mut failure) => {
                            failure.failure_schema = Some(FAILURE_SCHEMA);
                            let details = failure.details.get_or_insert_with(Map::new);
                            details.insert("sessionId".into(), json!(session_id));
                            details.insert("submission".into(), receipt.clone());
                            failure.source = Some(receipt["source"].clone());
                            prompt_failure(awaited_after_prompt(failure, until), format, locale)
                        }
                    }
                }
                Completion::Terminal {
                    binding,
                    handle,
                    waited,
                } => {
                    let outcome =
                        interrupted.map_or_else(|| waited.map(|waited| *waited).transpose(), Err);
                    match outcome {
                        Ok(matched) => {
                            terminal::success(binding, handle, receipt, matched, format, locale)
                        }
                        Err(failure) => terminal::failed_after_send(
                            failure, binding, receipt, until, format, locale,
                        ),
                    }
                }
            }
        }
    }
}

fn span(
    target: &AgentTarget,
    text: &str,
    wait: Option<&ResolvedWait>,
    start: Instant,
    total: WaitDeadline,
    signals: &mut ArmedSignals,
) -> Result<Sent, Failure> {
    let admission = total.step(Instant::now(), READ_BUDGET)?;
    let (mut client, _, snapshot) = query::snapshot_session_until(admission)?;
    let extended =
        crate::cli::execution::admit(crate::cli::execution::Capability::AgentProjection).is_ok();
    let selected = if extended {
        let resolved = query::named_terminal::resolve_named(
            &mut client,
            target,
            Some(&snapshot),
            admission,
            signals.poll_fd(),
        )?;
        match resolved.target {
            orchbee_agent_observation_protocol::AgentNameTarget::Session { session_id } => {
                query::resolve_session_terminal(
                    &mut client,
                    &snapshot,
                    &session_id,
                    resolved.presentation.as_deref(),
                    admission,
                    signals.poll_fd(),
                )?
            }
            orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding } => {
                return terminal::span(
                    &mut client,
                    terminal::Request {
                        target,
                        binding,
                        text,
                        wait,
                        start,
                        total,
                    },
                    signals,
                );
            }
        }
    } else {
        query::resolve_agent_terminal(&mut client, &snapshot, target, admission)?
    };
    let selected = if extended {
        // Refuse already-known blockers before collecting processes, then
        // check readiness again against the re-read pinned observation.
        check_ready(&selected.observation)?;
        query::verify_agent_terminal(&mut client, &snapshot, selected, admission)?
    } else {
        selected
    };
    check_ready(&selected.observation)?;
    if extended {
        let management = query::read_agent_management(
            &mut client,
            &selected.observation.session.snapshot.session_id,
            selected.observation.revision,
            admission,
            signals.poll_fd(),
        )?;
        if matches!(
            management.management,
            orchbee_agent_observation_protocol::AgentManagement::Pending { .. }
        ) {
            return Err(prompt_error(FailureKind::Rejected, "agent-launch-pending")
                .source(management.source));
        }
    }
    let mut journal = extended.then(query::AgentJournal::default);
    if let Some(journal) = journal.as_mut()
        && wait.is_some()
    {
        let (_, changes) = journal
            .read(&mut client, admission, signals.poll_fd())
            .map_err(|error| journal_failure(error, total, false, admission))?;
        if !changes.is_empty()
            || journal
                .observed_through()
                .is_none_or(|head| head.value() < selected.observation.revision.value())
        {
            return Err(prompt_error(
                FailureKind::ProtocolInvalid,
                "wait-read-failed",
            ));
        }
    }
    if let Some(client_id) = wait.and_then(|wait| wait.client.as_ref()) {
        crate::cli::agent_view::workflow::read(
            &mut client,
            &snapshot,
            client_id,
            &selected.observation.session.snapshot.session_id,
            admission.min(Instant::now() + Duration::from_secs(2)),
            signals,
        )
        .map_err(|failure| client_failure(failure, client_id, total))?;
    }
    let before = &selected.observation.session;
    let session_id = before.snapshot.session_id.clone();
    if crate::agent_runtime::wait_backoff(signals.poll_fd(), Duration::from_millis(1)) {
        return Err(prompt_error(FailureKind::InternalFailure, "interrupted"));
    }
    let codex_boundary = extended && before.snapshot.provider_id.as_str() == "codex";
    let receipt = send::submit_prompt_on_session(
        &mut client,
        &snapshot,
        &selected.terminal,
        text,
        codex_boundary,
        admission,
    )?;
    let waited = if receipt.accepted() {
        wait.map(|options| {
            observe(
                &mut client,
                before,
                options,
                start,
                total,
                signals,
                PromptObservation {
                    snapshot: &snapshot,
                    journal: journal.as_mut(),
                },
            )
        })
    } else {
        None
    };
    drop(client);
    Ok(Sent {
        receipt,
        completion: Completion::Session {
            session_id: session_id.as_str().to_owned(),
            waited,
        },
    })
}

fn check_ready(observation: &query::AgentObservationRead) -> Result<(), Failure> {
    let before = &observation.session;
    let blocked = !before.snapshot.pending_requests.is_empty()
        || matches!(before.status, AgentSemanticStatus::NeedsAttention { .. });
    if blocked {
        return Err(prompt_error(FailureKind::Rejected, "agent-blocked")
            .source(super::super::results::agent_source(&observation.request_id)));
    }
    if before.snapshot.observation.freshness != AgentObservationFreshness::Fresh
        || matches!(before.status, AgentSemanticStatus::Unknown { .. })
    {
        return Err(prompt_error(FailureKind::Rejected, "agent-not-ready")
            .source(super::super::results::agent_source(&observation.request_id)));
    }
    Ok(())
}

struct PromptObservation<'a> {
    snapshot: &'a orchbee_workbench_protocol::Snapshot,
    journal: Option<&'a mut query::AgentJournal>,
}

fn observe(
    client: &mut crate::local_client::LocalClient,
    before: &AgentSessionDetail,
    options: &ResolvedWait,
    start: Instant,
    total: WaitDeadline,
    signals: &mut ArmedSignals,
    evidence: PromptObservation<'_>,
) -> Result<Matched, Failure> {
    let PromptObservation {
        snapshot,
        mut journal,
    } = evidence;
    let effect = total.step(Instant::now(), READ_BUDGET)?;
    let mut active = matches!(before.status, AgentSemanticStatus::Working { .. });
    let mut observations = 0;
    let mut backoff = Duration::from_millis(50);
    loop {
        let now = Instant::now();
        let limit = if active {
            total.step(now, READ_BUDGET)?
        } else {
            effect
        };
        if total.expired(now) || (!active && now >= effect) {
            return Err(prompt_error(
                FailureKind::Timeout,
                if total.expired(Instant::now()) {
                    "wait-deadline"
                } else {
                    "prompt-stalled"
                },
            ));
        }
        let historical = if let Some(journal) = journal.as_mut() {
            let (request_id, changes) = journal
                .read(client, limit, signals.poll_fd())
                .map_err(|error| journal_failure(error, total, !active, effect))?;
            let mut matched = None;
            for change in changes {
                if change.session.session_id != before.snapshot.session_id {
                    continue;
                }
                let version = &change.session.observation;
                let old = &before.snapshot.observation;
                let newer = (version.generation.value(), version.sequence.value())
                    > (old.generation.value(), old.sequence.value());
                let gate_was_closed = !active;
                active |= version.freshness == AgentObservationFreshness::Fresh
                    && newer
                    && (matches!(
                        change.session.status,
                        AgentSemanticStatus::Working { .. }
                            | AgentSemanticStatus::NeedsAttention { .. }
                    ) || change.transition
                        == orchbee_agent_observation_protocol::AgentChangeTransition::TurnFinished);
                if gate_was_closed
                    && active
                    && matched.is_none()
                    && matches!(
                        change.session.status,
                        AgentSemanticStatus::Working { .. }
                            | AgentSemanticStatus::NeedsAttention { .. }
                    )
                {
                    let conditions: Vec<_> = options
                        .until
                        .iter()
                        .copied()
                        .filter(|condition| {
                            matches!(
                                condition,
                                AgentWaitCondition::Working
                                    | AgentWaitCondition::Blocked
                                    | AgentWaitCondition::NeedsAttention
                            ) && crate::cli::agent_wait::status_matches(
                                *condition,
                                version.freshness,
                                &change.session.status,
                            )
                        })
                        .collect();
                    if !conditions.is_empty() {
                        matched = Some((request_id.clone(), change, conditions));
                    }
                }
            }
            matched
        } else {
            None
        };
        let attention = if let Some(client_id) = &options.client {
            Some(
                crate::cli::agent_view::workflow::read(
                    client,
                    snapshot,
                    client_id,
                    &before.snapshot.session_id,
                    limit.min(Instant::now() + Duration::from_secs(2)),
                    signals,
                )
                .map_err(|failure| client_failure(failure, client_id, total))?,
            )
        } else {
            None
        };
        let result = query::read_agent_observation(
            client,
            &before.snapshot.session_id,
            limit,
            signals.poll_fd(),
        );
        let read = result.map_err(|failure| {
            let reason = if failure.kind == FailureKind::Timeout {
                if total.expired(Instant::now()) {
                    "wait-deadline"
                } else if !active && Instant::now() >= effect {
                    "prompt-stalled"
                } else {
                    "read-deadline"
                }
            } else {
                "wait-read-failed"
            };
            prompt_error(failure.kind, reason)
        })?;
        observations += 1;
        if journal.as_ref().is_some_and(|journal| {
            journal
                .observed_through()
                .is_some_and(|head| head.value() > read.revision.value())
        }) {
            return Err(prompt_error(
                FailureKind::ProtocolInvalid,
                "wait-read-failed",
            ));
        }
        let client_view = match (attention, options.client.as_ref()) {
            (Some(attention), Some(client_id)) => Some(Box::new(
                attention
                    .corroborate(client, snapshot, &read, limit, signals)
                    .map_err(|failure| client_failure(failure, client_id, total))?,
            )),
            _ => None,
        };
        // Never publish a match after the observation budget has elapsed.
        if Instant::now() >= limit {
            return Err(prompt_error(
                FailureKind::Timeout,
                if total.expired(Instant::now()) {
                    "wait-deadline"
                } else if !active {
                    "prompt-stalled"
                } else {
                    "read-deadline"
                },
            ));
        }
        if let Some((request_id, change, conditions)) = historical {
            return Ok(Matched {
                observation: read,
                conditions,
                observations,
                elapsed_ms: start.elapsed().as_millis() as u64,
                timeout_ms: options.timeout_ms,
                matched_change: Some((request_id, change)),
                client: client_view,
            });
        }
        active |= activity_after(before, &read.session);
        let conditions: Vec<_> = options
            .until
            .iter()
            .copied()
            .filter(|condition| {
                crate::cli::agent_wait::condition_matches(*condition, &read.session)
            })
            .collect();
        if active && !conditions.is_empty() {
            return Ok(Matched {
                observation: read,
                conditions,
                observations,
                elapsed_ms: start.elapsed().as_millis() as u64,
                timeout_ms: options.timeout_ms,
                matched_change: None,
                client: client_view,
            });
        }
        if read.session.snapshot.lifecycle == AgentLifecycle::Ended {
            return Err(prompt_error(FailureKind::NotFound, "session-ended"));
        }
        if journal.as_ref().is_some_and(|journal| journal.has_more()) {
            continue;
        }
        let remaining = limit.saturating_duration_since(Instant::now());
        if crate::agent_runtime::wait_backoff(signals.poll_fd(), backoff.min(remaining)) {
            return Err(prompt_error(FailureKind::InternalFailure, "interrupted"));
        }
        backoff = (backoff * 2).min(Duration::from_millis(500));
    }
}

fn client_failure(
    failure: Failure,
    client_id: &orchbee_workbench_protocol::WorkbenchClientId,
    total: WaitDeadline,
) -> Failure {
    let reason = match failure.kind {
        FailureKind::Timeout if total.expired(Instant::now()) => "wait-deadline",
        FailureKind::Timeout => "read-deadline",
        FailureKind::Cancelled => "cancelled",
        _ => "client-read-failed",
    };
    let mut failure = prompt_error(failure.kind, reason);
    if let Some(details) = &mut failure.details {
        details.insert("clientId".into(), json!(client_id));
    }
    failure
}

fn journal_failure(
    error: query::JournalError,
    total: WaitDeadline,
    awaiting_activity: bool,
    effect: Instant,
) -> Failure {
    let failure = match error {
        query::JournalError::Exchange(error) => {
            query::classify_agent_read(error, crate::local_client::AgentReadOperation::Changes)
        }
        query::JournalError::Failure(failure) => failure,
    };
    let reason = if failure.kind == FailureKind::Timeout {
        if total.expired(Instant::now()) {
            "wait-deadline"
        } else if awaiting_activity && Instant::now() >= effect {
            "prompt-stalled"
        } else {
            "read-deadline"
        }
    } else {
        match failure
            .details
            .as_ref()
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
        {
            Some("history-lost") => "history-lost",
            Some("cursor-invalid") => "cursor-invalid",
            _ => "wait-read-failed",
        }
    };
    prompt_error(failure.kind, reason)
}

fn activity_after(before: &AgentSessionDetail, now: &AgentSessionDetail) -> bool {
    let old = &before.snapshot.observation;
    let new = &now.snapshot.observation;
    now.snapshot.observation.freshness == AgentObservationFreshness::Fresh
        && (new.generation.value(), new.sequence.value())
            > (old.generation.value(), old.sequence.value())
        && (matches!(
            now.status,
            AgentSemanticStatus::Working { .. } | AgentSemanticStatus::NeedsAttention { .. }
        ) || now.snapshot.last_outcome.is_some()
            && now.snapshot.last_outcome != before.snapshot.last_outcome)
}

/// Name what the wait after a delivered prompt was waiting for when its own
/// deadline, its activity gate or an interrupt ended it. A read that outlived
/// its own budget is a request deadline and keeps the shared local-session
/// wording.
fn awaited_after_prompt(failure: Failure, until: &[AgentWaitCondition]) -> Failure {
    let reason = failure
        .details
        .as_ref()
        .and_then(|details| details.get("reason"))
        .and_then(Value::as_str);
    let step_budget_ms = match (failure.kind, reason) {
        (FailureKind::Timeout, Some("wait-deadline")) | (FailureKind::Cancelled, _) => None,
        (FailureKind::Timeout, Some("prompt-stalled" | "effect-deadline")) => {
            Some(u64::try_from(READ_BUDGET.as_millis()).unwrap_or(u64::MAX))
        }
        _ => return failure,
    };
    failure.wait_display(WaitDisplay {
        until: until.to_vec(),
        step_budget_ms,
    })
}

fn prompt_error(kind: FailureKind, reason: &'static str) -> Failure {
    Failure::new(kind)
        .failure_schema(FAILURE_SCHEMA)
        .details(Map::from_iter([
            ("stage".to_owned(), json!("agent-prompt")),
            ("reason".to_owned(), json!(reason)),
        ]))
}

fn prompt_failure(failure: Failure, format: OutputFormat, locale: Locale) -> Output {
    let hint = failure
        .details
        .as_ref()
        .and_then(|details| details.get("reason"))
        .and_then(Value::as_str)
        .and_then(|reason| CliText::new(locale).cli_prompt_hint(reason));
    let mut output = failure_output(failure, Some(format), locale);
    if let Some(hint) = hint {
        output.stderr.push_str(hint);
        output.stderr.push('\n');
    }
    output
}

fn success(
    session_id: &str,
    receipt: Value,
    matched: Option<Matched>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let matched_change = matched.as_ref().and_then(|matched| matched.matched_change.as_ref()).map(|(source,change)| {
        json!({"source":super::super::results::agent_source(source),"change":change})
    });
    let client_view = matched
        .as_ref()
        .and_then(|matched| matched.client.as_ref())
        .map(|client| client.document());
    let (observation, wait) = match matched {
        None => (Value::Null, Value::Null),
        Some(matched) => (
            json!({"source":super::super::results::agent_source(&matched.observation.request_id), "revision":matched.observation.revision, "session":matched.observation.session}),
            json!({"matchedConditions":matched.conditions.iter().map(|condition| condition.as_str()).collect::<Vec<_>>(), "timeoutMs":matched.timeout_ms, "observations":matched.observations, "elapsedMs":matched.elapsed_ms, "activityObserved":true}),
        ),
    };
    let extended =
        crate::cli::execution::admit(crate::cli::execution::Capability::AgentProjection).is_ok();
    let matched_change = if extended {
        json!(matched_change)
    } else {
        json!({"kind":"unsupported"})
    };
    let client_view = if extended {
        client_view.unwrap_or(Value::Null)
    } else {
        json!({"kind":"unsupported"})
    };
    let document = json!({"schema":"orchbee.agent.prompt", "version":1, "source":receipt["source"], "result":{"sessionId":session_id, "submission":receipt, "observation":observation, "wait":wait,"matchedChange":matched_change,"client":client_view}});
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => output::encode_text(&CliText::new(locale).cli_prompt_submitted(
            &format!("{session_id:?}"),
            !document["result"]["wait"].is_null(),
        )),
    };
    match encoded {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge).source(document["source"].clone()),
            Some(format),
            locale,
        ),
    }
}
#[cfg(test)]
mod tests;
