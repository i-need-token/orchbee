//! Validates known creation receipts against canonical membership and prepares the exact
//! projection and landing selection for installation. It does not launch a terminal or accept
//! a receipt as proof that an attachment is ready.

use super::{
    CarrierFailure, ClientContextTabs, ClientPhase, CommittedTabCreate, CreateOperationKey,
    CreateProjectionDecision, ReadyAttachment, SelectedTab, WorkbenchClientState,
    WorkbenchSnapshotOutcome, validate_created_tab,
};
use crate::workbench_projection::ValidatedCreatedProjection;

/// Safe creation facts for failure feedback; arguments and environment are never retained here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LaunchDescription {
    pub(crate) profile_name: Option<String>,
    pub(crate) executable: Option<String>,
}

#[derive(Debug)]
pub(crate) enum PreparedCreateSnapshot {
    Invalid,
    Unavailable {
        baseline: ClientContextTabs,
        committed: CommittedTabCreate,
    },
    Available {
        baseline: ClientContextTabs,
        committed: CommittedTabCreate,
        projection: ValidatedCreatedProjection,
    },
}

impl WorkbenchClientState {
    #[cfg(test)]
    pub(crate) fn settled_create_for_test(&self) -> Option<&CommittedTabCreate> {
        self.settled_create.as_ref()
    }
    pub(crate) fn create_membership_commit(&self) -> Option<&super::CommittedTabCreate> {
        match &self.phase {
            ClientPhase::CreateCommittedNeedDetach(committed)
            | ClientPhase::ProjectionUnavailableBeforeCleanup(committed) => Some(committed),
            _ => None,
        }
    }

    pub(crate) fn prepare_create_membership_candidate(
        &self,
        baseline: &ClientContextTabs,
        committed: &CommittedTabCreate,
        projection: ValidatedCreatedProjection,
    ) -> Result<super::PreparedMembershipCandidate, CarrierFailure> {
        if self.cache != *baseline || self.create_membership_commit() != Some(committed) {
            return Err(CarrierFailure::Protocol);
        }
        let ready = committed
            .anchor
            .ready()
            .filter(|_| committed.operation.preserves_selection());
        let preferred = ready.map_or(committed.committed_key.tab_id(), |ready| {
            ready.selected().key().anchor().tab_id()
        });
        let ordinal = ready.map_or(baseline.tabs().len(), |ready| ready.selected().index());
        self.prepare_membership_candidate(
            projection.candidate,
            Some(preferred),
            Some(ordinal),
            Some((
                committed.committed_key.tab_id(),
                committed.provenance.clone(),
            )),
            None,
        )
    }

    pub(crate) fn prepare_create_snapshot(
        &self,
        operation: &CreateOperationKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> PreparedCreateSnapshot {
        let committed = match &self.phase {
            ClientPhase::CreateCommittedNeedDetach(committed)
            | ClientPhase::ProjectionUnavailableBeforeCleanup(committed)
                if committed.operation.key == *operation =>
            {
                committed.clone()
            }
            _ => return PreparedCreateSnapshot::Invalid,
        };
        if !self.invariant_holds() {
            return PreparedCreateSnapshot::Invalid;
        }
        let baseline = self.cache.clone();
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return PreparedCreateSnapshot::Unavailable {
                baseline,
                committed,
            };
        };
        let Ok(candidate) = self.cache.project_snapshot(&snapshot) else {
            return PreparedCreateSnapshot::Invalid;
        };
        let Ok(projection) = validate_created_tab(&baseline, &committed.expectation, candidate)
        else {
            return PreparedCreateSnapshot::Invalid;
        };
        PreparedCreateSnapshot::Available {
            baseline,
            committed,
            projection,
        }
    }

    /// Only definite failure reaches this consumer; Available uses the shared membership owner.
    pub(crate) fn apply_prepared_create_snapshot(
        &mut self,
        prepared: PreparedCreateSnapshot,
    ) -> CreateProjectionDecision {
        match prepared {
            PreparedCreateSnapshot::Invalid => {
                self.retire(CarrierFailure::AuthorityDivergence);
                CreateProjectionDecision::Retire
            }
            PreparedCreateSnapshot::Unavailable {
                baseline,
                committed,
            } if self.cache == baseline && self.create_membership_commit() == Some(&committed) => {
                self.phase = ClientPhase::ProjectionUnavailableBeforeCleanup(committed);
                CreateProjectionDecision::ProjectionUnavailable
            }
            PreparedCreateSnapshot::Unavailable { .. }
            | PreparedCreateSnapshot::Available { .. } => {
                self.retire(CarrierFailure::Protocol);
                CreateProjectionDecision::Retire
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreateOriginKind {
    Ready,
    Empty,
    Unattached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CreateAnchor {
    Ready(ReadyAttachment),
    Empty,
    Unattached {
        selected: SelectedTab,
        retryable: bool,
    },
}

impl CreateAnchor {
    pub(super) const fn kind(&self) -> CreateOriginKind {
        match self {
            Self::Ready(_) => CreateOriginKind::Ready,
            Self::Empty => CreateOriginKind::Empty,
            Self::Unattached { .. } => CreateOriginKind::Unattached,
        }
    }
    pub(super) const fn ready(&self) -> Option<&ReadyAttachment> {
        match self {
            Self::Ready(ready) => Some(ready),
            Self::Empty | Self::Unattached { .. } => None,
        }
    }

    pub(super) const fn selected(&self) -> Option<&SelectedTab> {
        match self {
            Self::Ready(ready) => Some(&ready.selected),
            Self::Unattached { selected, .. } => Some(selected),
            Self::Empty => None,
        }
    }
}
