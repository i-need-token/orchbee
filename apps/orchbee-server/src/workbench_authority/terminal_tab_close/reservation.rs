use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult,
    WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason, serialize_workbench_request, serialize_workbench_response,
};

use super::super::ledger_size;

/// Sum simultaneously retained categories; maximize alternatives within each bounded cell.
pub(super) fn maximum_primary_charge(
    command: &WorkbenchCommand,
    acknowledgment: &WorkbenchCommandResult,
) -> Option<usize> {
    let (terminal_context_id, tab_id) = match &command.action {
        WorkbenchAction::TabClose {
            terminal_context_id,
            tab_id,
        }
        | WorkbenchAction::PanelClose {
            terminal_context_id,
            tab_id,
            ..
        } => (terminal_context_id, tab_id),
        _ => return None,
    };
    let maximum_revision = WorkbenchRevision::new(u64::MAX);
    let maximum_generation = WorkbenchTabCloseAttemptGeneration::new(u64::MAX).ok()?;
    let mut maximum_command = command.clone();
    maximum_command.expected_revision = maximum_revision;
    let mut maximum_acknowledgment = acknowledgment.clone();
    match &mut maximum_acknowledgment.outcome {
        WorkbenchCommandOutcome::TabCloseAccepted {
            accepted_revision, ..
        }
        | WorkbenchCommandOutcome::PanelCloseAccepted {
            accepted_revision, ..
        } => *accepted_revision = maximum_revision,
        _ => return None,
    }
    let command_charge = ledger_size(&maximum_command, &maximum_acknowledgment)?;
    let mut status_charge = 0;
    for status in statuses() {
        let encoded = serialize_workbench_response(&WorkbenchResponse::TabCloseRead(
            WorkbenchTabCloseReadResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id.clone(),
                outcome: WorkbenchTabCloseReadOutcome::Available {
                    close_request_id: command.request_id.clone(),
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    attempt_generation: maximum_generation,
                    status,
                },
            },
        ))
        .ok()?;
        status_charge = status_charge.max(encoded.len());
    }
    let resume = WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        site_id: command.site_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        close_request_id: command.request_id.clone(),
        expected_attempt_generation: maximum_generation,
    };
    let resume_request_charge =
        serialize_workbench_request(&WorkbenchRequest::TabCloseResume(resume.clone()))
            .ok()?
            .len();
    let mut resume_response_charge = 0;
    for outcome in resume_outcomes(maximum_generation) {
        let encoded = serialize_workbench_response(&WorkbenchResponse::TabCloseResume(
            super::control::resume_result(&resume, outcome),
        ))
        .ok()?;
        resume_response_charge = resume_response_charge.max(encoded.len());
    }
    command_charge
        .checked_add(status_charge)?
        .checked_add(resume_request_charge)?
        .checked_add(resume_response_charge)
}

pub(super) fn receipt_charge(
    request: &WorkbenchTabCloseResume,
    result: &WorkbenchTabCloseResumeResult,
) -> Option<usize> {
    serialize_workbench_request(&WorkbenchRequest::TabCloseResume(request.clone()))
        .ok()?
        .len()
        .checked_add(
            serialize_workbench_response(&WorkbenchResponse::TabCloseResume(result.clone()))
                .ok()?
                .len(),
        )
}

fn statuses() -> impl Iterator<Item = WorkbenchTabCloseStatus> {
    use WorkbenchTabCloseBlockedReason as Blocked;
    use WorkbenchTabClosePhase as Phase;
    use WorkbenchTabClosePublicationReason as Publication;
    use WorkbenchTabCloseUnknownPhase as UnknownPhase;
    use WorkbenchTabCloseUnknownReason as UnknownReason;
    [
        Phase::Queued,
        Phase::WaitingGrace,
        Phase::WaitingOutcome,
        Phase::Disposing,
    ]
    .into_iter()
    .map(|phase| WorkbenchTabCloseStatus::Closing { phase })
    .chain(
        [
            Blocked::WorkerUnavailable,
            Blocked::HostRefused,
            Blocked::CleanupIncomplete,
            Blocked::InventoryIncomplete,
            Blocked::IdentityConflict,
            Blocked::ContinuationBudgetExhausted,
            Blocked::ResumeGenerationExhausted,
        ]
        .into_iter()
        .flat_map(|reason| {
            [false, true]
                .into_iter()
                .map(move |resumable| WorkbenchTabCloseStatus::CleanupBlocked { reason, resumable })
        }),
    )
    .chain(
        [
            UnknownPhase::Terminate,
            UnknownPhase::Force,
            UnknownPhase::Dispose,
        ]
        .into_iter()
        .flat_map(|phase| {
            [
                UnknownReason::TransportLost,
                UnknownReason::Deadline,
                UnknownReason::InvalidResponse,
            ]
            .into_iter()
            .map(move |reason| WorkbenchTabCloseStatus::OutcomeUnknown { phase, reason })
        }),
    )
    .chain(
        [
            Publication::TargetCleanupPending,
            Publication::Preparing,
            Publication::PreparationFailed,
            Publication::AuthorityRetired,
        ]
        .into_iter()
        .flat_map(|reason| {
            [false, true].into_iter().map(move |resumable| {
                WorkbenchTabCloseStatus::DisposedAwaitingPublication { reason, resumable }
            })
        }),
    )
    .chain(std::iter::once(WorkbenchTabCloseStatus::Closed {
        revision: WorkbenchRevision::new(u64::MAX),
    }))
}

fn resume_outcomes(
    generation: WorkbenchTabCloseAttemptGeneration,
) -> impl Iterator<Item = WorkbenchTabCloseResumeOutcome> {
    use WorkbenchTabCloseResumeRejectionReason as Rejected;
    [
        WorkbenchTabCloseResumeOutcome::Resumed {
            attempt_generation: generation,
        },
        WorkbenchTabCloseResumeOutcome::Stale {
            attempt_generation: generation,
        },
        WorkbenchTabCloseResumeOutcome::Denied {},
        WorkbenchTabCloseResumeOutcome::Unavailable {
            reason: WorkbenchTabCloseUnavailableReason::ServerBusy,
        },
    ]
    .into_iter()
    .chain(
        [
            Rejected::OperationUnavailable,
            Rejected::OperationMismatch,
            Rejected::RequestIdentityConflict,
            Rejected::GenerationMismatch,
            Rejected::NotResumable,
            Rejected::GenerationExhausted,
        ]
        .into_iter()
        .map(|reason| WorkbenchTabCloseResumeOutcome::Rejected { reason }),
    )
}

#[cfg(test)]
mod tests;
