//! Owns Client context-selection provenance, retained context candidates, and admission of
//! exact navigation targets against canonical snapshots. It does not change canonical
//! membership or attach a viewer before the runtime settles the selection plan.

use super::recovery_fence::RecoveryFence;
use super::{
    CarrierFailure, ClientContextTabs, ClientPhase, CreateTerminalProvenance, OperationDeadlines,
    PreparedRefresh, WorkbenchClientState, WorkbenchClientView,
    context_removal::ContextRemovalPermit, validate_canonical_refresh,
};
use crate::workbench_projection::{
    ClientContext, ClientContextLocation, ClientMemberSelection, ClientTab,
    project_native_context_tabs, validate_canonical_candidate, validate_retained_context,
};
use orchbee_workbench_protocol::{
    MAXIMUM_TERMINAL_CONTEXTS, SessionId, Snapshot, WorkbenchRevision,
};

pub(super) struct ContextSelection {
    generation: u64,
    target: ClientContextLocation,
    deadlines: OperationDeadlines,
    member: Option<ClientMemberSelection>,
}

#[derive(Clone, Debug)]
pub(super) struct RetainedContext {
    session_id: SessionId,
    location: ClientContextLocation,
    revision: WorkbenchRevision,
    tabs: Vec<ClientTab>,
    groups: Vec<orchbee_workbench_protocol::TabGroup>,
    provenance: Vec<CreateTerminalProvenance>,
}

impl RetainedContext {
    /// The snapshot's last-known tab membership — the zoom GC reads
    /// departed-context truth only through this view.
    pub(super) fn tabs(&self) -> &[ClientTab] {
        &self.tabs
    }

    /// The snapshot's context identity: the zoom GC never consults a
    /// retained entry sharing the current cache's location — the fresh
    /// cache is already that location's complete truth, absence included.
    pub(super) fn location(&self) -> &ClientContextLocation {
        &self.location
    }
}

pub(super) struct ContextProvenance<'a> {
    tabs: &'a [ClientTab],
    provenance: &'a [CreateTerminalProvenance],
}

impl ContextProvenance<'_> {
    pub(super) fn for_tab(&self, candidate: &ClientTab) -> Option<&CreateTerminalProvenance> {
        self.tabs
            .iter()
            .position(|tab| tab.tab_id() == candidate.tab_id())
            .filter(|index| !candidate.has_replaced_terminals_from(&self.tabs[*index]))
            .map(|index| &self.provenance[index])
    }
}

impl WorkbenchClientState {
    /// Canonical Context entries without the Workspaces an accepted close has taken out of
    /// navigation. Navigator positions, pointer targets and step order all read this list, so they
    /// agree; the cache keeps the full canonical list.
    pub(crate) fn display_navigation(&self) -> Vec<&ClientContext> {
        let leaving = self.leaving_workspace_ids();
        self.cache
            .navigation()
            .iter()
            .filter(|entry| match &entry.location {
                ClientContextLocation::Workspace { workspace_id, .. } => {
                    !leaving.contains(workspace_id)
                }
                ClientContextLocation::NoWorkspace => true,
            })
            .collect()
    }

    /// The displayed position of `location`, or `None` when it is absent or an accepted close took
    /// it out of navigation. Context selection indexes the displayed list, so a caller that names
    /// its target by identity converts through this instead of the canonical order.
    pub(crate) fn display_position(&self, location: &ClientContextLocation) -> Option<usize> {
        self.display_navigation()
            .iter()
            .position(|entry| &entry.location == location)
    }

    /// The location the Context strip marks as current. An accepted close of the current
    /// Workspace has already taken its row out of navigation while the cache keeps it until the
    /// landing installs the neighbour, so the mark goes to that neighbour in the meantime.
    pub(crate) fn display_active_location(&self) -> Option<ClientContextLocation> {
        let leaving = self.leaving_workspace_ids();
        let current = self.cache.location();
        if !super::workspace_close::current_leaves(current, &leaving) {
            return Some(current.clone());
        }
        super::workspace_close::neighbour_after_leaving(self.cache.navigation(), &leaving, current)
    }

    pub(crate) fn context_selection_available(&self) -> bool {
        self.pending_refresh.is_none() && self.context_selection_quiet()
    }
    /// Only a background publication read keeps a Context switch from the refresh slot, so the
    /// switch can wait for that read instead of being refused.
    pub(crate) fn context_selection_awaits_background_read(&self) -> bool {
        self.background_refresh_only() && self.context_selection_quiet()
    }
    fn context_selection_quiet(&self) -> bool {
        self.invariant_holds()
            && self.pending_mutation.is_none()
            && !self.setup_blocks_structure()
            && !self.workspace_close_blocks_structure()
            && self.pending_membership.is_none()
            && !self.recovery_fences(RecoveryFence::Navigation)
            && !self.rename_draft_blocks_structure()
            && self.close.permits_retired_membership()
            && matches!(
                self.phase,
                ClientPhase::Empty
                    | ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
    }
    pub(super) fn context_invariant_holds(&self) -> bool {
        self.retained_contexts.len() <= MAXIMUM_TERMINAL_CONTEXTS
            && self
                .retained_contexts
                .iter()
                .all(|retained| retained.tabs.len() == retained.provenance.len())
            && self.context_selection.as_ref().is_none_or(|intent| {
                self.pending_refresh_generation() == Some(intent.generation)
                    && &intent.target != self.cache.location()
            })
    }

    pub(crate) fn start_context_selection(
        &mut self,
        generation: u64,
        next: bool,
        deadlines: OperationDeadlines,
    ) -> bool {
        if generation == 0 || !self.context_selection_available() {
            return false;
        }
        let navigation = self.display_navigation();
        let active = self.display_active_location();
        let position = active.as_ref().and_then(|active| {
            navigation
                .iter()
                .position(|entry| &entry.location == active)
        });
        if navigation.is_empty() || (position.is_some() && navigation.len() < 2) {
            return false;
        }
        if self.retained_contexts.len() >= MAXIMUM_TERMINAL_CONTEXTS
            && !self
                .retained_contexts
                .iter()
                .any(|retained| &retained.location == self.cache.location())
        {
            return false;
        }
        let target_index = match (position, next) {
            (Some(position), true) => (position + 1) % navigation.len(),
            (Some(position), false) => (position + navigation.len() - 1) % navigation.len(),
            (None, true) => 0,
            (None, false) => navigation.len() - 1,
        };
        let target = navigation[target_index].location.clone();
        self.remember_current_navigation();
        self.context_selection = Some(ContextSelection {
            generation,
            target,
            deadlines,
            member: None,
        });
        self.pending_refresh = Some(super::PendingRefreshFlight::recovery(generation));
        self.adopt_publication_debt();
        self.refresh_status = Some(super::NavigatorRefreshStatus::Refreshing);
        self.notice = None;
        true
    }

    /// Pointer variant of `start_context_selection`: the pointer resolved
    /// a concrete navigation row, so the target is the entry at `index`
    /// instead of a next/previous step. Identical guards and settlement.
    /// Selecting the already-active location or an out-of-range index is a
    /// no-op.
    pub(crate) fn start_context_selection_at(
        &mut self,
        generation: u64,
        index: usize,
        deadlines: OperationDeadlines,
    ) -> bool {
        self.start_context_selection_at_with_member(generation, index, deadlines, None)
    }

    pub(crate) fn start_context_selection_at_with_member(
        &mut self,
        generation: u64,
        index: usize,
        deadlines: OperationDeadlines,
        member: Option<ClientMemberSelection>,
    ) -> bool {
        if generation == 0 || !self.context_selection_available() {
            return false;
        }
        let navigation = self.display_navigation();
        let Some(target) = navigation.get(index).map(|entry| entry.location.clone()) else {
            return false;
        };
        if &target == self.cache.location() {
            return false;
        }
        if self.retained_contexts.len() >= MAXIMUM_TERMINAL_CONTEXTS
            && !self
                .retained_contexts
                .iter()
                .any(|retained| &retained.location == self.cache.location())
        {
            return false;
        }
        self.remember_current_navigation();
        self.context_selection = Some(ContextSelection {
            generation,
            target,
            deadlines,
            member,
        });
        self.pending_refresh = Some(super::PendingRefreshFlight::recovery(generation));
        self.adopt_publication_debt();
        self.refresh_status = Some(super::NavigatorRefreshStatus::Refreshing);
        self.notice = None;
        true
    }

    /// A requested Context switch owns the refresh but has not landed: focus still names the
    /// pane the user is leaving, so terminal input must not reach it.
    pub(crate) const fn context_selection_pending(&self) -> bool {
        self.context_selection.is_some()
    }

    /// Where the user's own landing switch is headed: the requested Context while its read is in
    /// flight, then the Context whose terminal attaches.
    pub(crate) fn landing_destination(&self) -> Option<&ClientContextLocation> {
        match &self.context_selection {
            Some(intent) => Some(&intent.target),
            None => matches!(self.phase, ClientPhase::Switching(_)).then(|| self.cache.location()),
        }
    }

    pub(crate) fn context_selection_deadlines(&self) -> Option<OperationDeadlines> {
        self.context_selection
            .as_ref()
            .map(|intent| intent.deadlines)
    }

    pub(super) fn prepare_context_selection(
        &self,
        generation: u64,
        snapshot: &Snapshot,
    ) -> PreparedRefresh {
        let Some(intent) = self
            .context_selection
            .as_ref()
            .filter(|intent| intent.generation == generation)
        else {
            return PreparedRefresh::Stale;
        };
        // Validate the currently owned authority before treating target absence/layout as a refusal.
        let valid_baseline = self
            .cache
            .project_snapshot(snapshot)
            .ok()
            .and_then(|candidate| validate_canonical_refresh(&self.cache, candidate).ok());
        if valid_baseline.is_none() {
            return PreparedRefresh::Divergent { generation };
        }
        let Ok(candidate) = project_native_context_tabs(snapshot, &intent.target) else {
            return PreparedRefresh::DefiniteFailure {
                generation,
                baseline_revision: self.cache.revision(),
            };
        };
        let preferred = intent
            .member
            .as_ref()
            .map(|selection| &selection.member.tab)
            .or_else(|| self.navigation_memory.tab(candidate.location()));
        match self.prepare_membership_candidate(candidate, preferred, None, None, None) {
            Ok(mut prepared) => {
                if let Some(selection) = &intent.member {
                    if !prepared.retarget_selected_bound_member(&intent.target, selection) {
                        return PreparedRefresh::DefiniteFailure {
                            generation,
                            baseline_revision: self.cache.revision(),
                        };
                    }
                } else if let Some(tab) = prepared.selected_tab() {
                    let panel_index = self.navigation_memory.panel_index(&intent.target, tab);
                    prepared.retarget_selected_panel(panel_index);
                }
                PreparedRefresh::Membership {
                    generation,
                    prepared,
                }
            }
            Err(_) => PreparedRefresh::Divergent { generation },
        }
    }

    pub(super) fn validate_candidate_provenance(
        &self,
        candidate: &ClientContextTabs,
        permit: Option<&ContextRemovalPermit>,
    ) -> Result<Option<ContextProvenance<'_>>, CarrierFailure> {
        if candidate.location() == self.cache.location() {
            validate_canonical_candidate(&self.cache, candidate)
                .map_err(|_| CarrierFailure::AuthorityDivergence)?;
            return Ok(Some(ContextProvenance {
                tabs: self.cache.tabs(),
                provenance: &self.provenance,
            }));
        }
        // The third authorized landing: the removal classifier's permit names
        // exactly this baseline and candidate pair — no ambient flag can reach
        // here because the permit only exists inside the prepared candidate.
        let removal_landing =
            permit.is_some_and(|permit| permit.authorizes(&self.cache, candidate));
        if candidate.session_id() != self.cache.session_id()
            || candidate.revision() < self.cache.revision()
            || (!removal_landing
                && self.context_selection.as_ref().is_none_or(|intent| {
                    self.pending_refresh_generation() != Some(intent.generation)
                        || &intent.target != candidate.location()
                })
                && !self.setup_target_matches(candidate.location()))
        {
            return Err(CarrierFailure::AuthorityDivergence);
        }
        let Some(retained) = self.retained_contexts.iter().find(|retained| {
            retained.location.terminal_context_id() == candidate.terminal_context_id()
        }) else {
            return Ok(None);
        };
        validate_retained_context(
            &retained.session_id,
            &retained.location,
            retained.revision,
            &retained.tabs,
            &retained.groups,
            candidate,
        )
        .map_err(|_| CarrierFailure::AuthorityDivergence)?;
        Ok(Some(ContextProvenance {
            tabs: &retained.tabs,
            provenance: &retained.provenance,
        }))
    }

    pub(super) fn prepare_retained_contexts(
        &self,
        candidate: &ClientContextTabs,
        permit: Option<&ContextRemovalPermit>,
    ) -> Option<Vec<RetainedContext>> {
        if candidate.location() == self.cache.location() {
            return None;
        }
        let mut retained_contexts = self.retained_contexts.clone();
        // Canonical navigation removal can prune settled history; uncertain Create facts are retained.
        retained_contexts.retain(|retained| {
            candidate.navigation().iter().any(|entry| {
                entry.location.terminal_context_id() == retained.location.terminal_context_id()
            }) || retained.provenance.iter().any(|provenance| {
                matches!(
                    provenance,
                    CreateTerminalProvenance::Pending | CreateTerminalProvenance::OutcomeUnknown
                )
            })
        });
        // A deleted context's cache is never parked back into retained — its
        // workspace close is the removal authority, and the server deletes the
        // context, so a resurrecting re-insert would mint a stale zombie entry.
        // Other contexts' Pending/OutcomeUnknown entries stay untouched.
        let removal_skips = permit.is_some_and(|permit| permit.removed() == self.cache.location());
        if removal_skips {
            return Some(retained_contexts);
        }
        let retained = RetainedContext {
            session_id: self.cache.session_id().clone(),
            location: self.cache.location().clone(),
            revision: self.cache.revision(),
            tabs: self.cache.tabs().to_vec(),
            groups: self.cache.groups().to_vec(),
            provenance: self.provenance.clone(),
        };
        if let Some(prior) = retained_contexts
            .iter_mut()
            .find(|prior| &prior.location == self.cache.location())
        {
            *prior = retained;
        } else {
            retained_contexts.push(retained);
        }
        Some(retained_contexts)
    }
}

pub struct ClientContextView<'a> {
    entry: &'a ClientContext,
    position: usize,
    active: bool,
}

impl<'a> ClientContextView<'a> {
    pub fn project_id(&self) -> Option<&'a orchbee_workbench_protocol::WorkbenchProjectId> {
        self.entry.project_id.as_ref()
    }
    pub(crate) const fn position(&self) -> usize {
        self.position
    }
    pub(crate) const fn active(&self) -> bool {
        self.active
    }
    pub fn project_label(&self) -> Option<&'a str> {
        self.entry
            .project_label
            .as_ref()
            .map(|label| label.as_str())
    }
    pub fn workspace_label(&self) -> Option<&'a str> {
        self.entry
            .workspace_label
            .as_ref()
            .map(|label| label.as_str())
    }

    /// The row's stable navigation identity — what a pointer hit region
    /// records so a later click resolves against current state.
    pub const fn location(&self) -> &'a ClientContextLocation {
        &self.entry.location
    }
}

impl<'a> WorkbenchClientView<'a> {
    /// An empty Workbench has no selected Workspace or terminal Context.
    pub(crate) fn no_current_workspace(&self) -> bool {
        matches!(
            self.state.cache.location(),
            ClientContextLocation::NoWorkspace
        ) && self.state.cache.tabs().is_empty()
    }

    pub(crate) fn navigation_session(&self) -> &SessionId {
        self.state.cache.session_id()
    }
    pub(crate) fn context_count(&self) -> usize {
        self.state.display_navigation().len()
    }
    pub(crate) fn active_context_position(&self) -> Option<usize> {
        let active = self.state.display_active_location()?;
        self.state
            .display_navigation()
            .iter()
            .position(|entry| entry.location == active)
            .map(|index| index + 1)
    }
    pub(crate) fn context_at(&self, index: usize) -> Option<ClientContextView<'a>> {
        let entry = *self.state.display_navigation().get(index)?;
        let active = self.state.display_active_location();
        Some(ClientContextView {
            entry,
            position: index + 1,
            active: active.as_ref() == Some(&entry.location),
        })
    }
}
