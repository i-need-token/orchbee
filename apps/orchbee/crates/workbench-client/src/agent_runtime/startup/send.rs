//! One terminal input submission, typed only at a confirmed idle prompt. Delivery truth is
//! independent of later readiness or CLI status.
use super::binding::Binding;
use crate::agent_runtime::SendVerdict;
use crate::local_client::{LocalClient, SubmissionError};
use orchbee_client_server_protocol::{
    SendCleanupStatus, SendKey, SendKeyStroke, SendPasteContent, SendPromptState, SendRecord,
    SendRecords, SendRejectedReason, SendStrokeKey, SendUnavailableReason, TerminalSendOutcome,
    TerminalSendRequest, TerminalSendResult,
};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::WorkbenchRevision;
use std::time::{Duration, Instant};

/// Separate preparation before any input submission from a submission's retained exchange evidence.
#[derive(Debug)]
pub enum Error {
    Preparation,
    Exchange(Box<ExchangeFailure>),
}

/// Preserve the attempted request's provenance and the Client's submitted-or-unknown evidence.
#[derive(Debug)]
pub struct ExchangeFailure {
    pub context: Context,
    pub failure: SubmissionError,
}

/// Frozen routing, Snapshot revision and request identity; none alone proves Server acceptance.
#[derive(Debug)]
pub struct Context {
    pub binding: Binding,
    pub revision: WorkbenchRevision,
    pub request: orchbee_client_server_protocol::RequestId,
}

/// The one input result and submitted payload dimensions, independent of later readiness or waits.
#[derive(Debug)]
pub struct Receipt {
    pub context: Context,
    pub result: TerminalSendResult,
    pub records: usize,
    pub bytes: usize,
}

/// Why input cannot count as fully accepted with confirmed cleanup.
/// Delivery progress remains in the receipt and must not be inferred from this category alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFailure {
    Rejected,
    Unavailable,
    Transport,
    Unknown,
    /// The Host typed nothing: it could not confirm that the shell waits at a fresh, empty
    /// prompt, and the state names the first rule it found unmet.
    PromptUnconfirmed(SendPromptState),
    /// The terminal's Host lacks the prompt gate, so nothing was typed.
    HostGateUnsupported,
}

impl Receipt {
    /// The Host saw a prompt younger than its quiet period and typed nothing.
    fn settling(&self) -> bool {
        matches!(
            self.result.outcome(),
            TerminalSendOutcome::Rejected {
                reason: SendRejectedReason::PromptUnconfirmed(SendPromptState::Settling),
                progress,
                ..
            } if progress.records_accepted() == 0 && progress.accepted_encoded_bytes() == 0
        )
    }
    /// Derive delivery truth from the Server outcome, independently of any later workflow failure.
    pub(crate) fn verdict(&self) -> SendVerdict {
        verdict(self.result.outcome())
    }
    /// Return no failure only for accepted input with confirmed cleanup.
    /// Partial input and zero-input protocol faults retain different failure categories.
    pub(crate) fn refusal(&self) -> Option<InputFailure> {
        match self.result.outcome() {
            TerminalSendOutcome::Accepted {
                cleanup: SendCleanupStatus::Confirmed,
                ..
            } => None,
            TerminalSendOutcome::Accepted { .. } => Some(InputFailure::Transport),
            TerminalSendOutcome::OutcomeUnknown { .. } => Some(InputFailure::Unknown),
            TerminalSendOutcome::Rejected {
                reason: SendRejectedReason::PromptUnconfirmed(state),
                ..
            } => Some(InputFailure::PromptUnconfirmed(*state)),
            TerminalSendOutcome::Rejected {
                reason: SendRejectedReason::HostPromptGateUnsupported,
                ..
            } => Some(InputFailure::HostGateUnsupported),
            TerminalSendOutcome::Rejected { .. } => Some(InputFailure::Rejected),
            TerminalSendOutcome::Unavailable {
                progress, reason, ..
            } => Some(if progress.records_accepted() > 0 {
                InputFailure::Rejected
            } else if matches!(reason, SendUnavailableReason::HostProtocolFault) {
                InputFailure::Transport
            } else {
                InputFailure::Unavailable
            }),
        }
    }
}

/// Build one Paste / optional Codex Right / Enter record sequence without performing input.
///
/// # Errors
/// Returns `Preparation` when the protocol cannot represent the payload or record sequence.
pub fn prompt_records(text: &str, codex_boundary: bool) -> Result<SendRecords, Error> {
    let mut records = vec![SendRecord::Paste {
        content: SendPasteContent::new(text.as_bytes().to_vec()).map_err(|_| Error::Preparation)?,
    }];
    if codex_boundary {
        // Keep the Codex paste-burst flush inside the same submission as Enter.
        records.push(SendRecord::Stroke(
            SendKeyStroke::new(SendStrokeKey::Named(SendKey::Right), 0)
                .map_err(|_| Error::Preparation)?,
        ));
    }
    records.push(SendRecord::Stroke(
        SendKeyStroke::new(SendStrokeKey::Named(SendKey::Enter), 0)
            .map_err(|_| Error::Preparation)?,
    ));
    SendRecords::new(records).map_err(|_| Error::Preparation)
}

/// Submit the prompt line on the borrowed Client and original budget, for the Host to type only
/// while the shell waits at a confirmed fresh, empty prompt. Never sent ungated.
/// Return delivery evidence even if a later startup observation or wait will fail.
///
/// The Host refuses a prompt younger than its quiet period as `settling`, a refusal that typed
/// nothing. Only that refusal is asked once more, as a new request, after the quiet period has
/// passed on this caller's thread; the wait holds no shared channel and ends early on the wake.
/// A second `settling` is final. The returned receipt is the last exchange's.
///
/// # Errors
/// `Preparation` proves this operation did not submit. `Exchange` retains the exact context and
/// Client failure needed to distinguish no submission from an unknown outcome; it is never retried.
pub(crate) fn submit_prompt(
    client: &mut LocalClient,
    binding: &Binding,
    revision: WorkbenchRevision,
    text: &str,
    codex_boundary: bool,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<Receipt, Error> {
    let receipt = submit_prompt_once(
        client,
        binding,
        revision,
        text,
        codex_boundary,
        deadline,
        wake,
    )?;
    let quiet = Duration::from_millis(orchbee_terminal_protocol::PROMPT_LINE_QUIET_MILLISECONDS);
    if !receipt.settling()
        || Instant::now() + quiet >= deadline
        || crate::agent_runtime::wait_backoff(wake, quiet)
    {
        return Ok(receipt);
    }
    submit_prompt_once(
        client,
        binding,
        revision,
        text,
        codex_boundary,
        deadline,
        wake,
    )
}

fn submit_prompt_once(
    client: &mut LocalClient,
    binding: &Binding,
    revision: WorkbenchRevision,
    text: &str,
    codex_boundary: bool,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<Receipt, Error> {
    let records = prompt_records(text, codex_boundary)?;
    let count = records.len();
    let request = TerminalSendRequest::idle_prompt(
        super::obcs_identity().map_err(|_| Error::Preparation)?,
        binding.session.clone(),
        binding.worker.clone(),
        binding.terminal.clone(),
        records,
    )
    .map_err(|_| Error::Preparation)?;
    let context = Context {
        binding: binding.clone(),
        revision,
        request: request.request_id().clone(),
    };
    match submit(client, request, deadline, wake) {
        Ok(result) => Ok(Receipt {
            context,
            result,
            records: count,
            bytes: text.len(),
        }),
        Err(failure) => Err(Error::Exchange(Box::new(ExchangeFailure {
            context,
            failure,
        }))),
    }
}

/// Forward one already-identified input request to the Client's matching cancellable operation.
/// This boundary neither creates another lane nor retries or renews the absolute deadline.
///
/// # Errors
/// Preserve `SubmissionError`, including the distinction between not submitted and outcome unknown.
pub fn submit(
    client: &mut LocalClient,
    request: TerminalSendRequest,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<TerminalSendResult, SubmissionError> {
    match wake {
        Some(wake) => client.submit_terminal_send_cancellable(request, deadline, Some(wake)),
        None => client.submit_terminal_send_before(request, deadline),
    }
}

/// Preserve delivery truth independently from the process status. Status 5
/// deliberately covers both accepted input with unconfirmed cleanup and a
/// zero-input Host protocol fault, so it cannot answer retry safety.
pub fn verdict(outcome: &TerminalSendOutcome) -> SendVerdict {
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

/// Crate-internal `Receipt` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait ReceiptTestAccess {
    fn verdict(&self) -> SendVerdict;
    fn refusal(&self) -> Option<InputFailure>;
}

#[cfg(any(test, feature = "test-support"))]
impl ReceiptTestAccess for Receipt {
    fn verdict(&self) -> SendVerdict {
        Receipt::verdict(self)
    }
    fn refusal(&self) -> Option<InputFailure> {
        Receipt::refusal(self)
    }
}
