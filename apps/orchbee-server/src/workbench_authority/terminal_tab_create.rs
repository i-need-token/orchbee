//! Stages and settles terminal creation against a single authoritative projection.
//! Directory Workspace launch shares this commit boundary and retained result ledger.
mod workspace_launch;

use std::sync::Arc;

use orchbee_workbench_protocol::{
    ContractError, NewTerminalTab, PresentationBinding, PresentationLaunchProgram,
    TabCreationPlacement, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
    WorkbenchDirectoryPath, WorkbenchPanelId, WorkbenchProtocolDocument, WorkbenchRejectionReason,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchTabGroupId, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalFailureReason, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason, WorkerId, create_terminal_tab,
    serialize_document, validate_tab_creation,
};

use super::terminal_membership::parse_tab_ids;
use super::{
    MAXIMUM_LEDGER_BYTES, RetainedRequest, SettledRequest, WorkbenchAuthority, WorkbenchGrant,
    allocate, ledger_size, result, unavailable,
};
use crate::workbench_projection::PreparedProjectionAdvance;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchTabCreateRuntimeUnavailableReason {
    NoCompatibleWorker,
    ServerBusy,
}

impl From<WorkbenchTabCreateRuntimeUnavailableReason> for WorkbenchCommandUnavailableReason {
    fn from(reason: WorkbenchTabCreateRuntimeUnavailableReason) -> Self {
        match reason {
            WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker => {
                Self::NoCompatibleWorker
            }
            WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy => Self::ServerBusy,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedWorkbenchTabCreateMetadata {
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    geometry: WorkbenchTerminalGeometry,
    target: WorkbenchTabCreateTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkbenchTabCreateTarget {
    worker_id: WorkerId,
    path: WorkbenchDirectoryPath,
}

impl StagedWorkbenchTabCreateMetadata {
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
    pub fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }

    #[must_use]
    pub const fn geometry(&self) -> WorkbenchTerminalGeometry {
        self.geometry
    }

    #[must_use]
    pub fn workspace_launch(&self) -> (&WorkerId, &WorkbenchDirectoryPath) {
        (&self.target.worker_id, &self.target.path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedWorkbenchTabCreateMetadata {
    request_id: WorkbenchRequestId,
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    binding: PresentationBinding,
    geometry: WorkbenchTerminalGeometry,
    workspace: Option<Box<WorkspaceLaunchIds>>,
}

impl CommittedWorkbenchTabCreateMetadata {
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
    pub fn presentation_id(&self) -> &TerminalPresentationId {
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceLaunchIds {
    project_id: Option<orchbee_workbench_protocol::WorkbenchProjectId>,
    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId,
    directory_id: orchbee_workbench_protocol::WorkbenchDirectoryId,
}

struct WorkspaceLaunchPreparation {
    candidate: orchbee_workbench_protocol::Snapshot,
    ids: WorkspaceLaunchIds,
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchTabCreateBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchTabCreateAdmission<'a>>),
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchTabCreatePreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchTabCreate<'a>>),
}

pub struct WorkbenchTabCreateAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    label: String,
    staged: StagedWorkbenchTabCreateMetadata,
    last_identity: u128,
    profile_use: Option<super::LaunchProfileUse>,
    plugin_use: Option<super::PluginPaneLaunch>,
    workspace: Option<WorkspaceLaunchPreparation>,
    created_group: Option<WorkbenchTabGroupId>,
}

impl<'a> WorkbenchTabCreateAdmission<'a> {
    #[must_use]
    pub fn staged_metadata(&self) -> &StagedWorkbenchTabCreateMetadata {
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
    pub const fn plugin_use(&self) -> Option<&super::PluginPaneLaunch> {
        self.plugin_use.as_ref()
    }

    #[must_use]
    pub fn runtime_unavailable(
        self,
        reason: WorkbenchTabCreateRuntimeUnavailableReason,
    ) -> WorkbenchCommandResult {
        result(&self.command, unavailable(reason.into()))
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retired = true;
        result(
            &self.command,
            unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
        )
    }

    /// Commits `binding` and the launched program its creation planned in one revision.
    #[must_use]
    pub fn finish(
        self,
        binding: PresentationBinding,
        launch_program: Option<PresentationLaunchProgram>,
    ) -> WorkbenchTabCreatePreparation<'a> {
        let Self {
            authority,
            command,
            label,
            staged,
            last_identity,
            profile_use: _,
            plugin_use,
            workspace,
            created_group,
        } = self;
        let (mut candidate, workspace) = match workspace {
            Some(prepared) => (prepared.candidate, Some(prepared.ids)),
            None => (authority.projection.snapshot().clone(), None),
        };
        let Some(context) = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == staged.terminal_context_id)
        else {
            return WorkbenchTabCreatePreparation::Ready(retire(authority, &command));
        };
        let automatic_label = matches!(
            command.action,
            WorkbenchAction::TabCreate {
                automatic_label: true,
                ..
            } | WorkbenchAction::WorkspaceLaunch { .. }
        );
        let placement = match &command.action {
            WorkbenchAction::TabCreate { placement, .. } => placement.as_ref(),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => None,
        };
        let Ok(label) = orchbee_workbench_protocol::WorkbenchLabel::parse(&label) else {
            return WorkbenchTabCreatePreparation::Ready(retire(authority, &command));
        };
        *context = match create_terminal_tab(
            context,
            placement,
            NewTerminalTab {
                tab_id: staged.tab_id.clone(),
                panel_id: staged.panel_id.clone(),
                presentation_id: staged.presentation_id.clone(),
                label,
                automatic_label,
                binding: binding.clone(),
                launch_program,
            },
            created_group.as_ref(),
        ) {
            Ok(context) => context,
            Err(reason) => {
                return WorkbenchTabCreatePreparation::Ready(retain_rejection(
                    authority, command, reason,
                ));
            }
        };
        candidate.revision = staged.revision;
        let plugins = match plugin_use
            .map(|resolved| {
                authority.plugin_panes.prepare(
                    &candidate,
                    &staged.presentation_id,
                    &binding,
                    resolved.selector,
                )
            })
            .transpose()
        {
            Ok(value) => value,
            Err(reason) => {
                return WorkbenchTabCreatePreparation::Ready(retain_rejection(
                    authority, command, reason,
                ));
            }
        };

        let encoded = match serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)) {
            Ok(encoded) => encoded,
            Err(ContractError::EncodedValueTooLarge { .. }) => {
                return WorkbenchTabCreatePreparation::Ready(retain_rejection(
                    authority,
                    command,
                    WorkbenchRejectionReason::ResourceLimit,
                ));
            }
            Err(ContractError::InvalidDocument | ContractError::WrongDocumentKind) => {
                return WorkbenchTabCreatePreparation::Ready(retire(authority, &command));
            }
        };
        let projection = match authority.projection.prepare_advance(&encoded) {
            Ok(projection) => projection,
            Err(_error) => {
                return WorkbenchTabCreatePreparation::Ready(retire(authority, &command));
            }
        };
        let metadata = CommittedWorkbenchTabCreateMetadata {
            request_id: command.request_id.clone(),
            revision: staged.revision,
            terminal_context_id: staged.terminal_context_id,
            tab_id: staged.tab_id,
            panel_id: staged.panel_id,
            presentation_id: staged.presentation_id,
            binding,
            geometry: staged.geometry,
            workspace: workspace.map(Box::new),
        };
        let Some(results) = TabCreateResultTable::new(&command, &metadata) else {
            return WorkbenchTabCreatePreparation::Ready(retire(authority, &command));
        };
        let reserved_bytes = results.maximum_charge();
        let Some(next_ledger_bytes) = authority.ledger_bytes.checked_add(reserved_bytes) else {
            return WorkbenchTabCreatePreparation::Ready(retire(authority, &command));
        };
        if next_ledger_bytes > MAXIMUM_LEDGER_BYTES {
            return WorkbenchTabCreatePreparation::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        let entry_seal = Arc::new(SettlementSeal);
        let token_seal = Arc::clone(&entry_seal);
        let request_id = command.request_id.clone();
        let pending_result = results.pending.result.clone();
        let entry = Box::new(PendingTabCreateEntry {
            command,
            pending_result,
            final_results: results,
            reserved_bytes,
            metadata: metadata.clone(),
            seal: entry_seal,
        });
        let pending = PendingWorkbenchTabCreate {
            metadata,
            seal: token_seal,
        };
        WorkbenchTabCreatePreparation::Prepared(Box::new(PreparedWorkbenchTabCreate {
            authority,
            projection,
            last_identity,
            next_ledger_bytes,
            request_id,
            entry,
            pending,
            plugins,
        }))
    }
}

pub struct PreparedWorkbenchTabCreate<'a> {
    authority: &'a mut WorkbenchAuthority,
    projection: PreparedProjectionAdvance,
    last_identity: u128,
    next_ledger_bytes: usize,
    request_id: WorkbenchRequestId,
    entry: Box<PendingTabCreateEntry>,
    pending: PendingWorkbenchTabCreate,
    plugins: Option<super::plugin_panes::Membership>,
}

impl PreparedWorkbenchTabCreate<'_> {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchTabCreateMetadata {
        self.pending.metadata()
    }

    #[must_use]
    pub fn commit(self) -> CommittedWorkbenchTabCreate {
        self.authority.projection.commit_prepared(self.projection);
        if let Some(plugins) = self.plugins {
            self.authority.plugin_panes = plugins;
        }
        self.authority.last_identity = self.last_identity;
        self.authority.ledger_bytes = self.next_ledger_bytes;
        self.authority.ledger.insert(
            self.request_id,
            RetainedRequest::PendingTabCreate(self.entry),
        );
        CommittedWorkbenchTabCreate {
            pending: self.pending,
        }
    }
}

pub struct CommittedWorkbenchTabCreate {
    pending: PendingWorkbenchTabCreate,
}

impl CommittedWorkbenchTabCreate {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchTabCreateMetadata {
        self.pending.metadata()
    }

    #[must_use]
    pub fn into_pending(self) -> PendingWorkbenchTabCreate {
        self.pending
    }
}

pub struct PendingWorkbenchTabCreate {
    metadata: CommittedWorkbenchTabCreateMetadata,
    seal: Arc<SettlementSeal>,
}

impl PendingWorkbenchTabCreate {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchTabCreateMetadata {
        &self.metadata
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchSettlementTokenMismatch;

pub struct PreparedWorkbenchTabSettlement<'a> {
    entry: &'a mut RetainedRequest,
    ledger_bytes: &'a mut usize,
    settled: Box<SettledRequest>,
    result: WorkbenchCommandResult,
    next_ledger_bytes: usize,
}

impl PreparedWorkbenchTabSettlement<'_> {
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

impl WorkbenchAuthority {
    pub fn begin_terminal_tab_create(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchTabCreateBegin<'_> {
        let authorized = matches!(&command.action, WorkbenchAction::TabCreate { .. })
            && grant.is_some_and(|grant| {
                grant.session_id == *self.session_id()
                    && grant.mutate_navigation
                    && grant.create_terminal
            });
        if !authorized {
            return ready(&command, WorkbenchCommandOutcome::Denied {});
        }
        if let Some(result) = self.command_preflight(&command) {
            return WorkbenchTabCreateBegin::Ready(result);
        }
        let WorkbenchAction::TabCreate {
            terminal_context_id,
            label,
            geometry,
            launch: _,
            profile,
            placement,
            ..
        } = command.action.clone()
        else {
            return ready(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            );
        };
        let snapshot = self.projection.snapshot();
        if command.session_id != snapshot.session_id {
            return WorkbenchTabCreateBegin::Ready(retain_rejection(
                self,
                command,
                WorkbenchRejectionReason::SessionMismatch,
            ));
        }
        if command.expected_revision != snapshot.revision {
            return WorkbenchTabCreateBegin::Ready(retain_rejection(
                self,
                command,
                WorkbenchRejectionReason::RevisionMismatch,
            ));
        }
        let Some(context) = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == terminal_context_id)
        else {
            return WorkbenchTabCreateBegin::Ready(retain_rejection(
                self,
                command,
                WorkbenchRejectionReason::TerminalContextUnavailable,
            ));
        };
        if let Err(reason) = validate_tab_creation(context, placement.as_ref()) {
            return WorkbenchTabCreateBegin::Ready(retain_rejection(self, command, reason));
        }
        let target = {
            let Some(workspace) = snapshot
                .navigation
                .workspaces
                .iter()
                .find(|workspace| workspace.terminal_context_id == terminal_context_id)
            else {
                return WorkbenchTabCreateBegin::Ready(retain_rejection(
                    self,
                    command,
                    WorkbenchRejectionReason::TerminalContextUnavailable,
                ));
            };
            let Some(directory_id) = workspace.primary_directory_id.as_ref() else {
                return WorkbenchTabCreateBegin::Ready(retain_rejection(
                    self,
                    command,
                    WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                ));
            };
            let Some(directory) = snapshot.navigation.directory(directory_id) else {
                return WorkbenchTabCreateBegin::Ready(retain_rejection(
                    self,
                    command,
                    WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                ));
            };
            WorkbenchTabCreateTarget {
                worker_id: directory.worker_id.clone(),
                path: directory.path.clone(),
            }
        };
        // An open Context Close fences every membership change in its
        // Terminal Context: a new member would invalidate the captured
        // ordered re-proof its publication performs.
        if self.open_close_fences_context(&terminal_context_id) {
            return WorkbenchTabCreateBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        let new_group = matches!(placement, Some(TabCreationPlacement::NewGroup {}));
        let last_identity = match allocate(self.last_identity, if new_group { 4 } else { 3 }) {
            Ok(last_identity) => last_identity,
            Err(super::PlanFailure::Rejected(reason)) => {
                return WorkbenchTabCreateBegin::Ready(retain_rejection(self, command, reason));
            }
            Err(super::PlanFailure::Internal) => {
                return WorkbenchTabCreateBegin::Ready(retire(self, &command));
            }
        };
        let Some(ids) = parse_tab_ids(last_identity) else {
            return WorkbenchTabCreateBegin::Ready(retire(self, &command));
        };
        let created_group = if new_group {
            let Ok(id) = WorkbenchTabGroupId::parse(&super::identity(last_identity - 3)) else {
                return WorkbenchTabCreateBegin::Ready(retire(self, &command));
            };
            Some(id)
        } else {
            None
        };
        let revision = match self.next_revision() {
            Ok(revision) => revision,
            Err(super::PlanFailure::Rejected(reason)) => {
                return WorkbenchTabCreateBegin::Ready(retain_rejection(self, command, reason));
            }
            Err(super::PlanFailure::Internal) => {
                return WorkbenchTabCreateBegin::Ready(retire(self, &command));
            }
        };
        // A `profile` selector resolves here — at prepare, under the
        // coordinator lock that owns the catalog — so the stored selection and
        // its provenance are frozen before the asynchronous gap. Resolution
        // failures are journaled like every other settled outcome, so an
        // identical request replays the same answer.
        let plugin_use = match &command.action {
            WorkbenchAction::TabCreate { options, .. }
            | WorkbenchAction::PanelSplit { options, .. } => {
                match options
                    .plugin_pane
                    .as_ref()
                    .map(|selector| {
                        super::plugin_panes::resolve(self.projection.snapshot(), &command, selector)
                    })
                    .transpose()
                {
                    Ok(value) => value,
                    Err(reason) => {
                        return WorkbenchTabCreateBegin::Ready(
                            self.retain_outcome(command, unavailable(reason)),
                        );
                    }
                }
            }
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => None,
        };
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
                    return WorkbenchTabCreateBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileUnavailable {},
                    ));
                }
                super::ProfileSelectorResolution::Ambiguous(candidate_ids) => {
                    return WorkbenchTabCreateBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids },
                    ));
                }
            },
        };
        WorkbenchTabCreateBegin::Admitted(Box::new(WorkbenchTabCreateAdmission {
            authority: self,
            command,
            label: label.as_str().to_owned(),
            staged: StagedWorkbenchTabCreateMetadata {
                revision,
                terminal_context_id,
                tab_id: ids.tab_id,
                panel_id: ids.panel_id,
                presentation_id: ids.presentation_id,
                geometry,
                target,
            },
            last_identity,
            profile_use,
            plugin_use,
            workspace: None,
            created_group,
        }))
    }

    pub fn prepare_terminal_tab_settlement(
        &mut self,
        pending: PendingWorkbenchTabCreate,
        outcome: WorkbenchTerminalOutcome,
    ) -> Result<PreparedWorkbenchTabSettlement<'_>, WorkbenchSettlementTokenMismatch> {
        let entry = self
            .ledger
            .get_mut(pending.metadata.request_id())
            .ok_or(WorkbenchSettlementTokenMismatch)?;
        let RetainedRequest::PendingTabCreate(retained) = entry else {
            return Err(WorkbenchSettlementTokenMismatch);
        };
        if !Arc::ptr_eq(&retained.seal, &pending.seal) || retained.metadata != pending.metadata {
            return Err(WorkbenchSettlementTokenMismatch);
        }
        let selected = retained
            .final_results
            .select(&outcome)
            .ok_or(WorkbenchSettlementTokenMismatch)?;
        let result = selected.result.clone();
        let settled = Box::new(SettledRequest {
            command: retained.command.clone(),
            result: result.clone(),
        });
        let next_ledger_bytes = self
            .ledger_bytes
            .checked_sub(retained.reserved_bytes)
            .and_then(|bytes| bytes.checked_add(selected.charged_bytes))
            .ok_or(WorkbenchSettlementTokenMismatch)?;
        Ok(PreparedWorkbenchTabSettlement {
            entry,
            ledger_bytes: &mut self.ledger_bytes,
            settled,
            result,
            next_ledger_bytes,
        })
    }
}

pub(super) struct PendingTabCreateEntry {
    pub(super) command: WorkbenchCommand,
    pub(super) pending_result: WorkbenchCommandResult,
    final_results: TabCreateResultTable,
    pub(super) reserved_bytes: usize,
    metadata: CommittedWorkbenchTabCreateMetadata,
    seal: Arc<SettlementSeal>,
}

impl PendingTabCreateEntry {
    /// The committed metadata — `panel.relocate` fences the minted leaf while
    /// its terminal settlement is outstanding.
    pub(super) fn metadata(&self) -> &CommittedWorkbenchTabCreateMetadata {
        &self.metadata
    }
}

struct SettlementSeal;

struct PrebuiltResult {
    result: WorkbenchCommandResult,
    charged_bytes: usize,
}

struct TabCreateResultTable {
    pending: PrebuiltResult,
    finals: [(WorkbenchTerminalOutcome, PrebuiltResult); 5],
}

impl TabCreateResultTable {
    fn new(
        command: &WorkbenchCommand,
        metadata: &CommittedWorkbenchTabCreateMetadata,
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

fn pending_outcome(metadata: &CommittedWorkbenchTabCreateMetadata) -> WorkbenchCommandOutcome {
    if let Some(workspace) = &metadata.workspace {
        return WorkbenchCommandOutcome::WorkspaceLaunchPending {
            revision: metadata.revision,
            project_id: workspace.project_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            directory_id: workspace.directory_id.clone(),
            terminal_context_id: metadata.terminal_context_id.clone(),
            tab_id: metadata.tab_id.clone(),
            panel_id: metadata.panel_id.clone(),
            presentation_id: metadata.presentation_id.clone(),
        };
    }

    WorkbenchCommandOutcome::TabCreatePending {
        revision: metadata.revision,
        terminal_context_id: metadata.terminal_context_id.clone(),
        tab_id: metadata.tab_id.clone(),
        panel_id: metadata.panel_id.clone(),
        presentation_id: metadata.presentation_id.clone(),
    }
}

fn final_outcome(
    metadata: &CommittedWorkbenchTabCreateMetadata,
    terminal_outcome: WorkbenchTerminalOutcome,
) -> WorkbenchCommandOutcome {
    if let Some(workspace) = &metadata.workspace {
        return WorkbenchCommandOutcome::WorkspaceLaunched {
            revision: metadata.revision,
            project_id: workspace.project_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            directory_id: workspace.directory_id.clone(),
            terminal_context_id: metadata.terminal_context_id.clone(),
            tab_id: metadata.tab_id.clone(),
            panel_id: metadata.panel_id.clone(),
            presentation_id: metadata.presentation_id.clone(),
            terminal_outcome,
        };
    }

    WorkbenchCommandOutcome::TabCreated {
        revision: metadata.revision,
        terminal_context_id: metadata.terminal_context_id.clone(),
        tab_id: metadata.tab_id.clone(),
        panel_id: metadata.panel_id.clone(),
        presentation_id: metadata.presentation_id.clone(),
        terminal_outcome,
    }
}

fn ready<'a>(
    command: &WorkbenchCommand,
    outcome: WorkbenchCommandOutcome,
) -> WorkbenchTabCreateBegin<'a> {
    WorkbenchTabCreateBegin::Ready(result(command, outcome))
}

fn retain_rejection(
    authority: &mut WorkbenchAuthority,
    command: WorkbenchCommand,
    reason: WorkbenchRejectionReason,
) -> WorkbenchCommandResult {
    authority.retain_rejection(command, reason)
}

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

#[cfg(test)]
mod tests;
