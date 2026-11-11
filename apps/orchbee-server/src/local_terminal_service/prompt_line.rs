//! One line typed into a shell at its confirmed prompt through the Host's gate: the records of
//! that line, the request that sends them to one exact terminal, and what the gate's answer means.
//! Agent session resume and Agent start delivery both type this way; whether and when to type
//! stays with each of them.
use super::create_reference;
use crate::session_coordinator::ProcessObservationTarget;
use orchbee_client_server_protocol::{
    SendKey, SendKeyStroke, SendPasteContent, SendPromptState, SendRecord, SendRecords,
    SendRejectedReason, SendStrokeKey, ServerMessage, SessionId, TerminalId, TerminalSendOutcome,
    TerminalSendRequest, WorkerId,
};

/// What the Host's gate did with one line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Answer {
    /// The Host typed the line.
    Accepted,
    /// The Host typed nothing: the shell was not at a fresh, empty, marked prompt.
    Refused(SendPromptState),
    /// The Host typed nothing because it predates the prompt gate; asking again cannot change that.
    GateUnsupported,
    /// Any other result. Part of the line may have been typed, so it is never sent again.
    Failed,
}

/// The idle-prompt request that types `text` into `target`, or `None` when the line or the
/// target's identities do not fit the request.
pub(super) fn request(
    target: &ProcessObservationTarget,
    text: &str,
    powershell: bool,
) -> Option<TerminalSendRequest> {
    TerminalSendRequest::idle_prompt(
        create_reference()?,
        SessionId::parse(target.session_id.as_str()).ok()?,
        WorkerId::parse(target.binding.worker_id.as_str()).ok()?,
        TerminalId::parse(target.binding.terminal_id.as_str()).ok()?,
        records(text, powershell)?,
    )
    .ok()
}

/// Reads one gated send's result. Only a refusal that typed nothing is a refusal.
pub(super) fn answer(message: &ServerMessage) -> Answer {
    let ServerMessage::TerminalSendResult(result) = message else {
        return Answer::Failed;
    };
    match result.outcome() {
        TerminalSendOutcome::Accepted { .. } => Answer::Accepted,
        TerminalSendOutcome::Rejected {
            reason, progress, ..
        } if progress.records_accepted() == 0 && progress.accepted_encoded_bytes() == 0 => {
            match reason {
                SendRejectedReason::PromptUnconfirmed(state) => Answer::Refused(*state),
                SendRejectedReason::HostPromptGateUnsupported => Answer::GateUnsupported,
                _ => Answer::Failed,
            }
        }
        _ => Answer::Failed,
    }
}

/// Paste, then Right for PowerShell, then Enter, in one submission: the one prompt line the Host's
/// gate admits, and the same records a Client's Agent start types into the same shells.
fn records(text: &str, powershell: bool) -> Option<SendRecords> {
    let mut records = vec![SendRecord::Paste {
        content: SendPasteContent::new(text.as_bytes().to_vec()).ok()?,
    }];
    if powershell {
        records.push(SendRecord::Stroke(
            SendKeyStroke::new(SendStrokeKey::Named(SendKey::Right), 0).ok()?,
        ));
    }
    records.push(SendRecord::Stroke(
        SendKeyStroke::new(SendStrokeKey::Named(SendKey::Enter), 0).ok()?,
    ));
    SendRecords::new(records).ok()
}
