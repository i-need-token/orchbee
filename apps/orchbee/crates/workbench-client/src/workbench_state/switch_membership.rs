//! Prepares Client membership for an exact Tab or context switch, retaining selection
//! provenance and required cleanup before installation. It does not open a transport, attach a
//! terminal, or redirect the switch to an unrequested live target.

use super::{
    AttachTarget, CarrierFailure, ClientPhase, ControllerMode, ExistingAttachment,
    FrameAcknowledgement, FrameIdentity, Geometry, PaneKey, PaneProjection, PaneRecord, PaneRef,
    PreparedMembershipCandidate, ReadyAttachment, SelectedTab, SwitchSetup, WorkbenchClientState,
    WorkbenchSnapshotOutcome,
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
    /// A focused pane whose viewer was lost while live siblings keep the anchor: the siblings'
    /// projections restore as an unbound selection's do, with the selection still retryable.
    Lost {
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
        let mode = record.mode().cloned()?;
        let frame = record.accepted_frame()?;
        if record.departing
            || match &mode {
                ControllerMode::ReadOnly => record.last_accepted_geometry().is_some(),
                ControllerMode::Controlling(_) => {
                    record.last_accepted_geometry() != Some(record.observed_geometry)
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
            accepted_geometry: record.last_accepted_geometry(),
            frame,
        })
    }

    fn can_restore(&self, state: &WorkbenchClientState, retained: &[PaneRef]) -> bool {
        retained.contains(&self.pane)
            && state.live_pane_record(&self.pane).is_some_and(|record| {
                record.attached()
                    && record.departing
                    && !record.projected()
                    && record.last_accepted_geometry().is_none()
                    && record.binding == self.binding
                    && record.mode() == Some(&self.mode)
                    && record.observed_geometry == self.observed_geometry
                    && record.last_admitted_geometry == self.last_admitted_geometry
            })
    }
}

impl StableAnchor {
    fn projections(&self) -> &[RetainedProjection] {
        match self {
            Self::Ready { projections, .. }
            | Self::Unbound { projections, .. }
            | Self::Lost { projections, .. } => projections,
            Self::Selected(_) | Self::Retryable(_) => &[],
        }
    }

    fn selected(&self) -> &SelectedTab {
        match self {
            Self::Ready { ready, .. } => ready.selected(),
            Self::Unbound { selected, .. }
            | Self::Lost { selected, .. }
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
pub(crate) struct SwitchRestoreKey {
    marker: PaneKey,
    restored: PaneKey,
    panes: Box<[PaneRef]>,
}

impl SwitchRestoreKey {
    pub(crate) const fn marker(&self) -> &PaneKey {
        &self.marker
    }

    pub(crate) const fn restored(&self) -> &PaneKey {
        &self.restored
    }

    pub(crate) fn panes(&self) -> &[PaneRef] {
        &self.panes
    }
}

pub(crate) struct PreparedSwitchRestore<'state> {
    state: &'state mut WorkbenchClientState,
    authority: SwitchRestoreKey,
    desired: PaneKey,
    phase: ClientPhase,
    observed_geometry: Geometry,
    projections: Box<[RetainedProjection]>,
    /// Whether the restored selection's own viewerless record returns beside its retained
    /// siblings, as a lost focused viewer's does.
    revives_selected: bool,
}

impl PreparedSwitchRestore<'_> {
    pub(crate) const fn restoration(&self) -> &SwitchRestoreKey {
        &self.authority
    }

    /// Restores the pre-switch focus and all captured live projections together.
    /// Already departing panes stay departing; the abandoned Switch candidate
    /// converges to disposal. No viewer or frame evidence is invented.
    pub(crate) fn commit(self) {
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
                    if let Some(attachment) = &mut record.attachment {
                        attachment.projection =
                            PaneProjection::Installed(FrameAcknowledgement::Accepted(saved.frame));
                        attachment.last_accepted_geometry = saved.accepted_geometry;
                    }
                } else if (self.projections.is_empty() || self.revives_selected)
                    && record.key() == &self.desired
                {
                    record.departing = false;
                }
            } else if !record.departing {
                record.departing = true;
                if let Some(attachment) = &mut record.attachment {
                    attachment.withdraw_projection();
                }
            }
        }
    }
}

impl WorkbenchClientState {
    /// Every live projection under the selection's anchor. A selection that lost its viewer
    /// leaves out its own record, which a restore revives without a projection.
    fn capture_anchor_projections(
        &self,
        selected: &SelectedTab,
        without_selected: bool,
    ) -> Option<Box<[RetainedProjection]>> {
        self.panes
            .iter()
            .filter(|record| {
                record.key.anchor() == selected.key.anchor()
                    && !record.departing
                    && !(without_selected && record.key == selected.key)
            })
            .map(RetainedProjection::capture)
            .collect()
    }

    /// The first attached record under the selection's anchor: the old attachment that a Switch
    /// from a selection without its own viewer cleans up first and may restore.
    fn attached_sibling(&self, selected: &SelectedTab) -> Option<ExistingAttachment> {
        self.panes
            .iter()
            .find(|record| record.key.anchor() == selected.key.anchor() && record.attached())
            .map(|record| ExistingAttachment {
                selected: SelectedTab::new_panel(
                    record.key.clone(),
                    record.index,
                    record.panel_index,
                ),
                mode: record.mode().cloned(),
            })
    }

    fn stable_switch_anchor(&self) -> Option<StableAnchor> {
        match &self.phase {
            ClientPhase::Ready(ready) => {
                let record = self.focus_record()?;
                record.accepted_frame()?;
                let accepted_geometry = record.last_accepted_geometry();
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
                    projections: self.capture_anchor_projections(ready.selected(), false)?,
                    observed_geometry: self.latest_geometry,
                })
            }
            ClientPhase::SelectedWithoutAttachment(selected)
                if self.selected_is_unbound() && self.has_attachments() =>
            {
                Some(StableAnchor::Unbound {
                    selected: selected.clone(),
                    projections: self.capture_anchor_projections(selected, false)?,
                    observed_geometry: self.latest_geometry,
                })
            }
            ClientPhase::SelectedWithoutAttachment(selected) => {
                Some(StableAnchor::Selected(selected.clone()))
            }
            ClientPhase::RetryableAttachUnavailable(selected)
                if self.attached_sibling(selected).is_some() =>
            {
                Some(StableAnchor::Lost {
                    selected: selected.clone(),
                    projections: self.capture_anchor_projections(selected, true)?,
                    observed_geometry: self.latest_geometry,
                })
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
                    .is_some_and(|record| record.attached())
                    .then(|| ExistingAttachment {
                        selected: setup.target.selected.clone(),
                        mode: setup.mode.clone(),
                    })
            }),
            // A selection without its own viewer may still own live siblings under its anchor,
            // which the Switch cleans up first: an unbound member, or a lost focused viewer.
            ClientPhase::SelectedWithoutAttachment(selected) if self.selected_is_unbound() => {
                self.attached_sibling(selected)
            }
            ClientPhase::RetryableAttachUnavailable(selected) => self.attached_sibling(selected),
            ClientPhase::SelectedWithoutAttachment(_) => None,
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
    pub(crate) fn invalidate_switch_anchor(&mut self) {
        if let ClientPhase::Switching(setup) = &mut self.phase
            && let Some(membership) = &mut setup.membership
        {
            membership.invalidate();
        }
    }

    pub(crate) fn invalidate_switch_anchor_for(&mut self, discarded: &PaneKey) {
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

    pub(crate) fn switch_marker_is_current(&self, marker: &PaneKey) -> bool {
        self.desired.as_ref() == Some(marker)
            && matches!(&self.phase, ClientPhase::Switching(setup)
                if setup.target.selected.key() == marker && setup.membership.is_some())
    }

    pub(crate) fn switch_deadlines(&self, marker: &PaneKey) -> Option<super::OperationDeadlines> {
        let ClientPhase::Switching(setup) = &self.phase else {
            return None;
        };
        (setup.target.selected.key() == marker).then(|| setup.target.deadlines())
    }

    pub(crate) fn prepare_switch_candidate(
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
            let selection = crate::workbench_projection::ClientMemberSelection {
                member: crate::workbench_projection::ClientMemberTarget {
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
            // The Host fence holds only while the Server still binds this exact member to the
            // fenced terminal. An Agent the Server replaced or removed after admission lands as
            // an ordinary Switch on what the Server now holds there, without the fence.
            prepared.retarget_selected_bound_member(self.cache.location(), &selection);
        }
        Ok(prepared)
    }

    pub(crate) fn prepare_switch_restore(
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
        let observed_geometry =
            match anchor {
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
                }
                | StableAnchor::Lost {
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
                    if self.panes.iter().any(|record| {
                        record.key().anchor() == restored.anchor() && record.attached()
                    }) || retained_projections
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
            StableAnchor::Lost { selected, .. } | StableAnchor::Retryable(selected) => {
                ClientPhase::RetryableAttachUnavailable(selected.clone())
            }
        };
        let revives_selected = matches!(anchor, StableAnchor::Lost { .. });
        let projections = anchor.projections().into();
        Ok(PreparedSwitchRestore {
            state: self,
            projections,
            authority,
            desired: restored,
            phase,
            observed_geometry,
            revives_selected,
        })
    }
}
