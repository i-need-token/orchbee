use super::{
    AttachTarget, CarrierFailure, ControllerMode, ExistingAttachment, FrameIdentity, Geometry,
    NativeSessionState, PaneKey, PreparedMembershipCandidate, ReadyAttachment, SelectedTab,
    SessionPhase, SwitchSetup, WorkbenchSnapshotOutcome,
};

/// This record exists only between Switch intent admission and candidate installation.
/// The shared membership owner separately retains any validated candidate awaiting cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SwitchMembership {
    anchor: Option<StableAnchor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StableAnchor {
    Ready {
        ready: ReadyAttachment,
        frame: FrameIdentity,
        observed_geometry: Geometry,
        accepted_geometry: Option<Geometry>,
    },
    Selected(SelectedTab),
    Retryable(SelectedTab),
}

impl StableAnchor {
    fn selected(&self) -> &SelectedTab {
        match self {
            Self::Ready { ready, .. } => ready.selected(),
            Self::Selected(selected) | Self::Retryable(selected) => selected,
        }
    }
}

impl SwitchMembership {
    pub(super) fn invalidate(&mut self) {
        self.anchor = None;
    }
}

/// Only the exact, exclusively borrowed state restoration plan can mint this authority.
/// Mailbox publication changes desired identity without rewinding its allocation cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct SwitchRestoreKey {
    marker: PaneKey,
    restored: PaneKey,
}

impl SwitchRestoreKey {
    pub(in crate::terminal_session) const fn marker(&self) -> &PaneKey {
        &self.marker
    }

    pub(in crate::terminal_session) const fn restored(&self) -> &PaneKey {
        &self.restored
    }
}

pub(in crate::terminal_session) struct PreparedSwitchRestore<'state> {
    state: &'state mut NativeSessionState,
    authority: SwitchRestoreKey,
    desired: PaneKey,
    phase: SessionPhase,
    observed_geometry: Geometry,
    frame: Option<FrameIdentity>,
    accepted_geometry: Option<Geometry>,
}

impl PreparedSwitchRestore<'_> {
    pub(in crate::terminal_session) const fn restoration(&self) -> &SwitchRestoreKey {
        &self.authority
    }

    /// Restores the pre-switch focus and revives every pane record of the
    /// restored anchor: records are only marked `departing` at switch entry,
    /// never freed, so the whole old membership returns with its live
    /// attachments intact; the focus pane's projection and frame facts are
    /// re-materialized from the stable anchor snapshot the same way the
    /// single-attachment fields were. Records outside the restored anchor
    /// (the abandoned switch candidate) converge to disposal.
    pub(in crate::terminal_session) fn commit(self) {
        self.state.desired = Some(self.desired.clone());
        self.state.phase = self.phase;
        self.state.latest_geometry = self.observed_geometry;
        for record in &mut self.state.panes {
            if record.key().anchor() == self.desired.anchor() {
                record.departing = false;
            } else if !record.departing {
                record.departing = true;
                record.projected = false;
                record.staged_frame = None;
                record.accepted_frame = None;
                record.last_accepted_geometry = None;
            }
        }
        if let Some(frame) = self.frame
            && let Some(record) = self.state.pane_record_mut(&self.desired)
        {
            record.projected = true;
            record.staged_frame = Some(frame);
            record.accepted_frame = Some(frame);
            record.last_accepted_geometry = self.accepted_geometry;
        }
    }
}

impl NativeSessionState {
    fn stable_switch_anchor(&self) -> Option<StableAnchor> {
        match &self.phase {
            SessionPhase::Ready(ready) => {
                let record = self.focus_record()?;
                let frame = record.accepted_frame?;
                if !record.attached
                    || !record.projected
                    || record.staged_frame != record.accepted_frame
                {
                    return None;
                }
                let accepted_geometry = record.last_accepted_geometry;
                match ready.mode() {
                    ControllerMode::Controlling(_)
                        if accepted_geometry != Some(self.latest_geometry) =>
                    {
                        return None;
                    }
                    ControllerMode::ReadOnly if accepted_geometry.is_some() => return None,
                    ControllerMode::Controlling(_) | ControllerMode::ReadOnly => {}
                }
                Some(StableAnchor::Ready {
                    ready: ready.clone(),
                    frame,
                    observed_geometry: self.latest_geometry,
                    accepted_geometry,
                })
            }
            SessionPhase::SelectedWithoutAttachment(selected) => {
                Some(StableAnchor::Selected(selected.clone()))
            }
            SessionPhase::RetryableAttachUnavailable(selected) => {
                Some(StableAnchor::Retryable(selected.clone()))
            }
            SessionPhase::Switching(setup) => setup.membership.as_ref()?.anchor.clone(),
            _ => None,
        }
    }

    pub(super) fn begin_switch_membership(&mut self, target: AttachTarget) -> bool {
        let Some(desired) = self.desired.as_ref() else {
            return false;
        };
        if !self.invariant_holds()
            || self.pending_refresh.is_some()
            || self.mutation_blocks_structure()
            || target.selected.key().anchor().generation() <= desired.anchor().generation()
            || self
                .cache
                .tabs()
                .get(target.selected.index())
                .is_none_or(|tab| tab.tab_id() != target.selected.key().anchor().tab_id())
            || target.site_id() != self.cache.site_id()
            || self.provenance.get(target.selected.index()) != Some(target.provenance())
        {
            return false;
        }
        let anchor = self.stable_switch_anchor();
        let previous = match &self.phase {
            SessionPhase::Ready(ready) => Some(ExistingAttachment {
                selected: ready.selected.clone(),
                mode: Some(ready.mode.clone()),
            }),
            SessionPhase::Switching(setup) => setup.previous.clone().or_else(|| {
                self.pane_record(setup.target.selected.key())
                    .is_some_and(|record| record.attached)
                    .then(|| ExistingAttachment {
                        selected: setup.target.selected.clone(),
                        mode: setup.mode.clone(),
                    })
            }),
            SessionPhase::SelectedWithoutAttachment(_)
            | SessionPhase::RetryableAttachUnavailable(_) => None,
            _ => return false,
        };
        self.depart_all_panes();
        self.desired = Some(target.selected.key().clone());
        self.install_pane_record(&target);
        self.phase = SessionPhase::Switching(SwitchSetup {
            target,
            previous,
            mode: None,
            accepted_geometry: None,
            membership: Some(SwitchMembership { anchor }),
        });
        true
    }

    /// Called only for an actual abandoned old event/causal result or an exact proof-changing result.
    pub(in crate::terminal_session) fn invalidate_switch_anchor(&mut self) {
        if let SessionPhase::Switching(setup) = &mut self.phase
            && let Some(membership) = &mut setup.membership
        {
            membership.invalidate();
        }
    }

    pub(in crate::terminal_session) fn invalidate_switch_anchor_for(
        &mut self,
        discarded: &PaneKey,
    ) {
        if let SessionPhase::Switching(setup) = &mut self.phase
            && let Some(membership) = &mut setup.membership
            && membership
                .anchor
                .as_ref()
                .is_some_and(|anchor| anchor.selected().key() == discarded)
        {
            membership.invalidate();
        }
    }

    pub(in crate::terminal_session) fn switch_marker_is_current(&self, marker: &PaneKey) -> bool {
        self.desired.as_ref() == Some(marker)
            && matches!(&self.phase, SessionPhase::Switching(setup)
                if setup.target.selected.key() == marker && setup.membership.is_some())
    }

    /// Legacy tests name the marker by attachment anchor; resolve it to the
    /// live pane key before checking.
    #[cfg(test)]
    pub(in crate::terminal_session) fn switch_marker_is_current_k<
        K: super::TestPaneKey + ?Sized,
    >(
        &self,
        marker: &K,
    ) -> bool {
        marker
            .resolve_pane_key(self)
            .is_some_and(|resolved| self.switch_marker_is_current(&resolved))
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn switch_deadlines_k<K: super::TestPaneKey + ?Sized>(
        &self,
        marker: &K,
    ) -> Option<super::OperationDeadlines> {
        marker
            .resolve_pane_key(self)
            .and_then(|resolved| self.switch_deadlines(&resolved))
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn prepare_switch_candidate_k<
        K: super::TestPaneKey + ?Sized,
    >(
        &self,
        marker: &K,
        outcome: WorkbenchSnapshotOutcome,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        match marker.resolve_pane_key(self) {
            Some(resolved) => self.prepare_switch_candidate(&resolved, outcome),
            None => Err(CarrierFailure::Protocol),
        }
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn invalidate_switch_anchor_for_k<
        K: super::TestPaneKey + ?Sized,
    >(
        &mut self,
        discarded: &K,
    ) {
        if let Some(resolved) = discarded.resolve_pane_key(self) {
            self.invalidate_switch_anchor_for(&resolved);
        }
    }

    /// Legacy single-projection form: `retained` is the old focused attachment.
    #[cfg(test)]
    pub(in crate::terminal_session) fn prepare_switch_restore_k<
        K: super::TestPaneKey + ?Sized,
        R: super::TestPaneKey + ?Sized,
    >(
        &mut self,
        marker: &K,
        retained: Option<&R>,
    ) -> Result<PreparedSwitchRestore<'_>, CarrierFailure> {
        let Some(resolved) = marker.resolve_pane_key(self) else {
            return Err(CarrierFailure::Protocol);
        };
        let retained: Vec<PaneKey> = retained
            .and_then(|key| key.resolve_pane_key(self))
            .into_iter()
            .collect();
        self.prepare_switch_restore(&resolved, &retained)
    }

    pub(in crate::terminal_session) fn switch_deadlines(
        &self,
        marker: &PaneKey,
    ) -> Option<super::OperationDeadlines> {
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (setup.target.selected.key() == marker).then(|| setup.target.deadlines())
    }

    pub(in crate::terminal_session) fn prepare_switch_candidate(
        &self,
        marker: &PaneKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        if !self.switch_marker_is_current(marker) {
            return Err(CarrierFailure::Protocol);
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Err(CarrierFailure::Protocol);
        };
        let candidate = self
            .cache
            .project_snapshot(&snapshot)
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
        self.prepare_membership_candidate(
            candidate,
            Some(marker.anchor().tab_id()),
            Some(setup.target.selected.index()),
            None,
            None,
        )
    }

    pub(in crate::terminal_session) fn prepare_switch_restore(
        &mut self,
        marker: &PaneKey,
        retained_projections: &[PaneKey],
    ) -> Result<PreparedSwitchRestore<'_>, CarrierFailure> {
        if !self.invariant_holds()
            || !self.switch_marker_is_current(marker)
            || self.pending_membership.is_some()
        {
            return Err(CarrierFailure::Protocol);
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        let anchor = setup
            .membership
            .as_ref()
            .and_then(|membership| membership.anchor.as_ref())
            .ok_or(CarrierFailure::Protocol)?;
        let restored = anchor.selected().key().clone();
        if restored.anchor().generation() >= marker.anchor().generation()
            || self
                .cache
                .tabs()
                .get(anchor.selected().index())
                .is_none_or(|tab| tab.tab_id() != restored.anchor().tab_id())
        {
            return Err(CarrierFailure::Protocol);
        }
        let observed_geometry = match anchor {
            StableAnchor::Ready {
                ready,
                observed_geometry,
                ..
            } => {
                let record = self
                    .pane_record(&restored)
                    .ok_or(CarrierFailure::Protocol)?;
                if !record.attached
                    || !retained_projections.iter().any(|key| key == &restored)
                    || self.latest_geometry != *observed_geometry
                    || setup.previous.as_ref().is_none_or(|previous| {
                        previous.selected != *ready.selected()
                            || previous.mode.as_ref() != Some(ready.mode())
                    })
                {
                    return Err(CarrierFailure::Protocol);
                }
                *observed_geometry
            }
            StableAnchor::Selected(_) | StableAnchor::Retryable(_) => {
                if self
                    .panes
                    .iter()
                    .any(|record| record.key().anchor() == restored.anchor() && record.attached)
                    || retained_projections
                        .iter()
                        .any(|key| key.anchor() == restored.anchor())
                    || setup.previous.is_some()
                {
                    return Err(CarrierFailure::Protocol);
                }
                self.latest_geometry
            }
        };
        let authority = SwitchRestoreKey {
            marker: marker.clone(),
            restored: restored.clone(),
        };
        let (phase, frame, accepted_geometry) = match anchor {
            StableAnchor::Ready {
                ready,
                frame,
                accepted_geometry,
                ..
            } => (
                SessionPhase::Ready(ready.clone()),
                Some(*frame),
                *accepted_geometry,
            ),
            StableAnchor::Selected(selected) => (
                SessionPhase::SelectedWithoutAttachment(selected.clone()),
                None,
                None,
            ),
            StableAnchor::Retryable(selected) => (
                SessionPhase::RetryableAttachUnavailable(selected.clone()),
                None,
                None,
            ),
        };
        Ok(PreparedSwitchRestore {
            state: self,
            authority,
            desired: restored,
            phase,
            observed_geometry,
            frame,
            accepted_geometry,
        })
    }
}
