use super::{
    AttachmentKey, CarrierFailure, NativeContextTabs, NativeSessionState, OperationDeadlines,
    PreparedMembershipCandidate, SessionPhase,
};
use crate::terminal_session::mailbox::PollKey;
use orchbee_client_server_protocol::RequestId;

/// Returned only by a normally joined worker after its current ordinary Poll was correlated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct UnavailablePoll {
    pub(in crate::terminal_session) request_id: RequestId,
    pub(in crate::terminal_session) poll: PollKey,
}

/// Canonical authority and ordinal are captured before retirement discards live phase fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RetiredMembershipOrigin {
    evidence: UnavailablePoll,
    baseline: NativeContextTabs,
    ordinal: usize,
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn capture_retired_membership_origin(
        &self,
        evidence: UnavailablePoll,
    ) -> Option<RetiredMembershipOrigin> {
        let SessionPhase::Ready(ready) = &self.phase else {
            return None;
        };
        if !self.invariant_holds()
            || !self.close.permits_retired_membership()
            || self.pending_refresh.is_some()
            || self.pending_mutation.is_some()
            || self.setup_blocks_structure()
            || self.move_recovery.is_some()
            || self.rename_recovery.is_some()
            || self.rename_uncertain.is_some()
            || self.pending_membership.is_some()
            || self.rename_draft_blocks_structure()
            || self.desired.as_ref() != Some(evidence.poll.pane())
            || !self.pane_attached(evidence.poll.pane_ref())
            || ready.selected.key() != evidence.poll.pane()
        {
            return None;
        }
        Some(RetiredMembershipOrigin {
            evidence,
            baseline: self.cache.clone(),
            ordinal: ready.selected.index(),
        })
    }

    pub(in crate::terminal_session) fn retired_membership_origin_matches(
        &self,
        origin: &RetiredMembershipOrigin,
    ) -> bool {
        matches!(self.phase, SessionPhase::Retired(_))
            && self.invariant_holds()
            && self.cache == origin.baseline
            && self.desired.as_ref() == Some(origin.evidence.poll.pane())
            && self.panes.iter().all(|record| {
                !record.attached
                    && !record.projected
                    && record.staged_frame.is_none()
                    && record.accepted_frame.is_none()
            })
            && self.close.permits_retired_membership()
            && self.pending_mutation.is_none()
            && !self.setup_blocks_structure()
            && self.move_recovery.is_none()
            && self.rename_recovery.is_none()
            && self.rename_uncertain.is_none()
            && self.pending_membership.is_none()
            && self.committed_target().is_none()
    }

    pub(in crate::terminal_session) fn admit_retired_membership_refresh(
        &mut self,
        origin: &RetiredMembershipOrigin,
        generation: u64,
    ) -> Result<(), CarrierFailure> {
        if !self.retired_membership_origin_matches(origin) || self.pending_refresh.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        if generation == 0 {
            return Err(CarrierFailure::Protocol);
        }
        Ok(())
    }

    pub(in crate::terminal_session) fn prepare_retired_membership_candidate(
        &self,
        origin: &RetiredMembershipOrigin,
        candidate: NativeContextTabs,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        if !self.retired_membership_origin_matches(origin) {
            return Err(CarrierFailure::AuthorityDivergence);
        }
        let tab_id = origin.evidence.poll.pane().anchor().tab_id();
        if let Some(index) = candidate.tab_index(tab_id) {
            let panel_id = origin.evidence.poll.pane().panel();
            let before = origin
                .baseline
                .tabs()
                .get(origin.ordinal)
                .and_then(|tab| tab.panel_index(panel_id).and_then(|index| tab.panel(index)))
                .ok_or(CarrierFailure::AuthorityDivergence)?;
            let tab = &candidate.tabs()[index];
            let panel_index = tab
                .panel_index(panel_id)
                .ok_or(CarrierFailure::AuthorityDivergence)?;
            let after = tab
                .panel(panel_index)
                .ok_or(CarrierFailure::AuthorityDivergence)?;
            if before.presentation_id() == after.presentation_id()
                || before.binding() == after.binding()
            {
                return Err(CarrierFailure::AuthorityDivergence);
            }
            // Canonical membership validation below proves fresh Workspace identities and same Worker.
            let mut prepared = self.prepare_membership_candidate(
                candidate,
                Some(tab_id),
                Some(origin.ordinal),
                None,
                None,
            )?;
            prepared.retarget_selected_panel(panel_index);
            return Ok(prepared);
        }
        self.prepare_membership_candidate(candidate, None, Some(origin.ordinal), None, None)
    }

    /// The retired-attempt variant of the removal classifier: the same landing
    /// evidence, guarded by the exact retired origin — panes are already empty
    /// because the old session and worker actually settled, not because
    /// records were cleared. The permit binds the attempt generation.
    pub(in crate::terminal_session) fn prepare_retired_context_removal(
        &self,
        origin: &RetiredMembershipOrigin,
        generation: u64,
        snapshot: &orchbee_workbench_protocol::Snapshot,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        if !self.retired_membership_origin_matches(origin) || generation == 0 {
            return Err(CarrierFailure::Protocol);
        }
        if !self.context_removal_evidence_hold() {
            return Err(CarrierFailure::AuthorityDivergence);
        }
        let Some(candidate) = self.removed_context_landing(snapshot) else {
            return Err(CarrierFailure::AuthorityDivergence);
        };
        if candidate
            .tab_index(origin.evidence.poll.pane().anchor().tab_id())
            .is_some()
        {
            return Err(CarrierFailure::AuthorityDivergence);
        }
        let permit = self.mint_context_removal_permit(generation, &candidate);
        self.prepare_membership_candidate(candidate, None, Some(origin.ordinal), None, Some(permit))
    }

    pub(in crate::terminal_session) fn prepare_retired_membership_install(
        &mut self,
        origin: &RetiredMembershipOrigin,
        generation: u64,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: OperationDeadlines,
    ) -> Result<super::membership::PreparedMembershipInstall<'_>, CarrierFailure> {
        if !self.retired_membership_origin_matches(origin)
            || generation == 0
            || prepared
                .context_removal()
                .is_some_and(|permit| permit.generation() != generation)
        {
            return Err(CarrierFailure::Protocol);
        }
        self.prepare_retired_membership_install_inner(prepared, desired, deadlines, generation)
    }
}
