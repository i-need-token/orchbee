//! Prompt composition retains delivery evidence independently of observation.
//! The shared terminal send owner alone constructs input and delivery receipts.

use super::{Failure, FailureKind, Output, failure_output};
use crate::invocation::{AgentPromptWait, OutputFormat};
use crate::localization::Locale;
use orchbee_agent_observation_protocol::AgentSessionSelector;

pub(super) fn run(
    target: &AgentSessionSelector,
    text: &str,
    wait: Option<&AgentPromptWait>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    #[cfg(any(target_os = "linux", windows))]
    {
        active::run(target, text, wait, format, locale)
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
    use super::*;
    use crate::cli::source::{ArmedSignals, Finish, SignalWatch};
    use crate::cli::wait_deadline::WaitDeadline;
    use crate::cli::{output, query, send};
    use crate::invocation::AgentWaitCondition;
    use orchbee_agent_observation_protocol::{
        AgentLifecycle, AgentObservationFreshness, AgentSemanticStatus, AgentSessionDetail,
    };
    use orchbee_local_ipc::LocalIo;
    use serde_json::{Map, Value, json};
    use std::time::{Duration, Instant};

    const READ_BUDGET: Duration = Duration::from_secs(5);
    const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-prompt";
    struct Sent {
        receipt: send::Receipt,
        session_id: String,
        waited: Option<Result<Matched, Failure>>,
    }
    struct Matched {
        observation: query::AgentObservationRead,
        conditions: Vec<AgentWaitCondition>,
        observations: u64,
        elapsed_ms: u64,
        timeout_ms: Option<u32>,
    }

    pub(super) fn run(
        target: &AgentSessionSelector,
        text: &str,
        wait: Option<&AgentPromptWait>,
        format: OutputFormat,
        locale: Locale,
    ) -> Output {
        let start = Instant::now();
        let timeout_ms = wait.map_or(Some(5000), |wait| wait.timeout_ms);
        let total = match WaitDeadline::new(start, timeout_ms) {
            Ok(total) => total,
            Err(failure) => return failure_output(failure, Some(format), locale),
        };
        let armed = match ArmedSignals::arm_interrupts() {
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
        let attempt = span(
            target,
            text,
            wait,
            start,
            total,
            SignalWatch::poll_fd(&armed),
        );
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
                let outcome = if fault {
                    Err(prompt_error(
                        FailureKind::InternalFailure,
                        "signal-finalization-failed",
                    ))
                } else if cancelled && sent.waited.is_some() {
                    Err(prompt_error(FailureKind::Cancelled, "cancelled"))
                } else {
                    sent.waited.transpose()
                };
                match outcome {
                    Ok(matched) => success(&sent.session_id, receipt, matched, format, locale),
                    Err(mut failure) => {
                        failure.failure_schema = Some(FAILURE_SCHEMA);
                        let details = failure.details.get_or_insert_with(Map::new);
                        details.insert("sessionId".to_owned(), json!(sent.session_id));
                        details.insert("submission".to_owned(), receipt.clone());
                        failure.source = Some(receipt["source"].clone());
                        prompt_failure(failure, format, locale)
                    }
                }
            }
        }
    }

    fn span(
        target: &AgentSessionSelector,
        text: &str,
        wait: Option<&AgentPromptWait>,
        start: Instant,
        total: WaitDeadline,
        wake: Option<LocalIo<'_>>,
    ) -> Result<Sent, Failure> {
        let admission = total.step(Instant::now(), READ_BUDGET)?;
        let (mut client, _, snapshot) = query::snapshot_session_until(admission)?;
        let selected = query::resolve_agent_terminal(&mut client, &snapshot, target, admission)?;
        let before = &selected.observation.session;
        let session_id = before.snapshot.session_id.clone();
        let blocked = !before.snapshot.pending_requests.is_empty()
            || matches!(before.status, AgentSemanticStatus::NeedsAttention { .. });
        if blocked {
            return Err(prompt_error(FailureKind::Rejected, "agent-blocked").source(
                super::super::results::agent_source(&selected.observation.request_id),
            ));
        }
        if before.snapshot.observation.freshness != AgentObservationFreshness::Fresh
            || matches!(before.status, AgentSemanticStatus::Unknown { .. })
        {
            return Err(
                prompt_error(FailureKind::Rejected, "agent-not-ready").source(
                    super::super::results::agent_source(&selected.observation.request_id),
                ),
            );
        }
        if query::wait_backoff(wake, Duration::from_millis(1)) {
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
            wait.map(|options| observe(&mut client, before, options, start, total, wake))
        } else {
            None
        };
        drop(client);
        Ok(Sent {
            receipt,
            session_id: session_id.as_str().to_owned(),
            waited,
        })
    }

    fn observe(
        client: &mut crate::local_client::LocalClient,
        before: &AgentSessionDetail,
        options: &AgentPromptWait,
        start: Instant,
        total: WaitDeadline,
        wake: Option<LocalIo<'_>>,
    ) -> Result<Matched, Failure> {
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
            let result =
                query::read_agent_observation(client, &before.snapshot.session_id, limit, wake);
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
                });
            }
            if read.session.snapshot.lifecycle == AgentLifecycle::Ended {
                return Err(prompt_error(FailureKind::NotFound, "session-ended"));
            }
            let remaining = limit.saturating_duration_since(Instant::now());
            if query::wait_backoff(wake, backoff.min(remaining)) {
                return Err(prompt_error(FailureKind::InternalFailure, "interrupted"));
            }
            backoff = (backoff * 2).min(Duration::from_millis(500));
        }
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
        let (observation, wait) = match matched {
            None => (Value::Null, Value::Null),
            Some(matched) => (
                json!({"source":super::super::results::agent_source(&matched.observation.request_id), "revision":matched.observation.revision, "session":matched.observation.session}),
                json!({"matchedConditions":matched.conditions.iter().map(|condition| condition.as_str()).collect::<Vec<_>>(), "timeoutMs":matched.timeout_ms, "observations":matched.observations, "elapsedMs":matched.elapsed_ms, "activityObserved":true}),
            ),
        };
        let document = json!({"schema":"orchbee.agent.prompt", "version":2, "source":receipt["source"], "result":{"sessionId":session_id, "submission":receipt, "observation":observation, "wait":wait}});
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
    mod tests {
        use super::*;
        use orchbee_client_server_protocol::{
            SendCleanupStatus, SendProgress, SendUnavailableReason, TerminalSendOutcome,
        };

        fn sent(outcome: TerminalSendOutcome, waiting: bool) -> Result<Sent, Failure> {
            Ok(Sent {
                receipt: send::test_receipt(outcome),
                session_id: "sess-alpha".to_owned(),
                waited: waiting
                    .then(|| Err(prompt_error(FailureKind::TransportLoss, "wait-read-failed"))),
            })
        }
        fn accepted() -> TerminalSendOutcome {
            TerminalSendOutcome::Accepted {
                progress: SendProgress::complete(2, 5),
                cleanup: SendCleanupStatus::Confirmed,
            }
        }
        fn document(output: &Output) -> Value {
            serde_json::from_slice(&output.stdout).unwrap()
        }
        fn validate(output: &Output, schema: &str) {
            let schema = serde_json::from_str::<Value>(schema).unwrap();
            assert!(
                jsonschema::validator_for(&schema)
                    .unwrap()
                    .is_valid(&document(output))
            );
        }

        #[test]
        fn cancellation_preserves_delivery_and_only_cancels_the_wait() {
            let submitted = settle(
                sent(accepted(), false),
                true,
                false,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(submitted.status, 0);
            validate(
                &submitted,
                include_str!("schemas/orchbee.agent.prompt.json"),
            );
            let waiting = settle(
                sent(accepted(), true),
                true,
                false,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(waiting.status, 130);
            assert_eq!(
                document(&waiting)["failure"]["details"]["submission"]["result"]["inputStatus"],
                "accepted"
            );
            validate(
                &waiting,
                include_str!("schemas/orchbee.cli.failure.agent-prompt.json"),
            );
            let guard_fault = settle(
                sent(accepted(), false),
                false,
                true,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(guard_fault.status, 1);
            validate(
                &guard_fault,
                include_str!("schemas/orchbee.cli.failure.agent-prompt.json"),
            );
            assert_eq!(
                document(&guard_fault)["failure"]["details"]["submission"]["result"]["inputStatus"],
                "accepted"
            );
        }

        #[test]
        fn cancellation_never_erases_unknown_or_partial_delivery() {
            let lost = settle(
                Err(Failure::new(FailureKind::OutcomeUnknown)),
                true,
                false,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(lost.status, 8);
            let unknown = TerminalSendOutcome::OutcomeUnknown {
                progress: SendProgress::stopped(1, 4),
                cleanup: SendCleanupStatus::Unconfirmed,
            };
            let output = settle(
                sent(unknown, false),
                true,
                false,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(output.status, 8);
            validate(&output, include_str!("schemas/orchbee.terminal.send.json"));
            let partial = TerminalSendOutcome::Unavailable {
                reason: SendUnavailableReason::InputBudgetExhausted,
                progress: SendProgress::stopped(1, 4),
                cleanup: SendCleanupStatus::Confirmed,
            };
            let output = settle(
                sent(partial, false),
                true,
                false,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(output.status, 9);
            validate(&output, include_str!("schemas/orchbee.terminal.send.json"));
            let before = settle(
                Err(Failure::new(FailureKind::TransportLoss)),
                true,
                false,
                OutputFormat::Json,
                Locale::EnUs,
            );
            assert_eq!(before.status, 130);
            validate(
                &before,
                include_str!("schemas/orchbee.cli.failure.agent-prompt.json"),
            );
            assert!(
                document(&before)["failure"]["details"]
                    .get("submission")
                    .is_none()
            );
        }
    }
}
