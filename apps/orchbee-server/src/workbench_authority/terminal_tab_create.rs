use std::sync::Arc;

use orchbee_workbench_protocol::{
    ContractError, MAXIMUM_PANEL_NODES_PER_CONTEXT, MAXIMUM_TABS_PER_CONTEXT, PresentationBinding,
    TerminalPresentationId, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchDirectoryPath,
    WorkbenchPanelId, WorkbenchProtocolDocument, WorkbenchRejectionReason, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalFailureReason,
    WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
    WorkerId, serialize_document,
};

use super::terminal_membership::{SimpleTabIds, append_simple_tab, parse_tab_ids};
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
enum WorkbenchTabCreateTarget {
    Standalone(super::WorkbenchTerminalMembership),
    Workspace {
        worker_id: WorkerId,
        path: WorkbenchDirectoryPath,
    },
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
    pub fn standalone_membership(&self) -> Option<&super::WorkbenchTerminalMembership> {
        match &self.target {
            WorkbenchTabCreateTarget::Standalone(membership) => Some(membership),
            WorkbenchTabCreateTarget::Workspace { .. } => None,
        }
    }

    #[must_use]
    pub fn workspace_launch(&self) -> Option<(&WorkerId, &WorkbenchDirectoryPath)> {
        match &self.target {
            WorkbenchTabCreateTarget::Standalone(_) => None,
            WorkbenchTabCreateTarget::Workspace { worker_id, path } => Some((worker_id, path)),
        }
    }

    #[must_use]
    pub fn is_first(&self) -> bool {
        matches!(
            &self.target,
            WorkbenchTabCreateTarget::Standalone(membership) if membership.is_empty()
        )
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
    standalone: bool,
    first: bool,
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

    #[must_use]
    pub const fn is_first(&self) -> bool {
        self.first
    }

    #[must_use]
    pub const fn is_standalone(&self) -> bool {
        self.standalone
    }
}

pub enum WorkbenchTabCreateBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchTabCreateAdmission<'a>>),
}

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
}

impl<'a> WorkbenchTabCreateAdmission<'a> {
    #[must_use]
    pub fn staged_metadata(&self) -> &StagedWorkbenchTabCreateMetadata {
        &self.staged
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

    #[must_use]
    pub fn finish(self, binding: PresentationBinding) -> WorkbenchTabCreatePreparation<'a> {
        let Self {
            authority,
            command,
            label,
            staged,
            last_identity,
        } = self;
        let mut candidate = authority.projection.snapshot().clone();
        let Some(context) = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == staged.terminal_context_id)
        else {
            return WorkbenchTabCreatePreparation::Ready(retire(authority, &command));
        };
        let ids = SimpleTabIds {
            tab_id: staged.tab_id.clone(),
            panel_id: staged.panel_id.clone(),
            presentation_id: staged.presentation_id.clone(),
        };
        append_simple_tab(context, &ids, &label, binding.clone());
        candidate.revision = staged.revision;
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
        let first = staged.is_first();
        let standalone = staged.standalone_membership().is_some();
        let metadata = CommittedWorkbenchTabCreateMetadata {
            request_id: command.request_id.clone(),
            revision: staged.revision,
            terminal_context_id: staged.terminal_context_id,
            tab_id: staged.tab_id,
            panel_id: staged.panel_id,
            presentation_id: staged.presentation_id,
            binding,
            geometry: staged.geometry,
            standalone,
            first,
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
}

impl PreparedWorkbenchTabCreate<'_> {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchTabCreateMetadata {
        self.pending.metadata()
    }

    #[must_use]
    pub fn commit(self) -> CommittedWorkbenchTabCreate {
        self.authority.projection.commit_prepared(self.projection);
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
                grant.site_id == *self.site_id() && grant.mutate_navigation && grant.create_terminal
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
        } = command.action.clone()
        else {
            return ready(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            );
        };
        let snapshot = self.projection.snapshot();
        if command.site_id != snapshot.site_id {
            return WorkbenchTabCreateBegin::Ready(retain_rejection(
                self,
                command,
                WorkbenchRejectionReason::SiteMismatch,
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
        let tab_count = context.tabs.len();
        let target = if terminal_context_id == snapshot.navigation.standalone_terminal_context_id {
            match self.standalone_membership() {
                Ok(membership) => WorkbenchTabCreateTarget::Standalone(membership),
                Err(_error) => return WorkbenchTabCreateBegin::Ready(retire(self, &command)),
            }
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
            let Some(directory) = project
                .directories
                .iter()
                .find(|directory| &directory.directory_id == directory_id)
            else {
                return WorkbenchTabCreateBegin::Ready(retain_rejection(
                    self,
                    command,
                    WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                ));
            };
            WorkbenchTabCreateTarget::Workspace {
                worker_id: directory.worker_id.clone(),
                path: directory.path.clone(),
            }
        };
        if tab_count >= MAXIMUM_TABS_PER_CONTEXT || tab_count >= MAXIMUM_PANEL_NODES_PER_CONTEXT {
            return WorkbenchTabCreateBegin::Ready(retain_rejection(
                self,
                command,
                WorkbenchRejectionReason::ResourceLimit,
            ));
        }
        let last_identity = match allocate(self.last_identity, 3) {
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
        let revision = match self.next_revision() {
            Ok(revision) => revision,
            Err(super::PlanFailure::Rejected(reason)) => {
                return WorkbenchTabCreateBegin::Ready(retain_rejection(self, command, reason));
            }
            Err(super::PlanFailure::Internal) => {
                return WorkbenchTabCreateBegin::Ready(retire(self, &command));
            }
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
