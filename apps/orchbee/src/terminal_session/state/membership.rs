use super::{
    AttachTarget, AttachmentKey, CarrierFailure, CreateTerminalProvenance, NativeSessionState,
    NativeStandaloneTabs, OperationDeadlines, SelectedTab, SessionPhase, SwitchSetup,
    WorkbenchTabId, validate_canonical_refresh,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum MembershipResultKey {
    Refresh(u64),
    Switch(AttachmentKey),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct MembershipCleanup {
    result: MembershipResultKey,
    old: AttachmentKey,
    deadlines: OperationDeadlines,
    installs_candidate: bool,
}

impl MembershipCleanup {
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
    baseline: NativeStandaloneTabs,
    baseline_desired: Option<AttachmentKey>,
    candidate: NativeStandaloneTabs,
    provenance: Vec<CreateTerminalProvenance>,
    selected_index: Option<usize>,
}

impl PreparedMembershipCandidate {
    pub(in crate::terminal_session) fn preserves_selection(&self) -> bool {
        self.selected_tab_id() == self.baseline_desired.as_ref().map(AttachmentKey::tab_id)
    }

    pub(in crate::terminal_session) fn selected_tab_id(&self) -> Option<&WorkbenchTabId> {
        self.selected_index
            .map(|index| self.candidate.tabs()[index].tab_id())
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
    desired: Option<AttachmentKey>,
    replacement_phase: SessionPhase,
    clear_attachment: bool,
    decision: MembershipInstallDecision,
    refresh_generation: Option<u64>,
}

impl PreparedMembershipInstall<'_> {
    pub(in crate::terminal_session) fn commit(self) -> MembershipInstallDecision {
        if self.refresh_generation.is_some() {
            self.state.pending_refresh_generation = None;
            self.state.refresh_status = Some(super::NavigatorRefreshStatus::Success(
                self.prepared.candidate.revision(),
            ));
            self.state.move_recovery = None;
        }
        self.state.cache = self.prepared.candidate;
        self.state.provenance = self.prepared.provenance;
        self.state.desired = self.desired;
        self.state.phase = self.replacement_phase;
        self.state.pending_membership = None;
        if self.clear_attachment {
            self.state.attached_key = None;
            self.state.projection_key = None;
            self.state.staged_frame = None;
            self.state.accepted_frame = None;
            self.state.last_admitted_geometry = None;
            self.state.last_accepted_geometry = None;
        }
        self.decision
    }
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn prepare_membership_candidate(
        &self,
        candidate: NativeStandaloneTabs,
        preferred_tab: Option<&WorkbenchTabId>,
        historical_ordinal: Option<usize>,
        creator: Option<(&WorkbenchTabId, CreateTerminalProvenance)>,
    ) -> Result<PreparedMembershipCandidate, CarrierFailure> {
        if !self.invariant_holds() {
            return Err(CarrierFailure::Protocol);
        }
        let candidate = validate_canonical_refresh(&self.cache, candidate)
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
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
                self.cache
                    .tab_index(tab.tab_id())
                    .map_or(CreateTerminalProvenance::BindingOnly, |index| {
                        self.provenance[index].clone()
                    })
            })
            .collect();
        Ok(PreparedMembershipCandidate {
            baseline: self.cache.clone(),
            baseline_desired: self.desired.clone(),
            candidate,
            provenance,
            selected_index,
        })
    }

    pub(in crate::terminal_session) fn prepare_membership_install(
        &mut self,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        self.prepare_membership_install_inner(prepared, desired, deadlines, None)
    }

    fn prepare_membership_install_inner(
        &mut self,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
        cleaned: Option<&MembershipCleanup>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        let switch_marker = match cleaned.map(MembershipCleanup::result) {
            Some(MembershipResultKey::Switch(marker)) => Some(marker),
            _ => match &self.phase {
                SessionPhase::Switching(setup) if setup.membership.is_some() => {
                    Some(setup.target.selected.key())
                }
                _ => None,
            },
        };
        let current_switch =
            switch_marker.is_some_and(|marker| self.switch_marker_is_current(marker));
        let exact_cleanup = cleaned.is_some_and(|cleanup| {
            self.pending_membership
                .as_ref()
                .is_some_and(|pending| &pending.cleanup == cleanup)
                && self.attached_key.as_ref() == Some(cleanup.old())
                && (matches!(self.phase, SessionPhase::ReconcilingMembership(_)) || current_switch)
        });
        if !self.invariant_holds()
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || desired.as_ref().map(AttachmentKey::tab_id) != prepared.selected_tab_id()
            || (!exact_cleanup
                && !current_switch
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
        let preserve = desired == self.desired;
        if current_switch && preserve {
            return Err(CarrierFailure::Protocol);
        }
        if !preserve
            && desired
                .as_ref()
                .zip(self.desired.as_ref())
                .is_some_and(|(next, old)| next.generation() <= old.generation())
        {
            return Err(CarrierFailure::Protocol);
        }
        if !preserve
            && !exact_cleanup
            && (self.attached_key.is_some()
                || self.projection_key.is_some()
                || self.staged_frame.is_some()
                || self.accepted_frame.is_some())
        {
            return Err(CarrierFailure::Protocol);
        }
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
        } else if let Some(key) = &desired {
            let index = prepared.selected_index.ok_or(CarrierFailure::Protocol)?;
            let selected = SelectedTab::new(key.clone(), index);
            let provenance = prepared.provenance[index].clone();
            if provenance.attachable() {
                let tab = &prepared.candidate.tabs()[index];
                let target = AttachTarget {
                    selected,
                    site_id: prepared.candidate.site_id().clone(),
                    binding: tab.binding().clone(),
                    provenance,
                    deadlines: deadlines.ok_or(CarrierFailure::Deadline)?,
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
        Ok(PreparedMembershipInstall {
            state: self,
            prepared,
            desired,
            replacement_phase,
            clear_attachment: !preserve,
            decision,
            refresh_generation: None,
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
            || self.pending_refresh_generation != Some(generation)
            || self.cache != prepared.baseline
            || self.desired != prepared.baseline_desired
            || prepared.preserves_selection()
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self.attached_key.clone().ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key() != &old || !matches!(self.phase, SessionPhase::Ready(_)) {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Refresh(generation),
            old,
            deadlines,
            installs_candidate: true,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        self.phase = SessionPhase::ReconcilingMembership(selected);
        Ok(cleanup)
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
        })
    }

    pub(in crate::terminal_session) fn prepare_cleaned_membership_install(
        &mut self,
        cleanup: &MembershipCleanup,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
    ) -> Result<PreparedMembershipInstall<'_>, CarrierFailure> {
        let refresh_generation = match &cleanup.result {
            MembershipResultKey::Refresh(generation) => {
                if self.pending_refresh_generation != Some(*generation) {
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
        };
        let mut plan = self.prepare_membership_install_inner(
            prepared,
            desired,
            Some(cleanup.deadlines),
            Some(cleanup),
        )?;
        plan.refresh_generation = refresh_generation;
        Ok(plan)
    }

    pub(super) fn membership_invariant_holds(&self) -> bool {
        match (&self.pending_membership, &self.phase) {
            (None, SessionPhase::ReconcilingMembership(_)) => false,
            (None, _) => true,
            (Some(pending), SessionPhase::ReconcilingMembership(selected)) => {
                let Some(prepared) = &pending.prepared else {
                    return false;
                };
                matches!(pending.cleanup.result, MembershipResultKey::Refresh(generation)
                    if self.pending_refresh_generation == Some(generation))
                    && &pending.cleanup.old == selected.key()
                    && pending.cleanup.installs_candidate
                    && self.cache == prepared.baseline
                    && self.desired == prepared.baseline_desired
                    && !prepared.preserves_selection()
            }
            (Some(pending), SessionPhase::Switching(setup)) => {
                matches!(&pending.cleanup.result, MembershipResultKey::Switch(marker)
                    if pending.prepared.as_ref().is_none_or(|prepared| prepared.baseline_desired.as_ref() == Some(marker))
                        && self.desired.as_ref().is_some_and(|desired| desired.generation() >= marker.generation()))
                    && pending.cleanup.installs_candidate == pending.prepared.is_some()
                    && setup.membership.is_some()
                    && pending
                        .prepared
                        .as_ref()
                        .is_none_or(|prepared| self.cache == prepared.baseline)
                    && self.attached_key.as_ref() == Some(pending.cleanup.old())
                    && setup
                        .previous
                        .as_ref()
                        .is_some_and(|previous| previous.selected.key() == pending.cleanup.old())
            }
            (Some(_), _) => false,
        }
    }

    pub(in crate::terminal_session) fn stage_switch_membership_cleanup(
        &mut self,
        marker: &AttachmentKey,
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
        let old = self.attached_key.clone().ok_or(CarrierFailure::Protocol)?;
        if setup.target.deadlines() != deadlines
            || setup
                .previous
                .as_ref()
                .is_none_or(|previous| previous.selected.key() != &old)
        {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Switch(marker.clone()),
            old,
            deadlines,
            installs_candidate: true,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: Some(prepared),
        });
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn stage_switch_expiry_cleanup(
        &mut self,
        marker: &AttachmentKey,
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
        let old = self.attached_key.clone().ok_or(CarrierFailure::Protocol)?;
        if setup
            .previous
            .as_ref()
            .is_none_or(|previous| previous.selected.key() != &old)
        {
            return Err(CarrierFailure::Protocol);
        }
        let cleanup = MembershipCleanup {
            result: MembershipResultKey::Switch(marker.clone()),
            old,
            deadlines: setup.target.deadlines(),
            installs_candidate: false,
        };
        self.pending_membership = Some(PendingMembership {
            cleanup: cleanup.clone(),
            prepared: None,
        });
        Ok(cleanup)
    }

    pub(in crate::terminal_session) fn prepare_switch_membership_install(
        &mut self,
        marker: &AttachmentKey,
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
        self.prepare_membership_install_inner(prepared, desired, Some(deadlines), None)
    }

    /// Admission already holds the mailbox gate. Only an authorized old call retains its candidate.
    pub(in crate::terminal_session) fn finish_switch_membership_admission(
        &mut self,
        abandoned_causal: bool,
        authorized_cleanup: bool,
    ) {
        if abandoned_causal {
            self.invalidate_switch_anchor();
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
            || self.attached_key.as_ref() != Some(cleanup.old())
        {
            return Err(CarrierFailure::Protocol);
        }
        let SessionPhase::Switching(setup) = &mut self.phase else {
            return Err(CarrierFailure::Protocol);
        };
        if setup
            .previous
            .as_ref()
            .is_none_or(|previous| previous.selected.key() != cleanup.old())
        {
            return Err(CarrierFailure::Protocol);
        }
        self.pending_membership = None;
        if detached {
            setup.previous = None;
            if let Some(membership) = &mut setup.membership {
                membership.invalidate();
            }
            self.attached_key = None;
            self.projection_key = None;
            self.staged_frame = None;
            self.accepted_frame = None;
            self.last_accepted_geometry = None;
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
        if self.pending_refresh_generation != Some(generation) {
            return Err(CarrierFailure::Protocol);
        }
        let mut plan = self.prepare_membership_install(prepared, desired, deadlines)?;
        plan.refresh_generation = Some(generation);
        Ok(plan)
    }
}
