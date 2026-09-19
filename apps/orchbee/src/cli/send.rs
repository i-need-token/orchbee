//! `terminal send`: resolve one bound terminal from the snapshot, then submit
//! exactly one `terminal.send` request on the same admitted session. The
//! Server authorizes the current binding during this admission and drives the
//! temporary input-only attach → input → detach sequence; the CLI never takes
//! a TUI controller, viewer, or resize lease and never retries an ambiguous
//! answer — an uncertain outcome surfaces as `outcome-unknown`, not a replay.

use super::Output;
#[cfg(not(target_os = "linux"))]
use super::{Failure, FailureKind, failure_output};
use crate::invocation::{OutputFormat, Target, TerminalSendInput};
use crate::localization::Locale;
#[cfg(not(target_os = "linux"))]
use serde_json::Map;
#[cfg(not(target_os = "linux"))]
use serde_json::json;

pub(crate) const TERMINAL_SEND_ID: &str = "orchbee.terminal.send";

/// Which stderr diagnostic a typed send answer carries. The send envelope on
/// stdout always reports both axes verbatim; this only picks the localized
/// guidance line — most importantly the cases where resending could type
/// twice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SendVerdict {
    /// `accepted` + `confirmed`: nothing to warn about.
    Quiet,
    /// The input landed but detach never confirmed; resending could type twice.
    AcceptedCleanupUnconfirmed,
    /// A typed refusal; the input provably did not reach the Host queue.
    Rejected,
    /// Never submitted; no Host write was attempted.
    Unavailable,
    /// The exchange never settled; the input may already have landed.
    OutcomeUnknown,
}

#[cfg(target_os = "linux")]
pub(super) fn run(
    target: &Target,
    input: &TerminalSendInput,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    linux::run(target, input, format, locale)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn run(
    target: &Target,
    input: &TerminalSendInput,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let _ = (target, input);
    failure_output(
        Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
            "platform",
            "unsupported-platform",
        )),
        Some(format),
        locale,
    )
}

#[cfg(target_os = "linux")]
mod linux {
    use std::time::{Duration, Instant};

    use orchbee_client_server_protocol::{
        SendCleanupStatus, SendKey, SendRejectedReason, SendText, SendUnavailableReason,
        SiteId as ObcsSiteId, TerminalId as ObcsTerminalId, TerminalSendOutcome,
        TerminalSendPayload, TerminalSendRequest, TerminalSendResult, WorkerId as ObcsWorkerId,
    };
    use orchbee_workbench_protocol::Snapshot;
    use serde_json::{Map, Value, json};

    use super::super::query::{obcs_request_id, snapshot_session_until};
    use super::super::{
        Failure, FailureKind, Output, failure_output, output, present, results, stage_details,
        target_details,
    };
    use super::{SendVerdict, TERMINAL_SEND_ID};
    use crate::invocation::{OutputFormat, Resource, Target, TerminalSendInput, TerminalSendKey};
    use crate::local_client::{OutcomeUnknownReason, SubmissionError};
    use crate::localization::Locale;

    /// The typed answer a send produced plus its process-facing projection:
    /// the envelope body, the exit status the outcome maps to, and the stderr
    /// verdict the localized diagnostic renders.
    struct SendAnswer {
        source: Value,
        result: Value,
        status: u8,
        verdict: SendVerdict,
        reason: Option<&'static str>,
        cleanup_unconfirmed: bool,
    }

    /// The whole send invocation — cohort, discovery, admission, snapshot,
    /// and the send exchange — shares this one absolute ceiling, sampled once
    /// at command entry.
    const SEND_BUDGET: Duration = Duration::from_secs(5);

    /// The parsed payload form projected into the document — never the
    /// payload contents.
    enum PayloadFact {
        Text { record_bytes: usize },
        Key { name: &'static str },
    }

    /// The resolved request facts the answer document echoes; collected
    /// before submission consumes the request handle.
    struct RequestFacts {
        site_id: String,
        worker_id: String,
        terminal_id: String,
    }

    pub(super) fn run(
        target: &Target,
        input: &TerminalSendInput,
        format: OutputFormat,
        locale: Locale,
    ) -> Output {
        match execute(target, input) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok(answer) => answer_output(&answer, format, locale),
        }
    }

    /// One admitted session, one snapshot read, one binding resolution, one
    /// `terminal.send` exchange — all under the single absolute deadline the
    /// command samples at entry. The session is fully dropped before the
    /// typed answer is mapped.
    fn execute(target: &Target, input: &TerminalSendInput) -> Result<SendAnswer, Failure> {
        let deadline = Instant::now()
            .checked_add(SEND_BUDGET)
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        let (mut client, deadline, snapshot) = snapshot_session_until(deadline)?;
        let (request, fact, facts) = prepare(target, input, &snapshot)?;
        let source = send_source(&snapshot, request.request_id().as_str());
        let submission = client.submit_terminal_send_before(request, deadline);
        // The Client Session is fully dropped before the typed answer — or
        // the submission failure — is mapped into output.
        drop(client);
        match submission {
            Err(error) => Err(classify_send(error).source(source)),
            Ok(result) => Ok(answer(&result, &fact, &facts, source)),
        }
    }

    /// Resolve the binding and build the single request. Every wire identity
    /// — site, worker, terminal — comes from the just-read snapshot; the
    /// payload constructor re-checks the byte bound the parser already
    /// enforced, so impossible argv cannot produce a malformed record.
    fn prepare(
        target: &Target,
        input: &TerminalSendInput,
        snapshot: &Snapshot,
    ) -> Result<(TerminalSendRequest, PayloadFact, RequestFacts), Failure> {
        let identity = match target {
            Target::Id(identity) => identity.as_str(),
            // The grammar admits only typed IDs for terminal targets.
            Target::Label(_) => return Err(Failure::new(FailureKind::InternalFailure)),
        };
        let row = results::terminal_binding(snapshot, identity).ok_or_else(|| {
            Failure::new(FailureKind::NotFound)
                .details(target_details(Resource::Terminal, identity))
                .source(results::snapshot_source(snapshot))
        })?;
        let (payload, fact) = match input {
            TerminalSendInput::Text { text, enter } => {
                let record_bytes = text.len() + usize::from(*enter);
                let text = SendText::new(text.clone())
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
                (
                    TerminalSendPayload::new_text(text, *enter)
                        .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
                    PayloadFact::Text { record_bytes },
                )
            }
            TerminalSendInput::Key(key) => (
                TerminalSendPayload::new_key(send_key(*key)),
                PayloadFact::Key { name: key.as_str() },
            ),
        };
        let facts = RequestFacts {
            site_id: snapshot.site_id.as_str().to_owned(),
            worker_id: row.worker_id.clone(),
            terminal_id: row.terminal_id.clone(),
        };
        Ok((
            TerminalSendRequest::new(
                obcs_request_id()?,
                ObcsSiteId::parse(snapshot.site_id.as_str())
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
                ObcsWorkerId::parse(row.worker_id.as_str())
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
                ObcsTerminalId::parse(row.terminal_id.as_str())
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
                payload,
            ),
            fact,
            facts,
        ))
    }

    /// Map the typed `terminal.send.result` into the two-axis receipt and the
    /// process verdict. `target()` is the physical incarnation the Server
    /// actually admitted — projected verbatim, never re-resolved or invented.
    fn answer(
        result: &TerminalSendResult,
        fact: &PayloadFact,
        facts: &RequestFacts,
        source: Value,
    ) -> SendAnswer {
        let mut document = Map::from_iter([
            ("action".to_owned(), json!("terminal.send")),
            ("requestId".to_owned(), json!(result.request_id().as_str())),
            ("siteId".to_owned(), json!(facts.site_id)),
            ("workerId".to_owned(), json!(facts.worker_id)),
            ("terminalId".to_owned(), json!(facts.terminal_id)),
        ]);
        match fact {
            PayloadFact::Text { record_bytes } => {
                document.insert("payloadKind".to_owned(), json!("text"));
                document.insert("recordBytes".to_owned(), json!(record_bytes));
            }
            PayloadFact::Key { name } => {
                document.insert("payloadKind".to_owned(), json!("key"));
                document.insert("key".to_owned(), json!(name));
            }
        }
        if let Some(target) = result.target() {
            document.insert(
                "target".to_owned(),
                json!({
                    "workerGeneration": target.worker_generation().to_string(),
                    "hostEpoch": target.host_epoch().as_str(),
                    "hostTerminalId": target.host_terminal_id().as_str(),
                }),
            );
        }
        let (input_status, cleanup, reason, accepted_bytes, status, verdict) =
            match result.outcome() {
                TerminalSendOutcome::Accepted {
                    accepted_bytes,
                    cleanup,
                } => (
                    "accepted",
                    *cleanup,
                    None,
                    Some(accepted_bytes.get()),
                    if matches!(cleanup, SendCleanupStatus::Confirmed) {
                        0
                    } else {
                        5
                    },
                    if matches!(cleanup, SendCleanupStatus::Confirmed) {
                        SendVerdict::Quiet
                    } else {
                        SendVerdict::AcceptedCleanupUnconfirmed
                    },
                ),
                TerminalSendOutcome::Rejected { reason, cleanup } => (
                    "rejected",
                    *cleanup,
                    Some(rejected_reason(*reason)),
                    None,
                    3,
                    SendVerdict::Rejected,
                ),
                TerminalSendOutcome::Unavailable { reason, cleanup } => (
                    "not-submitted",
                    *cleanup,
                    Some(unavailable_reason(*reason)),
                    None,
                    // A host protocol fault is a transport-class defect, not
                    // service unavailability.
                    if matches!(reason, SendUnavailableReason::HostProtocolFault) {
                        5
                    } else {
                        4
                    },
                    SendVerdict::Unavailable,
                ),
                TerminalSendOutcome::OutcomeUnknown { cleanup } => (
                    "unknown",
                    *cleanup,
                    None,
                    None,
                    8,
                    SendVerdict::OutcomeUnknown,
                ),
            };
        document.insert("inputStatus".to_owned(), json!(input_status));
        document.insert("cleanupStatus".to_owned(), json!(cleanup_status(cleanup)));
        if let Some(reason) = reason {
            document.insert("reason".to_owned(), json!(reason));
        }
        if let Some(accepted_bytes) = accepted_bytes {
            document.insert("acceptedBytes".to_owned(), json!(accepted_bytes));
        }
        SendAnswer {
            source,
            result: Value::Object(document),
            status,
            verdict,
            reason,
            cleanup_unconfirmed: matches!(cleanup, SendCleanupStatus::Unconfirmed),
        }
    }

    /// The submission failures that provably never placed a byte on the wire
    /// are clean transport loss; anything after a possible handoff leaves the
    /// input's fate undetermined — `outcome-unknown`, never a retry.
    fn classify_send(error: SubmissionError) -> Failure {
        match error {
            SubmissionError::SessionUnavailable | SubmissionError::RequestEncoding(_) => {
                Failure::new(FailureKind::InternalFailure)
            }
            SubmissionError::RequestNotSubmitted(_) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("send"))
            }
            SubmissionError::OutcomeUnknown { request_id, reason } => {
                Failure::new(FailureKind::OutcomeUnknown).details(Map::from_iter([
                    ("stage".to_owned(), json!("send")),
                    ("requestId".to_owned(), json!(request_id.as_str())),
                    ("reason".to_owned(), json!(unknown_reason(&reason))),
                ]))
            }
        }
    }

    /// Post-submission ambiguity tokens: what broke decides the token, and
    /// every variant means the input may already have landed.
    fn unknown_reason(reason: &OutcomeUnknownReason) -> &'static str {
        match reason {
            OutcomeUnknownReason::Write(_) => "write-failure",
            OutcomeUnknownReason::Read(_) => "response-loss",
            OutcomeUnknownReason::InvalidResult(_)
            | OutcomeUnknownReason::UnexpectedResult { .. } => "response-invalid",
            OutcomeUnknownReason::CorrelationMismatch { .. } => "correlation-mismatch",
        }
    }

    /// `source` for every send answer: the admitted request's own identity
    /// plus the snapshot revision the binding was resolved from.
    fn send_source(snapshot: &Snapshot, request_id: &str) -> Value {
        json!({
            "kind": "terminal.send",
            "siteId": snapshot.site_id.as_str(),
            "requestId": request_id,
            "snapshotRevision": snapshot.revision,
        })
    }

    /// Typed answers emit the send envelope on stdout in either format plus
    /// the localized stderr verdict; a preparation overflow degrades to the
    /// shared `output-too-large` failure under the caller's format.
    fn answer_output(answer: &SendAnswer, format: OutputFormat, locale: Locale) -> Output {
        let stderr =
            locale.cli_send_stderr(answer.verdict, answer.reason, answer.cleanup_unconfirmed);
        let document = json!({
            "schema": TERMINAL_SEND_ID,
            "version": 1,
            "source": answer.source,
            "result": answer.result,
        });
        let too_large = || {
            failure_output(
                Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                    "limitBytes".to_owned(),
                    json!(output::LIMIT),
                )])),
                Some(format),
                locale,
            )
        };
        match format {
            OutputFormat::Json => match output::encode_document(&document) {
                Ok(stdout) => Output {
                    status: answer.status,
                    stdout,
                    stderr,
                },
                Err(output::TooLarge) => too_large(),
            },
            OutputFormat::Text => {
                match output::render_text(|out| present::send_result(out, &answer.result)) {
                    Ok(stdout) => Output {
                        status: answer.status,
                        stdout,
                        stderr,
                    },
                    Err(output::TooLarge) => too_large(),
                }
            }
        }
    }

    fn send_key(key: TerminalSendKey) -> SendKey {
        match key {
            TerminalSendKey::Enter => SendKey::Enter,
            TerminalSendKey::Escape => SendKey::Escape,
            TerminalSendKey::Tab => SendKey::Tab,
            TerminalSendKey::Backspace => SendKey::Backspace,
            TerminalSendKey::Delete => SendKey::Delete,
            TerminalSendKey::Up => SendKey::Up,
            TerminalSendKey::Down => SendKey::Down,
            TerminalSendKey::Left => SendKey::Left,
            TerminalSendKey::Right => SendKey::Right,
            TerminalSendKey::Home => SendKey::Home,
            TerminalSendKey::End => SendKey::End,
            TerminalSendKey::PageUp => SendKey::PageUp,
            TerminalSendKey::PageDown => SendKey::PageDown,
            TerminalSendKey::CtrlC => SendKey::CtrlC,
            TerminalSendKey::CtrlD => SendKey::CtrlD,
        }
    }

    fn cleanup_status(cleanup: SendCleanupStatus) -> &'static str {
        match cleanup {
            SendCleanupStatus::NotNeeded => "not-needed",
            SendCleanupStatus::Confirmed => "confirmed",
            SendCleanupStatus::Unconfirmed => "unconfirmed",
        }
    }

    fn rejected_reason(reason: SendRejectedReason) -> &'static str {
        match reason {
            SendRejectedReason::TerminalUnavailable => "terminal-unavailable",
            SendRejectedReason::StaleWorkerGeneration => "stale-worker-generation",
            SendRejectedReason::StaleHostEpoch => "stale-host-epoch",
            SendRejectedReason::HostRejected => "host-rejected",
            SendRejectedReason::HostInputInvalid => "host-input-invalid",
            SendRejectedReason::HostInputLimitExceeded => "host-input-limit-exceeded",
            SendRejectedReason::HostInputBackpressure => "host-input-backpressure",
            SendRejectedReason::HostInputUnavailable => "host-input-unavailable",
        }
    }

    fn unavailable_reason(reason: SendUnavailableReason) -> &'static str {
        match reason {
            SendUnavailableReason::ServerBusy => "server-busy",
            SendUnavailableReason::WorkerUnavailable => "worker-unavailable",
            SendUnavailableReason::TerminalRuntimeUnavailable => "terminal-runtime-unavailable",
            SendUnavailableReason::HostUnavailable => "host-unavailable",
            SendUnavailableReason::HostProtocolFault => "host-protocol-fault",
        }
    }
}
