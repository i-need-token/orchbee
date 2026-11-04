//! Computes checked wire and retained-heap charges for complete close records and their
//! possible receipts before admission. It reserves no memory or ledger entry itself and does
//! not advance close state.

use std::mem::size_of;

use orchbee_capability_node_protocol::TerminalLifecycleRequest;
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabClosePhase, WorkbenchTabClosePublicationReason, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceCloseCounts, WorkbenchWorkspaceCloseMember,
    WorkbenchWorkspaceCloseMemberStatus, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeOutcome, WorkbenchWorkspaceCloseResumeResult,
    WorkbenchWorkspaceId, serialize_workbench_request, serialize_workbench_response,
};

use super::super::{RetainedRequest, ledger_size};
use super::continuation::CloseContinuation;
use super::{AcceptedResumeReceipt, CloseRecord, CloseScope, WorkbenchTabCloseTarget};

/// Two receipt payload slots are reserved per record: the retained receipt
/// and the freshly constructed candidate coexist inside one resume commit
/// window before the old value is replaced.
const RECEIPT_SLOTS: usize = 2;

/// The wire-budget cells of one primary reservation — command plus
/// acknowledgment, the worst legal status document, and the worst legal
/// resume request plus result — maximized within each bounded cell.
/// `member_count` is the frozen captured membership — only the Context
/// scope's member-dependent status wire consumes it. Retained heap is a
/// separate, independently checked budget measured off the real record.
pub(super) fn maximum_wire_charge(
    command: &WorkbenchCommand,
    acknowledgment: &WorkbenchCommandResult,
    member_count: usize,
) -> Option<usize> {
    if matches!(
        &command.action,
        WorkbenchAction::WorkspaceClose { .. } | WorkbenchAction::WorkspaceCloseIfIdle { .. }
    ) {
        return maximum_workspace_charge(command, acknowledgment, member_count);
    }
    let (terminal_context_id, tab_id) = match &command.action {
        WorkbenchAction::TabClose {
            terminal_context_id,
            tab_id,
        }
        | WorkbenchAction::TabCloseIfIdle {
            terminal_context_id,
            tab_id,
        }
        | WorkbenchAction::PanelClose {
            terminal_context_id,
            tab_id,
            ..
        }
        | WorkbenchAction::PanelCloseIfIdle {
            terminal_context_id,
            tab_id,
            ..
        } => (terminal_context_id, tab_id),
        // Close retention accounting applies only to admitted Close commands and their acceptance receipts.
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
        // Close retention accounting applies only to admitted Close commands and their acceptance receipts.
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
        session_id: command.session_id.clone(),
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

/// The complete ledger reservation for one admitted Close: the exact wire
/// cells, the ledger key's inline `WorkbenchRequestId` head plus its owned
/// buffer, the `RetainedRequest` inline enum cell holding the record, the
/// committed record's actual controlled heap, and both receipt payload
/// slots — one checked sum, nothing sampled. The key head and enum cell are
/// known-type payloads inside the `BTreeMap` node and are charged here;
/// only the node's unknown structural overhead stays inside the exclusion.
pub(super) fn entry_reservation(
    command: &WorkbenchCommand,
    acknowledgment: &WorkbenchCommandResult,
    member_count: usize,
    record: &CloseRecord,
    ledger_key: &WorkbenchRequestId,
) -> Option<usize> {
    maximum_wire_charge(command, acknowledgment, member_count)?
        .checked_add(size_of::<WorkbenchRequestId>())?
        .checked_add(ledger_key.owned_capacity())?
        .checked_add(size_of::<RetainedRequest>())?
        .checked_add(record_controlled_heap(record)?)?
        .checked_add(RECEIPT_SLOTS.checked_mul(receipt_slot_bytes(&record.scope)?)?)
}

/// The record's actual controlled-payload heap: the boxed record body —
/// which already covers every inline field — plus every owned `String`
/// capacity, both `Vec` capacities at element size, one future
/// `Arc<TerminalLifecycleRequest>` payload per member, and the stored
/// receipt's owned payload buffers. `Arc` headers, `BTreeMap` internals,
/// and allocator metadata are outside this budget by contract.
pub(super) fn record_controlled_heap(record: &CloseRecord) -> Option<usize> {
    let mut charge = size_of::<CloseRecord>();
    charge = charge.checked_add(command_owned_bytes(&record.command)?)?;
    charge = charge.checked_add(result_owned_bytes(&record.result)?)?;
    if let CloseScope::Context { workspace_id } = &record.scope {
        charge = charge.checked_add(workspace_id.owned_capacity())?;
    }
    charge = charge.checked_add(record.terminal_context_id.owned_capacity())?;
    for target in &record.targets {
        charge = charge.checked_add(target_owned_bytes(target)?)?;
    }
    charge = charge.checked_add(
        record
            .targets
            .capacity()
            .checked_mul(size_of::<WorkbenchTabCloseTarget>())?,
    )?;
    charge = charge.checked_add(
        record
            .continuations
            .capacity()
            .checked_mul(size_of::<CloseContinuation>())?,
    )?;
    charge = charge.checked_add(
        record
            .continuations
            .len()
            .checked_mul(size_of::<TerminalLifecycleRequest>())?,
    )?;
    if let Some(receipt) = &record.resume_receipt {
        charge = charge.checked_add(receipt_owned_bytes(receipt)?)?;
    }
    Some(charge)
}

/// Owned payload buffer bytes inside a Tab/Panel-scoped resume receipt —
/// the request and result envelopes' identity buffers only.
pub(super) fn tab_receipt_owned_bytes(
    request: &WorkbenchTabCloseResume,
    result: &WorkbenchTabCloseResumeResult,
) -> Option<usize> {
    request
        .request_id
        .owned_capacity()
        .checked_add(request.session_id.owned_capacity())?
        .checked_add(request.terminal_context_id.owned_capacity())?
        .checked_add(request.tab_id.owned_capacity())?
        .checked_add(request.close_request_id.owned_capacity())?
        .checked_add(result.request_id.owned_capacity())?
        .checked_add(result.session_id.owned_capacity())?
        .checked_add(result.terminal_context_id.owned_capacity())?
        .checked_add(result.tab_id.owned_capacity())?
        .checked_add(result.close_request_id.owned_capacity())
}

/// Owned payload buffer bytes inside a Workspace-scoped resume receipt —
/// the request and result envelopes' identity buffers only.
pub(super) fn workspace_receipt_owned_bytes(
    request: &WorkbenchWorkspaceCloseResume,
    result: &WorkbenchWorkspaceCloseResumeResult,
) -> Option<usize> {
    request
        .request_id
        .owned_capacity()
        .checked_add(request.session_id.owned_capacity())?
        .checked_add(request.workspace_id.owned_capacity())?
        .checked_add(request.close_request_id.owned_capacity())?
        .checked_add(result.request_id.owned_capacity())?
        .checked_add(result.session_id.owned_capacity())?
        .checked_add(result.workspace_id.owned_capacity())?
        .checked_add(result.close_request_id.owned_capacity())
}

/// Owned payload buffers of whichever receipt shape the record stores.
fn receipt_owned_bytes(receipt: &AcceptedResumeReceipt) -> Option<usize> {
    match receipt {
        AcceptedResumeReceipt::Tab { request, result } => tab_receipt_owned_bytes(request, result),
        AcceptedResumeReceipt::Workspace { request, result } => {
            workspace_receipt_owned_bytes(request, result)
        }
    }
}

/// One receipt payload slot: the same-shape maximum legal receipt for this
/// scope — a real template built through the same result constructor the
/// commit path uses, then measured for its actual owned buffers.
pub(super) fn receipt_slot_bytes(scope: &CloseScope) -> Option<usize> {
    let maximum_generation = WorkbenchTabCloseAttemptGeneration::new(u64::MAX).ok()?;
    match scope {
        CloseScope::Context { .. } => {
            let request = WorkbenchWorkspaceCloseResume {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&id_max()).ok()?,
                session_id: SessionId::parse(&id_max()).ok()?,
                workspace_id: WorkbenchWorkspaceId::parse(&id_max()).ok()?,
                close_request_id: WorkbenchRequestId::parse(&id_max()).ok()?,
                expected_attempt_generation: maximum_generation,
            };
            let result = super::control::workspace_resume_result(
                &request,
                WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                    attempt_generation: maximum_generation,
                },
            );
            workspace_receipt_owned_bytes(&request, &result)
        }
        CloseScope::Tab | CloseScope::Panel => {
            let request = WorkbenchTabCloseResume {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&id_max()).ok()?,
                session_id: SessionId::parse(&id_max()).ok()?,
                terminal_context_id: WorkbenchTerminalContextId::parse(&id_max()).ok()?,
                tab_id: WorkbenchTabId::parse(&id_max()).ok()?,
                close_request_id: WorkbenchRequestId::parse(&id_max()).ok()?,
                expected_attempt_generation: maximum_generation,
            };
            let result = super::control::resume_result(
                &request,
                WorkbenchTabCloseResumeOutcome::Resumed {
                    attempt_generation: maximum_generation,
                },
            );
            tab_receipt_owned_bytes(&request, &result)
        }
    }
}

fn command_owned_bytes(command: &WorkbenchCommand) -> Option<usize> {
    let action = match &command.action {
        WorkbenchAction::TabClose {
            terminal_context_id,
            tab_id,
        }
        | WorkbenchAction::TabCloseIfIdle {
            terminal_context_id,
            tab_id,
        } => terminal_context_id
            .owned_capacity()
            .checked_add(tab_id.owned_capacity()),
        WorkbenchAction::PanelClose {
            terminal_context_id,
            tab_id,
            panel_id,
        }
        | WorkbenchAction::PanelCloseIfIdle {
            terminal_context_id,
            tab_id,
            panel_id,
        } => terminal_context_id
            .owned_capacity()
            .checked_add(tab_id.owned_capacity())?
            .checked_add(panel_id.owned_capacity()),
        WorkbenchAction::WorkspaceClose { workspace_id }
        | WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } => {
            Some(workspace_id.owned_capacity())
        }
        // A retained Close record only ever carries a close action.
        _ => None,
    }?;
    command
        .request_id
        .owned_capacity()
        .checked_add(command.session_id.owned_capacity())?
        .checked_add(action)
}

fn result_owned_bytes(result: &WorkbenchCommandResult) -> Option<usize> {
    let outcome = match &result.outcome {
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id,
            terminal_context_id,
            tab_id,
            ..
        } => close_request_id
            .owned_capacity()
            .checked_add(terminal_context_id.owned_capacity())?
            .checked_add(tab_id.owned_capacity()),
        WorkbenchCommandOutcome::PanelCloseAccepted {
            close_request_id,
            terminal_context_id,
            tab_id,
            panel_id,
            ..
        } => close_request_id
            .owned_capacity()
            .checked_add(terminal_context_id.owned_capacity())?
            .checked_add(tab_id.owned_capacity())?
            .checked_add(panel_id.owned_capacity()),
        WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            close_request_id,
            workspace_id,
            terminal_context_id,
            ..
        } => close_request_id
            .owned_capacity()
            .checked_add(workspace_id.owned_capacity())?
            .checked_add(terminal_context_id.owned_capacity()),
        // A retained Close record only ever carries an accepted outcome.
        _ => None,
    }?;
    result.request_id.owned_capacity().checked_add(outcome)
}

fn target_owned_bytes(target: &WorkbenchTabCloseTarget) -> Option<usize> {
    let member = &target.member;
    let binding = member.binding();
    target
        .terminal_context_id
        .owned_capacity()
        .checked_add(member.tab_id().owned_capacity())?
        .checked_add(member.panel_id().owned_capacity())?
        .checked_add(member.presentation_id().owned_capacity())?
        .checked_add(binding.terminal_id.owned_capacity())?
        .checked_add(binding.worker_id.owned_capacity())
}

/// The Workspace Close wire charge = the same wire cells as the Tab Close
/// (command+ack, worst status document, resume request+result) plus the
/// member-dependent status-document cells the Context scope adds:
/// `member_count` member entries at their worst spelling.
fn maximum_workspace_charge(
    command: &WorkbenchCommand,
    acknowledgment: &WorkbenchCommandResult,
    member_count: usize,
) -> Option<usize> {
    let workspace_id = match &command.action {
        WorkbenchAction::WorkspaceClose { workspace_id }
        | WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } => workspace_id,
        // Close retention accounting applies only to admitted Close commands and their acceptance receipts.
        _ => return None,
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
        // Close retention accounting applies only to admitted Close commands and their acceptance receipts.
        _ => return None,
    }
    let command_charge = ledger_size(&maximum_command, &maximum_acknowledgment)?;
    let terminal_context_id = match &acknowledgment.outcome {
        WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            terminal_context_id,
            ..
        } => terminal_context_id.clone(),
        // Close retention accounting applies only to admitted Close commands and their acceptance receipts.
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
        session_id: command.session_id.clone(),
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
        .checked_add(resume_response_charge)
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
