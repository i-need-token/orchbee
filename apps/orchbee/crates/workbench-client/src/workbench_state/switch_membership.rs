//! Prepares Client membership for an exact Tab or context switch, retaining selection
//! provenance and required cleanup before installation. It does not open a transport, attach a
//! terminal, or redirect the switch to an unrequested live target.

use super::{
    AttachTarget, CarrierFailure, ClientPhase, ControllerMode, ExistingAttachment,
    FrameAcknowledgement, FrameIdentity, Geometry, MembershipCleanup, PaneFault, PaneKey,
    PaneProjection, PaneRecord, PaneRef, PreparedMembershipCandidate, ReadyAttachment, SelectedTab,
    SwitchSetup, WorkbenchClientState, WorkbenchSnapshotOutcome,
};

/// This record exists only between Switch intent admission and candidate installation.
/// The shared membership owner separately retains any validated candidate awaiting cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SwitchMembership {
    anchor: Option<StableAnchor>,
    /// The selection a refused Switch returns to, restorable or not.
    origin: SelectedTab,
    /// A refused Switch that cannot restore its anchor parks the origin once the viewers under
    /// that anchor are detached.
    parks_origin: bool,
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
            || (matches!(mode, ControllerMode::ReadOnly)
                && record.last_accepted_geometry().is_some())
        {
            return None;
        }
        Some(Self {
            pane: record.pane_ref(),
            binding: record.binding.clone(),
            mode,
            observed_geometry: record.observed_geometry,
            last_admitted_geometry: record.last_admitted_geometry,
            // A controlling pane's geometry is known once the Host acknowledged a Resize to its
            // observed geometry. Until then, as while a Resize still awaits the Host, it is
            // unknown.
            accepted_geometry: record
                .last_accepted_geometry()
                .filter(|accepted| *accepted == record.observed_geometry),
            frame,
        })
    }

    /// A controlling pane whose geometry the restore cannot vouch for owes the Host its Resize
    /// again. A Host resize is idempotent, so re-sending one needs no proof of what it applied.
    fn geometry_unknown(&self) -> bool {
        matches!(self.mode, ControllerMode::Controlling(_)) && self.accepted_geometry.is_none()
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

    fn projections_mut(&mut self) -> &mut [RetainedProjection] {
        match self {
            Self::Ready { projections, .. }
            | Self::Unbound { projections, .. }
            | Self::Lost { projections, .. } => projections,
            Self::Selected(_) | Self::Retryable(_) => &mut [],
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

    /// The Host applied a Resize to a retained pane while the Switch was in flight. The pane's
    /// geometry becomes unknown, which a restore resolves by re-sending the Resize.
    pub(super) fn forget_geometry(&mut self, pane: &PaneRef) {
        if let Some(saved) = self.anchor.as_mut().and_then(|anchor| {
            anchor
                .projections_mut()
                .iter_mut()
                .find(|saved| saved.pane == *pane)
        }) {
            saved.accepted_geometry = None;
        }
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
                    // The next observation re-sends the Resize, as it resizes any visible pane.
                    if saved.geometry_unknown() {
                        record.last_admitted_geometry = None;
                    }
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

pub(crate) struct PreparedSwitchPark<'state> {
    state: &'state mut WorkbenchClientState,
    authority: SwitchRestoreKey,
    origin: SelectedTab,
}

impl PreparedSwitchPark<'_> {
    pub(crate) const fn restoration(&self) -> &SwitchRestoreKey {
        &self.authority
    }

    /// Returns to the origin without a viewer. The focus record is revived, or freshly minted
    /// when the cleanup dropped it; the origin's other records stay departing until an
    /// observation revives them. A definite attach fault is kept.
    pub(crate) fn commit(self) {
        let state = self.state;
        let origin = self.origin;
        state.desired = Some(origin.key.clone());
        for record in &mut state.panes {
            if record.key == origin.key {
                record.departing = false;
            } else if !record.departing {
                record.departing = true;
                if let Some(attachment) = &mut record.attachment {
                    attachment.withdraw_projection();
                }
            }
        }
        state.install_unattached_pane(&origin);
        let attachable = match state.pane_record_mut(&origin.key) {
            Some(record) if record.attachable() => {
                if matches!(
                    record.fault,
                    None | Some(PaneFault::ControlLost | PaneFault::ViewerLost)
                ) {
                    record.fault = Some(PaneFault::ViewerLost);
                }
                true
            }
            _ => false,
        };
        state.phase = if attachable {
            ClientPhase::RetryableAttachUnavailable(origin)
        } else {
            ClientPhase::SelectedWithoutAttachment(origin)
        };
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
                // The focus is captured with its siblings: a controlling one whose Resize still
                // awaits the Host is retained with its geometry unknown.
                self.focus_record()?.accepted_frame()?;
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
            // which the Switch cleans up first: an unbound member, a member that failed to
            // launch, or a lost focused viewer.
            ClientPhase::SelectedWithoutAttachment(selected)
            | ClientPhase::RetryableAttachUnavailable(selected) => self.attached_sibling(selected),
            _ => return false,
        };
        // A Switch admitted over another one returns where that one would: to its origin before
        // its candidate installed, and to the installed target after.
        let Some(origin) = (match &self.phase {
            ClientPhase::Switching(setup) => Some(setup.membership.as_ref().map_or_else(
                || setup.target.selected.clone(),
                |membership| membership.origin.clone(),
            )),
            _ => self.active().cloned(),
        }) else {
            return false;
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
            membership: Some(SwitchMembership {
                anchor,
                origin,
                parks_origin: false,
            }),
        });
        true
    }

    /// A refused Switch whose anchor cannot be restored detaches every viewer under that anchor
    /// before it parks the origin, as an expired Switch does before it retires.
    pub(crate) fn stage_switch_refusal_cleanup(
        &mut self,
        marker: &PaneKey,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        let cleanup = self.stage_switch_expiry_cleanup(marker)?;
        let ClientPhase::Switching(setup) = &mut self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        setup
            .membership
            .as_mut()
            .ok_or(CarrierFailure::Protocol)?
            .parks_origin = true;
        Ok(cleanup)
    }

    pub(crate) fn switch_parks_origin(&self, marker: &PaneKey) -> bool {
        self.switch_marker_is_current(marker)
            && matches!(&self.phase, ClientPhase::Switching(setup)
                if setup.membership.as_ref().is_some_and(|membership| membership.parks_origin))
    }

    /// Parks the origin of a refused Switch that cannot be restored, once no viewer is left.
    /// A focus that had a viewer waits as a lost one, so the Client re-attaches it after a pause;
    /// its siblings attach again as any parked focus's do.
    pub(crate) fn prepare_switch_park(
        &mut self,
        marker: &PaneKey,
    ) -> Result<PreparedSwitchPark<'_>, CarrierFailure> {
        if !self.invariant_holds()
            || !self.switch_marker_is_current(marker)
            || self.pending_membership.is_some()
            || self.has_attachments()
        {
            return Err(CarrierFailure::Protocol);
        }
        let ClientPhase::Switching(setup) = &self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        let origin = setup
            .membership
            .as_ref()
            .map(|membership| membership.origin.clone())
            .ok_or(CarrierFailure::Protocol)?;
        if origin.key.anchor().generation() >= marker.anchor().generation()
            || self.cache.tabs().get(origin.index).is_none_or(|tab| {
                tab.tab_id() != origin.key.anchor().tab_id()
                    || origin.panel_index >= tab.panel_count()
            })
        {
            return Err(CarrierFailure::Protocol);
        }
        Ok(PreparedSwitchPark {
            state: self,
            authority: SwitchRestoreKey {
                marker: marker.clone(),
                restored: origin.key.clone(),
                panes: Box::default(),
            },
            origin,
        })
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
        // A window observed at another size while the Switch was in flight is a newer
        // observation, not changed pane evidence: the retained panes were never resized. The
        // restore returns the focus to the geometry it last observed, and the next observation
        // resizes the restored panes as it resizes any visible pane.
        let observed_geometry =
            match anchor {
                StableAnchor::Ready {
                    ready,
                    observed_geometry,
                    ..
                } => {
                    if setup.previous.as_ref().is_none_or(|previous| {
                        previous.selected != *ready.selected()
                            || previous.mode.as_ref() != Some(ready.mode())
                    }) {
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
                    if setup.previous.as_ref().is_none_or(|previous| {
                        !projections.iter().any(|saved| {
                            saved.pane.pane() == previous.selected.key()
                                && previous.mode.as_ref() == Some(&saved.mode)
                        })
                    }) {
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
