//! Prompt composition retains delivery evidence independently of observation.
//! The shared terminal send owner alone constructs input and delivery receipts.

use super::{Failure, FailureKind, Output, failure_output};
use crate::invocation::AgentTarget;
use crate::invocation::{AgentPromptWait, OutputFormat};
use crate::localization::Locale;

#[cfg(any(target_os = "linux", windows))]
struct ResolvedWait {
    client: Option<orchbee_workbench_protocol::WorkbenchClientId>,
    until: Vec<crate::invocation::AgentWaitCondition>,
    timeout_ms: Option<u32>,
}

pub(super) fn run(
    target: &AgentTarget,
    text: &str,
    wait: Option<&AgentPromptWait>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    #[cfg(any(target_os = "linux", windows))]
    {
        let wait = wait.map(|request| ResolvedWait {
            client: request.client.clone(),
            until: super::agent_request::wait_conditions(request.until.as_deref()).to_vec(),
            timeout_ms: super::agent_request::wait_timeout(request.timeout_ms),
        });
        active::run(target, text, wait.as_ref(), format, locale)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (target, text, wait);
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

#[cfg(any(target_os = "linux", windows))]
mod active {
    #[cfg(windows)]
    mod terminal;
    use super::{
        AgentTarget, Failure, FailureKind, Locale, Output, OutputFormat, ResolvedWait,
        failure_output,
    };
    use crate::cli::source::{ArmedSignals, Finish, SignalWatch};
    use crate::cli::wait_deadline::WaitDeadline;
    use crate::cli::{output, query, send};
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
        #[cfg(windows)]
        Terminal {
            binding: orchbee_agent_observation_protocol::AgentLaunchBinding,
            waited: Option<Result<query::terminal_agent_wait::Matched, Failure>>,
        },
    }
    impl Completion {
        fn waiting(&self) -> bool {
            match self {
                Self::Session { waited, .. } => waited.is_some(),
                #[cfg(windows)]
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
        #[cfg(windows)]
        client: Option<Box<crate::cli::results::AgentClientViewDoc>>,
        #[cfg(windows)]
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
        #[cfg(not(windows))]
        if wait.is_some_and(|wait| wait.client.is_some()) {
            return failure_output(
                Failure::new(FailureKind::ServiceUnavailable).details(
                    crate::cli::stage_reason_details("platform", "unsupported-platform"),
                ),
                Some(format),
                locale,
            );
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
        let mut output = settle(attempt, cancelled, fault, format, locale);
        output.finalization = finalization;
        output
    }

    fn settle(
        attempt: Result<Sent, Failure>,
        cancelled: bool,
        fault: bool,
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
            Ok(sent) if !sent.receipt.accepted() => {
                send::receipt_output(sent.receipt, format, locale)
            }
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
                                prompt_failure(failure, format, locale)
                            }
                        }
                    }
                    #[cfg(windows)]
                    Completion::Terminal { binding, waited } => {
                        let outcome = interrupted.map_or_else(|| waited.transpose(), Err);
                        match outcome {
                            Ok(matched) => {
                                terminal::success(binding, receipt, matched, format, locale)
                            }
                            Err(failure) => terminal::failed_after_send(
                                failure, binding, receipt, format, locale,
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
        #[cfg(windows)]
        let selected = {
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
        };
        #[cfg(not(windows))]
        let selected = query::resolve_agent_terminal(&mut client, &snapshot, target, admission)?;
        #[cfg(windows)]
        let selected = {
            // Refuse already-known blockers before collecting processes, then
            // check readiness again against the re-read pinned observation.
            check_ready(&selected.observation)?;
            query::verify_agent_terminal(&mut client, &snapshot, selected, admission)?
        };
        check_ready(&selected.observation)?;
        #[cfg(windows)]
        {
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
        #[cfg(windows)]
        let mut journal = query::AgentJournal::default();
        #[cfg(windows)]
        if wait.is_some() {
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
        #[cfg(windows)]
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
        if query::wait_backoff(signals.poll_fd(), Duration::from_millis(1)) {
            return Err(prompt_error(FailureKind::InternalFailure, "interrupted"));
        }
        let codex_boundary = cfg!(windows) && before.snapshot.provider_id.as_str() == "codex";
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
                    #[cfg(windows)]
                    PromptObservation {
                        snapshot: &snapshot,
                        journal: &mut journal,
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

    #[cfg(windows)]
    struct PromptObservation<'a> {
        snapshot: &'a orchbee_workbench_protocol::Snapshot,
        journal: &'a mut query::AgentJournal,
    }

    fn observe(
        client: &mut crate::local_client::LocalClient,
        before: &AgentSessionDetail,
        options: &ResolvedWait,
        start: Instant,
        total: WaitDeadline,
        signals: &mut ArmedSignals,
        #[cfg(windows)] windows: PromptObservation<'_>,
    ) -> Result<Matched, Failure> {
        #[cfg(windows)]
        let PromptObservation { snapshot, journal } = windows;
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
            #[cfg(windows)]
            let historical = {
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
                    active |= version.freshness == AgentObservationFreshness::Fresh && newer
                        && (matches!(change.session.status, AgentSemanticStatus::Working { .. } | AgentSemanticStatus::NeedsAttention { .. })
                            || change.transition == orchbee_agent_observation_protocol::AgentChangeTransition::TurnFinished);
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
                                ) && query::status_matches(
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
            };
            #[cfg(windows)]
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
            #[cfg(windows)]
            if journal
                .observed_through()
                .is_some_and(|head| head.value() > read.revision.value())
            {
                return Err(prompt_error(
                    FailureKind::ProtocolInvalid,
                    "wait-read-failed",
                ));
            }
            #[cfg(windows)]
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
            #[cfg(windows)]
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
                .filter(|condition| query::condition_matches(*condition, &read.session))
                .collect();
            if active && !conditions.is_empty() {
                return Ok(Matched {
                    observation: read,
                    conditions,
                    observations,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    timeout_ms: options.timeout_ms,
                    #[cfg(windows)]
                    matched_change: None,
                    #[cfg(windows)]
                    client: client_view,
                });
            }
            if read.session.snapshot.lifecycle == AgentLifecycle::Ended {
                return Err(prompt_error(FailureKind::NotFound, "session-ended"));
            }
            #[cfg(windows)]
            if journal.has_more() {
                continue;
            }
            let remaining = limit.saturating_duration_since(Instant::now());
            if query::wait_backoff(signals.poll_fd(), backoff.min(remaining)) {
                return Err(prompt_error(FailureKind::InternalFailure, "interrupted"));
            }
            backoff = (backoff * 2).min(Duration::from_millis(500));
        }
    }

    #[cfg(windows)]
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

    #[cfg(windows)]
    fn journal_failure(
        error: query::JournalError,
        total: WaitDeadline,
        awaiting_activity: bool,
        effect: Instant,
    ) -> Failure {
        let failure = match error {
            query::JournalError::Exchange(error) => query::classify_agent_read(
                error,
                crate::local_client::agent::AgentReadOperation::Changes,
            ),
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
            .and_then(|reason| locale.cli_prompt_hint(reason));
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
        #[cfg(windows)]
        let matched_change = matched.as_ref().and_then(|matched| matched.matched_change.as_ref()).map(|(source,change)| {
            json!({"source":super::super::results::agent_source(source),"change":change})
        });
        #[cfg(windows)]
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
        #[cfg(windows)]
        let matched_change = json!(matched_change);
        #[cfg(not(windows))]
        let matched_change = json!({"kind":"unsupported"});
        #[cfg(windows)]
        let client_view = client_view.unwrap_or(Value::Null);
        #[cfg(not(windows))]
        let client_view = json!({"kind":"unsupported"});
        let document = json!({"schema":"orchbee.agent.prompt", "version":1, "source":receipt["source"], "result":{"sessionId":session_id, "submission":receipt, "observation":observation, "wait":wait,"matchedChange":matched_change,"client":client_view}});
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => output::encode_text(&locale.cli_prompt_submitted(
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
}
