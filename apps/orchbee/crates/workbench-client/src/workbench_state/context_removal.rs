//! Classifies canonical removal of the current context and mints a narrowly scoped permit for
//! installing its validated fallback location. It does not grant ordinary navigation authority
//! or bypass outstanding operation and cleanup fences.

use super::recovery_fence::RecoveryFence;
use super::{
    ClientContextTabs, ClientPhase, CreateTerminalProvenance, PreparedRefresh, WorkbenchClientState,
};
use crate::workbench_projection::{
    ClientContextLocation, preferred_workspace_location, project_native_context_tabs,
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchRevision};

/// Cross-context install authority minted only by the removal classifier: the
/// candidate may land on a navigation location the session never selected
/// because its current context was deleted from canonical membership. It
/// travels as a bounded private field of `PreparedMembershipCandidate`, is
/// bound to the flight generation that minted it, and is spent exactly once
/// by the install it authorizes — never ambient Client state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContextRemovalPermit {
    generation: u64,
    removed: ClientContextLocation,
    landing: ClientContextLocation,
    revision: WorkbenchRevision,
}

impl ContextRemovalPermit {
    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) const fn removed(&self) -> &ClientContextLocation {
        &self.removed
    }
    /// The candidate location and revision the permit was minted for — the
    /// provenance guard accepts a cross-context candidate only on this exact
    /// pair against the removed baseline.
    pub(crate) fn authorizes(
        &self,
        baseline: &ClientContextTabs,
        candidate: &ClientContextTabs,
    ) -> bool {
        baseline.location() == &self.removed
            && candidate.location() == &self.landing
            && candidate.revision() == self.revision
    }
}

impl WorkbenchClientState {
    /// The refresh reply's current context no longer projects. A peer
    /// workspace close removes the context: every navigation entry lacks its
    /// id while the preferred surviving Workspace projects — that shape mints the
    /// permit the landing candidate carries. Any other projection failure is
    /// plain divergence and retires without touching canonical state.
    pub(super) fn prepare_context_removal(
        &self,
        generation: u64,
        snapshot: &Snapshot,
    ) -> PreparedRefresh {
        let Some(candidate) = self.removed_context_landing(snapshot) else {
            return PreparedRefresh::Divergent { generation };
        };
        if !self.context_removal_guards_hold() {
            return PreparedRefresh::Divergent { generation };
        }
        let permit = self.mint_context_removal_permit(generation, &candidate);
        match self.prepare_membership_candidate(candidate, None, None, None, Some(permit)) {
            Ok(prepared) => PreparedRefresh::Membership {
                generation,
                prepared,
            },
            Err(_) => PreparedRefresh::Divergent { generation },
        }
    }

    /// Shared evidence core for both entries: envelope checks, the current
    /// context's id absent from every canonical navigation owner, and the preferred
    /// surviving Workspace projecting cleanly. No remaining Workspace yields an identity-free landing.
    pub(super) fn removed_context_landing(&self, snapshot: &Snapshot) -> Option<ClientContextTabs> {
        if &snapshot.session_id != self.cache.session_id()
            || snapshot.revision < self.cache.revision()
        {
            return None;
        }
        if self.cache.terminal_context_id().is_some_and(|removed_id| {
            snapshot
                .navigation
                .workspaces
                .iter()
                .any(|workspace| &workspace.terminal_context_id == removed_id)
        }) {
            return None;
        }
        let landing = preferred_workspace_location(snapshot).ok()?;
        project_native_context_tabs(snapshot, &landing).ok()
    }

    /// Every structural guard must be quiet before the permit mints: no pending
    /// mutation, membership, setup, recovery or Close flight owns the session,
    /// and the removed context's own creation evidence is fully settled —
    /// uncertain provenance is preserved by retiring, never erased to land.
    /// The retired entry adds its own phase discipline on top.
    pub(super) fn context_removal_evidence_hold(&self) -> bool {
        self.invariant_holds()
            && self.pending_mutation.is_none()
            && !self.setup_blocks_structure()
            && !self.workspace_close_blocks_membership()
            && self.pending_membership.is_none()
            && !self.recovery_fences(RecoveryFence::Removal)
            && !self.rename_draft_blocks_structure()
            && self.close.permits_retired_membership()
            && self.panel_close.confirmation().is_none()
            && self.committed_target().is_none()
            && self.provenance.iter().all(|provenance| {
                !matches!(
                    provenance,
                    CreateTerminalProvenance::Pending | CreateTerminalProvenance::OutcomeUnknown
                )
            })
    }

    /// The ordinary entry additionally requires a phase that admits a
    /// membership install directly or through the staged cleanup.
    fn context_removal_guards_hold(&self) -> bool {
        self.context_removal_evidence_hold()
            && matches!(
                self.phase,
                ClientPhase::Empty
                    | ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
    }

    /// Mint the bounded private permit the landing candidate carries.
    pub(super) fn mint_context_removal_permit(
        &self,
        generation: u64,
        candidate: &ClientContextTabs,
    ) -> ContextRemovalPermit {
        ContextRemovalPermit {
            generation,
            removed: self.cache.location().clone(),
            landing: candidate.location().clone(),
            revision: candidate.revision(),
        }
    }
}
