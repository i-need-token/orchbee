//! Bounded wait continuity for the admitted history capability; historical evidence never edits a Get sample.
use super::{
    AgentSafeInteger, BorrowedFd, Failure, FailureKind, Instant, LocalClient, ObapRequestId,
    agent_protocol_invalid, agent_unavailable_details, obap_request_id,
};
use crate::cli::results::agent_source;
use crate::local_client::{AgentExchangeError, AgentReadOperation};
use orchbee_agent_observation_protocol::{
    AgentChange, AgentChangesCursor, AgentChangesOutcome, AgentChangesRejectionReason,
    AgentChangesRequest, AgentChangesResult,
};

#[derive(Default)]
pub(crate) struct AgentJournal {
    cursor: Option<AgentChangesCursor>,
    revision: Option<AgentSafeInteger>,
    has_more: bool,
}

pub(crate) enum JournalError {
    Exchange(AgentExchangeError),
    Failure(Failure),
}

impl AgentJournal {
    pub(crate) fn has_more(&self) -> bool {
        self.has_more
    }

    pub(crate) fn observed_through(&self) -> Option<AgentSafeInteger> {
        self.revision
    }

    pub(crate) fn read(
        &mut self,
        client: &mut LocalClient,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<(ObapRequestId, Vec<AgentChange>), JournalError> {
        let request_id = obap_request_id().map_err(JournalError::Failure)?;
        let AgentChangesResult::Read { result, .. } = client
            .submit_agent_changes_before_cancellable(
                AgentChangesRequest::Read {
                    request_id: request_id.clone(),
                    cursor: self.cursor.clone(),
                    limit: AgentSafeInteger::new(100),
                },
                deadline,
                wake,
            )
            .map_err(JournalError::Exchange)?;
        self.accept(result.outcome)
            .map(|changes| (request_id.clone(), changes))
            .map_err(|failure| JournalError::Failure(failure.source(agent_source(&request_id))))
    }

    fn accept(&mut self, outcome: AgentChangesOutcome) -> Result<Vec<AgentChange>, Failure> {
        match outcome {
            AgentChangesOutcome::Current { cursor, revision } => {
                if self.revision.is_some_and(|previous| previous != revision) {
                    return Err(agent_protocol_invalid(AgentReadOperation::Wait));
                }
                self.cursor = Some(cursor);
                self.revision = Some(revision);
                self.has_more = false;
                Ok(vec![])
            }
            AgentChangesOutcome::Changes {
                changes,
                cursor,
                observed_through_revision,
                runtime_revision,
                has_more,
            } => {
                let Some(previous) = self.revision else {
                    return Err(agent_protocol_invalid(AgentReadOperation::Wait));
                };
                // Validate the whole page before accepting even its first match.
                if changes.is_empty()
                    || changes.len() > 100
                    || changes.iter().enumerate().any(|(index, change)| {
                        change.revision.value() != previous.value() + index as u64 + 1
                    })
                    || changes.last().map(|change| change.revision)
                        != Some(observed_through_revision)
                    || runtime_revision.value() < observed_through_revision.value()
                    || has_more != (runtime_revision != observed_through_revision)
                {
                    return Err(agent_protocol_invalid(AgentReadOperation::Wait));
                }
                self.cursor = Some(cursor);
                self.revision = Some(observed_through_revision);
                self.has_more = has_more;
                Ok(changes)
            }
            AgentChangesOutcome::ResetRequired { .. } => Err(Failure::new(FailureKind::Rejected)
                .details(super::agent_rejection_details(
                    AgentReadOperation::Wait,
                    "history-lost",
                ))),
            AgentChangesOutcome::Rejected { reason } => Err(Failure::new(
                if reason == AgentChangesRejectionReason::ReadDenied {
                    FailureKind::Denied
                } else {
                    FailureKind::Rejected
                },
            )
            .details(super::agent_rejection_details(
                AgentReadOperation::Changes,
                if reason == AgentChangesRejectionReason::ReadDenied {
                    "read-denied"
                } else {
                    "cursor-invalid"
                },
            ))),
            AgentChangesOutcome::Unavailable { .. } => {
                Err(Failure::new(FailureKind::ServiceUnavailable)
                    .details(agent_unavailable_details(AgentReadOperation::Wait)))
            }
        }
    }
}

#[cfg(test)]
mod tests;
