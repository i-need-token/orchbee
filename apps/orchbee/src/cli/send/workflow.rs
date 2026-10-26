//! Shared single-submission send lifecycle, source arbitration and delivery receipts.
use std::time::{Duration, Instant};

use orchbee_client_server_protocol::{
    SendBytes, SendCleanupStatus, SendKey, SendKeyStroke, SendPasteContent, SendRecord,
    SendRecords, SendRejectedReason, SendStrokeKey, SendText, SendTextInput, SendUnavailableReason,
    SessionId as ObcsSessionId, TerminalId as ObcsTerminalId, TerminalSendOutcome,
    TerminalSendRequest, TerminalSendResult, WorkerId as ObcsWorkerId,
};
use orchbee_workbench_protocol::Snapshot;
use serde_json::{Map, Value, json};

use super::super::query::{obcs_request_id, snapshot_session_until};
use super::super::source::{self, ArmedSignals, Finish, SourceFailure, source_details};
use super::super::{
    Failure, FailureKind, Output, SourceFinalization, failure_output, output, present, results,
    source_failure_output, stage_details,
};
use super::{SendTarget, SendVerdict, TERMINAL_SEND_ID};
use crate::invocation::{OutputFormat, Target, TerminalSendInput};
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
/// source materialization, and the send exchange — shares this one
/// absolute ceiling, sampled once at command entry.
const SEND_BUDGET: Duration = Duration::from_secs(5);

/// How the command fails. `Failure` keeps the existing
/// `orchbee.cli.failure` envelope; `Source` carries the
/// `orchbee.cli.failure.source` envelope with its explicit status — a
/// handled source interruption exits `128 + signo`, which no failure kind
/// can express — plus the noncloneable finalization token the common
/// emit path must consume after the output attempt.
enum SendError {
    Failure(Failure),
    Source(Box<SourceError>),
}

/// The typed source-failure channel, boxed so the rarely-reached guard
/// finalization token does not enlarge every `Result<_, SendError>`.
struct SourceError {
    failure: Failure,
    status: u8,
    finalization: Option<SourceFinalization>,
}

/// The explicit typed conversion from a materialization outcome to the
/// process-facing failure: source timeout keeps the existing timeout
/// mapping (7), source cancellation exits with the conventional
/// `128 + signo`, and every validation/local cause stays on the standard
/// path. `timeoutMs` reports the configured shared budget, never a
/// measured duration.
fn send_error(failure: SourceFailure) -> SendError {
    match failure {
        SourceFailure::Standard(failure) => SendError::Failure(failure),
        SourceFailure::Timeout { source } => SendError::Source(Box::new(SourceError {
            failure: Failure::new(FailureKind::Timeout).details(source_details(
                source,
                Some(u64::try_from(SEND_BUDGET.as_millis()).unwrap_or(u64::MAX)),
            )),
            status: FailureKind::Timeout.exit_code(),
            finalization: None,
        })),
        SourceFailure::Cancelled { source, signal } => cancelled(source, signal, None),
    }
}

/// The source-cancellation error: the `orchbee.cli.failure.source`
/// envelope names only the declared source, and the status follows the
/// conventional `128 + signo` of the first-read signal. `finalization`
/// carries the owed mask restore to the common emit path whenever the
/// guard still holds it.
fn cancelled(
    source: source::InputSource,
    signal: i32,
    finalization: Option<SourceFinalization>,
) -> SendError {
    SendError::Source(Box::new(SourceError {
        failure: Failure::new(FailureKind::Cancelled).details(source_details(source, None)),
        status: 128_u8.saturating_add(u8::try_from(signal).unwrap_or(0)),
        finalization,
    }))
}

/// The parsed payload form projected into the document — never the
/// payload contents. `record_bytes` echoes the single content record's
/// decoded size only when the request carries exactly one such record.
struct PayloadFact {
    kind: &'static str,
    records: usize,
    record_bytes: Option<usize>,
}

/// The resolved request facts the answer document echoes; collected
/// before submission consumes the request handle.
struct RequestFacts {
    session_id: String,
    worker_id: String,
    terminal_id: String,
}

pub(super) fn run(
    target: SendTarget<'_>,
    input: &TerminalSendInput,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) =
        crate::cli::execution::admit(crate::cli::execution::Capability::TerminalInput)
    {
        return failure_output(failure, Some(format), locale);
    }
    match execute(target, input) {
        Err(SendError::Failure(failure)) => failure_output(failure, Some(format), locale),
        Err(SendError::Source(error)) => {
            let SourceError {
                failure,
                status,
                finalization,
            } = *error;
            let mut output = source_failure_output(failure, status, format, locale);
            output.finalization = finalization;
            output
        }
        Ok(answer) => answer_output(&answer, format, locale),
    }
}

/// Which declared source produced the input — needed to name the
/// `inputSource` in a finish-boundary cancellation envelope.
fn input_source(input: &TerminalSendInput) -> source::InputSource {
    match input {
        TerminalSendInput::File { .. } => source::InputSource::File,
        _ => source::InputSource::Stdin,
    }
}

/// Source materialization first — a declared `--file`/`--stdin` becomes
/// the same bounded text record before any runtime effect, with the
/// private signal guard armed for exactly that interval — then one
/// admitted session, one snapshot read, one binding resolution, one
/// `terminal.send` exchange — all under the single absolute deadline the
/// command samples at entry. The session is fully dropped before the
/// typed answer is mapped.
fn execute(target: SendTarget<'_>, input: &TerminalSendInput) -> Result<SendAnswer, SendError> {
    let deadline = Instant::now()
        .checked_add(SEND_BUDGET)
        .ok_or_else(|| SendError::Failure(Failure::new(FailureKind::InternalFailure)))?;
    // Grammar and target syntax were validated before this command ran;
    // the source is the first effectful step and its failures carry no
    // request identity.
    let input = match input {
        TerminalSendInput::File { .. } | TerminalSendInput::Stdin { .. } => {
            let mut armed = ArmedSignals::arm()
                .map_err(|_| SendError::Failure(Failure::new(FailureKind::InternalFailure)))?;
            match source::materialize(input, deadline, &mut armed) {
                // A captured signal was already consumed mid-read: the
                // first-read signo selects cancellation and the held
                // mask travels with the output as the owed finalization.
                Err(SourceFailure::Cancelled { source, signal }) => {
                    return Err(cancelled(source, signal, Some(armed.into_finalization())));
                }
                outcome => match armed.finish() {
                    // A signal consumed at the finish boundary wins over
                    // whatever the materialization outcome was.
                    Ok(Finish::Cancelled {
                        signo,
                        finalization,
                    }) => {
                        return Err(cancelled(input_source(input), signo, Some(finalization)));
                    }
                    // A signalfd or restore failure is terminal — the
                    // owed restore was already attempted inside finish.
                    Err(_) => {
                        return Err(SendError::Failure(Failure::new(
                            FailureKind::InternalFailure,
                        )));
                    }
                    // The exact inherited mask is restored and the
                    // descriptor closed — the materialization outcome
                    // owns the result from here.
                    Ok(Finish::Clear) => match outcome {
                        Ok(materialized) => materialized,
                        Err(failure) => return Err(send_error(failure)),
                    },
                },
            }
        }
        TerminalSendInput::Text { .. }
        | TerminalSendInput::Keys(_)
        | TerminalSendInput::Bytes { .. }
        | TerminalSendInput::Paste { .. } => std::borrow::Cow::Borrowed(input),
    };
    let (mut client, deadline, snapshot) =
        snapshot_session_until(deadline).map_err(SendError::Failure)?;
    let agent_target;
    let target = match target {
        SendTarget::Terminal(target) => target,
        SendTarget::Agent(target) => {
            if crate::cli::execution::admit(crate::cli::execution::Capability::AgentProjection)
                .is_ok()
            {
                agent_target = super::super::query::named_terminal_target(
                    &mut client,
                    &snapshot,
                    target,
                    deadline,
                    None,
                    true,
                )
                .map_err(SendError::Failure)?;
            } else {
                agent_target = super::super::query::resolve_agent_terminal(
                    &mut client,
                    &snapshot,
                    target,
                    deadline,
                )
                .map_err(SendError::Failure)?
                .terminal;
            }
            &agent_target
        }
    };
    let result = submit_on_session(&mut client, &snapshot, target, &input, deadline);
    drop(client);
    result.map(Receipt::into_answer).map_err(SendError::Failure)
}

/// A held-session caller retains typed delivery facts until its session
/// closes. JSON projection and output remain after that release boundary.
pub(crate) struct Receipt {
    result: TerminalSendResult,
    fact: PayloadFact,
    facts: RequestFacts,
    source: Value,
}

impl Receipt {
    pub(crate) fn exit_status(&self) -> u8 {
        answer(&self.result, &self.fact, &self.facts, self.source.clone()).status
    }

    pub(crate) fn accepted(&self) -> bool {
        matches!(
            self.result.outcome(),
            TerminalSendOutcome::Accepted {
                cleanup: SendCleanupStatus::Confirmed,
                ..
            }
        )
    }

    pub(crate) fn verdict(&self) -> SendVerdict {
        send_verdict(self.result.outcome())
    }
    fn into_answer(self) -> SendAnswer {
        answer(&self.result, &self.fact, &self.facts, self.source)
    }
    pub(crate) fn document(&self) -> Value {
        let projected = answer(&self.result, &self.fact, &self.facts, self.source.clone());
        json!({"schema": TERMINAL_SEND_ID, "version":1, "source":projected.source, "result":projected.result})
    }
}

fn submit_on_session(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    input: &TerminalSendInput,
    deadline: Instant,
) -> Result<Receipt, Failure> {
    submit_prepared(
        client,
        snapshot,
        prepare(target, input, snapshot)?,
        deadline,
        None,
    )
}

pub(crate) fn submit_prompt_on_session(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    text: &str,
    codex_boundary: bool,
    deadline: Instant,
) -> Result<Receipt, Failure> {
    submit_prompt_on_session_cancellable(
        client,
        snapshot,
        target,
        text,
        codex_boundary,
        deadline,
        None,
    )
}

pub(crate) fn submit_prompt_on_session_cancellable(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    text: &str,
    codex_boundary: bool,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<Receipt, Failure> {
    let internal = |_| Failure::new(FailureKind::InternalFailure);
    let mut records = vec![SendRecord::Paste {
        content: SendPasteContent::new(text.as_bytes().to_vec()).map_err(internal)?,
    }];
    if codex_boundary {
        // A semantic key flushes Windows Codex's paste-burst buffer before
        // Enter. Keep it inside this one sequence, never a second send.
        records.push(SendRecord::Stroke(
            SendKeyStroke::new(SendStrokeKey::Named(SendKey::Right), 0)
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
        ));
    }
    records.push(SendRecord::Stroke(
        SendKeyStroke::new(SendStrokeKey::Named(SendKey::Enter), 0)
            .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
    ));
    let records = SendRecords::new(records).map_err(internal)?;
    let prepared = prepare_records(target, snapshot, (records, "sequence", Some(text.len())))?;
    submit_prepared(client, snapshot, prepared, deadline, wake)
}

fn submit_prepared(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    (request, fact, facts): (TerminalSendRequest, PayloadFact, RequestFacts),
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<Receipt, Failure> {
    let source = send_source(snapshot, request.request_id().as_str());
    let result = match wake {
        Some(wake) => client.submit_terminal_send_cancellable(request, deadline, Some(wake)),
        None => client.submit_terminal_send_before(request, deadline),
    }
    .map_err(|error| classify_send(error).source(source.clone()))?;
    Ok(Receipt {
        result,
        fact,
        facts,
        source,
    })
}

/// Typed protocol fixture for the prompt owner's cross-family cancellation
/// witnesses. No runtime/test hook exists in production builds.
#[cfg(test)]
pub(crate) fn test_receipt(outcome: TerminalSendOutcome) -> Receipt {
    use orchbee_client_server_protocol::{HostEpoch, HostTerminalId, RequestId, SendTarget};
    let request = RequestId::parse("11111111111111111111111111111111").unwrap();
    let target = SendTarget::new(
        std::num::NonZeroU64::MIN,
        HostEpoch::parse("22222222222222222222222222222222").unwrap(),
        HostTerminalId::parse("33333333333333333333333333333333").unwrap(),
    );
    Receipt {
        result: TerminalSendResult::new(request.clone(), Some(target), outcome).unwrap(),
        fact: PayloadFact {
            kind: "sequence",
            records: 2,
            record_bytes: None,
        },
        facts: RequestFacts {
            session_id: "44444444444444444444444444444444".to_owned(),
            worker_id: "55555555555555555555555555555555".to_owned(),
            terminal_id: "66666666666666666666666666666666".to_owned(),
        },
        source: json!({"kind":"terminal.send", "sessionId":"44444444444444444444444444444444", "requestId":request.as_str(), "snapshotRevision":"1"}),
    }
}

pub(crate) fn receipt_output(receipt: Receipt, format: OutputFormat, locale: Locale) -> Output {
    answer_output(&receipt.into_answer(), format, locale)
}

/// Resolve the binding and build the single request. Every wire identity
/// — session, worker, terminal — comes from the just-read snapshot; the
/// payload constructor re-checks the byte bound the parser already
/// enforced, so impossible argv cannot produce a malformed record.
fn prepare(
    target: &Target,
    input: &TerminalSendInput,
    snapshot: &Snapshot,
) -> Result<(TerminalSendRequest, PayloadFact, RequestFacts), Failure> {
    prepare_records(target, snapshot, send_records(input)?)
}

fn prepare_records(
    target: &Target,
    snapshot: &Snapshot,
    (records, kind, record_bytes): (SendRecords, &'static str, Option<usize>),
) -> Result<(TerminalSendRequest, PayloadFact, RequestFacts), Failure> {
    // Resolve a full ID or @terminal selector from the current snapshot.
    let row = results::terminal_select(snapshot, target)
        .map_err(|failure| failure.source(results::snapshot_source(snapshot)))?;
    let fact = PayloadFact {
        kind,
        records: records.len(),
        record_bytes,
    };
    let facts = RequestFacts {
        session_id: snapshot.session_id.as_str().to_owned(),
        worker_id: row.worker_id.clone(),
        terminal_id: row.terminal_id.clone(),
    };
    Ok((
        TerminalSendRequest::new(
            obcs_request_id()?,
            ObcsSessionId::parse(snapshot.session_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            ObcsWorkerId::parse(row.worker_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            ObcsTerminalId::parse(row.terminal_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            records,
        ),
        fact,
        facts,
    ))
}

/// The ordered wire records the parsed input produces plus the honest
/// `payloadKind` label. `recordBytes` is reported only for a single
/// content-bearing record, even when followed by semantic keys; it never
/// guesses their encoded byte length. Every constructor re-checks the
/// bounds the parser already enforced, so impossible argv cannot
/// produce a malformed record.
fn send_records(
    input: &TerminalSendInput,
) -> Result<(SendRecords, &'static str, Option<usize>), Failure> {
    let internal = || Failure::new(FailureKind::InternalFailure);
    let (records, kind, record_bytes) = match input {
        TerminalSendInput::Text { text, enter } => {
            let record = SendRecord::Text(
                SendTextInput::new(SendText::new(text.clone()).map_err(|_| internal())?, *enter)
                    .map_err(|_| internal())?,
            );
            (vec![record], "text", Some(text.len() + usize::from(*enter)))
        }
        TerminalSendInput::Keys(keys) => {
            let records: Vec<SendRecord> = keys.iter().copied().map(SendRecord::Stroke).collect();
            let kind = if records.len() == 1 {
                "key"
            } else {
                "sequence"
            };
            (records, kind, None)
        }
        TerminalSendInput::Bytes { bytes, enter } => {
            // `--enter` on literal bytes folds a trailing CR into the
            // record itself — a Bytes record is opaque content.
            let mut bytes = bytes.clone();
            if *enter {
                bytes.push(b'\r');
            }
            let record_bytes = bytes.len();
            (
                vec![SendRecord::Bytes {
                    bytes: SendBytes::new(bytes).map_err(|_| internal())?,
                }],
                "bytes",
                Some(record_bytes),
            )
        }
        TerminalSendInput::Paste { content, enter } => {
            // `--enter` on a paste is a separate trailing Enter record —
            // a CR inside bracketed content is literal text, not submit.
            let mut records = vec![SendRecord::Paste {
                content: SendPasteContent::new(content.clone()).map_err(|_| internal())?,
            }];
            if *enter {
                records.push(SendRecord::Stroke(
                    SendKeyStroke::new(SendStrokeKey::Named(SendKey::Enter), 0)
                        .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
                ));
            }
            let kind = if *enter { "sequence" } else { "paste" };
            (records, kind, Some(content.len()))
        }
        // Materialization folds every declared source into `Bytes` or
        // `Paste` before this point; reaching these arms is internal.
        TerminalSendInput::File { .. } | TerminalSendInput::Stdin { .. } => {
            return Err(internal());
        }
    };
    Ok((
        SendRecords::new(records).map_err(|_| internal())?,
        kind,
        record_bytes,
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
        ("sessionId".to_owned(), json!(facts.session_id)),
        ("workerId".to_owned(), json!(facts.worker_id)),
        ("terminalId".to_owned(), json!(facts.terminal_id)),
    ]);
    document.insert("payloadKind".to_owned(), json!(fact.kind));
    document.insert("records".to_owned(), json!(fact.records));
    if let Some(record_bytes) = fact.record_bytes {
        document.insert("recordBytes".to_owned(), json!(record_bytes));
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
    // The typed outcome always knows its delivery cursor: emit the
    // confirmed prefix, the first not-confirmed index, and the summed
    // Host-encoded byte count verbatim. A lost whole-request answer
    // never reaches this mapping — the failure envelope carries no
    // progress fields, so absent stays distinguishable from known-zero.
    let progress = result.outcome().progress();
    document.insert(
        "recordsAccepted".to_owned(),
        json!(progress.records_accepted()),
    );
    document.insert(
        "firstStoppedRecord".to_owned(),
        json!(progress.first_stopped_record()),
    );
    document.insert(
        "acceptedEncodedBytes".to_owned(),
        json!(progress.accepted_encoded_bytes()),
    );
    let verdict = send_verdict(result.outcome());
    let (input_status, cleanup, reason, status) = match result.outcome() {
        TerminalSendOutcome::Accepted { cleanup, .. } => (
            "accepted",
            *cleanup,
            None,
            if matches!(cleanup, SendCleanupStatus::Confirmed) {
                0
            } else {
                5
            },
        ),
        TerminalSendOutcome::Rejected {
            reason, cleanup, ..
        } => {
            // A refusal with a confirmed prefix is a determinate
            // partial application — exit 9, never the refused-nothing 3.
            let partial = progress.records_accepted() > 0;
            (
                if partial {
                    "partial-accepted"
                } else {
                    "rejected"
                },
                *cleanup,
                Some(rejected_reason(*reason)),
                if partial { 9 } else { 3 },
            )
        }
        TerminalSendOutcome::Unavailable {
            reason, cleanup, ..
        } => {
            let partial = progress.records_accepted() > 0;
            (
                if partial {
                    "partial-accepted"
                } else {
                    "not-submitted"
                },
                *cleanup,
                Some(unavailable_reason(*reason)),
                if partial {
                    9
                // A host protocol fault is a transport-class defect, not
                // service unavailability.
                } else if matches!(reason, SendUnavailableReason::HostProtocolFault) {
                    5
                } else {
                    4
                },
            )
        }
        TerminalSendOutcome::OutcomeUnknown { cleanup, .. } => ("unknown", *cleanup, None, 8),
    };
    document.insert("inputStatus".to_owned(), json!(input_status));
    document.insert("cleanupStatus".to_owned(), json!(cleanup_status(cleanup)));
    if let Some(reason) = reason {
        document.insert("reason".to_owned(), json!(reason));
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

/// Preserve delivery truth independently from the process status. Status 5
/// deliberately covers both accepted input with unconfirmed cleanup and a
/// zero-input Host protocol fault, so it cannot answer retry safety.
fn send_verdict(outcome: &TerminalSendOutcome) -> SendVerdict {
    match outcome {
        TerminalSendOutcome::Accepted { cleanup, .. } => {
            if matches!(cleanup, SendCleanupStatus::Confirmed) {
                SendVerdict::Quiet
            } else {
                SendVerdict::AcceptedCleanupUnconfirmed
            }
        }
        TerminalSendOutcome::Rejected { .. } => {
            if outcome.progress().records_accepted() > 0 {
                SendVerdict::PartialAccepted
            } else {
                SendVerdict::Rejected
            }
        }
        TerminalSendOutcome::Unavailable { .. } => {
            if outcome.progress().records_accepted() > 0 {
                SendVerdict::PartialAccepted
            } else {
                SendVerdict::Unavailable
            }
        }
        TerminalSendOutcome::OutcomeUnknown { .. } => SendVerdict::OutcomeUnknown,
    }
}

/// The submission failures that provably never placed a byte on the wire
/// are clean transport loss; anything after a possible handoff leaves the
/// input's fate undetermined — `outcome-unknown`, never a retry.
fn classify_send(error: SubmissionError) -> Failure {
    match error {
        SubmissionError::ConnectionUnavailable | SubmissionError::RequestEncoding(_) => {
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
        OutcomeUnknownReason::InvalidResult(_) | OutcomeUnknownReason::UnexpectedResult { .. } => {
            "response-invalid"
        }
        OutcomeUnknownReason::CorrelationMismatch { .. } => "correlation-mismatch",
    }
}

/// `source` for every send answer: the admitted request's own identity
/// plus the snapshot revision the binding was resolved from.
fn send_source(snapshot: &Snapshot, request_id: &str) -> Value {
    json!({
        "kind": "terminal.send",
        "sessionId": snapshot.session_id.as_str(),
        "requestId": request_id,
        "snapshotRevision": snapshot.revision,
    })
}

/// JSON retains the full receipt. Human output confirms successful input
/// submission once; nonzero outcomes use the localized stderr verdict.
/// A preparation overflow degrades to the
/// shared `output-too-large` failure under the caller's format.
fn answer_output(answer: &SendAnswer, format: OutputFormat, locale: Locale) -> Output {
    let stderr = locale.cli_send_stderr(answer.verdict, answer.reason, answer.cleanup_unconfirmed);
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
                finalization: None,
            },
            Err(output::TooLarge) => too_large(),
        },
        OutputFormat::Text => {
            if answer.status != 0 {
                return Output {
                    status: answer.status,
                    stdout: Vec::new(),
                    stderr,
                    finalization: None,
                };
            }
            match output::render_text(|out| present::send_result(out, &answer.result, locale)) {
                Ok(stdout) => Output {
                    status: answer.status,
                    stdout,
                    stderr,
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            }
        }
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
        SendUnavailableReason::InputBudgetExhausted => "input-budget-exhausted",
    }
}
