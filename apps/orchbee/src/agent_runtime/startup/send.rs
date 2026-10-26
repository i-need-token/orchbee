//! One terminal input submission. Delivery truth is independent of later readiness or CLI status.
use super::binding::Binding;
use crate::agent_runtime::SendVerdict;
use crate::local_client::{LocalClient, SubmissionError};
use orchbee_client_server_protocol::{
    SendCleanupStatus, SendKey, SendKeyStroke, SendPasteContent, SendRecord, SendRecords,
    SendStrokeKey, SendUnavailableReason, TerminalSendOutcome, TerminalSendRequest,
    TerminalSendResult,
};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::WorkbenchRevision;
use std::time::Instant;

#[derive(Debug)]
pub(crate) enum Error {
    Preparation,
    Exchange(Box<ExchangeFailure>),
}

#[derive(Debug)]
pub(crate) struct ExchangeFailure {
    pub(crate) context: Context,
    pub(crate) failure: SubmissionError,
}

#[derive(Debug)]
pub(crate) struct Context {
    pub(crate) binding: Binding,
    pub(crate) revision: WorkbenchRevision,
    pub(crate) request: orchbee_client_server_protocol::RequestId,
}

#[derive(Debug)]
pub(crate) struct Receipt {
    pub(crate) context: Context,
    pub(crate) result: TerminalSendResult,
    pub(crate) records: usize,
    pub(crate) bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputFailure {
    Rejected,
    Unavailable,
    Transport,
    Unknown,
}

impl Receipt {
    pub(crate) fn verdict(&self) -> SendVerdict {
        verdict(self.result.outcome())
    }
    pub(crate) fn refusal(&self) -> Option<InputFailure> {
        match self.result.outcome() {
            TerminalSendOutcome::Accepted {
                cleanup: SendCleanupStatus::Confirmed,
                ..
            } => None,
            TerminalSendOutcome::Accepted { .. } => Some(InputFailure::Transport),
            TerminalSendOutcome::OutcomeUnknown { .. } => Some(InputFailure::Unknown),
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

pub(crate) fn prompt_records(text: &str, codex_boundary: bool) -> Result<SendRecords, Error> {
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

pub(crate) fn submit_prompt(
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
    let request = TerminalSendRequest::new(
        super::obcs_identity().map_err(|_| Error::Preparation)?,
        binding.session.clone(),
        binding.worker.clone(),
        binding.terminal.clone(),
        records,
    );
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

pub(crate) fn submit(
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
pub(crate) fn verdict(outcome: &TerminalSendOutcome) -> SendVerdict {
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
