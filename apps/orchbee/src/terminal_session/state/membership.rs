use super::{
    AttachTarget, AttachmentKey, CarrierFailure, CreateTerminalProvenance, NativeSessionState,
    NativeStandaloneTabs, OperationDeadlines, SelectedTab, SessionPhase, SwitchSetup,
    WorkbenchTabId, validate_canonical_refresh,
};
mod close;
use orchbee_workbench_protocol::WorkbenchRequestId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum MembershipResultKey {
    Refresh(u64),
    Create(super::CreateOperationKey),
    Close(super::CloseFlightKey),
    Switch(AttachmentKey),
    Move(WorkbenchRequestId),
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
    settled_create: Option<super::CommittedTabCreate>,
    close_consumption: Option<super::tab_close::PreparedCloseConsumption>,
    mutation_completion: MembershipMutationCompletion,
}

enum MembershipMutationCompletion {
    None,
    Move,
    Rename(WorkbenchRequestId),
}

impl PreparedMembershipInstall<'_> {
    pub(in crate::terminal_session) fn commit(self) -> MembershipInstallDecision {
        if let Some(committed) = self.settled_create {
            self.state.settled_create = Some(committed);
        }
        if matches!(self.mutation_completion, MembershipMutationCompletion::Move) {
            self.state.pending_mutation = None;
            self.state.move_recovery = None;
        }
        if self.refresh_generation.is_some() {
            self.state.pending_refresh_generation = None;
            self.state.refresh_status = Some(super::NavigatorRefreshStatus::Success(
                self.prepared.candidate.revision(),
            ));
            self.state.move_recovery = None;
            self.state.rename_recovery = None;
        }
        self.state.cache = self.prepared.candidate;
        self.state.provenance = self.prepared.provenance;
        self.state.desired = self.desired;
        self.state.phase = self.replacement_phase;
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
        if self.clear_attachment {
            self.state.attached_key = None;
            self.state.projection_key = None;
            self.state.staged_frame = None;
            self.state.accepted_frame = None;
            self.state.last_admitted_geometry = None;
            self.state.last_accepted_geometry = None;
        }
        if let Some(consumption) = self.close_consumption {
            self.state.close.commit_consumption(consumption);
        }
        self.decision
    }
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn stage_create_refresh_cleanup(
        &mut self,
        generation: u64,
        operation: &super::CreateOperationKey,
        prepared: PreparedMembershipCandidate,
        deadlines: OperationDeadlines,
    ) -> Result<MembershipCleanup, CarrierFailure> {
        if self.pending_refresh_generation != Some(generation) {
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
        if self.pending_refresh_generation != Some(generation) {
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
        let old = self.attached_key.clone().ok_or(CarrierFailure::Protocol)?;
        if self
            .create_membership_commit()
            .and_then(|committed| committed.anchor.ready())
            .is_none_or(|ready| ready.selected.key() != &old)
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
        self.prepare_membership_install_inner(prepared, desired, Some(deadlines), None, None)
    }

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
        self.prepare_membership_install_inner(prepared, desired, deadlines, None, None)
    }

    fn prepare_membership_install_inner(
        &mut self,
        prepared: PreparedMembershipCandidate,
        desired: Option<AttachmentKey>,
        deadlines: Option<OperationDeadlines>,
        cleaned: Option<&MembershipCleanup>,
        close_marker: Option<&super::CloseReconciliationMarker>,
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
                && self.attached_key.as_ref() == Some(cleanup.old())
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
        let preserve = desired == self.desired && !current_create && !close_recovery;
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
            settled_create: committed_create,
            close_consumption,
            mutation_completion: MembershipMutationCompletion::None,
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
        let old = self.attached_key.clone().ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key() != &old || !matches!(self.phase, SessionPhase::Ready(_)) {
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
            MembershipResultKey::Move(request_id) => {
                if !self.move_membership_is_current(request_id) {
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
        )?;
        plan.refresh_generation = refresh_generation;
        if matches!(cleanup.result(), MembershipResultKey::Move(_)) {
            plan.mutation_completion = MembershipMutationCompletion::Move;
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
                        self.pending_refresh_generation == Some(*generation)
                    }
                    MembershipResultKey::Close(_)
                    | MembershipResultKey::Rename(_)
                    | MembershipResultKey::Move(_)
                    | MembershipResultKey::Switch(_) => false,
                };
                exact_origin
                    && pending.cleanup.installs_candidate
                    && self.cache == prepared.baseline
                    && self.desired == prepared.baseline_desired
                    && self.attached_key.as_ref() == Some(pending.cleanup.old())
                    && committed
                        .anchor
                        .ready()
                        .is_some_and(|ready| ready.selected.key() == pending.cleanup.old())
            }
            (Some(pending), SessionPhase::ReconcilingMembership(selected)) => {
                let Some(prepared) = &pending.prepared else {
                    return false;
                };
                (match &pending.cleanup.result {
                    MembershipResultKey::Refresh(generation) => {
                        self.pending_refresh_generation == Some(*generation)
                    }
                    MembershipResultKey::Move(request_id) => {
                        self.move_membership_is_current(request_id)
                    }
                    MembershipResultKey::Rename(request_id) => {
                        self.rename_membership_is_current(request_id)
                    }
                    MembershipResultKey::Close(_) => pending
                        .cleanup
                        .close_marker()
                        .is_some_and(|marker| self.close.prepare_consumption(marker).is_ok()),
                    MembershipResultKey::Create(_) | MembershipResultKey::Switch(_) => false,
                }) && &pending.cleanup.old == selected.key()
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
        self.prepare_membership_install_inner(prepared, desired, Some(deadlines), None, None)
    }

    /// Admission already holds the mailbox gate. Only an authorized old call retains its candidate.
    pub(in crate::terminal_session) fn finish_switch_membership_admission(
        &mut self,
        discarded: Option<&AttachmentKey>,
        inherited: Option<&AttachmentKey>,
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
            || !matches!(self.phase, SessionPhase::Ready(_))
        {
            return Err(CarrierFailure::Protocol);
        }
        let old = self.attached_key.clone().ok_or(CarrierFailure::Protocol)?;
        let selected = self.active().cloned().ok_or(CarrierFailure::Protocol)?;
        if selected.key() != &old {
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
}
