use super::{
    AttachTarget, AttachmentKey, CarrierFailure, CreateTerminalProvenance, NativeContextTabs,
    NativeSessionState, OperationDeadlines, PaneKey, SelectedTab, SessionPhase, SwitchSetup,
    WorkbenchTabId, context_removal::ContextRemovalPermit,
};
mod close;
use crate::terminal_session::NativePanel;
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchRequestId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum MembershipResultKey {
    Refresh(u64),
    Create(super::CreateOperationKey),
    Close(super::CloseFlightKey),
    Switch(PaneKey),
    Move(WorkbenchRequestId),
    Panel(WorkbenchRequestId),
    Rename(WorkbenchRequestId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct MembershipCleanup {
    result: MembershipResultKey,
    old: AttachmentKey,
    deadlines: OperationDeadlines,
    installs_candidate: bool,
    close_marker: Option<Box<super::CloseReconciliationMarker>>,
    closed_proof: Option<std::sync::Arc<super::DefinitiveClosedAttachment>>,
}

impl MembershipCleanup {
    pub(in crate::terminal_session) fn close_marker(
        &self,
    ) -> Option<&super::CloseReconciliationMarker> {
        self.close_marker.as_deref()
    }
    pub(in crate::terminal_session) fn closed_proof(
        &self,
    ) -> Option<&std::sync::Arc<super::DefinitiveClosedAttachment>> {
        self.closed_proof.as_ref()
    }
    pub(in crate::terminal_session) const fn result(&self) -> &MembershipResultKey {
        &self.result
    }
    pub(in crate::terminal_session) const fn old(&self) -> &AttachmentKey {
        &self.old
    }
    pub(in crate::terminal_session) const fn deadlines(&self) -> OperationDeadlines {
        self.deadlines
    }
    pub(in crate::terminal_session) const fn installs_candidate(&self) -> bool {
        self.installs_candidate
    }
}

pub(super) struct PendingMembership {
    cleanup: MembershipCleanup,
    prepared: Option<PreparedMembershipCandidate>,
}

/// Prepared outside the reply mutex; this owns all candidate allocations and its exact baseline.
#[derive(Debug)]
pub(in crate::terminal_session) struct PreparedMembershipCandidate {
    baseline: NativeContextTabs,
    baseline_desired: Option<PaneKey>,
    candidate: NativeContextTabs,
    provenance: Vec<CreateTerminalProvenance>,
    selected_index: Option<usize>,
    selected_panel: Option<usize>,
    presentation_override: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    retained_contexts: Option<Vec<super::context_selection::RetainedContext>>,
    setup_completion: Option<Box<super::workspace_setup::PreparedSetupCompletion>>,
    /// One-shot cross-context install authority minted by the removal
    /// classifier; `None` on every ordinary candidate. Bounded, private and
    /// consumed by the single install it authorizes.
    context_removal: Option<ContextRemovalPermit>,
}

impl PreparedMembershipCandidate {
    #[cfg(windows)]
    pub(in crate::terminal_session) fn context_location(
        &self,
    ) -> &crate::terminal_session::NativeContextLocation {
        self.candidate.location()
    }
    #[cfg(windows)]
    pub(in crate::terminal_session) fn menu_context_source(
        &self,
    ) -> Option<crate::terminal_session::workbench_projection::NativeMenuContextSource> {
        self.candidate
            .menu_context_source(self.candidate.location())
    }
    #[cfg(windows)]
    pub(in crate::terminal_session) fn menu_selected_tab_matches(
        &self,
        tab: &super::NativeSimpleTab,
    ) -> bool {
        self.selected_tab() == Some(tab)
    }
    pub(in crate::terminal_session) fn context_removal(&self) -> Option<&ContextRemovalPermit> {
        self.context_removal.as_ref()
    }

    pub(in crate::terminal_session) fn preserves_selection(&self) -> bool {
        self.selected_panel.is_none()
            && self.candidate.location() == self.baseline.location()
            && self.selected_tab_id()
                == self
                    .baseline_desired
                    .as_ref()
                    .map(|key| key.anchor().tab_id())
    }

    /// The surviving Tab kept its identity but the selected leaf vanished: the membership install
    /// must re-attach to the repaired leaf instead of preserving the stale attachment.
    pub(in crate::terminal_session) fn retarget_selected_panel(&mut self, panel_index: usize) {
        self.selected_panel = Some(panel_index);
    }

    pub(in crate::terminal_session) fn retarget_selected_member(
        &mut self,
        location: &crate::terminal_session::NativeContextLocation,
        member: &crate::terminal_session::NativeMemberTarget,
    ) -> bool {
        if self.candidate.location() != location {
            return false;
        }
        let Some(tab_index) = self.candidate.tab_index(&member.tab) else {
            return false;
        };
        let tab = &self.candidate.tabs()[tab_index];
        let Some(panel_index) = tab.panel_index(&member.panel) else {
            return false;
        };
        if tab
            .panel(panel_index)
            .is_none_or(|panel| panel.primary().presentation_id() != &member.presentation)
        {
            return false;
        }
        self.selected_index = Some(tab_index);
        self.selected_panel = Some(panel_index);
        self.presentation_override = Some(member.presentation.clone());
        true
    }

    pub(in crate::terminal_session) fn selected_tab_id(&self) -> Option<&WorkbenchTabId> {
        self.selected_index
            .map(|index| self.candidate.tabs()[index].tab_id())
    }

    pub(super) fn selected_tab(&self) -> Option<&super::NativeSimpleTab> {
        self.selected_index
            .map(|index| &self.candidate.tabs()[index])
    }

    /// The panel the install will focus — the same resolution the state-side
    /// install applies (`selected_panel`, defaulting to the first leaf). The
    /// mailbox desired-install must name this exact panel or the focus pane
    /// would diverge from the record the state mints.
    pub(in crate::terminal_session) fn focus_panel_id(&self) -> Option<&WorkbenchPanelId> {
        let index = self.selected_index?;
        self.candidate
            .tabs()
            .get(index)?
            .panel(self.selected_panel.unwrap_or(0))
            .map(NativePanel::panel_id)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum MembershipInstallDecision {
    Preserved,
    Attach(AttachTarget),
    SelectedWithoutAttachment,
    Empty,
}

/// Exclusive state borrowing prevents a validated plan from surviving another state mutation.
pub(in crate::terminal_session) struct PreparedMembershipInstall<'a> {
    state: &'a mut NativeSessionState,
    prepared: PreparedMembershipCandidate,
    desired: Option<PaneKey>,
    replacement_phase: SessionPhase,
    selected_presentation: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    preserve: bool,
    decision: MembershipInstallDecision,
    refresh_generation: Option<u64>,
    settled_create: Option<super::CommittedTabCreate>,
    #[cfg(windows)]
    duplicate_witness: Option<super::tab_duplication::DuplicateInstallWitness>,
    close_consumption: Option<super::tab_close::PreparedCloseConsumption>,
    mutation_completion: MembershipMutationCompletion,
    /// The anchor whose viewers the cleanup already disposed (wire detach or
    /// closed proof): its records drop outright at commit — no per-pane
    /// disposal work remains for them.
    drop_anchor: Option<AttachmentKey>,
}

enum MembershipMutationCompletion {
    None,
    Move,
    Panel,
    Rename(WorkbenchRequestId),
}

impl PreparedMembershipInstall<'_> {
    pub(in crate::terminal_session) fn commit(self) -> MembershipInstallDecision {
        self.state.record_workspace_close_landing(
            self.prepared.context_removal.as_ref(),
            self.refresh_generation,
            &self.prepared.candidate,
        );
        if let Some(consumption) = &self.close_consumption {
            self.state
                .before_close_set_member_install(consumption.target());
        }
        #[cfg(windows)]
        let foreground_created = self
            .settled_create
            .as_ref()
            .filter(|committed| !committed.operation.preserves_selection())
            .and_then(|committed| {
                self.prepared
                    .candidate
                    .tabs()
                    .iter()
                    .find(|tab| tab.tab_id() == committed.committed_key.tab_id())
            });
        #[cfg(windows)]
        let clear_selection = foreground_created.is_some();
        #[cfg(windows)]
        let activated_group = foreground_created
            .and_then(|tab| tab.group())
            .map(|group| group.group_id.clone());
        if let Some(committed) = self
            .settled_create
            .as_ref()
            .filter(|committed| committed.operation.preserves_selection())
            .filter(|committed| {
                #[cfg(windows)]
                {
                    !self
                        .state
                        .duplicate_owns_operation(committed.operation.key())
                }
                #[cfg(not(windows))]
                {
                    let _ = committed;
                    true
                }
            })
        {
            self.state.notice = match committed.provenance {
                CreateTerminalProvenance::Pending => Some(super::SessionNotice::DuplicatePending),
                CreateTerminalProvenance::Failed(_) | CreateTerminalProvenance::Unavailable(_) => {
                    Some(super::SessionNotice::DuplicateNotStarted)
                }
                CreateTerminalProvenance::OutcomeUnknown => {
                    Some(super::SessionNotice::DuplicateOutcomeUnknown)
                }
                _ => None,
            };
        }
        if let Some(committed) = self.settled_create {
            self.state.settled_create = Some(committed);
        }
        match self.mutation_completion {
            MembershipMutationCompletion::Move => {
                self.state.pending_mutation = None;
                self.state.move_recovery = None;
            }
            MembershipMutationCompletion::Panel => {
                self.state.pending_mutation = None;
                self.state.panel_recovery = None;
            }
            MembershipMutationCompletion::None | MembershipMutationCompletion::Rename(_) => {}
        }
        if self.refresh_generation.is_some() {
            self.state.pending_refresh = None;
            self.state.install_publication_debt();
            self.state.refresh_status = Some(super::NavigatorRefreshStatus::Success(
                self.prepared.candidate.revision(),
            ));
            self.state.move_recovery = None;
            self.state.panel_recovery = None;
            self.state.rename_recovery = None;
        }
        if let Some(retained) = self.prepared.retained_contexts {
            self.state.retained_contexts = retained;
        }
        if self.refresh_generation.is_some() {
            self.state.context_selection = None;
        }
        // The candidate's leaf set is computed before the move so the pane sync
        // can compare the old records against the new membership.
        let leaf_ids = if self.preserve {
            std::collections::BTreeSet::new()
        } else {
            self.prepared
                .selected_index
                .and_then(|index| self.prepared.candidate.tabs().get(index))
                .map(|tab| tab.leaf_panel_ids().into_iter().collect())
                .unwrap_or_default()
        };
        #[cfg(windows)]
        let overlay_focus = self.state.focus_panel_id().cloned();
        #[cfg(windows)]
        self.state.remember_tab_group();
        self.state.cache = self.prepared.candidate;
        self.state.provenance = self.prepared.provenance;
        self.state.desired = self.desired.clone();
        self.state.phase = self.replacement_phase;
        #[cfg(windows)]
        if clear_selection {
            self.state.clear_tab_selection();
        }
        #[cfg(windows)]
        if let Some(group) = activated_group
            && self.state.group_collapsed(&group)
        {
            self.state.toggle_tab_group(&group);
        }
        if let Some(id) = self.selected_presentation
            && let Some(selected) = self.state.active().cloned()
        {
            self.state
                .navigation_memory
                .remember_presentation(&self.state.cache, &selected, id);
        }
        #[cfg(windows)]
        self.state.prune_plugin_overlays(overlay_focus.as_ref());
        self.state.prune_zoomed_tabs();
        self.state.prune_navigation_memory();
        if let Some(completion) = self.prepared.setup_completion {
            self.state.complete_setup_refresh(*completion);
        }
        self.state.pending_membership = None;
        if let MembershipMutationCompletion::Rename(request_id) = self.mutation_completion {
            debug_assert!(
                self.state
                    .pending_rename()
                    .is_some_and(|operation| operation.request_id() == &request_id)
            );
            self.state.pending_mutation = None;
            self.state.rename_recovery = None;
        }
        if let Some(anchor) = &self.drop_anchor {
            self.state.drop_anchor_panes(anchor);
        }
        if !self.preserve {
            Self::sync_membership_panes(
                self.state,
                self.desired.as_ref(),
                &leaf_ids,
                &self.decision,
            );
        }
        if let Some(consumption) = self.close_consumption {
            let target = consumption.target().clone();
            self.state.close.commit_consumption(consumption);
            self.state.installed_close_set_member(&target);
        }
        #[cfg(windows)]
        if let Some(witness) = self.duplicate_witness {
            self.state.commit_duplicate_install(witness);
        }
        self.decision
    }

    /// Installs the new epoch's pane set: records the desired anchor does not
    /// keep become departing (canonical disposal follows through the epoch
    /// cleanup or the next reconcile), never-attached departing records drop
    /// outright, and the focus pane's fresh record is minted. Runs after the
    /// candidate cache is installed so record installs resolve the new facts.
    fn sync_membership_panes(
        state: &mut NativeSessionState,
        desired: Option<&PaneKey>,
        leaf_ids: &std::collections::BTreeSet<orchbee_workbench_protocol::WorkbenchPanelId>,
        decision: &MembershipInstallDecision,
    ) {
        for record in &mut state.panes {
            let survives = desired.is_some_and(|pane| {
                record.key.anchor() == pane.anchor() && leaf_ids.contains(record.key.panel())
            });
            record.departing = !survives;
        }
        state
            .panes
            .retain(|record| !record.departing || record.attached);
        match decision {
            MembershipInstallDecision::Attach(target) => {
                state.install_pane_record(target);
            }
            MembershipInstallDecision::SelectedWithoutAttachment => {
                if let Some(selected) = state.active().cloned() {
                    state.install_unattached_pane(&selected);
                }
            }
            MembershipInstallDecision::Preserved | MembershipInstallDecision::Empty => {}
        }
    }
}

impl NativeSessionState {
    /// Selection may be empty while this epoch still owns live sibling viewers.
    pub(super) fn membership_cleanup_ready(&self) -> bool {
        (matches!(self.phase, SessionPhase::Ready(_))
            || self.focused_viewer_lost()
            || (matches!(self.phase, SessionPhase::SelectedWithoutAttachment(_))
                && self.selected_is_unbound()))
            && self.desired.as_ref().is_some_and(|desired| {
                self.panes
                    .iter()
                    .any(|record| record.key.anchor() == desired.anchor() && record.attached)
            })
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(super) fn prepare_retired_membership_install_inner(
        &mut self,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: OperationDeadlines,
        generation: u64,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        let mut plan = self.prepare_membership_install_inner(
            prepared,
            desired,
            Some(deadlines),
            None,
            None,
            true,
        )?;
        plan.refresh_generation = Some(generation);
        Ok(plan)
    }

    pub(in crate::terminal_session) fn stage_create_refresh_cleanup(
        &mut self,
        generation: u64,
        operation: &super::CreateOperationKey,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if self.pending_refresh_generation() != Some(generation) {
            return Err(CarrierFailure::Protocol);
        }
        self.stage_create_cleanup_inner(
            operation,
            prepared,
            deadlines,
            MembershipResultKey::Refresh(generation),
        )
    }

    pub(in crate::terminal_session) fn prepare_create_refresh_install(
        &mut self,
        generation: u64,
        operation: &super::CreateOperationKey,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: OperationDeadlines,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if self.pending_refresh_generation() != Some(generation) {
            return Err(CarrierFailure::Protocol);
        }
        let mut plan = self
            .prepare_create_membership_install_unchecked(operation, prepared, desired, deadlines)?;
        plan.refresh_generation = Some(generation);
        Ok(plan)
    }

    pub(in crate::terminal_session) fn stage_create_membership_cleanup(
        &mut self,
        operation: &super::CreateOperationKey,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if self
            .create_membership_commit()
            .is_none_or(|committed| committed.operation.deadlines() != deadlines)
        {
            return Err(CarrierFailure::Protocol);
        }
        self.stage_create_cleanup_inner(
            operation,
            prepared,
            deadlines,
            MembershipResultKey::Create(operation.clone()),
        )
    }

    fn stage_create_cleanup_inner(
        &mut self,
        operation: &super::CreateOperationKey,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
        result: MembershipResultKey,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if !self.invariant_holds()
            || self.pending_membership.is_some()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || self
                .create_membership_commit()
                .is_none_or(|committed| &committed.operation.key != operation)
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self
            .desired
            .as_ref()
            .map(|key| key.anchor().clone())
            .ok_or(CarrierFailure::Protocol)?;
        if self
            .create_membership_commit()
            .and_then(|committed| committed.anchor.ready())
            .is_none_or(|ready| ready.selected.key().anchor() != &old)
        {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result,
            old,
            deadlines,
            installs_candidate: true,
            close_marker: None,
            closed_proof: None,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn prepare_create_membership_install(
        &mut self,
        operation: &super::CreateOperationKey,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: OperationDeadlines,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if self
            .create_membership_commit()
            .is_none_or(|committed| committed.operation.deadlines() != deadlines)
        {
            return Err(CarrierFailure::Protocol);
        }
        self.prepare_create_membership_install_unchecked(operation, prepared, desired, deadlines)
    }

    fn prepare_create_membership_install_unchecked(
        &mut self,
        operation: &super::CreateOperationKey,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: OperationDeadlines,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if self.pending_membership.is_some()
            || self
                .create_membership_commit()
                .is_none_or(|committed| &committed.operation.key != operation)
        {
            return Err(CarrierFailure::Protocol);
        }
        self.prepare_membership_install_inner(prepared, desired, Some(deadlines), None, None, false)
    }

    pub(in crate::terminal_session) fn prepare_membership_candidate(
        &self,
        candidate: NativeContextTabs,
        preferred_tab: Option<&WorkbenchTabId>,
        historical_ordinal: Option<usize>,
        creator: Option<(&WorkbenchTabId, CreateTerminalProvenance)>,
        context_removal: Option<ContextRemovalPermit>,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        if !self.invariant_holds() {
            return Err(CarrierFailure::Protocol);
        }
        let baseline = self.validate_candidate_provenance(&candidate, context_removal.as_ref())?;
        let selected_index = preferred_tab
            .and_then(|tab| candidate.tab_index(tab))
            .or_else(|| {
                (!candidate.tabs().is_empty()).then(|| {
                    historical_ordinal
                        .unwrap_or(0)
                        .min(candidate.tabs().len() - 1)
                })
            });
        let provenance = candidate
            .tabs()
            .iter()
            .map(|tab| {
                if let Some((created, provenance)) = &creator
                    && *created == tab.tab_id()
                {
                    return provenance.clone();
                }
                baseline
                    .as_ref()
                    .and_then(|history| history.for_tab(tab))
                    .cloned()
                    .unwrap_or(CreateTerminalProvenance::BindingOnly)
            })
            .collect();
        // A Switch retains an exact Panel only while its requested Tab survives.
        // Resolve the ID against this candidate, never reuse its old ordinal.
        // A removed Tab keeps the existing fallback to another Tab or Empty.
        let selected_panel = match (&self.phase, selected_index) {
            (
                SessionPhase::CreateCommittedNeedDetach(committed)
                | SessionPhase::ProjectionUnavailableBeforeCleanup(committed),
                Some(index),
            ) if committed.operation.preserves_selection() => {
                committed.anchor.ready().and_then(|ready| {
                    let selected = ready.selected();
                    let tab = &candidate.tabs()[index];
                    (tab.tab_id() == selected.key().anchor().tab_id())
                        .then(|| tab.panel_index(selected.key().panel()))
                        .flatten()
                })
            }
            (SessionPhase::Switching(setup), Some(index))
                if setup.membership.is_some()
                    && candidate.tabs()[index].tab_id()
                        == setup.target.selected.key().anchor().tab_id() =>
            {
                Some(
                    candidate.tabs()[index]
                        .panel_index(setup.target.selected.key().panel())
                        .ok_or(CarrierFailure::AuthorityDivergence)?,
                )
            }
            _ => None,
        };
        Ok(PreparedMembershipCandidate {
            retained_contexts: self.prepare_retained_contexts(&candidate, context_removal.as_ref()),
            setup_completion: self.prepare_setup_completion(&candidate),
            baseline: self.cache.clone(),
            baseline_desired: self.desired.clone(),
            candidate,
            provenance,
            selected_index,
            selected_panel,
            presentation_override: None,
            context_removal,
        })
    }

    pub(in crate::terminal_session) fn prepare_membership_install(
        &mut self,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        self.prepare_membership_install_inner(prepared, desired, deadlines, None, None, false)
    }

    fn prepare_membership_install_inner(
        &mut self,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
        cleaned: Option<&MembershipCleanup>,
        close_marker: Option<&super::CloseReconciliationMarker>,
        retired_membership: bool,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        let close_consumption = close_marker
            .map(|marker| {
                self.close
                    .prepare_consumption(marker)
                    .map_err(|_| CarrierFailure::Protocol)
            })
            .transpose()?;
        let close_recovery =
            close_marker.is_some_and(|marker| self.close_membership_recovery_matches(marker));
        let switch_marker = match cleaned.map(MembershipCleanup::result) {
            Some(MembershipResultKey::Switch(marker)) => Some(marker.clone()),
            _ => match &self.phase {
                SessionPhase::Switching(setup) if setup.membership.is_some() => {
                    Some(setup.target.selected.key().clone())
                }
                _ => None,
            },
        };
        let current_switch =
            switch_marker.is_some_and(|marker| self.switch_marker_is_current(&marker));
        let committed_create = self.create_membership_commit().cloned();
        let current_create = committed_create.is_some();
        if let Some(committed) = &committed_create {
            let valid_desired = match &desired {
                Some(key) if key.tab_id() == committed.committed_key.tab_id() => {
                    key == &committed.committed_key
                }
                Some(key) => key.generation() > committed.committed_key.generation(),
                None => true,
            };
            if !valid_desired {
                return Err(CarrierFailure::Protocol);
            }
        }
        let exact_cleanup = cleaned.is_some_and(|cleanup| {
            self.pending_membership
                .as_ref()
                .is_some_and(|pending| &pending.cleanup == cleanup)
                && self
                    .panes
                    .iter()
                    .any(|record| &record.key.anchor == cleanup.old())
                && (matches!(self.phase, SessionPhase::ReconcilingMembership(_))
                    || current_switch
                    || current_create)
        });
        if !self.invariant_holds()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || desired.as_ref().map(AttachmentKey::tab_id) != prepared.selected_tab_id()
            || (!exact_cleanup
                && !current_switch
                && !current_create
                && !close_recovery
                && !retired_membership
                && !matches!(
                    self.phase,
                    SessionPhase::Empty
                        | SessionPhase::Ready(_)
                        | SessionPhase::SelectedWithoutAttachment(_)
                        | SessionPhase::RetryableAttachUnavailable(_)
                ))
        {
            return Err(CarrierFailure::Protocol);
        }
        let preserve = prepared.preserves_selection()
            && desired.as_ref() == self.desired.as_ref().map(PaneKey::anchor)
            && !current_create
            && !current_switch
            && !close_recovery
            && !retired_membership;
        if current_switch && preserve {
            return Err(CarrierFailure::Protocol);
        }
        if !preserve
            && desired
                .as_ref()
                .zip(self.desired.as_ref())
                .is_some_and(|(next, old)| next.generation() <= old.anchor().generation())
        {
            return Err(CarrierFailure::Protocol);
        }
        if !preserve
            && !exact_cleanup
            && self.panes.iter().any(|record| {
                record.attached
                    || record.projected
                    || record.staged_frame.is_some()
                    || record.accepted_frame.is_some()
            })
        {
            return Err(CarrierFailure::Protocol);
        }
        let selected_panel_index = prepared.selected_panel.unwrap_or(0);
        let desired_pane = if preserve {
            self.desired.clone()
        } else {
            match &desired {
                Some(anchor) => prepared
                    .selected_index
                    .and_then(|index| prepared.candidate.tabs().get(index))
                    .and_then(|tab| tab.panel(selected_panel_index))
                    .map(|panel| PaneKey::new(anchor.clone(), panel.panel_id().clone())),
                None => None,
            }
        };
        let mut selected_presentation = None;
        let (replacement_phase, decision) = if preserve {
            let mut phase = self.phase.clone();
            match &mut phase {
                SessionPhase::Ready(ready) => {
                    ready.selected.index =
                        prepared.selected_index.ok_or(CarrierFailure::Protocol)?
                }
                SessionPhase::SelectedWithoutAttachment(selected)
                | SessionPhase::RetryableAttachUnavailable(selected) => {
                    selected.index = prepared.selected_index.ok_or(CarrierFailure::Protocol)?
                }
                SessionPhase::Empty => {}
                _ => return Err(CarrierFailure::Protocol),
            }
            (phase, MembershipInstallDecision::Preserved)
        } else if let Some(pane) = &desired_pane {
            let index = prepared.selected_index.ok_or(CarrierFailure::Protocol)?;
            let selected = SelectedTab::new_panel(pane.clone(), index, selected_panel_index);
            let tab = &prepared.candidate.tabs()[index];
            let panel = tab
                .panel(selected.panel_index())
                .ok_or(CarrierFailure::Protocol)?;
            let member = if let Some(presentation) = &prepared.presentation_override {
                panel
                    .presentations()
                    .find(|member| member.presentation_id() == presentation)
                    .ok_or(CarrierFailure::AuthorityDivergence)?
            } else if prepared.candidate.location() == self.cache.location() {
                #[cfg(windows)]
                {
                    self.focus_panel_id()
                        .and_then(|removed| self.overlay_return_presentation(tab, removed, panel))
                        .unwrap_or_else(|| self.selected_presentation(panel))
                }
                #[cfg(not(windows))]
                {
                    self.selected_presentation(panel)
                }
            } else {
                self.navigation_memory
                    .presentation(prepared.candidate.location(), panel)
            };
            let provenance = if member.presentation_id() == panel.presentation_id() {
                prepared.provenance[index].clone()
            } else {
                CreateTerminalProvenance::BindingOnly
            };
            selected_presentation = Some(member.presentation_id().clone());
            if let Some(binding) = member
                .binding()
                .cloned()
                .filter(|_| provenance.attachable())
            {
                // A surviving record reuses its incarnation so a live viewer is
                // never re-attached; a fresh pane mints the next incarnation.
                let incarnation = match self.pane_record(pane) {
                    Some(record) => record.incarnation(),
                    None => {
                        let incarnation = self.next_incarnation;
                        self.next_incarnation += 1;
                        incarnation
                    }
                };
                let target = AttachTarget {
                    presentation_id: Box::new(member.presentation_id().clone()),
                    selected,
                    site_id: prepared.candidate.site_id().clone(),
                    binding: Some(binding),
                    provenance,
                    deadlines: deadlines.ok_or(CarrierFailure::Deadline)?,
                    incarnation,
                };
                (
                    SessionPhase::Switching(SwitchSetup {
                        target: target.clone(),
                        previous: None,
                        mode: None,
                        accepted_geometry: None,
                        membership: None,
                    }),
                    MembershipInstallDecision::Attach(target),
                )
            } else {
                (
                    SessionPhase::SelectedWithoutAttachment(selected),
                    MembershipInstallDecision::SelectedWithoutAttachment,
                )
            }
        } else {
            (SessionPhase::Empty, MembershipInstallDecision::Empty)
        };
        #[cfg(windows)]
        let duplicate_witness = self.prepare_duplicate_install(
            committed_create.as_ref(),
            &prepared.candidate,
            &decision,
        );
        Ok(PreparedMembershipInstall {
            state: self,
            prepared,
            desired: desired_pane,
            replacement_phase,
            selected_presentation,
            preserve,
            decision,
            refresh_generation: None,
            settled_create: committed_create,
            #[cfg(windows)]
            duplicate_witness,
            close_consumption,
            mutation_completion: MembershipMutationCompletion::None,
            drop_anchor: cleaned.map(|cleanup| cleanup.old().clone()),
        })
    }

    pub(in crate::terminal_session) fn stage_refresh_membership_cleanup(
        &mut self,
        generation: u64,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if !self.invariant_holds()
            || self.pending_membership.is_some()
            || self.pending_refresh_generation() != Some(generation)
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || prepared.preserves_selection()
            || prepared
                .context_removal
                .as_ref()
                .is_some_and(|permit| permit.generation() != generation)
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self
            .desired
            .as_ref()
            .map(|key| key.anchor().clone())
            .ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key().anchor() != &old || !self.membership_cleanup_ready() {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Refresh(generation),
            old,
            deadlines,
            installs_candidate: true,
            close_marker: None,
            closed_proof: None,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        self.phase = SessionPhase::ReconcilingMembership(selected);
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn stage_rename_membership_cleanup(
        &mut self,
        request_id: &orchbee_workbench_protocol::WorkbenchRequestId,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if !self.invariant_holds()
            || !self.rename_membership_is_current(request_id)
            || self.pending_membership.is_some()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || prepared.preserves_selection()
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self
            .desired
            .as_ref()
            .map(|key| key.anchor().clone())
            .ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key().anchor() != &old || !self.membership_cleanup_ready() {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Rename(request_id.clone()),
            old,
            deadlines,
            installs_candidate: true,
            close_marker: None,
            closed_proof: None,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        self.phase = SessionPhase::ReconcilingMembership(selected);
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn prepare_rename_membership_install(
        &mut self,
        request_id: &orchbee_workbench_protocol::WorkbenchRequestId,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if !self.rename_membership_is_current(request_id) {
            return Err(CarrierFailure::Protocol);
        }
        let mut plan = self.prepare_membership_install(prepared, desired, deadlines)?;
        plan.mutation_completion = MembershipMutationCompletion::Rename(request_id.clone());
        Ok(plan)
    }

    /// Duplicate only the bounded prepared data outside the reply mutex; the pending owner stays put
    /// until both install plans commit. Dropping either plan therefore preserves the pending receipt.
    pub(in crate::terminal_session) fn prepare_cleaned_membership_candidate(
        &self,
        cleanup: &MembershipCleanup,
        detached: &AttachmentKey,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        let pending = self
            .pending_membership
            .as_ref()
            .ok_or(CarrierFailure::Protocol)?;
        if &pending.cleanup != cleanup || cleanup.old() != detached || !self.invariant_holds() {
            return Err(CarrierFailure::Protocol);
        }
        let prepared = pending.prepared.as_ref().ok_or(CarrierFailure::Protocol)?;
        Ok(PreparedMembershipCandidate {
            baseline: prepared.baseline.clone(),
            baseline_desired: prepared.baseline_desired.clone(),
            candidate: prepared.candidate.clone(),
            provenance: prepared.provenance.clone(),
            selected_index: prepared.selected_index,
            selected_panel: prepared.selected_panel,
            presentation_override: prepared.presentation_override.clone(),
            retained_contexts: prepared.retained_contexts.clone(),
            setup_completion: prepared.setup_completion.clone(),
            context_removal: prepared.context_removal.clone(),
        })
    }

    pub(in crate::terminal_session) fn prepare_cleaned_membership_install(
        &mut self,
        cleanup: &MembershipCleanup,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        let refresh_generation = match &cleanup.result {
            MembershipResultKey::Rename(request_id) => {
                if !self.rename_membership_is_current(request_id) {
                    return Err(CarrierFailure::Protocol);
                }
                None
            }
            MembershipResultKey::Create(operation) => {
                if self
                    .create_membership_commit()
                    .is_none_or(|committed| &committed.operation.key != operation)
                {
                    return Err(CarrierFailure::Protocol);
                }
                None
            }
            MembershipResultKey::Close(key) => {
                if cleanup.close_marker().is_none_or(|marker| {
                    marker.origin().key() != key || self.close.prepare_consumption(marker).is_err()
                }) {
                    return Err(CarrierFailure::Protocol);
                }
                None
            }
            MembershipResultKey::Refresh(generation) => {
                if self.pending_refresh_generation() != Some(*generation) {
                    return Err(CarrierFailure::Protocol);
                }
                Some(*generation)
            }
            MembershipResultKey::Switch(marker) => {
                if !self.switch_marker_is_current(marker) {
                    return Err(CarrierFailure::Protocol);
                }
                None
            }
            MembershipResultKey::Move(request_id) => {
                if !self.move_membership_is_current(request_id) {
                    return Err(CarrierFailure::Protocol);
                }
                None
            }
            MembershipResultKey::Panel(request_id) => {
                if !self.panel_membership_is_current(request_id) {
                    return Err(CarrierFailure::Protocol);
                }
                None
            }
        };
        let mut plan = self.prepare_membership_install_inner(
            prepared,
            desired,
            Some(cleanup.deadlines),
            Some(cleanup),
            cleanup.close_marker(),
            false,
        )?;
        plan.refresh_generation = refresh_generation;
        if matches!(cleanup.result(), MembershipResultKey::Move(_)) {
            plan.mutation_completion = MembershipMutationCompletion::Move;
        }
        if matches!(cleanup.result(), MembershipResultKey::Panel(_)) {
            plan.mutation_completion = MembershipMutationCompletion::Panel;
        }
        if let MembershipResultKey::Rename(request_id) = cleanup.result() {
            plan.mutation_completion = MembershipMutationCompletion::Rename(request_id.clone());
        }
        Ok(plan)
    }

    pub(super) fn membership_invariant_holds(&self) -> bool {
        match (&self.pending_membership, &self.phase) {
            (None, SessionPhase::ReconcilingMembership(_)) => false,
            (None, _) => true,
            (
                Some(pending),
                SessionPhase::CreateCommittedNeedDetach(committed)
                | SessionPhase::ProjectionUnavailableBeforeCleanup(committed),
            ) => {
                let Some(prepared) = &pending.prepared else {
                    return false;
                };
                let exact_origin = match &pending.cleanup.result {
                    MembershipResultKey::Create(operation) => operation == &committed.operation.key,
                    MembershipResultKey::Refresh(generation) => {
                        self.pending_refresh_generation() == Some(*generation)
                    }
                    MembershipResultKey::Close(_)
                    | MembershipResultKey::Rename(_)
                    | MembershipResultKey::Move(_)
                    | MembershipResultKey::Panel(_)
                    | MembershipResultKey::Switch(_) => false,
                };
                exact_origin
                    && pending.cleanup.installs_candidate
                    && self.cache == prepared.baseline
                    && self.desired == prepared.baseline_desired
                    && self.panes.iter().any(|record| {
                        &record.key.anchor == pending.cleanup.old() && record.attached
                    })
                    && committed
                        .anchor
                        .ready()
                        .is_some_and(|ready| ready.selected.key().anchor() == pending.cleanup.old())
            }
            (Some(pending), SessionPhase::ReconcilingMembership(selected)) => {
                let Some(prepared) = &pending.prepared else {
                    return false;
                };
                (match &pending.cleanup.result {
                    MembershipResultKey::Refresh(generation) => {
                        self.pending_refresh_generation() == Some(*generation)
                    }
                    MembershipResultKey::Move(request_id) => {
                        self.move_membership_is_current(request_id)
                    }
                    MembershipResultKey::Panel(request_id) => {
                        self.panel_membership_is_current(request_id)
                    }
                    MembershipResultKey::Rename(request_id) => {
                        self.rename_membership_is_current(request_id)
                    }
                    MembershipResultKey::Close(_) => pending
                        .cleanup
                        .close_marker()
                        .is_some_and(|marker| self.close.prepare_consumption(marker).is_ok()),
                    MembershipResultKey::Create(_) | MembershipResultKey::Switch(_) => false,
                }) && selected.key().anchor() == &pending.cleanup.old
                    && pending.cleanup.installs_candidate
                    && self.cache == prepared.baseline
                    && self.desired == prepared.baseline_desired
                    && !prepared.preserves_selection()
            }
            (Some(pending), SessionPhase::Switching(setup)) => {
                matches!(&pending.cleanup.result, MembershipResultKey::Switch(marker)
                    if pending.prepared.as_ref().is_none_or(|prepared| prepared.baseline_desired.as_ref() == Some(marker))
                        && self.desired.as_ref().is_some_and(|desired| desired.anchor().generation() >= marker.anchor().generation()))
                    && pending.cleanup.installs_candidate == pending.prepared.is_some()
                    && setup.membership.is_some()
                    && pending
                        .prepared
                        .as_ref()
                        .is_none_or(|prepared| self.cache == prepared.baseline)
                    && self.panes.iter().any(|record| {
                        &record.key.anchor == pending.cleanup.old() && record.attached
                    })
                    && setup.previous.as_ref().is_some_and(|previous| {
                        previous.selected.key().anchor() == &pending.cleanup.old
                    })
            }
            (Some(_), _) => false,
        }
    }

    pub(in crate::terminal_session) fn stage_switch_membership_cleanup(
        &mut self,
        marker: &PaneKey,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if !self.invariant_holds()
            || !self.switch_marker_is_current(marker)
            || self.pending_membership.is_some()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
        {
            return Err(CarrierFailure::Protocol);
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        // The cleaned anchor is the outgoing pane's — `desired` already names
        // the switch target, so the previous selection (or any still-attached
        // non-desired record) supplies it.
        let old = setup
            .previous
            .as_ref()
            .map(|previous| previous.selected.key().anchor().clone())
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|record| record.attached && Some(&record.key) != self.desired.as_ref())
                    .map(|record| record.key.anchor().clone())
            })
            .ok_or(CarrierFailure::Protocol)?;
        if setup.target.deadlines() != deadlines
            || setup
                .previous
                .as_ref()
                .is_none_or(|previous| previous.selected.key().anchor() != &old)
        {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Switch(marker.clone()),
            old,
            deadlines,
            installs_candidate: true,
            close_marker: None,
            closed_proof: None,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn stage_switch_expiry_cleanup(
        &mut self,
        marker: &PaneKey,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if !self.invariant_holds()
            || !self.switch_marker_is_current(marker)
            || self.pending_membership.is_some()
        {
            return Err(CarrierFailure::Protocol);
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        // The cleaned anchor is the outgoing pane's — `desired` already names
        // the switch target, so the previous selection (or any still-attached
        // non-desired record) supplies it.
        let old = setup
            .previous
            .as_ref()
            .map(|previous| previous.selected.key().anchor().clone())
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|record| record.attached && Some(&record.key) != self.desired.as_ref())
                    .map(|record| record.key.anchor().clone())
            })
            .ok_or(CarrierFailure::Protocol)?;
        if setup
            .previous
            .as_ref()
            .is_none_or(|previous| previous.selected.key().anchor() != &old)
        {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Switch(marker.clone()),
            old,
            deadlines: setup.target.deadlines(),
            installs_candidate: false,
            close_marker: None,
            closed_proof: None,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: None,
        });
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn prepare_switch_membership_install(
        &mut self,
        marker: &PaneKey,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if !self.switch_marker_is_current(marker) || self.pending_membership.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        let SessionPhase::Switching(setup) = &self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        let deadlines = setup.target.deadlines();
        self.prepare_membership_install_inner(prepared, desired, Some(deadlines), None, None, false)
    }

    /// Admission already holds the mailbox gate. Only an authorized old call retains its candidate.
    pub(in crate::terminal_session) fn finish_switch_membership_admission(
        &mut self,
        discarded: Option<&PaneKey>,
        inherited: Option<&PaneKey>,
        authorized_cleanup: bool,
    ) {
        for key in [discarded, inherited].into_iter().flatten() {
            self.invalidate_switch_anchor_for(key);
        }
        if !authorized_cleanup
            && matches!(
                self.pending_membership
                    .as_ref()
                    .map(|pending| pending.cleanup.result()),
                Some(MembershipResultKey::Switch(_))
            )
        {
            self.pending_membership = None;
        }
    }

    /// This settles only the exact old cleanup; it never installs a superseded candidate.
    pub(in crate::terminal_session) fn settle_switch_membership_cleanup(
        &mut self,
        cleanup: &MembershipCleanup,
        detached: bool,
    ) -> Result<(), CarrierFailure> {
        if !self.invariant_holds()
            || !matches!(cleanup.result(), MembershipResultKey::Switch(_))
            || self
                .pending_membership
                .as_ref()
                .is_none_or(|pending| &pending.cleanup != cleanup)
            || !self
                .panes
                .iter()
                .any(|record| &record.key.anchor == cleanup.old() && record.attached)
        {
            return Err(CarrierFailure::Protocol);
        }
        {
            let SessionPhase::Switching(setup) = &mut self.phase else {
                return Err(CarrierFailure::Protocol);
            };
            if setup
                .previous
                .as_ref()
                .is_none_or(|previous| previous.selected.key().anchor() != cleanup.old())
            {
                return Err(CarrierFailure::Protocol);
            }
            self.pending_membership = None;
            if detached {
                setup.previous = None;
                if let Some(membership) = &mut setup.membership {
                    membership.invalidate();
                }
            }
        }
        if detached {
            self.drop_anchor_panes(cleanup.old());
        }
        Ok(())
    }

    pub(in crate::terminal_session) fn prepare_refresh_membership_install(
        &mut self,
        generation: u64,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if self.pending_refresh_generation() != Some(generation)
            || prepared
                .context_removal
                .as_ref()
                .is_some_and(|permit| permit.generation() != generation)
        {
            return Err(CarrierFailure::Protocol);
        }
        let mut plan = self.prepare_membership_install(prepared, desired, deadlines)?;
        plan.refresh_generation = Some(generation);
        Ok(plan)
    }

    pub(in crate::terminal_session) fn stage_move_membership_cleanup(
        &mut self,
        request_id: &WorkbenchRequestId,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if !self.invariant_holds()
            || !self.move_membership_is_current(request_id)
            || self.pending_membership.is_some()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || prepared.preserves_selection()
            || !self.membership_cleanup_ready()
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self
            .desired
            .as_ref()
            .map(|key| key.anchor().clone())
            .ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key().anchor() != &old {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Move(request_id.clone()),
            old,
            deadlines,
            installs_candidate: true,
            close_marker: None,
            closed_proof: None,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        self.phase = SessionPhase::ReconcilingMembership(selected);
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn prepare_move_membership_install(
        &mut self,
        request_id: &WorkbenchRequestId,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if !self.move_membership_is_current(request_id) || self.pending_membership.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        let mut plan = self.prepare_membership_install(prepared, desired, deadlines)?;
        plan.mutation_completion = MembershipMutationCompletion::Move;
        Ok(plan)
    }

    pub(in crate::terminal_session) fn stage_panel_membership_cleanup(
        &mut self,
        request_id: &WorkbenchRequestId,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if !self.invariant_holds()
            || !self.panel_membership_is_current(request_id)
            || self.pending_membership.is_some()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || prepared.preserves_selection()
            || !self.membership_cleanup_ready()
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self
            .desired
            .as_ref()
            .map(|key| key.anchor().clone())
            .ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key().anchor() != &old {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Panel(request_id.clone()),
            old,
            deadlines,
            installs_candidate: true,
            close_marker: None,
            closed_proof: None,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        self.phase = SessionPhase::ReconcilingMembership(selected);
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn prepare_panel_membership_install(
        &mut self,
        request_id: &WorkbenchRequestId,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        if !self.panel_membership_is_current(request_id) || self.pending_membership.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        let mut plan = self.prepare_membership_install(prepared, desired, deadlines)?;
        plan.mutation_completion = MembershipMutationCompletion::Panel;
        Ok(plan)
    }
}
