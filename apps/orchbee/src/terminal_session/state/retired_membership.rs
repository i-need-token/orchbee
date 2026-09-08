use super::{
    AttachmentKey, CarrierFailure, NativeSessionState, NativeStandaloneTabs, OperationDeadlines,
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
    baseline: NativeStandaloneTabs,
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
            || self.pending_refresh_generation.is_some()
            || self.pending_mutation.is_some()
            || self.move_recovery.is_some()
            || self.rename_recovery.is_some()
            || self.rename_uncertain.is_some()
            || self.pending_membership.is_some()
            || self.rename_draft_blocks_structure()
            || self.desired.as_ref() != Some(evidence.poll.attachment())
            || self.attached_key.as_ref() != Some(evidence.poll.attachment())
            || ready.selected.key() != evidence.poll.attachment()
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
            && self.desired.as_ref() == Some(origin.evidence.poll.attachment())
            && self.attached_key.is_none()
            && self.projection_key.is_none()
            && self.staged_frame.is_none()
            && self.accepted_frame.is_none()
            && self.close.permits_retired_membership()
            && self.pending_mutation.is_none()
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
        if !self.retired_membership_origin_matches(origin)
            || self.pending_refresh_generation.is_some()
        {
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
        candidate: NativeStandaloneTabs,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        if !self.retired_membership_origin_matches(origin)
            || candidate
                .tab_index(origin.evidence.poll.attachment().tab_id())
                .is_some()
        {
            return Err(CarrierFailure::AuthorityDivergence);
        }
        self.prepare_membership_candidate(candidate, None, Some(origin.ordinal), None)
    }

    pub(in crate::terminal_session) fn prepare_retired_membership_install(
        &mut self,
        origin: &RetiredMembershipOrigin,
        generation: u64,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: OperationDeadlines,
    ) -> Result<super::membership::PreparedMembershipInstall<'_>, CarrierFailure> {
        if !self.retired_membership_origin_matches(origin) || generation == 0 {
            return Err(CarrierFailure::Protocol);
        }
        self.prepare_retired_membership_install_inner(prepared, desired, deadlines, generation)
    }
}
