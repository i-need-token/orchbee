use std::sync::Arc;

use orchbee_workbench_protocol::{
    ContractError, MAXIMUM_PANEL_NODES_PER_CONTEXT, PanelNode, PanelOrientation, PanelSlot,
    Presentation, PresentationBinding, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchPanelSplitDirection, WorkbenchProtocolDocument,
    WorkbenchRejectionReason, WorkbenchRequestId, WorkbenchRevision, WorkbenchSplitId,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalFailureReason,
    WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
    WorkerId, serialize_document,
};

use super::terminal_membership::{WorkbenchTerminalMembership, tree_terminal_membership};
use super::{
    MAXIMUM_LEDGER_BYTES, PlanFailure, RetainedRequest, SettledRequest, WorkbenchAuthority,
    WorkbenchGrant, ledger_size, result, unavailable,
};
use crate::workbench_projection::PreparedProjectionAdvance;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedWorkbenchPanelSplitMetadata {
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    new_panel_id: WorkbenchPanelId,
    split_id: WorkbenchSplitId,
    first_slot_id: WorkbenchPanelSlotId,
    second_slot_id: WorkbenchPanelSlotId,
    presentation_id: orchbee_workbench_protocol::TerminalPresentationId,
    geometry: WorkbenchTerminalGeometry,
    direction: WorkbenchPanelSplitDirection,
    target: WorkbenchPanelSplitTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkbenchPanelSplitTarget {
    Standalone(WorkbenchTerminalMembership),
    Workspace {
        worker_id: WorkerId,
        path: orchbee_workbench_protocol::WorkbenchDirectoryPath,
    },
}

impl StagedWorkbenchPanelSplitMetadata {
    #[must_use]
    pub const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    #[must_use]
    pub fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }

    #[must_use]
    pub fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }

    #[must_use]
    pub fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }

    #[must_use]
    pub fn new_panel_id(&self) -> &WorkbenchPanelId {
        &self.new_panel_id
    }

    #[must_use]
    pub fn split_id(&self) -> &WorkbenchSplitId {
        &self.split_id
    }

    #[must_use]
    pub fn first_slot_id(&self) -> &WorkbenchPanelSlotId {
        &self.first_slot_id
    }

    #[must_use]
    pub fn second_slot_id(&self) -> &WorkbenchPanelSlotId {
        &self.second_slot_id
    }

    #[must_use]
    pub fn presentation_id(&self) -> &orchbee_workbench_protocol::TerminalPresentationId {
        &self.presentation_id
    }

    #[must_use]
    pub const fn geometry(&self) -> WorkbenchTerminalGeometry {
        self.geometry
    }

    #[must_use]
    pub const fn direction(&self) -> WorkbenchPanelSplitDirection {
        self.direction
    }

    #[must_use]
    pub fn standalone_membership(&self) -> Option<&WorkbenchTerminalMembership> {
        match &self.target {
            WorkbenchPanelSplitTarget::Standalone(membership) => Some(membership),
            WorkbenchPanelSplitTarget::Workspace { .. } => None,
        }
    }

    #[must_use]
    pub fn workspace_launch(
        &self,
    ) -> Option<(
        &WorkerId,
        &orchbee_workbench_protocol::WorkbenchDirectoryPath,
    )> {
        match &self.target {
            WorkbenchPanelSplitTarget::Standalone(_) => None,
            WorkbenchPanelSplitTarget::Workspace { worker_id, path } => Some((worker_id, path)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedWorkbenchPanelSplitMetadata {
    request_id: WorkbenchRequestId,
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    new_panel_id: WorkbenchPanelId,
    split_id: WorkbenchSplitId,
    first_slot_id: WorkbenchPanelSlotId,
    second_slot_id: WorkbenchPanelSlotId,
    presentation_id: orchbee_workbench_protocol::TerminalPresentationId,
    binding: PresentationBinding,
    geometry: WorkbenchTerminalGeometry,
    direction: WorkbenchPanelSplitDirection,
    standalone: bool,
}

impl CommittedWorkbenchPanelSplitMetadata {
    #[must_use]
    pub fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    #[must_use]
    pub fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }

    #[must_use]
    pub fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }

    #[must_use]
    pub fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }

    #[must_use]
    pub fn new_panel_id(&self) -> &WorkbenchPanelId {
        &self.new_panel_id
    }

    #[must_use]
    pub fn split_id(&self) -> &WorkbenchSplitId {
        &self.split_id
    }

    #[must_use]
    pub fn first_slot_id(&self) -> &WorkbenchPanelSlotId {
        &self.first_slot_id
    }

    #[must_use]
    pub fn second_slot_id(&self) -> &WorkbenchPanelSlotId {
        &self.second_slot_id
    }

    #[must_use]
    pub fn presentation_id(&self) -> &orchbee_workbench_protocol::TerminalPresentationId {
        &self.presentation_id
    }

    #[must_use]
    pub fn binding(&self) -> &PresentationBinding {
        &self.binding
    }

    #[must_use]
    pub const fn geometry(&self) -> WorkbenchTerminalGeometry {
        self.geometry
    }

    #[must_use]
    pub const fn direction(&self) -> WorkbenchPanelSplitDirection {
        self.direction
    }

    #[must_use]
    pub const fn is_standalone(&self) -> bool {
        self.standalone
    }
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelSplitBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchPanelSplitAdmission<'a>>),
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelSplitPreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchPanelSplit<'a>>),
}

pub struct WorkbenchPanelSplitAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    label: String,
    staged: StagedWorkbenchPanelSplitMetadata,
    last_identity: u128,
    profile_use: Option<super::LaunchProfileUse>,
}

impl<'a> WorkbenchPanelSplitAdmission<'a> {
    #[must_use]
    pub fn staged_metadata(&self) -> &StagedWorkbenchPanelSplitMetadata {
        &self.staged
    }

    /// The catalog resolution frozen at prepare: `Some` carries the stored
    /// launch selection plus profile provenance, `None` means the command's
    /// explicit `launch` (or the default launch) applies.
    #[must_use]
    pub const fn profile_use(&self) -> Option<&super::LaunchProfileUse> {
        self.profile_use.as_ref()
    }

    #[must_use]
    pub fn runtime_unavailable(
        self,
        reason: WorkbenchCommandUnavailableReason,
    ) -> WorkbenchCommandResult {
        result(&self.command, unavailable(reason))
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retired = true;
        result(
            &self.command,
            unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
        )
    }

    #[must_use]
    pub fn finish(self, binding: PresentationBinding) -> WorkbenchPanelSplitPreparation<'a> {
        let Self {
            authority,
            command,
            label,
            staged,
            last_identity,
            profile_use: _,
        } = self;
        let mut candidate = authority.projection.snapshot().clone();
        let Some(context) = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == staged.terminal_context_id)
        else {
            return WorkbenchPanelSplitPreparation::Ready(retire(authority, &command));
        };
        let Some(tab) = context
            .tabs
            .iter_mut()
            .find(|tab| tab.tab_id == staged.tab_id)
        else {
            return WorkbenchPanelSplitPreparation::Ready(retire(authority, &command));
        };
        let replacement = PanelSplitReplacement {
            target_panel_id: &staged.panel_id,
            direction: staged.direction,
            new_panel_id: &staged.new_panel_id,
            split_id: &staged.split_id,
            first_slot_id: &staged.first_slot_id,
            second_slot_id: &staged.second_slot_id,
            presentation_id: &staged.presentation_id,
            label: &label,
            binding: binding.clone(),
        };
        if !replace_panel(&mut tab.root, &replacement) {
            return WorkbenchPanelSplitPreparation::Ready(retire(authority, &command));
        }
        candidate.revision = staged.revision;
        let encoded = match serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)) {
            Ok(encoded) => encoded,
            Err(ContractError::EncodedValueTooLarge { .. }) => {
                return WorkbenchPanelSplitPreparation::Ready(
                    authority.retain_rejection(command, WorkbenchRejectionReason::ResourceLimit),
                );
            }
            Err(ContractError::InvalidDocument | ContractError::WrongDocumentKind) => {
                return WorkbenchPanelSplitPreparation::Ready(retire(authority, &command));
            }
        };
        let projection = match authority.projection.prepare_advance(&encoded) {
            Ok(projection) => projection,
            Err(_error) => {
                return WorkbenchPanelSplitPreparation::Ready(retire(authority, &command));
            }
        };
        let standalone = staged.standalone_membership().is_some();
        let metadata = CommittedWorkbenchPanelSplitMetadata {
            request_id: command.request_id.clone(),
            revision: staged.revision,
            terminal_context_id: staged.terminal_context_id,
            tab_id: staged.tab_id,
            panel_id: staged.panel_id,
            new_panel_id: staged.new_panel_id,
            split_id: staged.split_id,
            first_slot_id: staged.first_slot_id,
            second_slot_id: staged.second_slot_id,
            presentation_id: staged.presentation_id,
            binding,
            geometry: staged.geometry,
            direction: staged.direction,
            standalone,
        };
        let Some(results) = PanelSplitResultTable::new(&command, &metadata) else {
            return WorkbenchPanelSplitPreparation::Ready(retire(authority, &command));
        };
        let reserved_bytes = results.maximum_charge();
        let Some(next_ledger_bytes) = authority.ledger_bytes.checked_add(reserved_bytes) else {
            return WorkbenchPanelSplitPreparation::Ready(retire(authority, &command));
        };
        if next_ledger_bytes > MAXIMUM_LEDGER_BYTES {
            return WorkbenchPanelSplitPreparation::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        let entry_seal = Arc::new(PanelSplitSettlementSeal);
        let token_seal = Arc::clone(&entry_seal);
        let request_id = command.request_id.clone();
        let pending_result = results.pending.result.clone();
        let entry = Box::new(PendingPanelSplitEntry {
            command,
            pending_result,
            final_results: results,
            reserved_bytes,
            metadata: metadata.clone(),
            seal: entry_seal,
        });
        let pending = PendingWorkbenchPanelSplit {
            metadata,
            seal: token_seal,
        };
        WorkbenchPanelSplitPreparation::Prepared(Box::new(PreparedWorkbenchPanelSplit {
            authority,
            projection,
            last_identity,
            next_ledger_bytes,
            request_id,
            entry,
            pending,
        }))
    }
}

pub struct PreparedWorkbenchPanelSplit<'a> {
    authority: &'a mut WorkbenchAuthority,
    projection: PreparedProjectionAdvance,
    last_identity: u128,
    next_ledger_bytes: usize,
    request_id: WorkbenchRequestId,
    entry: Box<PendingPanelSplitEntry>,
    pending: PendingWorkbenchPanelSplit,
}

impl PreparedWorkbenchPanelSplit<'_> {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchPanelSplitMetadata {
        &self.entry.metadata
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        self.commit_state().0
    }

    #[must_use]
    pub fn commit_into_pending(self) -> PendingWorkbenchPanelSplit {
        self.commit_state().1
    }

    fn commit_state(self) -> (WorkbenchCommandResult, PendingWorkbenchPanelSplit) {
        let Self {
            authority,
            projection,
            last_identity,
            next_ledger_bytes,
            request_id,
            entry,
            pending,
        } = self;
        authority.projection.commit_prepared(projection);
        authority.last_identity = last_identity;
        authority.ledger_bytes = next_ledger_bytes;
        let response = entry.pending_result.clone();
        authority
            .ledger
            .insert(request_id, RetainedRequest::PendingPanelSplit(entry));
        (response, pending)
    }
}

pub struct PendingWorkbenchPanelSplit {
    metadata: CommittedWorkbenchPanelSplitMetadata,
    seal: Arc<PanelSplitSettlementSeal>,
}

impl PendingWorkbenchPanelSplit {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchPanelSplitMetadata {
        &self.metadata
    }
}

pub struct PreparedWorkbenchPanelSplitSettlement<'a> {
    entry: &'a mut RetainedRequest,
    ledger_bytes: &'a mut usize,
    settled: Box<SettledRequest>,
    result: WorkbenchCommandResult,
    next_ledger_bytes: usize,
}

impl PreparedWorkbenchPanelSplitSettlement<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchCommandResult {
        &self.result
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        *self.entry = RetainedRequest::Settled(self.settled);
        *self.ledger_bytes = self.next_ledger_bytes;
        self.result
    }
}

struct PanelSplitSettlementSeal;

struct PrebuiltResult {
    result: WorkbenchCommandResult,
    charged_bytes: usize,
}

struct PanelSplitResultTable {
    pending: PrebuiltResult,
    finals: [(WorkbenchTerminalOutcome, PrebuiltResult); 5],
}

impl PanelSplitResultTable {
    fn new(
        command: &WorkbenchCommand,
        metadata: &CommittedWorkbenchPanelSplitMetadata,
    ) -> Option<Self> {
        let pending = prebuild(command, pending_outcome(metadata))?;
        let outcomes = [
            WorkbenchTerminalOutcome::Started {
                geometry: metadata.geometry,
            },
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
            },
            WorkbenchTerminalOutcome::Unavailable {
                reason: WorkbenchTerminalUnavailableReason::ServerBusy,
            },
            WorkbenchTerminalOutcome::Failed {
                reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
            },
            WorkbenchTerminalOutcome::OutcomeUnknown {},
        ];
        let finals = outcomes.map(|terminal_outcome| {
            let built = prebuild(command, final_outcome(metadata, terminal_outcome.clone()));
            built.map(|result| (terminal_outcome, result))
        });
        Some(Self {
            pending,
            finals: [
                finals[0].as_ref().map(clone_prebuilt)?,
                finals[1].as_ref().map(clone_prebuilt)?,
                finals[2].as_ref().map(clone_prebuilt)?,
                finals[3].as_ref().map(clone_prebuilt)?,
                finals[4].as_ref().map(clone_prebuilt)?,
            ],
        })
    }

    fn maximum_charge(&self) -> usize {
        self.finals
            .iter()
            .map(|(_outcome, result)| result.charged_bytes)
            .chain(std::iter::once(self.pending.charged_bytes))
            .max()
            .unwrap_or(0)
    }

    fn select(&self, outcome: &WorkbenchTerminalOutcome) -> Option<&PrebuiltResult> {
        self.finals
            .iter()
            .find(|(candidate, _result)| candidate == outcome)
            .map(|(_candidate, result)| result)
    }
}

pub(super) struct PendingPanelSplitEntry {
    pub(super) command: WorkbenchCommand,
    pub(super) pending_result: WorkbenchCommandResult,
    final_results: PanelSplitResultTable,
    pub(super) reserved_bytes: usize,
    metadata: CommittedWorkbenchPanelSplitMetadata,
    seal: Arc<PanelSplitSettlementSeal>,
}

impl PendingPanelSplitEntry {
    /// The committed metadata — `panel.relocate` fences the minted leaf while
    /// its terminal settlement is outstanding.
    pub(super) fn metadata(&self) -> &CommittedWorkbenchPanelSplitMetadata {
        &self.metadata
    }
}

fn clone_prebuilt(
    value: &(WorkbenchTerminalOutcome, PrebuiltResult),
) -> (WorkbenchTerminalOutcome, PrebuiltResult) {
    (
        value.0.clone(),
        PrebuiltResult {
            result: value.1.result.clone(),
            charged_bytes: value.1.charged_bytes,
        },
    )
}

fn prebuild(
    command: &WorkbenchCommand,
    outcome: WorkbenchCommandOutcome,
) -> Option<PrebuiltResult> {
    let result = result(command, outcome);
    Some(PrebuiltResult {
        charged_bytes: ledger_size(command, &result)?,
        result,
    })
}

fn pending_outcome(metadata: &CommittedWorkbenchPanelSplitMetadata) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelSplitPending {
        revision: metadata.revision,
        terminal_context_id: metadata.terminal_context_id.clone(),
        tab_id: metadata.tab_id.clone(),
        panel_id: metadata.panel_id.clone(),
        new_panel_id: metadata.new_panel_id.clone(),
        split_id: metadata.split_id.clone(),
        first_slot_id: metadata.first_slot_id.clone(),
        second_slot_id: metadata.second_slot_id.clone(),
        presentation_id: metadata.presentation_id.clone(),
    }
}

fn final_outcome(
    metadata: &CommittedWorkbenchPanelSplitMetadata,
    terminal_outcome: WorkbenchTerminalOutcome,
) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelSplitCreated {
        revision: metadata.revision,
        terminal_context_id: metadata.terminal_context_id.clone(),
        tab_id: metadata.tab_id.clone(),
        panel_id: metadata.panel_id.clone(),
        new_panel_id: metadata.new_panel_id.clone(),
        split_id: metadata.split_id.clone(),
        first_slot_id: metadata.first_slot_id.clone(),
        second_slot_id: metadata.second_slot_id.clone(),
        presentation_id: metadata.presentation_id.clone(),
        terminal_outcome,
    }
}

impl WorkbenchAuthority {
    pub fn begin_terminal_panel_split(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchPanelSplitBegin<'_> {
        let authorized = matches!(&command.action, WorkbenchAction::PanelSplit { .. })
            && grant.is_some_and(|grant| {
                grant.site_id == *self.site_id() && grant.mutate_navigation && grant.create_terminal
            });
        if !authorized {
            return WorkbenchPanelSplitBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(result) = self.command_preflight(&command) {
            return WorkbenchPanelSplitBegin::Ready(result);
        }
        let WorkbenchAction::PanelSplit {
            terminal_context_id,
            tab_id,
            panel_id,
            direction,
            label,
            geometry,
            launch: _,
            profile,
            ..
        } = command.action.clone()
        else {
            return WorkbenchPanelSplitBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            ));
        };
        let snapshot = self.projection.snapshot();
        if command.site_id != snapshot.site_id {
            return WorkbenchPanelSplitBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::SiteMismatch),
            );
        }
        if command.expected_revision != snapshot.revision {
            return WorkbenchPanelSplitBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::RevisionMismatch),
            );
        }
        let Some(context) = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == terminal_context_id)
        else {
            return WorkbenchPanelSplitBegin::Ready(self.retain_rejection(
                command,
                WorkbenchRejectionReason::TerminalContextUnavailable,
            ));
        };
        let Some(tab) = context.tabs.iter().find(|tab| tab.tab_id == tab_id) else {
            return WorkbenchPanelSplitBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::TabUnavailable),
            );
        };
        if !contains_leaf_panel(&tab.root, &panel_id) {
            return WorkbenchPanelSplitBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::PanelUnavailable),
            );
        }
        let node_count = count_panel_nodes(context);
        if node_count
            .checked_add(2)
            .is_none_or(|count| count > MAXIMUM_PANEL_NODES_PER_CONTEXT)
        {
            return WorkbenchPanelSplitBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::ResourceLimit),
            );
        }
        let target = if terminal_context_id == snapshot.navigation.standalone_terminal_context_id {
            let membership = match tree_terminal_membership(snapshot, &terminal_context_id) {
                Ok(membership) => membership,
                Err(_error) => return WorkbenchPanelSplitBegin::Ready(retire(self, &command)),
            };
            WorkbenchPanelSplitTarget::Standalone(membership)
        } else {
            let Some((project, workspace)) =
                snapshot.navigation.projects.iter().find_map(|project| {
                    project
                        .workspaces
                        .iter()
                        .find(|workspace| workspace.terminal_context_id == terminal_context_id)
                        .map(|workspace| (project, workspace))
                })
            else {
                return WorkbenchPanelSplitBegin::Ready(self.retain_rejection(
                    command,
                    WorkbenchRejectionReason::TerminalContextUnavailable,
                ));
            };
            let Some(directory_id) = workspace.primary_directory_id.as_ref() else {
                return WorkbenchPanelSplitBegin::Ready(self.retain_rejection(
                    command,
                    WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                ));
            };
            let Some(directory) = project
                .directories
                .iter()
                .find(|directory| &directory.directory_id == directory_id)
            else {
                return WorkbenchPanelSplitBegin::Ready(self.retain_rejection(
                    command,
                    WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                ));
            };
            WorkbenchPanelSplitTarget::Workspace {
                worker_id: directory.worker_id.clone(),
                path: directory.path.clone(),
            }
        };
        let last_identity = match allocate(self.last_identity, 5) {
            Ok(last_identity) => last_identity,
            Err(PlanFailure::Rejected(reason)) => {
                return WorkbenchPanelSplitBegin::Ready(self.retain_rejection(command, reason));
            }
            Err(PlanFailure::Internal) => {
                return WorkbenchPanelSplitBegin::Ready(retire(self, &command));
            }
        };
        let Some(ids) = split_ids(last_identity) else {
            return WorkbenchPanelSplitBegin::Ready(retire(self, &command));
        };
        let revision = match self.next_revision() {
            Ok(revision) => revision,
            Err(PlanFailure::Rejected(reason)) => {
                return WorkbenchPanelSplitBegin::Ready(self.retain_rejection(command, reason));
            }
            Err(PlanFailure::Internal) => {
                return WorkbenchPanelSplitBegin::Ready(retire(self, &command));
            }
        };
        // An open aggregate Close fences the same-Tab split: its publication re-proves the
        // complete captured ordered leaf membership this commit would invalidate. The
        // refusal is transient — nothing is retained and no terminal launch is staged, so
        // the same request can be retried after the Close publishes.
        if self.aggregate_close_fences_tab(&terminal_context_id, &tab_id) {
            return WorkbenchPanelSplitBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        // A `profile` selector resolves here — at prepare, under the
        // coordinator lock that owns the catalog — so the stored selection and
        // its provenance are frozen before the asynchronous gap. Resolution
        // failures are journaled like every other settled outcome, so an
        // identical request replays the same answer.
        let profile_use = match &profile {
            None => None,
            Some(selector) => match super::resolve_profile_selector(&self.catalog, selector) {
                super::ProfileSelectorResolution::Found(entry) => Some(super::LaunchProfileUse {
                    launch: entry.launch.clone(),
                    profile_id: entry.profile_id.clone(),
                    catalog_revision: orchbee_workbench_protocol::LaunchProfileCatalogRevision::new(
                        self.catalog_revision,
                    ),
                }),
                super::ProfileSelectorResolution::NotFound => {
                    return WorkbenchPanelSplitBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileUnavailable {},
                    ));
                }
                super::ProfileSelectorResolution::Ambiguous(candidate_ids) => {
                    return WorkbenchPanelSplitBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids },
                    ));
                }
            },
        };
        WorkbenchPanelSplitBegin::Admitted(Box::new(WorkbenchPanelSplitAdmission {
            authority: self,
            command,
            label: label.as_str().to_owned(),
            staged: StagedWorkbenchPanelSplitMetadata {
                revision,
                terminal_context_id,
                tab_id,
                panel_id,
                new_panel_id: ids.new_panel_id,
                split_id: ids.split_id,
                first_slot_id: ids.first_slot_id,
                second_slot_id: ids.second_slot_id,
                presentation_id: ids.presentation_id,
                geometry,
                direction,
                target,
            },
            last_identity,
            profile_use,
        }))
    }

    pub fn prepare_terminal_panel_split_settlement(
        &mut self,
        pending: PendingWorkbenchPanelSplit,
        outcome: WorkbenchTerminalOutcome,
    ) -> Result<PreparedWorkbenchPanelSplitSettlement<'_>, WorkbenchPanelSplitSettlementTokenMismatch>
    {
        let entry = self
            .ledger
            .get_mut(pending.metadata.request_id())
            .ok_or(WorkbenchPanelSplitSettlementTokenMismatch)?;
        let RetainedRequest::PendingPanelSplit(retained) = entry else {
            return Err(WorkbenchPanelSplitSettlementTokenMismatch);
        };
        if !Arc::ptr_eq(&retained.seal, &pending.seal) || retained.metadata != pending.metadata {
            return Err(WorkbenchPanelSplitSettlementTokenMismatch);
        }
        let selected = retained
            .final_results
            .select(&outcome)
            .ok_or(WorkbenchPanelSplitSettlementTokenMismatch)?;
        let result = selected.result.clone();
        let settled = Box::new(SettledRequest {
            command: retained.command.clone(),
            result: result.clone(),
        });
        let next_ledger_bytes = self
            .ledger_bytes
            .checked_sub(retained.reserved_bytes)
            .and_then(|bytes| bytes.checked_add(selected.charged_bytes))
            .ok_or(WorkbenchPanelSplitSettlementTokenMismatch)?;
        Ok(PreparedWorkbenchPanelSplitSettlement {
            entry,
            ledger_bytes: &mut self.ledger_bytes,
            settled,
            result,
            next_ledger_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchPanelSplitSettlementTokenMismatch;

fn retire(
    authority: &mut WorkbenchAuthority,
    command: &WorkbenchCommand,
) -> WorkbenchCommandResult {
    authority.retired = true;
    result(
        command,
        unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
    )
}

fn allocate(last: u128, count: u128) -> Result<u128, PlanFailure> {
    last.checked_add(count).ok_or(PlanFailure::Rejected(
        WorkbenchRejectionReason::IdentityExhausted,
    ))
}

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

struct SplitIds {
    new_panel_id: WorkbenchPanelId,
    split_id: WorkbenchSplitId,
    first_slot_id: WorkbenchPanelSlotId,
    second_slot_id: WorkbenchPanelSlotId,
    presentation_id: orchbee_workbench_protocol::TerminalPresentationId,
}

fn split_ids(last_identity: u128) -> Option<SplitIds> {
    Some(SplitIds {
        new_panel_id: WorkbenchPanelId::parse(&identity(last_identity.checked_sub(4)?)).ok()?,
        split_id: WorkbenchSplitId::parse(&identity(last_identity.checked_sub(3)?)).ok()?,
        first_slot_id: WorkbenchPanelSlotId::parse(&identity(last_identity.checked_sub(2)?))
            .ok()?,
        second_slot_id: WorkbenchPanelSlotId::parse(&identity(last_identity.checked_sub(1)?))
            .ok()?,
        presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&identity(
            last_identity,
        ))
        .ok()?,
    })
}

fn count_panel_nodes(context: &orchbee_workbench_protocol::TerminalContext) -> usize {
    context.tabs.iter().map(|tab| count_node(&tab.root)).sum()
}

fn count_node(node: &PanelNode) -> usize {
    match node {
        PanelNode::Panel { .. } => 1,
        PanelNode::Split { children, .. } => {
            1 + children
                .iter()
                .map(|child| count_node(&child.node))
                .sum::<usize>()
        }
    }
}

fn contains_leaf_panel(node: &PanelNode, panel_id: &WorkbenchPanelId) -> bool {
    match node {
        PanelNode::Panel {
            child_presentations,
            panel_id: candidate,
            ..
        } => candidate == panel_id && child_presentations.is_empty(),
        PanelNode::Split { children, .. } => children
            .iter()
            .any(|child| contains_leaf_panel(&child.node, panel_id)),
    }
}

struct PanelSplitReplacement<'a> {
    target_panel_id: &'a WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
    new_panel_id: &'a WorkbenchPanelId,
    split_id: &'a WorkbenchSplitId,
    first_slot_id: &'a WorkbenchPanelSlotId,
    second_slot_id: &'a WorkbenchPanelSlotId,
    presentation_id: &'a orchbee_workbench_protocol::TerminalPresentationId,
    label: &'a str,
    binding: PresentationBinding,
}

fn replace_panel(node: &mut PanelNode, replacement: &PanelSplitReplacement<'_>) -> bool {
    match node {
        PanelNode::Panel {
            child_presentations,
            panel_id,
            ..
        } if panel_id == replacement.target_panel_id && child_presentations.is_empty() => {
            let original = node.clone();
            let new_panel = PanelNode::Panel {
                child_presentations: Vec::new(),
                panel_id: replacement.new_panel_id.clone(),
                primary_presentation: Presentation {
                    custom_label: None,
                    binding: Some(replacement.binding.clone()),
                    label: replacement.label.to_owned(),
                    presentation_id: replacement.presentation_id.clone(),
                },
            };
            let new_first = matches!(
                replacement.direction,
                WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Left
            );
            let (first_node, second_node) = if new_first {
                (new_panel, original)
            } else {
                (original, new_panel)
            };
            *node =
                PanelNode::Split {
                    children: Box::new([
                        PanelSlot {
                            node: first_node,
                            slot_id: replacement.first_slot_id.clone(),
                        },
                        PanelSlot {
                            node: second_node,
                            slot_id: replacement.second_slot_id.clone(),
                        },
                    ]),
                    orientation: match replacement.direction {
                        WorkbenchPanelSplitDirection::Above
                        | WorkbenchPanelSplitDirection::Below => PanelOrientation::Vertical,
                        WorkbenchPanelSplitDirection::Left
                        | WorkbenchPanelSplitDirection::Right => PanelOrientation::Horizontal,
                    },
                    split_id: replacement.split_id.clone(),
                };
            true
        }
        PanelNode::Panel { .. } => false,
        PanelNode::Split { children, .. } => {
            for child in children.iter_mut() {
                if replace_panel(&mut child.node, replacement) {
                    return true;
                }
            }
            false
        }
    }
}

#[cfg(test)]
mod tests;
