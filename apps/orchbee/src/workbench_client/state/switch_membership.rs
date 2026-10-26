use super::{
    AttachTarget, CarrierFailure, ClientPhase, ControllerMode, ExistingAttachment, FrameIdentity,
    Geometry, PaneKey, PaneRecord, PaneRef, PreparedMembershipCandidate, ReadyAttachment,
    SelectedTab, SwitchSetup, WorkbenchClientState, WorkbenchSnapshotOutcome,
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
        projections: Box<[RetainedProjection]>,
        observed_geometry: Geometry,
    },
    Unbound {
        selected: SelectedTab,
        projections: Box<[RetainedProjection]>,
        observed_geometry: Geometry,
    },
    Selected(SelectedTab),
    Retryable(SelectedTab),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedProjection {
    pane: PaneRef,
    binding: super::ClientTerminalBinding,
    mode: ControllerMode,
    observed_geometry: Geometry,
    last_admitted_geometry: Option<Geometry>,
    accepted_geometry: Option<Geometry>,
    frame: FrameIdentity,
}

impl RetainedProjection {
    fn capture(record: &PaneRecord) -> Option<Self> {
        let mode = record.mode.clone()?;
        let frame = record.accepted_frame?;
        if !record.attached
            || !record.projected
            || record.departing
            || record.staged_frame != Some(frame)
            || match &mode {
                ControllerMode::ReadOnly => record.last_accepted_geometry.is_some(),
                ControllerMode::Controlling(_) => {
                    record.last_accepted_geometry != Some(record.observed_geometry)
                }
            }
        {
            return None;
        }
        Some(Self {
            pane: record.pane_ref(),
            binding: record.binding.clone(),
            mode,
            observed_geometry: record.observed_geometry,
            last_admitted_geometry: record.last_admitted_geometry,
            accepted_geometry: record.last_accepted_geometry,
            frame,
        })
    }

    fn can_restore(&self, state: &WorkbenchClientState, retained: &[PaneRef]) -> bool {
        retained.contains(&self.pane)
            && state.live_pane_record(&self.pane).is_some_and(|record| {
                record.attached
                    && record.departing
                    && !record.projected
                    && record.staged_frame.is_none()
                    && record.accepted_frame.is_none()
                    && record.last_accepted_geometry.is_none()
                    && record.binding == self.binding
                    && record.mode.as_ref() == Some(&self.mode)
                    && record.observed_geometry == self.observed_geometry
                    && record.last_admitted_geometry == self.last_admitted_geometry
            })
    }
}

impl StableAnchor {
    fn projections(&self) -> &[RetainedProjection] {
        match self {
            Self::Ready { projections, .. } | Self::Unbound { projections, .. } => projections,
            Self::Selected(_) | Self::Retryable(_) => &[],
        }
    }

    fn selected(&self) -> &SelectedTab {
        match self {
            Self::Ready { ready, .. } => ready.selected(),
            Self::Unbound { selected, .. }
            | Self::Selected(selected)
            | Self::Retryable(selected) => selected,
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
pub(in crate::workbench_client) struct SwitchRestoreKey {
    marker: PaneKey,
    restored: PaneKey,
    panes: Box<[PaneRef]>,
}

impl SwitchRestoreKey {
    pub(in crate::workbench_client) const fn marker(&self) -> &PaneKey {
        &self.marker
    }

    pub(in crate::workbench_client) const fn restored(&self) -> &PaneKey {
        &self.restored
    }

    pub(in crate::workbench_client) fn panes(&self) -> &[PaneRef] {
        &self.panes
    }
}

pub(in crate::workbench_client) struct PreparedSwitchRestore<'state> {
    state: &'state mut WorkbenchClientState,
    authority: SwitchRestoreKey,
    desired: PaneKey,
    phase: ClientPhase,
    observed_geometry: Geometry,
    projections: Box<[RetainedProjection]>,
}

impl PreparedSwitchRestore<'_> {
    pub(in crate::workbench_client) const fn restoration(&self) -> &SwitchRestoreKey {
        &self.authority
    }

    /// Restores the pre-switch focus and all captured live projections together.
    /// Already departing panes stay departing; the abandoned Switch candidate
    /// converges to disposal. No viewer or frame evidence is invented.
    pub(in crate::workbench_client) fn commit(self) {
        self.state.desired = Some(self.desired.clone());
        self.state.phase = self.phase;
        self.state.latest_geometry = self.observed_geometry;
        for record in &mut self.state.panes {
            if record.key().anchor() == self.desired.anchor() {
                if let Some(saved) = self
                    .projections
                    .iter()
                    .find(|saved| saved.pane == record.pane_ref())
                {
                    record.departing = false;
                    record.projected = true;
                    record.staged_frame = Some(saved.frame);
                    record.accepted_frame = Some(saved.frame);
                    record.last_accepted_geometry = saved.accepted_geometry;
                } else if self.projections.is_empty() && record.key() == &self.desired {
                    record.departing = false;
                }
            } else if !record.departing {
                record.departing = true;
                record.projected = false;
                record.staged_frame = None;
                record.accepted_frame = None;
                record.last_accepted_geometry = None;
            }
        }
    }
}

impl WorkbenchClientState {
    fn capture_anchor_projections(
        &self,
        selected: &SelectedTab,
    ) -> Option<Box<[RetainedProjection]>> {
        self.panes
            .iter()
            .filter(|record| record.key.anchor() == selected.key.anchor() && !record.departing)
            .map(RetainedProjection::capture)
            .collect()
    }

    fn stable_switch_anchor(&self) -> Option<StableAnchor> {
        match &self.phase {
            ClientPhase::Ready(ready) => {
                let record = self.focus_record()?;
                record.accepted_frame?;
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
                    projections: self.capture_anchor_projections(ready.selected())?,
                    observed_geometry: self.latest_geometry,
                })
            }
            ClientPhase::SelectedWithoutAttachment(selected)
                if self.selected_is_unbound() && self.has_attachments() =>
            {
                Some(StableAnchor::Unbound {
                    selected: selected.clone(),
                    projections: self.capture_anchor_projections(selected)?,
                    observed_geometry: self.latest_geometry,
                })
            }
            ClientPhase::SelectedWithoutAttachment(selected) => {
                Some(StableAnchor::Selected(selected.clone()))
            }
            ClientPhase::RetryableAttachUnavailable(selected) => {
                Some(StableAnchor::Retryable(selected.clone()))
            }
            ClientPhase::Switching(setup) => setup.membership.as_ref()?.anchor.clone(),
            _ => None,
        }
    }

    pub(super) fn begin_switch_membership(&mut self, target: AttachTarget) -> bool {
        self.cancel_duplicate_sequence();
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
            || target.session_id() != self.cache.session_id()
            || !self.target_provenance_valid(&target)
        {
            return false;
        }
        let anchor = self.stable_switch_anchor();
        let previous = match &self.phase {
            ClientPhase::Ready(ready) => Some(ExistingAttachment {
                selected: ready.selected.clone(),
                mode: Some(ready.mode.clone()),
            }),
            ClientPhase::Switching(setup) => setup.previous.clone().or_else(|| {
                self.pane_record(setup.target.selected.key())
                    .is_some_and(|record| record.attached)
                    .then(|| ExistingAttachment {
                        selected: setup.target.selected.clone(),
                        mode: setup.mode.clone(),
                    })
            }),
            ClientPhase::SelectedWithoutAttachment(selected) if self.selected_is_unbound() => self
                .panes
                .iter()
                .find(|record| record.key.anchor() == selected.key.anchor() && record.attached)
                .map(|record| ExistingAttachment {
                    selected: SelectedTab::new_panel(
                        record.key.clone(),
                        record.index,
                        record.panel_index,
                    ),
                    mode: record.mode.clone(),
                }),
            ClientPhase::SelectedWithoutAttachment(_)
            | ClientPhase::RetryableAttachUnavailable(_) => None,
            _ => return false,
        };
        self.remember_current_navigation();
        self.depart_all_panes();
        self.desired = Some(target.selected.key().clone());
        self.install_pane_record(&target);
        self.phase = ClientPhase::Switching(SwitchSetup {
            target,
            previous,
            mode: None,
            accepted_geometry: None,
            membership: Some(SwitchMembership { anchor }),
        });
        true
    }

    /// Called only for an actual abandoned old event/causal result or an exact proof-changing result.
    pub(in crate::workbench_client) fn invalidate_switch_anchor(&mut self) {
        if let ClientPhase::Switching(setup) = &mut self.phase
            && let Some(membership) = &mut setup.membership
        {
            membership.invalidate();
        }
    }

    pub(in crate::workbench_client) fn invalidate_switch_anchor_for(
        &mut self,
        discarded: &PaneKey,
    ) {
        if let ClientPhase::Switching(setup) = &mut self.phase
            && let Some(membership) = &mut setup.membership
            && membership.anchor.as_ref().is_some_and(|anchor| {
                anchor.selected().key() == discarded
                    || anchor
                        .projections()
                        .iter()
                        .any(|saved| saved.pane.pane() == discarded)
            })
        {
            membership.invalidate();
        }
    }

    pub(in crate::workbench_client) fn switch_marker_is_current(&self, marker: &PaneKey) -> bool {
        self.desired.as_ref() == Some(marker)
            && matches!(&self.phase, ClientPhase::Switching(setup)
                if setup.target.selected.key() == marker && setup.membership.is_some())
    }

    pub(in crate::workbench_client) fn switch_deadlines(
        &self,
        marker: &PaneKey,
    ) -> Option<super::OperationDeadlines> {
        let ClientPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (setup.target.selected.key() == marker).then(|| setup.target.deadlines())
    }

    pub(in crate::workbench_client) fn prepare_switch_candidate(
        &self,
        marker: &PaneKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        if !self.switch_marker_is_current(marker) {
            return Err(CarrierFailure::Protocol);
        }
        let ClientPhase::Switching(setup) = &self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Err(CarrierFailure::Protocol);
        };
        let candidate = self
            .cache
            .project_snapshot(&snapshot)
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
        let mut prepared = self.prepare_membership_candidate(
            candidate,
            Some(marker.anchor().tab_id()),
            Some(setup.target.selected.index()),
            None,
            None,
        )?;
        if let Some(expected_host) = &setup.target.expected_host {
            let binding = setup
                .target
                .binding()
                .ok_or(CarrierFailure::AuthorityDivergence)?;
            let selection = crate::workbench_client::workbench_projection::ClientMemberSelection {
                member: crate::workbench_client::ClientMemberTarget {
                    tab: marker.anchor().tab_id().clone(),
                    panel: marker.panel().clone(),
                    presentation: (*setup.target.presentation_id).clone(),
                },
                binding: orchbee_workbench_protocol::PresentationBinding {
                    worker_id: binding.worker_id().clone(),
                    terminal_id: binding.terminal_id().clone(),
                },
                expected_host: Some(expected_host.as_ref().clone()),
            };
            if !prepared.retarget_selected_bound_member(self.cache.location(), &selection) {
                return Err(CarrierFailure::AuthorityDivergence);
            }
        }
        Ok(prepared)
    }

    pub(in crate::workbench_client) fn prepare_switch_restore(
        &mut self,
        marker: &PaneKey,
        retained_projections: &[PaneRef],
    ) -> Result<PreparedSwitchRestore<'_>, CarrierFailure> {
        if !self.invariant_holds()
            || !self.switch_marker_is_current(marker)
            || self.pending_membership.is_some()
        {
            return Err(CarrierFailure::Protocol);
        }
        let ClientPhase::Switching(setup) = &self.phase else {
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
            || anchor
                .projections()
                .iter()
                .any(|saved| !saved.can_restore(self, retained_projections))
        {
            return Err(CarrierFailure::Protocol);
        }
        let observed_geometry = match anchor {
            StableAnchor::Ready {
                ready,
                observed_geometry,
                ..
            } => {
                if self.latest_geometry != *observed_geometry
                    || setup.previous.as_ref().is_none_or(|previous| {
                        previous.selected != *ready.selected()
                            || previous.mode.as_ref() != Some(ready.mode())
                    })
                {
                    return Err(CarrierFailure::Protocol);
                }
                *observed_geometry
            }
            StableAnchor::Unbound {
                projections,
                observed_geometry,
                ..
            } => {
                if self.latest_geometry != *observed_geometry
                    || setup.previous.as_ref().is_none_or(|previous| {
                        !projections.iter().any(|saved| {
                            saved.pane.pane() == previous.selected.key()
                                && previous.mode.as_ref() == Some(&saved.mode)
                        })
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
            panes: anchor
                .projections()
                .iter()
                .map(|saved| saved.pane.clone())
                .collect(),
        };
        let phase = match anchor {
            StableAnchor::Ready { ready, .. } => ClientPhase::Ready(ready.clone()),
            StableAnchor::Unbound { selected, .. } | StableAnchor::Selected(selected) => {
                ClientPhase::SelectedWithoutAttachment(selected.clone())
            }
            StableAnchor::Retryable(selected) => {
                ClientPhase::RetryableAttachUnavailable(selected.clone())
            }
        };
        let projections = anchor.projections().into();
        Ok(PreparedSwitchRestore {
            state: self,
            projections,
            authority,
            desired: restored,
            phase,
            observed_geometry,
        })
    }
}
