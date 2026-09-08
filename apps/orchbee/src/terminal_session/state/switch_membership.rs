use super::{
    AttachTarget, AttachmentKey, CarrierFailure, ControllerMode, ExistingAttachment, FrameIdentity,
    Geometry, NativeSessionState, PreparedMembershipCandidate, ReadyAttachment, SelectedTab,
    SessionPhase, SwitchSetup, WorkbenchSnapshotOutcome, project_native_standalone_tabs,
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
    marker: AttachmentKey,
    restored: AttachmentKey,
}

impl SwitchRestoreKey {
    pub(in crate::terminal_session) const fn marker(&self) -> &AttachmentKey {
        &self.marker
    }

    pub(in crate::terminal_session) const fn restored(&self) -> &AttachmentKey {
        &self.restored
    }
}

pub(in crate::terminal_session) struct PreparedSwitchRestore<'state> {
    state: &'state mut NativeSessionState,
    authority: SwitchRestoreKey,
    desired: AttachmentKey,
    phase: SessionPhase,
    projection: Option<AttachmentKey>,
    staged_frame: Option<(AttachmentKey, FrameIdentity)>,
    accepted_frame: Option<(AttachmentKey, FrameIdentity)>,
    accepted_geometry: Option<(AttachmentKey, Geometry)>,
}

impl PreparedSwitchRestore<'_> {
    pub(in crate::terminal_session) const fn restoration(&self) -> &SwitchRestoreKey {
        &self.authority
    }

    pub(in crate::terminal_session) fn commit(self) {
        self.state.desired = Some(self.desired);
        self.state.phase = self.phase;
        self.state.projection_key = self.projection;
        self.state.staged_frame = self.staged_frame;
        self.state.accepted_frame = self.accepted_frame;
        self.state.last_accepted_geometry = self.accepted_geometry;
    }
}

impl NativeSessionState {
    fn stable_switch_anchor(&self) -> Option<StableAnchor> {
        match &self.phase {
            SessionPhase::Ready(ready) => {
                let key = ready.selected().key();
                let (frame_key, frame) = self.accepted_frame.as_ref()?;
                if self.attached_key.as_ref() != Some(key)
                    || self.projection_key.as_ref() != Some(key)
                    || frame_key != key
                    || self.staged_frame.as_ref() != self.accepted_frame.as_ref()
                {
                    return None;
                }
                let accepted_geometry = self.last_accepted_geometry(key);
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
                    frame: *frame,
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
            || self.pending_refresh_generation.is_some()
            || self.move_blocks_structure()
            || target.selected.key.generation <= desired.generation
            || self
                .cache
                .tabs()
                .get(target.selected.index)
                .is_none_or(|tab| tab.tab_id() != target.selected.key.tab_id())
            || target.site_id != *self.cache.site_id()
            || self.provenance.get(target.selected.index) != Some(&target.provenance)
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
                (self.attached_key.as_ref() == Some(&setup.target.selected.key)).then(|| {
                    ExistingAttachment {
                        selected: setup.target.selected.clone(),
                        mode: setup.mode.clone(),
                    }
                })
            }),
            SessionPhase::SelectedWithoutAttachment(_)
            | SessionPhase::RetryableAttachUnavailable(_) => None,
            _ => return false,
        };
        self.desired = Some(target.selected.key.clone());
        self.projection_key = None;
        self.staged_frame = None;
        self.accepted_frame = None;
        self.last_accepted_geometry = None;
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
        discarded: &AttachmentKey,
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

    pub(in crate::terminal_session) fn switch_marker_is_current(
        &self,
        marker: &AttachmentKey,
    ) -> bool {
        self.desired.as_ref() == Some(marker)
            && matches!(&self.phase, SessionPhase::Switching(setup)
                if setup.target.selected.key == *marker && setup.membership.is_some())
    }

    pub(in crate::terminal_session) fn switch_deadlines(
        &self,
        marker: &AttachmentKey,
    ) -> Option<super::OperationDeadlines> {
        let SessionPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (setup.target.selected.key() == marker).then(|| setup.target.deadlines())
    }

    pub(in crate::terminal_session) fn prepare_switch_candidate(
        &self,
        marker: &AttachmentKey,
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
        let candidate = project_native_standalone_tabs(&snapshot)
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
        self.prepare_membership_candidate(
            candidate,
            Some(marker.tab_id()),
            Some(setup.target.selected.index),
            None,
        )
    }

    pub(in crate::terminal_session) fn prepare_switch_restore(
        &mut self,
        marker: &AttachmentKey,
        retained_projection: Option<&AttachmentKey>,
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
        let restored = anchor.selected().key();
        if restored.generation() >= marker.generation()
            || self
                .cache
                .tabs()
                .get(anchor.selected().index())
                .is_none_or(|tab| tab.tab_id() != restored.tab_id())
        {
            return Err(CarrierFailure::Protocol);
        }
        match anchor {
            StableAnchor::Ready {
                ready,
                observed_geometry,
                ..
            } => {
                if self.attached_key.as_ref() != Some(restored)
                    || retained_projection != Some(restored)
                    || self.latest_geometry != *observed_geometry
                    || setup.previous.as_ref().is_none_or(|previous| {
                        previous.selected != *ready.selected()
                            || previous.mode.as_ref() != Some(ready.mode())
                    })
                {
                    return Err(CarrierFailure::Protocol);
                }
            }
            StableAnchor::Selected(_) | StableAnchor::Retryable(_) => {
                if self.attached_key.is_some()
                    || retained_projection.is_some()
                    || setup.previous.is_some()
                {
                    return Err(CarrierFailure::Protocol);
                }
            }
        }
        let authority = SwitchRestoreKey {
            marker: marker.clone(),
            restored: restored.clone(),
        };
        let desired = restored.clone();
        let (phase, projection, frame, accepted_geometry) = match anchor {
            StableAnchor::Ready {
                ready,
                frame,
                accepted_geometry,
                ..
            } => (
                SessionPhase::Ready(ready.clone()),
                Some(restored.clone()),
                Some((restored.clone(), *frame)),
                accepted_geometry.map(|geometry| (restored.clone(), geometry)),
            ),
            StableAnchor::Selected(selected) => (
                SessionPhase::SelectedWithoutAttachment(selected.clone()),
                None,
                None,
                None,
            ),
            StableAnchor::Retryable(selected) => (
                SessionPhase::RetryableAttachUnavailable(selected.clone()),
                None,
                None,
                None,
            ),
        };
        let staged_frame = frame.clone();
        Ok(PreparedSwitchRestore {
            state: self,
            authority,
            desired,
            phase,
            projection,
            staged_frame,
            accepted_frame: frame,
            accepted_geometry,
        })
    }
}
