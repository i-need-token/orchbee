use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult,
    WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason, WorkbenchWorkspaceCloseCounts, WorkbenchWorkspaceCloseMember,
    WorkbenchWorkspaceCloseMemberStatus, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceCloseResumeResult,
    serialize_workbench_request, serialize_workbench_response,
};

use super::super::ledger_size;

/// Retained heap per captured member — target plus continuation plus the
/// member's owned identity strings and its pending-step cell, per the
/// Workspace Close accounting model; never assumed, summed under checked
/// arithmetic at admission.
const RETAINED_HEAP_PER_MEMBER: usize = 1024;

/// Sum simultaneously retained categories; maximize alternatives within each bounded cell.
/// `member_count` is the frozen captured membership — only the Context scope's
/// member-dependent status wire and retained-heap charges consume it.
pub(super) fn maximum_primary_charge(
    command: &WorkbenchCommand,
    acknowledgment: &WorkbenchCommandResult,
    member_count: usize,
) -> Option<usize> {
    if matches!(&command.action, WorkbenchAction::WorkspaceClose { .. }) {
        return maximum_workspace_charge(command, acknowledgment, member_count);
    }
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

pub(super) fn workspace_receipt_charge(
    request: &WorkbenchWorkspaceCloseResume,
    result: &WorkbenchWorkspaceCloseResumeResult,
) -> Option<usize> {
    serialize_workbench_request(&WorkbenchRequest::WorkspaceCloseResume(request.clone()))
        .ok()?
        .len()
        .checked_add(
            serialize_workbench_response(&WorkbenchResponse::WorkspaceCloseResume(result.clone()))
                .ok()?
                .len(),
        )
}

/// The Workspace Close charge = the same wire/control cells as the Tab Close
/// (command+ack, worst status document, resume request+result) plus the
/// member-dependent cells the Context scope adds: `member_count` member
/// entries at their worst spelling inside the status document, and the
/// per-member retained heap the record owns.
fn maximum_workspace_charge(
    command: &WorkbenchCommand,
    acknowledgment: &WorkbenchCommandResult,
    member_count: usize,
) -> Option<usize> {
    let WorkbenchAction::WorkspaceClose {
        project_id,
        workspace_id,
    } = &command.action
    else {
        return None;
    };
    let maximum_revision = WorkbenchRevision::new(u64::MAX);
    let maximum_generation = WorkbenchTabCloseAttemptGeneration::new(u64::MAX).ok()?;
    let mut maximum_command = command.clone();
    maximum_command.expected_revision = maximum_revision;
    let mut maximum_acknowledgment = acknowledgment.clone();
    match &mut maximum_acknowledgment.outcome {
        WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            accepted_revision, ..
        } => *accepted_revision = maximum_revision,
        _ => return None,
    }
    let command_charge = ledger_size(&maximum_command, &maximum_acknowledgment)?;
    let terminal_context_id = match &acknowledgment.outcome {
        WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            terminal_context_id,
            ..
        } => terminal_context_id.clone(),
        _ => return None,
    };
    let maximum_counts = WorkbenchWorkspaceCloseCounts {
        closing: u32::MAX,
        cleanup_blocked: u32::MAX,
        outcome_unknown: u32::MAX,
        disposed_awaiting_publication: u32::MAX,
    };
    // Exact per-member encoding cost — measured through the live serializer
    // as `members:[m]` minus `members:[]`, never a sampled or guessed size.
    let read_result = |status: WorkbenchTabCloseStatus,
                       members: Vec<WorkbenchWorkspaceCloseMember>| {
        serialize_workbench_response(&WorkbenchResponse::WorkspaceCloseRead(
            WorkbenchWorkspaceCloseReadResult {
                protocol_version: ProtocolVersion,
                request_id: command.request_id.clone(),
                outcome: WorkbenchWorkspaceCloseReadOutcome::Available {
                    close_request_id: command.request_id.clone(),
                    project_id: project_id.clone(),
                    workspace_id: workspace_id.clone(),
                    terminal_context_id: terminal_context_id.clone(),
                    attempt_generation: maximum_generation,
                    status,
                    counts: maximum_counts,
                    members,
                },
            },
        ))
        .ok()
    };
    let member = |status: WorkbenchWorkspaceCloseMemberStatus| {
        Some(WorkbenchWorkspaceCloseMember {
            tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&id_max()).ok()?,
            panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id_max()).ok()?,
            presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&id_max())
                .ok()?,
            status,
        })
    };
    let empty_len = read_result(
        WorkbenchTabCloseStatus::Closing {
            phase: WorkbenchTabClosePhase::Queued,
        },
        Vec::new(),
    )?
    .len();
    let member_len = member_statuses()
        .into_iter()
        .map(|status| {
            read_result(
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                },
                vec![member(status)?],
            )
            .and_then(|encoded| encoded.len().checked_sub(empty_len))
        })
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .max()?;
    let mut status_charge = 0;
    for status in statuses() {
        // `members: [m; n]` costs len0 + n*member_len + (n-1) separators —
        // the exact array shape, computed not sampled.
        let encoded = read_result(status, Vec::new())?;
        let charge = encoded
            .len()
            .checked_add(member_count.checked_mul(member_len)?)?
            .checked_add(member_count.saturating_sub(1))?;
        status_charge = status_charge.max(charge);
    }
    let resume = WorkbenchWorkspaceCloseResume {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        site_id: command.site_id.clone(),
        project_id: project_id.clone(),
        workspace_id: workspace_id.clone(),
        close_request_id: command.request_id.clone(),
        expected_attempt_generation: maximum_generation,
    };
    let resume_request_charge =
        serialize_workbench_request(&WorkbenchRequest::WorkspaceCloseResume(resume.clone()))
            .ok()?
            .len();
    let mut resume_response_charge = 0;
    for outcome in workspace_resume_outcomes(maximum_generation) {
        let encoded = serialize_workbench_response(&WorkbenchResponse::WorkspaceCloseResume(
            super::control::workspace_resume_result(&resume, outcome),
        ))
        .ok()?;
        resume_response_charge = resume_response_charge.max(encoded.len());
    }
    command_charge
        .checked_add(status_charge)?
        .checked_add(resume_request_charge)?
        .checked_add(resume_response_charge)?
        .checked_add(member_count.checked_mul(RETAINED_HEAP_PER_MEMBER)?)
}

fn id_max() -> String {
    "ffffffffffffffffffffffffffffffff".to_owned()
}

fn member_statuses() -> Vec<WorkbenchWorkspaceCloseMemberStatus> {
    use WorkbenchTabCloseBlockedReason as Blocked;
    use WorkbenchTabClosePhase as Phase;
    use WorkbenchTabClosePublicationReason as Publication;
    use WorkbenchTabCloseUnknownPhase as UnknownPhase;
    use WorkbenchTabCloseUnknownReason as UnknownReason;
    use WorkbenchWorkspaceCloseMemberStatus as Member;
    let mut statuses = Vec::new();
    for phase in [
        Phase::Queued,
        Phase::WaitingGrace,
        Phase::WaitingOutcome,
        Phase::Disposing,
    ] {
        statuses.push(Member::Closing { phase });
    }
    for reason in [
        Blocked::WorkerUnavailable,
        Blocked::HostRefused,
        Blocked::CleanupIncomplete,
        Blocked::InventoryIncomplete,
        Blocked::IdentityConflict,
        Blocked::ContinuationBudgetExhausted,
        Blocked::ResumeGenerationExhausted,
    ] {
        for resumable in [false, true] {
            statuses.push(Member::CleanupBlocked { reason, resumable });
        }
    }
    for phase in [
        UnknownPhase::Terminate,
        UnknownPhase::Force,
        UnknownPhase::Dispose,
    ] {
        for reason in [
            UnknownReason::TransportLost,
            UnknownReason::Deadline,
            UnknownReason::InvalidResponse,
        ] {
            statuses.push(Member::OutcomeUnknown { phase, reason });
        }
    }
    for reason in [
        Publication::TargetCleanupPending,
        Publication::Preparing,
        Publication::PreparationFailed,
        Publication::AuthorityRetired,
    ] {
        for resumable in [false, true] {
            statuses.push(Member::DisposedAwaitingPublication { reason, resumable });
        }
    }
    statuses
}

fn workspace_resume_outcomes(
    generation: WorkbenchTabCloseAttemptGeneration,
) -> impl Iterator<Item = WorkbenchWorkspaceCloseResumeOutcome> {
    use WorkbenchTabCloseResumeRejectionReason as Rejected;
    [
        WorkbenchWorkspaceCloseResumeOutcome::Resumed {
            attempt_generation: generation,
        },
        WorkbenchWorkspaceCloseResumeOutcome::Stale {
            attempt_generation: generation,
        },
        WorkbenchWorkspaceCloseResumeOutcome::Denied {},
        WorkbenchWorkspaceCloseResumeOutcome::Unavailable {
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
        .map(|reason| WorkbenchWorkspaceCloseResumeOutcome::Rejected { reason }),
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
