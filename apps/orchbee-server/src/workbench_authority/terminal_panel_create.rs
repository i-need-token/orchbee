//! Owns logical Panel creation admission, reserved identities and receipts, and exact prepared
//! membership publication and settlement. Physical launch authorization and Worker execution
//! remain with the coordinating runtime.

use std::sync::Arc;

mod placement;
use placement::TerminalPlacement;

use orchbee_workbench_protocol::{
    ContractError, PanelNode, PanelOrientation, PanelSlot, Presentation, PresentationBinding,
    PresentationLaunchProgram, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchPanelSplitDirection, WorkbenchProtocolDocument,
    WorkbenchRejectionReason, WorkbenchRequestId, WorkbenchRevision, WorkbenchSplitId,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchTerminalFailureReason,
    WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
    WorkerId, serialize_document,
};

use super::{
    MAXIMUM_LEDGER_BYTES, PlanFailure, RetainedRequest, SettledRequest, WorkbenchAuthority,
    WorkbenchGrant, ledger_size, result, unavailable,
};
use crate::workbench_projection::PreparedProjectionAdvance;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedWorkbenchPanelCreateMetadata {
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    placement: TerminalPlacement,
    presentation_id: orchbee_workbench_protocol::TerminalPresentationId,
    geometry: WorkbenchTerminalGeometry,
    target: WorkbenchPanelCreateTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkbenchPanelCreateTarget {
    worker_id: WorkerId,
    path: orchbee_workbench_protocol::WorkbenchDirectoryPath,
}

impl StagedWorkbenchPanelCreateMetadata {
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
    pub fn terminal_panel_id(&self) -> &WorkbenchPanelId {
        self.placement.terminal_panel(&self.panel_id)
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
    pub const fn direction(&self) -> Option<WorkbenchPanelSplitDirection> {
        self.placement.direction()
    }

    #[must_use]
    pub fn workspace_launch(
        &self,
    ) -> (
        &orchbee_workbench_protocol::WorkerId,
        &orchbee_workbench_protocol::WorkbenchDirectoryPath,
    ) {
        (&self.target.worker_id, &self.target.path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedWorkbenchPanelCreateMetadata {
    request_id: WorkbenchRequestId,
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    placement: TerminalPlacement,
    presentation_id: orchbee_workbench_protocol::TerminalPresentationId,
    binding: PresentationBinding,
    geometry: WorkbenchTerminalGeometry,
}

impl CommittedWorkbenchPanelCreateMetadata {
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
    pub fn terminal_panel_id(&self) -> &WorkbenchPanelId {
        self.placement.terminal_panel(&self.panel_id)
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
    pub const fn direction(&self) -> Option<WorkbenchPanelSplitDirection> {
        self.placement.direction()
    }
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelCreateBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchPanelCreateAdmission<'a>>),
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchPanelCreatePreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchPanelCreate<'a>>),
}

pub struct WorkbenchPanelCreateAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    staged: StagedWorkbenchPanelCreateMetadata,
    last_identity: u128,
    profile_use: Option<super::LaunchProfileUse>,
    plugin_use: Option<super::PluginPaneLaunch>,
}

impl<'a> WorkbenchPanelCreateAdmission<'a> {
    #[must_use]
    pub fn staged_metadata(&self) -> &StagedWorkbenchPanelCreateMetadata {
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
        reason: WorkbenchCommandUnavailableReason,
    ) -> WorkbenchCommandResult {
        result(&self.command, unavailable(reason))
    }

    /// A definite planning rejection commits no membership or terminal reservation.
    #[must_use]
    pub fn preflight_rejected(self, reason: WorkbenchRejectionReason) -> WorkbenchCommandResult {
        result(
            &self.command,
            WorkbenchCommandOutcome::Rejected {
                revision: self.authority.projection.snapshot().revision,
                reason,
            },
        )
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
    ) -> WorkbenchPanelCreatePreparation<'a> {
        let Self {
            authority,
            command,
            staged,
            last_identity,
            profile_use: _,
            plugin_use,
        } = self;
        let mut candidate = authority.projection.snapshot().clone();
        let Some(context) = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == staged.terminal_context_id)
        else {
            return WorkbenchPanelCreatePreparation::Ready(retire(authority, &command));
        };
        let Some(tab) = context
            .tabs
            .iter_mut()
            .find(|tab| tab.tab_id == staged.tab_id)
        else {
            return WorkbenchPanelCreatePreparation::Ready(retire(authority, &command));
        };
        if !staged.placement.apply(
            &mut tab.root,
            &staged.panel_id,
            &staged.presentation_id,
            &command.action,
            binding.clone(),
            launch_program,
        ) {
            return WorkbenchPanelCreatePreparation::Ready(retire(authority, &command));
        }
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
                return WorkbenchPanelCreatePreparation::Ready(
                    authority.retain_rejection(command, reason),
                );
            }
        };

        let encoded = match serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)) {
            Ok(encoded) => encoded,
            Err(ContractError::EncodedValueTooLarge { .. }) => {
                return WorkbenchPanelCreatePreparation::Ready(
                    authority.retain_rejection(command, WorkbenchRejectionReason::ResourceLimit),
                );
            }
            Err(ContractError::InvalidDocument | ContractError::WrongDocumentKind) => {
                return WorkbenchPanelCreatePreparation::Ready(retire(authority, &command));
            }
        };
        let projection = match authority.projection.prepare_advance(&encoded) {
            Ok(projection) => projection,
            Err(_error) => {
                return WorkbenchPanelCreatePreparation::Ready(retire(authority, &command));
            }
        };
        let metadata = CommittedWorkbenchPanelCreateMetadata {
            request_id: command.request_id.clone(),
            revision: staged.revision,
            terminal_context_id: staged.terminal_context_id,
            tab_id: staged.tab_id,
            panel_id: staged.panel_id,
            placement: staged.placement,
            presentation_id: staged.presentation_id,
            binding,
            geometry: staged.geometry,
        };
        let Some(results) = PanelCreateResultTable::new(&command, &metadata) else {
            return WorkbenchPanelCreatePreparation::Ready(retire(authority, &command));
        };
        let reserved_bytes = results.maximum_charge();
        let Some(next_ledger_bytes) = authority.ledger_bytes.checked_add(reserved_bytes) else {
            return WorkbenchPanelCreatePreparation::Ready(retire(authority, &command));
        };
        if next_ledger_bytes > MAXIMUM_LEDGER_BYTES {
            return WorkbenchPanelCreatePreparation::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        let entry_seal = Arc::new(PanelCreateSettlementSeal);
        let token_seal = Arc::clone(&entry_seal);
        let request_id = command.request_id.clone();
        let pending_result = results.pending.result.clone();
        let entry = Box::new(PendingPanelCreateEntry {
            command,
            pending_result,
            final_results: results,
            reserved_bytes,
            metadata: metadata.clone(),
            seal: entry_seal,
        });
        let pending = PendingWorkbenchPanelCreate {
            metadata,
            seal: token_seal,
        };
        WorkbenchPanelCreatePreparation::Prepared(Box::new(PreparedWorkbenchPanelCreate {
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

pub struct PreparedWorkbenchPanelCreate<'a> {
    authority: &'a mut WorkbenchAuthority,
    projection: PreparedProjectionAdvance,
    last_identity: u128,
    next_ledger_bytes: usize,
    request_id: WorkbenchRequestId,
    entry: Box<PendingPanelCreateEntry>,
    pending: PendingWorkbenchPanelCreate,
    plugins: Option<super::plugin_panes::Membership>,
}

impl PreparedWorkbenchPanelCreate<'_> {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchPanelCreateMetadata {
        &self.entry.metadata
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        self.commit_state().0
    }

    #[must_use]
    pub fn commit_into_pending(self) -> PendingWorkbenchPanelCreate {
        self.commit_state().1
    }

    fn commit_state(self) -> (WorkbenchCommandResult, PendingWorkbenchPanelCreate) {
        let Self {
            authority,
            projection,
            last_identity,
            next_ledger_bytes,
            request_id,
            entry,
            pending,
            plugins,
        } = self;
        authority.projection.commit_prepared(projection);
        if let Some(plugins) = plugins {
            authority.plugin_panes = plugins;
        }
        authority.last_identity = last_identity;
        authority.ledger_bytes = next_ledger_bytes;
        let response = entry.pending_result.clone();
        authority
            .ledger
            .insert(request_id, RetainedRequest::PendingPanelCreate(entry));
        (response, pending)
    }
}

pub struct PendingWorkbenchPanelCreate {
    metadata: CommittedWorkbenchPanelCreateMetadata,
    seal: Arc<PanelCreateSettlementSeal>,
}

impl PendingWorkbenchPanelCreate {
    #[must_use]
    pub fn metadata(&self) -> &CommittedWorkbenchPanelCreateMetadata {
        &self.metadata
    }
}

pub struct PreparedWorkbenchPanelCreateSettlement<'a> {
    entry: &'a mut RetainedRequest,
    ledger_bytes: &'a mut usize,
    settled: Box<SettledRequest>,
    result: WorkbenchCommandResult,
    next_ledger_bytes: usize,
}

impl PreparedWorkbenchPanelCreateSettlement<'_> {
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

struct PanelCreateSettlementSeal;

struct PrebuiltResult {
    result: WorkbenchCommandResult,
    charged_bytes: usize,
}

struct PanelCreateResultTable {
    pending: PrebuiltResult,
    finals: [(WorkbenchTerminalOutcome, PrebuiltResult); 5],
}

impl PanelCreateResultTable {
    fn new(
        command: &WorkbenchCommand,
        metadata: &CommittedWorkbenchPanelCreateMetadata,
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

pub(super) struct PendingPanelCreateEntry {
    pub(super) command: WorkbenchCommand,
    pub(super) pending_result: WorkbenchCommandResult,
    final_results: PanelCreateResultTable,
    pub(super) reserved_bytes: usize,
    metadata: CommittedWorkbenchPanelCreateMetadata,
    seal: Arc<PanelCreateSettlementSeal>,
}

impl PendingPanelCreateEntry {
    /// The committed metadata — `panel.relocate` fences the minted leaf while
    /// its terminal settlement is outstanding.
    pub(super) fn metadata(&self) -> &CommittedWorkbenchPanelCreateMetadata {
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

fn pending_outcome(metadata: &CommittedWorkbenchPanelCreateMetadata) -> WorkbenchCommandOutcome {
    match &metadata.placement {
        TerminalPlacement::Split {
            new_panel_id,
            split_id,
            first_slot_id,
            second_slot_id,
            ..
        } => WorkbenchCommandOutcome::PanelSplitPending {
            revision: metadata.revision,
            terminal_context_id: metadata.terminal_context_id.clone(),
            tab_id: metadata.tab_id.clone(),
            panel_id: metadata.panel_id.clone(),
            new_panel_id: new_panel_id.clone(),
            split_id: split_id.clone(),
            first_slot_id: first_slot_id.clone(),
            second_slot_id: second_slot_id.clone(),
            presentation_id: metadata.presentation_id.clone(),
        },
        TerminalPlacement::Child | TerminalPlacement::Unbound => {
            WorkbenchCommandOutcome::PanelTerminalPending {
                revision: metadata.revision,
                terminal_context_id: metadata.terminal_context_id.clone(),
                tab_id: metadata.tab_id.clone(),
                panel_id: metadata.panel_id.clone(),
                presentation_id: metadata.presentation_id.clone(),
            }
        }
    }
}

fn final_outcome(
    metadata: &CommittedWorkbenchPanelCreateMetadata,
    terminal_outcome: WorkbenchTerminalOutcome,
) -> WorkbenchCommandOutcome {
    match &metadata.placement {
        TerminalPlacement::Split {
            new_panel_id,
            split_id,
            first_slot_id,
            second_slot_id,
            ..
        } => WorkbenchCommandOutcome::PanelSplitCreated {
            revision: metadata.revision,
            terminal_context_id: metadata.terminal_context_id.clone(),
            tab_id: metadata.tab_id.clone(),
            panel_id: metadata.panel_id.clone(),
            new_panel_id: new_panel_id.clone(),
            split_id: split_id.clone(),
            first_slot_id: first_slot_id.clone(),
            second_slot_id: second_slot_id.clone(),
            presentation_id: metadata.presentation_id.clone(),
            terminal_outcome,
        },
        TerminalPlacement::Child | TerminalPlacement::Unbound => {
            WorkbenchCommandOutcome::PanelTerminalCreated {
                revision: metadata.revision,
                terminal_context_id: metadata.terminal_context_id.clone(),
                tab_id: metadata.tab_id.clone(),
                panel_id: metadata.panel_id.clone(),
                presentation_id: metadata.presentation_id.clone(),
                terminal_outcome,
            }
        }
    }
}

impl WorkbenchAuthority {
    pub fn begin_terminal_panel_create(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchPanelCreateBegin<'_> {
        let authorized = matches!(
            &command.action,
            WorkbenchAction::PanelSplit { .. } | WorkbenchAction::PanelCreateTerminal { .. }
        ) && grant.is_some_and(|grant| {
            grant.session_id == *self.session_id()
                && grant.mutate_navigation
                && grant.create_terminal
        });
        if !authorized {
            return WorkbenchPanelCreateBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(result) = self.command_preflight(&command) {
            return WorkbenchPanelCreateBegin::Ready(result);
        }
        let (terminal_context_id, tab_id, panel_id, geometry, profile) =
            match command.action.clone() {
                WorkbenchAction::PanelSplit {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    geometry,
                    profile,
                    ..
                }
                | WorkbenchAction::PanelCreateTerminal {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    geometry,
                    profile,
                    ..
                } => (terminal_context_id, tab_id, panel_id, geometry, profile),
                // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
                _ => return WorkbenchPanelCreateBegin::Ready(retire(self, &command)),
            };
        let snapshot = self.projection.snapshot();
        if command.session_id != snapshot.session_id {
            return WorkbenchPanelCreateBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::SessionMismatch),
            );
        }
        if command.expected_revision != snapshot.revision {
            return WorkbenchPanelCreateBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::RevisionMismatch),
            );
        }
        let Some(context) = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == terminal_context_id)
        else {
            return WorkbenchPanelCreateBegin::Ready(self.retain_rejection(
                command,
                WorkbenchRejectionReason::TerminalContextUnavailable,
            ));
        };
        let Some(tab) = context.tabs.iter().find(|tab| tab.tab_id == tab_id) else {
            return WorkbenchPanelCreateBegin::Ready(
                self.retain_rejection(command, WorkbenchRejectionReason::TabUnavailable),
            );
        };
        let placement = match placement::prepare(
            context,
            &tab.root,
            &panel_id,
            &command.action,
            self.last_identity,
        ) {
            Ok(placement) => placement,
            Err(PlanFailure::Rejected(reason)) => {
                return WorkbenchPanelCreateBegin::Ready(self.retain_rejection(command, reason));
            }
            Err(PlanFailure::Internal) => {
                return WorkbenchPanelCreateBegin::Ready(retire(self, &command));
            }
        };
        let target = {
            let Some(workspace) = snapshot
                .navigation
                .workspaces
                .iter()
                .find(|workspace| workspace.terminal_context_id == terminal_context_id)
            else {
                return WorkbenchPanelCreateBegin::Ready(self.retain_rejection(
                    command,
                    WorkbenchRejectionReason::TerminalContextUnavailable,
                ));
            };
            let Some(directory_id) = workspace.primary_directory_id.as_ref() else {
                return WorkbenchPanelCreateBegin::Ready(self.retain_rejection(
                    command,
                    WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                ));
            };
            let Some(directory) = snapshot.navigation.directory(directory_id) else {
                return WorkbenchPanelCreateBegin::Ready(self.retain_rejection(
                    command,
                    WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
                ));
            };
            WorkbenchPanelCreateTarget {
                worker_id: directory.worker_id.clone(),
                path: directory.path.clone(),
            }
        };
        let last_identity = placement.last_identity;
        let revision = match self.next_revision() {
            Ok(revision) => revision,
            Err(PlanFailure::Rejected(reason)) => {
                return WorkbenchPanelCreateBegin::Ready(self.retain_rejection(command, reason));
            }
            Err(PlanFailure::Internal) => {
                return WorkbenchPanelCreateBegin::Ready(retire(self, &command));
            }
        };
        // An open aggregate Close fences the same-Tab split: its publication re-proves the
        // complete captured ordered leaf membership this commit would invalidate. The
        // refusal is transient — nothing is retained and no terminal launch is staged, so
        // the same request can be retried after the Close publishes.
        if self.aggregate_close_fences_tab(&terminal_context_id, &tab_id)
            || (matches!(command.action, WorkbenchAction::PanelCreateTerminal { .. })
                && self.open_close_covers_panel(&terminal_context_id, &tab_id, &panel_id))
        {
            return WorkbenchPanelCreateBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
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
                        return WorkbenchPanelCreateBegin::Ready(
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
                    return WorkbenchPanelCreateBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileUnavailable {},
                    ));
                }
                super::ProfileSelectorResolution::Ambiguous(candidate_ids) => {
                    return WorkbenchPanelCreateBegin::Ready(self.retain_outcome(
                        command,
                        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids },
                    ));
                }
            },
        };
        WorkbenchPanelCreateBegin::Admitted(Box::new(WorkbenchPanelCreateAdmission {
            authority: self,
            command,
            staged: StagedWorkbenchPanelCreateMetadata {
                revision,
                terminal_context_id,
                tab_id,
                panel_id,
                placement: placement.placement,
                presentation_id: placement.presentation_id,
                geometry,
                target,
            },
            last_identity,
            profile_use,
            plugin_use,
        }))
    }

    pub fn prepare_terminal_panel_create_settlement(
        &mut self,
        pending: PendingWorkbenchPanelCreate,
        outcome: WorkbenchTerminalOutcome,
    ) -> Result<
        PreparedWorkbenchPanelCreateSettlement<'_>,
        WorkbenchPanelCreateSettlementTokenMismatch,
    > {
        let entry = self
            .ledger
            .get_mut(pending.metadata.request_id())
            .ok_or(WorkbenchPanelCreateSettlementTokenMismatch)?;
        let RetainedRequest::PendingPanelCreate(retained) = entry else {
            return Err(WorkbenchPanelCreateSettlementTokenMismatch);
        };
        if !Arc::ptr_eq(&retained.seal, &pending.seal) || retained.metadata != pending.metadata {
            return Err(WorkbenchPanelCreateSettlementTokenMismatch);
        }
        let selected = retained
            .final_results
            .select(&outcome)
            .ok_or(WorkbenchPanelCreateSettlementTokenMismatch)?;
        let result = selected.result.clone();
        let settled = Box::new(SettledRequest {
            command: retained.command.clone(),
            result: result.clone(),
        });
        let next_ledger_bytes = self
            .ledger_bytes
            .checked_sub(retained.reserved_bytes)
            .and_then(|bytes| bytes.checked_add(selected.charged_bytes))
            .ok_or(WorkbenchPanelCreateSettlementTokenMismatch)?;
        Ok(PreparedWorkbenchPanelCreateSettlement {
            entry,
            ledger_bytes: &mut self.ledger_bytes,
            settled,
            result,
            next_ledger_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchPanelCreateSettlementTokenMismatch;

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

struct PanelSplitReplacement<'a> {
    target_panel_id: &'a WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
    new_panel_id: &'a WorkbenchPanelId,
    split_id: &'a WorkbenchSplitId,
    first_slot_id: &'a WorkbenchPanelSlotId,
    second_slot_id: &'a WorkbenchPanelSlotId,
    presentation_id: &'a orchbee_workbench_protocol::TerminalPresentationId,
    label: &'a str,
    custom_label: Option<&'a orchbee_workbench_protocol::PanelCustomLabel>,
    right_click: orchbee_workbench_protocol::PanelRightClickTarget,
    binding: PresentationBinding,
    launch_program: Option<PresentationLaunchProgram>,
}

fn replace_panel(node: &mut PanelNode, replacement: &PanelSplitReplacement<'_>) -> bool {
    match node {
        PanelNode::Panel { panel_id, .. } if panel_id == replacement.target_panel_id => {
            let original = node.clone();
            let new_panel = PanelNode::Panel {
                right_click: replacement.right_click,
                child_presentations: Vec::new(),
                panel_id: replacement.new_panel_id.clone(),
                primary_presentation: Presentation {
                    custom_label: replacement.custom_label.cloned(),
                    binding: Some(replacement.binding.clone()),
                    label: replacement.label.to_owned(),
                    presentation_id: replacement.presentation_id.clone(),
                    launch_failure: None,
                    agent_not_resumed: None,
                    launch_program: replacement.launch_program.clone(),
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
                    ratio: Default::default(),
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
