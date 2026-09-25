//! Read-only adapter over the canonical journal, after admitted drain preparation.

use super::{AgentReadFault, AgentService, PreparedUnavailable, map};
use crate::agent::{AgentChangeReadInput, AgentRuntimeChangeReadResult, AgentRuntimeSnapshot};
use orchbee_agent_observation_protocol::{
    AgentChangesOutcome as Outcome, AgentChangesOutcomeEnvelope,
    AgentChangesRejectionReason as Rejection, AgentChangesRequest, AgentChangesResult,
    AgentUnavailableReason,
};

fn unavailable() -> Outcome {
    Outcome::Unavailable {
        reason: AgentUnavailableReason::RuntimeUnavailable,
    }
}

impl AgentService {
    pub(crate) fn execute_changes_prepared(
        &self,
        request: AgentChangesRequest,
        prepare: impl FnOnce() -> Result<AgentRuntimeSnapshot, PreparedUnavailable>,
    ) -> Result<AgentChangesResult, AgentReadFault> {
        let AgentChangesRequest::Read {
            request_id,
            cursor,
            limit,
        } = request;
        let outcome = if !self.policy.read {
            Outcome::Rejected {
                reason: Rejection::ReadDenied,
            }
        } else if std::panic::catch_unwind(std::panic::AssertUnwindSafe(prepare))
            .unwrap_or(Err(PreparedUnavailable))
            .is_err()
        {
            unavailable()
        } else {
            self.with_state(|state| {
                if state.runtime.is_disposed() {
                    return Ok(unavailable());
                }
                let head = state.runtime.snapshot().revision;
                let Some(cursor) = cursor else {
                    return Ok(Outcome::Current {
                        cursor: self.cursors.mint_changes(head),
                        revision: map::revision(head)?,
                    });
                };
                let Ok(after) = self.cursors.verify_changes(&cursor, head) else {
                    return Ok(Outcome::Rejected {
                        reason: Rejection::CursorInvalid,
                    });
                };
                let result = state
                    .runtime
                    .read_changes(AgentChangeReadInput {
                        after_revision: after as f64,
                        limit: limit.value() as f64,
                    })
                    .map_err(|_| {
                        AgentReadFault("admitted change request violates canonical bounds")
                    })?;
                Ok(match result {
                    AgentRuntimeChangeReadResult::Changes {
                        changes,
                        has_more,
                        observed_through_revision,
                        runtime_revision,
                    } => Outcome::Changes {
                        changes: changes
                            .iter()
                            .map(map::runtime_change)
                            .collect::<Result<_, _>>()?,
                        cursor: self.cursors.mint_changes(observed_through_revision),
                        observed_through_revision: map::revision(observed_through_revision)?,
                        runtime_revision: map::revision(runtime_revision)?,
                        has_more,
                    },
                    AgentRuntimeChangeReadResult::Current {
                        observed_through_revision,
                        runtime_revision,
                    } => {
                        if observed_through_revision != runtime_revision {
                            return Err(AgentReadFault(
                                "canonical current change revision mismatch",
                            ));
                        }
                        Outcome::Current {
                            cursor: self.cursors.mint_changes(runtime_revision),
                            revision: map::revision(runtime_revision)?,
                        }
                    }
                    AgentRuntimeChangeReadResult::ResetRequired {
                        observed_through_revision,
                        runtime_revision,
                    } => {
                        if observed_through_revision != runtime_revision {
                            return Err(AgentReadFault("canonical reset change revision mismatch"));
                        }
                        Outcome::ResetRequired {
                            cursor: self.cursors.mint_changes(runtime_revision),
                            revision: map::revision(runtime_revision)?,
                        }
                    }
                    AgentRuntimeChangeReadResult::RevisionAhead {
                        observed_through_revision,
                        runtime_revision,
                    } => {
                        if observed_through_revision <= runtime_revision {
                            return Err(AgentReadFault("canonical ahead change revision mismatch"));
                        }
                        Outcome::Rejected {
                            reason: Rejection::CursorInvalid,
                        }
                    }
                })
            })
            .unwrap_or_else(|_| Ok(unavailable()))?
        };
        Ok(AgentChangesResult::Read {
            request_id,
            result: AgentChangesOutcomeEnvelope { outcome },
        })
    }
}

#[cfg(test)]
mod tests;
