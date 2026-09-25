//! Bounded Windows wait continuity; historical evidence never edits a Get sample.
use super::*;
use orchbee_agent_observation_protocol::{
    AgentChange, AgentChangesCursor, AgentChangesOutcome, AgentChangesRejectionReason,
    AgentChangesRequest, AgentChangesResult,
};

#[derive(Default)]
pub(super) struct WaitJournal {
    cursor: Option<AgentChangesCursor>,
    revision: Option<AgentSafeInteger>,
    has_more: bool,
}

pub(super) enum JournalError {
    Exchange(AgentExchangeError),
    Failure(Failure),
}

type HistoricalMatch = (ObapRequestId, AgentChange, Vec<AgentWaitCondition>);

impl WaitJournal {
    pub(super) fn has_more(&self) -> bool {
        self.has_more
    }

    pub(super) fn observed_through(&self) -> Option<AgentSafeInteger> {
        self.revision
    }

    pub(super) fn read(
        &mut self,
        client: &mut LocalClient,
        session: &AgentSessionId,
        until: &[AgentWaitCondition],
        after: Option<AgentSafeInteger>,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Option<HistoricalMatch>, JournalError> {
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
        self.accept(result.outcome, session, until, after)
            .map(|matched| {
                matched.map(|(change, conditions)| (request_id.clone(), change, conditions))
            })
            .map_err(|failure| JournalError::Failure(failure.source(agent_source(&request_id))))
    }

    fn accept(
        &mut self,
        outcome: AgentChangesOutcome,
        session: &AgentSessionId,
        until: &[AgentWaitCondition],
        after: Option<AgentSafeInteger>,
    ) -> Result<Option<(AgentChange, Vec<AgentWaitCondition>)>, Failure> {
        match outcome {
            AgentChangesOutcome::Current { cursor, revision } => {
                if self.revision.is_some_and(|previous| previous != revision) {
                    return Err(agent_protocol_invalid(AgentReadOperation::Wait));
                }
                self.cursor = Some(cursor);
                self.revision = Some(revision);
                self.has_more = false;
                Ok(None)
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
                Ok(changes.into_iter().find_map(|change| {
                    if change.session.session_id != *session
                        || after.is_some_and(|fence| change.revision.value() <= fence.value())
                    {
                        return None;
                    }
                    let conditions: Vec<_> = until
                        .iter()
                        .copied()
                        .filter(|condition| {
                            status_matches(
                                *condition,
                                change.session.observation.freshness,
                                &change.session.status,
                            )
                        })
                        .collect();
                    (!conditions.is_empty()).then_some((change, conditions))
                }))
            }
            AgentChangesOutcome::ResetRequired { .. } => {
                Err(Failure::new(FailureKind::Rejected).details(
                    super::super::agent_rejection_details(AgentReadOperation::Wait, "history-lost"),
                ))
            }
            AgentChangesOutcome::Rejected { reason } => Err(Failure::new(
                if reason == AgentChangesRejectionReason::ReadDenied {
                    FailureKind::Denied
                } else {
                    FailureKind::Rejected
                },
            )
            .details(super::super::agent_rejection_details(
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
