use std::{collections::VecDeque, fmt};

use orchbee_workbench_protocol::{
    ContractError, Delta, DeltaChange, MAXIMUM_DELTA_CHANGES, MAXIMUM_RESULT_PUBLICATIONS,
    Snapshot, WorkbenchProtocolDocument, WorkbenchRevision, apply_workbench_delta, parse_snapshot,
    serialize_document,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPublication {
    Delta(Delta),
    Snapshot(Snapshot),
}

impl WorkbenchPublication {
    fn revision(&self) -> WorkbenchRevision {
        match self {
            Self::Delta(delta) => delta.revision,
            Self::Snapshot(snapshot) => snapshot.revision,
        }
    }
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
            Self::SiteMismatch => formatter.write_str("Workbench projection Session mismatch"),
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

#[derive(Clone, Debug)]
pub(crate) struct WorkbenchProjectionStore {
    events: crate::semantic_events::SiteEventJournal,
    graphics: std::sync::Arc<std::sync::Mutex<crate::PaneGraphicsAuthority>>,
    snapshot: Snapshot,
    /// Bounded journal of committed publications, in revision order. A retained Snapshot entry is a
    /// replay boundary: everything committed before it is subsumed, so it is stored alone at the
    /// front of the post-commit history.
    publications: VecDeque<WorkbenchPublication>,
}

pub(crate) struct PreparedProjectionAdvance {
    candidate: Snapshot,
    publication: WorkbenchPublication,
}

impl WorkbenchProjectionStore {
    pub(crate) fn load(encoded_snapshot: &[u8]) -> Result<Self, ContractError> {
        let snapshot = parse_snapshot(encoded_snapshot)?;
        Ok(Self {
            events: crate::semantic_events::SiteEventJournal::new(&snapshot)?,
            graphics: std::sync::Arc::default(),
            snapshot,
            publications: VecDeque::new(),
        })
    }

    pub(crate) fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub(crate) fn graphics(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<crate::PaneGraphicsAuthority>> {
        self.graphics.clone()
    }
    pub(crate) fn events(&self) -> crate::semantic_events::SiteEventJournal {
        self.events.clone()
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
        let before = std::mem::replace(&mut self.snapshot, prepared.candidate);
        // Canonical owner always precedes the layer lock. Retirement cannot await a future graphics read.
        let _ = self
            .graphics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(&self.snapshot);
        self.events.projection_committed(&before, &self.snapshot);
        if matches!(prepared.publication, WorkbenchPublication::Snapshot(_)) {
            self.publications.clear();
        }
        self.publications.push_back(prepared.publication.clone());
        while self.publications.len() > MAXIMUM_RESULT_PUBLICATIONS {
            self.publications.pop_front();
        }
        prepared.publication
    }

    /// Returns the committed publications a reader at `known_revision` must apply to reach the
    /// current revision. An empty batch means the reader is already current. When the retained
    /// journal can no longer connect `known_revision` to the head — because entries were evicted,
    /// the revision was never committed, or it is ahead of the store — the batch is a single
    /// current Snapshot that replaces the reader's projection outright.
    pub(crate) fn publications_since(
        &self,
        known_revision: WorkbenchRevision,
    ) -> Vec<WorkbenchPublication> {
        if known_revision == self.snapshot.revision {
            return Vec::new();
        }
        let Some(start) = self
            .publications
            .iter()
            .position(|entry| entry.revision() > known_revision)
        else {
            return vec![WorkbenchPublication::Snapshot(self.snapshot.clone())];
        };
        let connected = match &self.publications[start] {
            WorkbenchPublication::Delta(delta) => delta.base_revision == known_revision,
            WorkbenchPublication::Snapshot(_) => true,
        };
        if connected {
            self.publications.iter().skip(start).cloned().collect()
        } else {
            vec![WorkbenchPublication::Snapshot(self.snapshot.clone())]
        }
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
