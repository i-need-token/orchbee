use super::{
    CarrierFailure, CreateTerminalProvenance, NativeContextTabs, NativeSessionState,
    NativeSessionView, OperationDeadlines, PreparedRefresh, SessionPhase,
    context_removal::ContextRemovalPermit, validate_canonical_refresh,
};
use crate::terminal_session::workbench_projection::{
    NativeContext, NativeContextLocation, NativeSimpleTab, project_native_context_tabs,
    validate_canonical_candidate, validate_retained_context,
};
use orchbee_workbench_protocol::{MAXIMUM_TERMINAL_CONTEXTS, SiteId, Snapshot, WorkbenchRevision};

pub(super) struct ContextSelection {
    generation: u64,
    target: NativeContextLocation,
    deadlines: OperationDeadlines,
}

#[derive(Clone, Debug)]
pub(super) struct RetainedContext {
    site_id: SiteId,
    location: NativeContextLocation,
    revision: WorkbenchRevision,
    tabs: Vec<NativeSimpleTab>,
    provenance: Vec<CreateTerminalProvenance>,
}

impl RetainedContext {
    /// The snapshot's last-known tab membership — the zoom GC reads
    /// departed-context truth only through this view.
    pub(super) fn tabs(&self) -> &[NativeSimpleTab] {
        &self.tabs
    }

    /// The snapshot's context identity: the zoom GC never consults a
    /// retained entry sharing the current cache's location — the fresh
    /// cache is already that location's complete truth, absence included.
    pub(super) fn location(&self) -> &NativeContextLocation {
        &self.location
    }
}

pub(super) struct ContextProvenance<'a> {
    tabs: &'a [NativeSimpleTab],
    provenance: &'a [CreateTerminalProvenance],
}

impl ContextProvenance<'_> {
    pub(super) fn for_tab(
        &self,
        tab_id: &orchbee_workbench_protocol::WorkbenchTabId,
    ) -> Option<&CreateTerminalProvenance> {
        self.tabs
            .iter()
            .position(|tab| tab.tab_id() == tab_id)
            .map(|index| &self.provenance[index])
    }
}

impl NativeSessionState {
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

    pub(in crate::terminal_session) fn start_context_selection(
        &mut self,
        generation: u64,
        next: bool,
        deadlines: OperationDeadlines,
    ) -> bool {
        if generation == 0
            || !self.invariant_holds()
            || self.pending_refresh.is_some()
            || self.pending_mutation.is_some()
            || self.setup_blocks_structure()
            || self.pending_membership.is_some()
            || self.move_recovery.is_some()
            || self.rename_recovery.is_some()
            || self.rename_uncertain.is_some()
            || self.rename_draft_blocks_structure()
            || !self.close.permits_retired_membership()
            || !matches!(
                self.phase,
                SessionPhase::Empty
                    | SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
        {
            return false;
        }
        let navigation = self.cache.navigation();
        let Some(position) = navigation
            .iter()
            .position(|entry| &entry.location == self.cache.location())
        else {
            return false;
        };
        if navigation.len() < 2 {
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
        let target_index = if next {
            (position + 1) % navigation.len()
        } else {
            (position + navigation.len() - 1) % navigation.len()
        };
        self.context_selection = Some(ContextSelection {
            generation,
            target: navigation[target_index].location.clone(),
            deadlines,
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
    pub(in crate::terminal_session) fn start_context_selection_at(
        &mut self,
        generation: u64,
        index: usize,
        deadlines: OperationDeadlines,
    ) -> bool {
        if generation == 0
            || !self.invariant_holds()
            || self.pending_refresh.is_some()
            || self.pending_mutation.is_some()
            || self.setup_blocks_structure()
            || self.pending_membership.is_some()
            || self.move_recovery.is_some()
            || self.rename_recovery.is_some()
            || self.rename_uncertain.is_some()
            || self.rename_draft_blocks_structure()
            || !self.close.permits_retired_membership()
            || !matches!(
                self.phase,
                SessionPhase::Empty
                    | SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
        {
            return false;
        }
        let navigation = self.cache.navigation();
        let Some(target) = navigation.get(index) else {
            return false;
        };
        if &target.location == self.cache.location() {
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
        let target = target.location.clone();
        self.context_selection = Some(ContextSelection {
            generation,
            target,
            deadlines,
        });
        self.pending_refresh = Some(super::PendingRefreshFlight::recovery(generation));
        self.adopt_publication_debt();
        self.refresh_status = Some(super::NavigatorRefreshStatus::Refreshing);
        self.notice = None;
        true
    }

    pub(in crate::terminal_session) fn context_selection_deadlines(
        &self,
    ) -> Option<OperationDeadlines> {
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
        match self.prepare_membership_candidate(candidate, None, None, None, None) {
            Ok(prepared) => PreparedRefresh::Membership {
                generation,
                prepared,
            },
            Err(_) => PreparedRefresh::Divergent { generation },
        }
    }

    pub(super) fn validate_candidate_provenance(
        &self,
        candidate: &NativeContextTabs,
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
        if candidate.site_id() != self.cache.site_id()
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
            &retained.site_id,
            &retained.location,
            retained.revision,
            &retained.tabs,
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
        candidate: &NativeContextTabs,
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
            site_id: self.cache.site_id().clone(),
            location: self.cache.location().clone(),
            revision: self.cache.revision(),
            tabs: self.cache.tabs().to_vec(),
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

pub(crate) struct NativeContextView<'a> {
    entry: &'a NativeContext,
    position: usize,
    active: bool,
}

impl<'a> NativeContextView<'a> {
    pub(crate) const fn position(&self) -> usize {
        self.position
    }
    pub(crate) const fn active(&self) -> bool {
        self.active
    }
    pub(crate) fn project_label(&self) -> Option<&'a str> {
        self.entry
            .project_label
            .as_ref()
            .map(|label| label.as_str())
    }
    pub(crate) fn workspace_label(&self) -> Option<&'a str> {
        self.entry
            .workspace_label
            .as_ref()
            .map(|label| label.as_str())
    }

    /// The row's stable navigation identity — what a pointer hit region
    /// records so a later click resolves against current state.
    pub(crate) const fn location(&self) -> &'a NativeContextLocation {
        &self.entry.location
    }
}

impl<'a> NativeSessionView<'a> {
    pub(in crate::terminal_session) fn context_count(&self) -> usize {
        self.state.cache.navigation().len()
    }
    pub(in crate::terminal_session) fn active_context_position(&self) -> Option<usize> {
        self.state
            .cache
            .navigation()
            .iter()
            .position(|entry| &entry.location == self.state.cache.location())
            .map(|index| index + 1)
    }
    pub(in crate::terminal_session) fn context_at(
        &self,
        index: usize,
    ) -> Option<NativeContextView<'a>> {
        let entry = self.state.cache.navigation().get(index)?;
        Some(NativeContextView {
            entry,
            position: index + 1,
            active: &entry.location == self.state.cache.location(),
        })
    }
}
