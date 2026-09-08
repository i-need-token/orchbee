use super::{
    CarrierFailure, CommittedTabCreate, CreateOperationKey, CreateProjectionDecision,
    NativeSessionState, NativeStandaloneTabs, ReadyAttachment, SelectedTab, SessionPhase,
    SwitchSetup, WorkbenchSnapshotOutcome, project_native_standalone_tabs, validate_created_tab,
};
use crate::terminal_session::workbench_projection::ValidatedCreatedProjection;

#[derive(Debug)]
pub(in crate::terminal_session) enum PreparedCreateSnapshot {
    Invalid,
    Unavailable {
        baseline: NativeStandaloneTabs,
        committed: CommittedTabCreate,
    },
    Available {
        baseline: NativeStandaloneTabs,
        committed: CommittedTabCreate,
        projection: ValidatedCreatedProjection,
    },
}

impl NativeSessionState {
    #[cfg(test)]
    pub(in crate::terminal_session) fn settled_create_for_test(
        &self,
    ) -> Option<&CommittedTabCreate> {
        self.settled_create.as_ref()
    }
    pub(in crate::terminal_session) fn create_membership_commit(
        &self,
    ) -> Option<&super::CommittedTabCreate> {
        match &self.phase {
            SessionPhase::CreateCommittedNeedDetach(committed)
            | SessionPhase::ProjectionUnavailableBeforeCleanup(committed) => Some(committed),
            _ => None,
        }
    }

    pub(in crate::terminal_session) fn prepare_create_membership_candidate(
        &self,
        baseline: &NativeStandaloneTabs,
        committed: &CommittedTabCreate,
        projection: ValidatedCreatedProjection,
    ) -> Result<super::PreparedMembershipCandidate, CarrierFailure> {
        if self.cache != *baseline || self.create_membership_commit() != Some(committed) {
            return Err(CarrierFailure::Protocol);
        }
        self.prepare_membership_candidate(
            projection.candidate,
            Some(committed.committed_key.tab_id()),
            Some(baseline.tabs().len()),
            Some((
                committed.committed_key.tab_id(),
                committed.provenance.clone(),
            )),
        )
    }

    pub(in crate::terminal_session) fn prepare_create_snapshot(
        &self,
        operation: &CreateOperationKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> PreparedCreateSnapshot {
        let committed = match &self.phase {
            SessionPhase::CreateCommittedNeedDetach(committed)
            | SessionPhase::ProjectionUnavailableBeforeCleanup(committed)
            | SessionPhase::AwaitingCommittedProjection(committed)
            | SessionPhase::ProjectionUnavailable(committed)
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
        let Ok(candidate) = project_native_standalone_tabs(&snapshot) else {
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

    /// The existing post-detach consumer remains a separate transitional entry until reordered execution lands.
    pub(in crate::terminal_session) fn apply_prepared_create_snapshot(
        &mut self,
        prepared: PreparedCreateSnapshot,
    ) -> CreateProjectionDecision {
        let (baseline, committed, projection) = match prepared {
            PreparedCreateSnapshot::Invalid => {
                self.retire(CarrierFailure::AuthorityDivergence);
                return CreateProjectionDecision::Retire;
            }
            PreparedCreateSnapshot::Unavailable {
                baseline,
                committed,
            } => {
                if self.cache == baseline
                    && matches!(&self.phase,
                    SessionPhase::CreateCommittedNeedDetach(current)
                    | SessionPhase::ProjectionUnavailableBeforeCleanup(current) if current == &committed)
                {
                    self.phase = SessionPhase::ProjectionUnavailableBeforeCleanup(committed);
                    return CreateProjectionDecision::ProjectionUnavailable;
                }
                (baseline, committed, None)
            }
            PreparedCreateSnapshot::Available {
                baseline,
                committed,
                projection,
            } => (baseline, committed, Some(projection)),
        };
        if self.cache != baseline
            || !matches!(&self.phase, SessionPhase::AwaitingCommittedProjection(current) if current == &committed)
        {
            self.retire(CarrierFailure::Protocol);
            return CreateProjectionDecision::Retire;
        }
        let Some(ValidatedCreatedProjection {
            candidate,
            created_index: Some(created_index),
        }) = projection
        else {
            if projection.is_none() {
                self.phase = SessionPhase::ProjectionUnavailable(committed);
                return CreateProjectionDecision::ProjectionUnavailable;
            }
            self.retire(CarrierFailure::AuthorityDivergence);
            return CreateProjectionDecision::Retire;
        };
        if !self.merge_projection(candidate, Some(committed.provenance.clone())) {
            self.retire(CarrierFailure::AuthorityDivergence);
            return CreateProjectionDecision::Retire;
        }
        let selected = SelectedTab::new(committed.committed_key.clone(), created_index);
        self.desired = Some(selected.key.clone());
        if committed.provenance.attachable() {
            let target = self.attach_target(
                selected,
                committed.provenance,
                committed.operation.deadlines,
            );
            self.phase = SessionPhase::Switching(SwitchSetup {
                target: target.clone(),
                previous: None,
                mode: None,
                accepted_geometry: None,
                membership: None,
            });
            CreateProjectionDecision::Attach(target)
        } else {
            self.phase = SessionPhase::SelectedWithoutAttachment(selected);
            CreateProjectionDecision::SettledWithoutAttachment
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum CreateOriginKind {
    Ready,
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CreateAnchor {
    Ready(ReadyAttachment),
    Empty,
}

impl CreateAnchor {
    pub(super) const fn kind(&self) -> CreateOriginKind {
        match self {
            Self::Ready(_) => CreateOriginKind::Ready,
            Self::Empty => CreateOriginKind::Empty,
        }
    }
    pub(super) const fn ready(&self) -> Option<&ReadyAttachment> {
        match self {
            Self::Ready(ready) => Some(ready),
            Self::Empty => None,
        }
    }
    pub(super) fn ready_mut(&mut self) -> Option<&mut ReadyAttachment> {
        match self {
            Self::Ready(ready) => Some(ready),
            Self::Empty => None,
        }
    }
}
