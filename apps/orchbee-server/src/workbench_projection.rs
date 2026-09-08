use std::fmt;

use orchbee_workbench_protocol::{
    ContractError, Delta, DeltaChange, MAXIMUM_DELTA_CHANGES, Snapshot, WorkbenchProtocolDocument,
    apply_workbench_delta, parse_snapshot, serialize_document,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPublication {
    Delta(Delta),
    Snapshot(Snapshot),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectionAdvanceError {
    Contract(ContractError),
    SiteMismatch,
    NonIncreasingRevision,
    PublicationInvariant,
}

impl fmt::Display for ProjectionAdvanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => error.fmt(formatter),
            Self::SiteMismatch => formatter.write_str("Workbench projection Site mismatch"),
            Self::NonIncreasingRevision => {
                formatter.write_str("Workbench projection revision must increase")
            }
            Self::PublicationInvariant => {
                formatter.write_str("Workbench publication invariant failed")
            }
        }
    }
}

impl std::error::Error for ProjectionAdvanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::SiteMismatch | Self::NonIncreasingRevision | Self::PublicationInvariant => None,
        }
    }
}

impl From<ContractError> for ProjectionAdvanceError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchProjectionStore {
    snapshot: Snapshot,
}

pub(crate) struct PreparedProjectionAdvance {
    candidate: Snapshot,
    publication: WorkbenchPublication,
}

impl WorkbenchProjectionStore {
    pub(crate) fn load(encoded_snapshot: &[u8]) -> Result<Self, ContractError> {
        Ok(Self {
            snapshot: parse_snapshot(encoded_snapshot)?,
        })
    }

    pub(crate) fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    #[cfg(test)]
    pub(crate) fn advance_projection(
        &mut self,
        encoded_candidate: &[u8],
    ) -> Result<WorkbenchPublication, ProjectionAdvanceError> {
        let prepared = self.prepare_advance(encoded_candidate)?;
        Ok(self.commit_prepared(prepared))
    }

    pub(crate) fn prepare_advance(
        &self,
        encoded_candidate: &[u8],
    ) -> Result<PreparedProjectionAdvance, ProjectionAdvanceError> {
        let candidate = parse_snapshot(encoded_candidate)?;
        if candidate.site_id != self.snapshot.site_id {
            return Err(ProjectionAdvanceError::SiteMismatch);
        }
        if candidate.revision <= self.snapshot.revision {
            return Err(ProjectionAdvanceError::NonIncreasingRevision);
        }

        let changes = derive_changes(&self.snapshot, &candidate);
        let publication = if changes.len() > MAXIMUM_DELTA_CHANGES {
            preflight_snapshot_publication(&candidate)?
        } else {
            let delta = Delta {
                base_revision: self.snapshot.revision,
                changes,
                protocol_version: candidate.protocol_version,
                revision: candidate.revision,
                site_id: candidate.site_id.clone(),
            };
            match serialize_document(&WorkbenchProtocolDocument::Delta(delta.clone())) {
                Ok(_encoded_delta) => {
                    let applied = apply_workbench_delta(&self.snapshot, &delta)
                        .map_err(|_reason| ProjectionAdvanceError::PublicationInvariant)?;
                    if applied != candidate {
                        return Err(ProjectionAdvanceError::PublicationInvariant);
                    }
                    WorkbenchPublication::Delta(delta)
                }
                Err(ContractError::EncodedValueTooLarge {
                    actual: _actual,
                    maximum: _maximum,
                }) => preflight_snapshot_publication(&candidate)?,
                Err(ContractError::InvalidDocument | ContractError::WrongDocumentKind) => {
                    return Err(ProjectionAdvanceError::PublicationInvariant);
                }
            }
        };

        Ok(PreparedProjectionAdvance {
            candidate,
            publication,
        })
    }

    pub(crate) fn commit_prepared(
        &mut self,
        prepared: PreparedProjectionAdvance,
    ) -> WorkbenchPublication {
        self.snapshot = prepared.candidate;
        prepared.publication
    }
}

fn derive_changes(current: &Snapshot, candidate: &Snapshot) -> Vec<DeltaChange> {
    let mut changes = Vec::new();
    if current.navigation != candidate.navigation {
        changes.push(DeltaChange::NavigationReplaced {
            navigation: candidate.navigation.clone(),
        });
    }

    let mut current_index = 0;
    let mut candidate_index = 0;
    while current_index < current.terminal_contexts.len()
        && candidate_index < candidate.terminal_contexts.len()
    {
        let current_context = &current.terminal_contexts[current_index];
        let candidate_context = &candidate.terminal_contexts[candidate_index];
        match current_context
            .terminal_context_id
            .cmp(&candidate_context.terminal_context_id)
        {
            std::cmp::Ordering::Less => {
                changes.push(DeltaChange::TerminalContextRemoved {
                    terminal_context_id: current_context.terminal_context_id.clone(),
                });
                current_index += 1;
            }
            std::cmp::Ordering::Equal => {
                if current_context != candidate_context {
                    changes.push(DeltaChange::TerminalContextUpserted {
                        terminal_context: candidate_context.clone(),
                    });
                }
                current_index += 1;
                candidate_index += 1;
            }
            std::cmp::Ordering::Greater => {
                changes.push(DeltaChange::TerminalContextUpserted {
                    terminal_context: candidate_context.clone(),
                });
                candidate_index += 1;
            }
        }
    }
    for current_context in &current.terminal_contexts[current_index..] {
        changes.push(DeltaChange::TerminalContextRemoved {
            terminal_context_id: current_context.terminal_context_id.clone(),
        });
    }
    for candidate_context in &candidate.terminal_contexts[candidate_index..] {
        changes.push(DeltaChange::TerminalContextUpserted {
            terminal_context: candidate_context.clone(),
        });
    }
    changes
}

fn preflight_snapshot_publication(
    snapshot: &Snapshot,
) -> Result<WorkbenchPublication, ProjectionAdvanceError> {
    serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
        .map_err(|_error| ProjectionAdvanceError::PublicationInvariant)?;
    Ok(WorkbenchPublication::Snapshot(snapshot.clone()))
}

#[cfg(test)]
mod tests;
