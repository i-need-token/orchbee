use super::*;
use crate::terminal_session::{
    state::CloseReconciliationMarker,
    workbench_projection::{project_native_standalone_tabs, validate_closed_tab},
};
use orchbee_workbench_protocol::WorkbenchSnapshotOutcome;

impl NativeSessionState {
    pub(in crate::terminal_session) fn prepare_close_membership_candidate(
        &self,
        marker: &CloseReconciliationMarker,
        outcome: WorkbenchSnapshotOutcome,
    ) -> Result<Option<PreparedMembershipCandidate>, CarrierFailure> {
        self.close
            .prepare_consumption(marker)
            .map_err(|_| CarrierFailure::Protocol)?;
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Ok(None);
        };
        let candidate = project_native_standalone_tabs(&snapshot)
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
        let candidate = validate_closed_tab(&self.cache, marker.origin().expectation(), candidate)
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
        let selected = self.desired.as_ref().map(AttachmentKey::tab_id);
        let ordinal = selected.and_then(|tab| self.cache.tab_index(tab));
        self.prepare_membership_candidate(candidate, selected, ordinal, None)
            .map(Some)
    }

    pub(in crate::terminal_session) fn stage_close_membership_cleanup(
        &mut self,
        marker: &CloseReconciliationMarker,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        self.close
            .prepare_consumption(marker)
            .map_err(|_| CarrierFailure::Protocol)?;
        if !self.invariant_holds()
            || self.pending_membership.is_some()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || prepared.preserves_selection()
            || !matches!(self.phase, SessionPhase::Ready(_))
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self.attached_key.clone().ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key() != &old {
            return Err(CarrierFailure::Protocol);
        }
        let binding = self
            .cache
            .tabs()
            .get(selected.index)
            .ok_or(CarrierFailure::Protocol)?
            .binding();
        let proof = self
            .definitive_closed_attachment(marker.origin(), &old, binding)
            .map(std::sync::Arc::new);
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Close(marker.origin().key().clone()),
            old,
            deadlines,
            installs_candidate: true,
            close_marker: Some(Box::new(marker.clone())),
            closed_proof: proof,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        self.phase = SessionPhase::ReconcilingMembership(selected);
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn prepare_close_membership_install(
        &mut self,
        marker: &CloseReconciliationMarker,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        self.prepare_membership_install_inner(prepared, desired, deadlines, None, Some(marker))
    }

    pub(in crate::terminal_session) fn validate_membership_gone(
        &self,
        cleanup: &MembershipCleanup,
        proof: &super::super::DefinitiveClosedAttachment,
    ) -> bool {
        cleanup
            .closed_proof()
            .is_some_and(|expected| expected.as_ref() == proof)
            && proof.attachment_key() == cleanup.old()
            && cleanup
                .close_marker()
                .is_some_and(|marker| marker.origin().expectation() == proof.closed_expectation())
            && self
                .pending_membership
                .as_ref()
                .is_some_and(|pending| &pending.cleanup == cleanup)
    }
}
