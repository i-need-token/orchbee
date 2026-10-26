//! Legacy single-pane identity adapters for tests that do not prove physical identity.
//!
//! Capture an exact `PaneRef` before detach, rebind or retirement and call the production API
//! directly when testing rejection. Resolving an anchor after removal can return early without
//! invoking that API. Remove these adapters as their legacy callers adopt exact identities.

use super::super::switch_membership::PreparedSwitchRestore;
use super::super::*;

/// Test-only anchor comparisons: legacy single-pane tests assert against
/// `AttachmentKey` values, which name exactly the pane's anchor.
impl PartialEq<AttachmentKey> for PaneKey {
    fn eq(&self, other: &AttachmentKey) -> bool {
        self.anchor() == other
    }
}

impl PartialEq<PaneKey> for AttachmentKey {
    fn eq(&self, other: &PaneKey) -> bool {
        self == other.anchor()
    }
}

impl PartialEq<AttachmentKey> for PaneRef {
    fn eq(&self, other: &AttachmentKey) -> bool {
        self.anchor() == other
    }
}

// Weak legacy comparisons for single-pane test assertions: anchor spelling
// ignores the panel axis and the `PaneRef` comparisons ignore incarnation, so
// they can never distinguish a stale incarnation or a sibling pane. Identity
// proofs must assert on exact `PaneRef` values directly.
impl PartialEq<PaneRef> for AttachmentKey {
    fn eq(&self, other: &PaneRef) -> bool {
        self == other.anchor()
    }
}

impl PartialEq<PaneKey> for PaneRef {
    fn eq(&self, other: &PaneKey) -> bool {
        self.pane() == other
    }
}

impl PartialEq<PaneRef> for PaneKey {
    fn eq(&self, other: &PaneRef) -> bool {
        self == other.pane()
    }
}

/// Resolves any spelling of a pane identity — attachment anchor, logical pane
/// key, or exact incarnation — into the `PaneRef` the live membership minted.
/// Legacy single-pane tests keep their `AttachmentKey` fixtures and route
/// through the `_t` compatibility methods below.
///
/// WEAK CLAIM BOUNDARY: anchor and `PaneKey` spellings resolve the CURRENT
/// record at call time — an anchor ignores the panel axis entirely and a stale
/// incarnation can never be expressed this way. These adapters exist so legacy
/// single-pane assertions keep compiling; they can never prove stale-identity
/// fencing. Tests that exercise detach/rebind/stale/poll/draw/ack identity
/// must construct or capture the exact `PaneRef` before the rebind and call
/// the production methods directly, never this layer.
pub(in crate::workbench_client) trait TestPaneRef {
    fn resolve_pane_ref(&self, state: &WorkbenchClientState) -> Option<PaneRef>;
}

impl TestPaneRef for PaneRef {
    fn resolve_pane_ref(&self, _state: &WorkbenchClientState) -> Option<PaneRef> {
        Some(self.clone())
    }
}

impl TestPaneRef for PaneKey {
    fn resolve_pane_ref(&self, state: &WorkbenchClientState) -> Option<PaneRef> {
        state.pane_ref_for(self)
    }
}

impl TestPaneRef for AttachmentKey {
    fn resolve_pane_ref(&self, state: &WorkbenchClientState) -> Option<PaneRef> {
        state.pane_ref_for_anchor(self)
    }
}

/// Resolves any spelling of pane identity into the logical `PaneKey` — switch
/// markers and membership fences are pane-keyed while legacy tests name the
/// anchor.
pub(in crate::workbench_client) trait TestPaneKey {
    fn resolve_pane_key(&self, state: &WorkbenchClientState) -> Option<PaneKey>;
}

impl TestPaneKey for PaneKey {
    fn resolve_pane_key(&self, _state: &WorkbenchClientState) -> Option<PaneKey> {
        Some(self.clone())
    }
}

impl TestPaneKey for PaneRef {
    fn resolve_pane_key(&self, _state: &WorkbenchClientState) -> Option<PaneKey> {
        Some(self.pane().clone())
    }
}

impl TestPaneKey for AttachmentKey {
    fn resolve_pane_key(&self, state: &WorkbenchClientState) -> Option<PaneKey> {
        state.pane_key_for_anchor(self)
    }
}

impl WorkbenchClientState {
    /// Compatibility variants of the pane-scoped state API for legacy
    /// single-pane tests: the key argument may be an `AttachmentKey`,
    /// `PaneKey`, or `PaneRef`; anchors and logical keys resolve to the live
    /// incarnation. An unresolvable key reports the same rejection the exact
    /// call would produce for a dead incarnation.
    pub(in crate::workbench_client) fn install_projection_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.install_projection(&resolved))
    }

    pub(in crate::workbench_client) fn record_setup_configuration_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> bool {
        key.resolve_pane_ref(self).is_some_and(|resolved| {
            self.record_setup_configuration(&resolved, mode, applied_geometry)
        })
    }

    pub(in crate::workbench_client) fn stage_drawn_frame_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.stage_drawn_frame(&resolved, frame))
    }

    pub(in crate::workbench_client) fn mark_feedback_accepted_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.mark_feedback_accepted(&resolved, frame))
    }

    pub(in crate::workbench_client) fn publish_ready_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.publish_ready(&resolved, frame, mode, accepted_geometry))
    }

    pub(in crate::workbench_client) fn settle_ready_candidate_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
        mode: ControllerMode,
        accepted_geometry: Option<Geometry>,
    ) -> Result<ReadyDecision, CarrierFailure> {
        match key.resolve_pane_ref(self) {
            Some(resolved) => {
                self.settle_ready_candidate(&resolved, frame, mode, accepted_geometry)
            }
            None => Ok(ReadyDecision::Superseded),
        }
    }

    pub(in crate::workbench_client) fn settle_attached_event_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
    ) -> Option<SetupEventDisposition> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.settle_attached_event(&resolved))
    }

    pub(in crate::workbench_client) fn settle_feedback_event_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        frame: FrameIdentity,
    ) -> Option<SetupEventDisposition> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.settle_feedback_event(&resolved, frame))
    }

    pub(in crate::workbench_client) fn settle_setup_configuration_event_t<
        K: TestPaneRef + ?Sized,
    >(
        &mut self,
        key: &K,
        mode: ControllerMode,
        applied_geometry: Option<Geometry>,
    ) -> Option<SetupEventDisposition> {
        key.resolve_pane_ref(self).and_then(|resolved| {
            self.settle_setup_configuration_event(&resolved, mode, applied_geometry)
        })
    }

    pub(in crate::workbench_client) fn settle_switch_entry_expired_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
        settled_at: Instant,
    ) -> Option<CleanupContinuation> {
        key.resolve_pane_ref(self).and_then(|resolved| {
            self.settle_switch_entry_expired(&resolved, target_index, target_provenance, settled_at)
        })
    }

    pub(in crate::workbench_client) fn settle_cleanup_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.settle_cleanup(&resolved, target_index, target_provenance))
    }

    pub(in crate::workbench_client) fn settle_unattached_setup_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        target_index: usize,
        target_provenance: &CreateTerminalProvenance,
    ) -> Option<CleanupContinuation> {
        key.resolve_pane_ref(self).and_then(|resolved| {
            self.settle_unattached_setup(&resolved, target_index, target_provenance)
        })
    }

    pub(in crate::workbench_client) fn record_admitted_geometry_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
        geometry: Geometry,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.record_admitted_geometry(&resolved, geometry))
    }

    pub(in crate::workbench_client) fn expected_frame_geometry_t<K: TestPaneRef + ?Sized>(
        &self,
        key: &K,
    ) -> Option<Geometry> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.expected_frame_geometry(&resolved))
    }

    pub(in crate::workbench_client) fn last_admitted_geometry_t<K: TestPaneRef + ?Sized>(
        &self,
        key: &K,
    ) -> Option<Geometry> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.last_admitted_geometry(&resolved))
    }

    pub(in crate::workbench_client) fn last_accepted_geometry_t<K: TestPaneRef + ?Sized>(
        &self,
        key: &K,
    ) -> Option<Geometry> {
        key.resolve_pane_ref(self)
            .and_then(|resolved| self.last_accepted_geometry(&resolved))
    }

    pub(in crate::workbench_client) fn invalidate_projection_frame_t<K: TestPaneRef + ?Sized>(
        &mut self,
        key: &K,
    ) -> bool {
        key.resolve_pane_ref(self)
            .is_some_and(|resolved| self.invalidate_projection_frame(&resolved))
    }

    /// Legacy tests name the marker by attachment anchor.
    pub(in crate::workbench_client) fn stage_switch_membership_cleanup_k<
        K: TestPaneKey + ?Sized,
    >(
        &mut self,
        marker: &K,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        match marker.resolve_pane_key(self) {
            Some(resolved) => self.stage_switch_membership_cleanup(&resolved, prepared, deadlines),
            None => Err(CarrierFailure::Protocol),
        }
    }

    pub(in crate::workbench_client) fn prepare_switch_membership_install_k<
        K: TestPaneKey + ?Sized,
    >(
        &mut self,
        marker: &K,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        match marker.resolve_pane_key(self) {
            Some(resolved) => self.prepare_switch_membership_install(&resolved, prepared, desired),
            None => Err(CarrierFailure::Protocol),
        }
    }

    /// Legacy tests name the marker by attachment anchor; resolve it to the
    /// live pane key before checking.
    pub(in crate::workbench_client) fn switch_marker_is_current_k<K: TestPaneKey + ?Sized>(
        &self,
        marker: &K,
    ) -> bool {
        marker
            .resolve_pane_key(self)
            .is_some_and(|resolved| self.switch_marker_is_current(&resolved))
    }

    pub(in crate::workbench_client) fn switch_deadlines_k<K: TestPaneKey + ?Sized>(
        &self,
        marker: &K,
    ) -> Option<OperationDeadlines> {
        marker
            .resolve_pane_key(self)
            .and_then(|resolved| self.switch_deadlines(&resolved))
    }

    pub(in crate::workbench_client) fn prepare_switch_candidate_k<K: TestPaneKey + ?Sized>(
        &self,
        marker: &K,
        outcome: WorkbenchSnapshotOutcome,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        match marker.resolve_pane_key(self) {
            Some(resolved) => self.prepare_switch_candidate(&resolved, outcome),
            None => Err(CarrierFailure::Protocol),
        }
    }

    pub(in crate::workbench_client) fn invalidate_switch_anchor_for_k<K: TestPaneKey + ?Sized>(
        &mut self,
        discarded: &K,
    ) {
        if let Some(resolved) = discarded.resolve_pane_key(self) {
            self.invalidate_switch_anchor_for(&resolved);
        }
    }

    /// Legacy single-projection form: `retained` is the old focused attachment.
    pub(in crate::workbench_client) fn prepare_switch_restore_k<
        K: TestPaneKey + ?Sized,
        R: TestPaneKey + ?Sized,
    >(
        &mut self,
        marker: &K,
        retained: Option<&R>,
    ) -> Result<PreparedSwitchRestore<'_>, CarrierFailure> {
        let Some(resolved) = marker.resolve_pane_key(self) else {
            return Err(CarrierFailure::Protocol);
        };
        let retained: Vec<PaneRef> = retained
            .and_then(|key| key.resolve_pane_key(self))
            .and_then(|key| self.pane_ref_for(&key))
            .into_iter()
            .collect();
        self.prepare_switch_restore(&resolved, &retained)
    }
}
