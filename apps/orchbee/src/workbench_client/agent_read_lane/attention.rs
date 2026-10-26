//! A bounded alignment of canonical history and current attachment metadata.
use super::{
    AgentSafeInteger, AgentSessionSummary, Arc, AtomicBool, Instant, LaneFailure, LocalClient,
    LocalClientOrigin, LocalOperationClock, MAXIMUM_LIST_LIMIT, Ordering, list_round,
    mint_request_id,
};
use orchbee_agent_observation_protocol::{
    AgentChange, AgentChangesCursor, AgentChangesOutcome, AgentChangesRejectionReason,
    AgentChangesRequest, AgentChangesResult,
};

const MAXIMUM_ROUND_STEPS: usize = 12;
const MAXIMUM_CHANGES: usize = 512;

pub(in crate::workbench_client) struct AttentionRound {
    pub(in crate::workbench_client) items: Vec<AgentSessionSummary>,
    pub(in crate::workbench_client) changes: Vec<AgentChange>,
    pub(in crate::workbench_client) prior_revision: Option<u64>,
    pub(in crate::workbench_client) revision: u64,
    pub(in crate::workbench_client) cursor: AgentChangesCursor,
    pub(in crate::workbench_client) reset: bool,
}

#[expect(clippy::too_many_arguments)]
pub(super) fn read_round(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    client: &mut Option<LocalClient>,
    mut cursor: Option<AgentChangesCursor>,
    mut prior_revision: Option<u64>,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
    shutdown: &Arc<AtomicBool>,
) -> Result<AttentionRound, LaneFailure> {
    if cursor.is_some() != prior_revision.is_some() {
        return Err(LaneFailure::Transport);
    }
    let mut head = prior_revision;
    let mut changes = Vec::new();
    let mut reset = false;
    for _ in 0..MAXIMUM_ROUND_STEPS {
        if shutdown.load(Ordering::SeqCst) || clock.now() >= deadline {
            return Err(LaneFailure::Transport);
        }
        let result = submit_changes(origin, clock, client, cursor.take(), deadline, wake)?;
        let AgentChangesResult::Read { result, .. } = result;
        match result.outcome {
            AgentChangesOutcome::Current {
                cursor: next,
                revision,
            } => {
                if head.is_some_and(|head| head != revision.value()) {
                    return Err(LaneFailure::Transport);
                }
                head = Some(revision.value());
                cursor = Some(next);
            }
            AgentChangesOutcome::Changes {
                changes: page,
                cursor: next,
                observed_through_revision,
                has_more,
                ..
            } => {
                if page.first().map(|first| first.revision.value())
                    != head.and_then(|head| head.checked_add(1))
                    || changes.len() + page.len() > MAXIMUM_CHANGES
                {
                    return Err(LaneFailure::Transport);
                }
                changes.extend(page);
                head = Some(observed_through_revision.value());
                cursor = Some(next);
                if has_more {
                    continue;
                }
            }
            AgentChangesOutcome::ResetRequired {
                cursor: next,
                revision,
            } => {
                reset = true;
                changes.clear();
                prior_revision = None;
                head = Some(revision.value());
                cursor = Some(next);
            }
            AgentChangesOutcome::Rejected {
                reason: AgentChangesRejectionReason::CursorInvalid,
            } => {
                reset = true;
                changes.clear();
                prior_revision = None;
                head = None;
                cursor = None;
                continue;
            }
            AgentChangesOutcome::Rejected { .. } => return Err(LaneFailure::Denied),
            AgentChangesOutcome::Unavailable { .. } => return Err(LaneFailure::Unavailable),
        }
        let listed = list_round(origin, clock, client, deadline, wake, shutdown)?;
        if !listed.complete || listed.revision.is_none() || listed.revision < head {
            return Err(LaneFailure::Transport);
        }
        if listed.revision == head {
            return Ok(AttentionRound {
                items: listed.items,
                changes,
                prior_revision,
                revision: head.ok_or(LaneFailure::Transport)?,
                cursor: cursor.ok_or(LaneFailure::Transport)?,
                reset,
            });
        }
        // The list advanced after the journal sample. Read the missing
        // transitions, then obtain metadata at that same head; never splice.
    }
    Err(LaneFailure::Transport)
}

fn submit_changes(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    slot: &mut Option<LocalClient>,
    cursor: Option<AgentChangesCursor>,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
) -> Result<AgentChangesResult, LaneFailure> {
    if slot.is_none() {
        *slot = Some(
            origin
                .reconnect_before_cancellable(deadline, clock.clone(), Some(wake))
                .map_err(|_| LaneFailure::Transport)?,
        );
    }
    let client = slot.as_mut().ok_or(LaneFailure::Transport)?;
    let result = client.submit_agent_changes_before_cancellable(
        AgentChangesRequest::Read {
            request_id: mint_request_id()?,
            cursor,
            limit: AgentSafeInteger::new(MAXIMUM_LIST_LIMIT),
        },
        deadline,
        Some(wake),
    );
    match result {
        Ok(result) => Ok(result),
        Err(_) => {
            *slot = None;
            Err(LaneFailure::Transport)
        }
    }
}

#[cfg(test)]
mod tests;
