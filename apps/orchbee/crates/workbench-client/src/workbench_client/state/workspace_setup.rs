//! Owns Client Workspace setup drafts, captured operations, known commit evidence, and
//! canonical projection settlement before selection installation. It does not create Server
//! membership locally or treat submission as a completed launch.

mod draft;
mod labels;
mod organization;
pub(in crate::workbench_client) use organization::OrganizationChoice;
pub(crate) use organization::WorkspaceOrganizationRequest;
mod navigation_configuration;
pub(crate) use navigation_configuration::SidebarNavigationProjection;
mod tab_configuration;
pub(crate) use labels::ProjectRenameRequest;
mod preview;
mod projection;
pub(crate) use preview::{WorkspaceCreationPhase, WorkspaceCreationPreview};
pub(in crate::workbench_client) mod quick;
use super::recovery_fence::RecoveryFence;
use super::{
    ClientContextTabs, ClientPhase, NavigatorRefreshStatus, OperationDeadlines, PreparedRefresh,
    SelectedTab, WorkbenchClientState, validate_canonical_refresh,
};
use crate::exchange_values::workbench::workspace_setup::{
    WorkbenchSetupCommit, WorkbenchSetupRequest,
};
use crate::tui_presentation::PresentationPhase;
use crate::workbench_client::workbench_projection::{
    ClientContextLocation, project_native_context_tabs,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetupToken(u64);
impl SetupToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum SetupIntent {
    Organize(Box<WorkspaceOrganizationRequest>),
    TabGroupName(Box<orchbee_workbench_protocol::TabConfiguration>),
    ConfigureDirectory,
    RenameLocation(LocationLabelTarget),
    RenameProject(Box<ProjectRenameRequest>),
}
#[derive(Clone, Debug)]
struct SetupDraft {
    organization: Option<Box<WorkspaceOrganizationRequest>>,
    tab_configuration: Option<orchbee_workbench_protocol::TabConfiguration>,
    project_rename: Option<Box<ProjectRenameRequest>>,
    continuation: Option<super::OperationDeadlines>,
    quick: Option<quick::QuickWorkspaceFields>,
    token: SetupToken,
    baseline: ClientContextTabs,
    stage: SetupStage,
    selected: usize,
    project: Option<WorkbenchProjectId>,
    editor: LineEditor,
    shown: bool,
    complete: bool,
    faulted: bool,
    parent: Option<Box<draft::SetupParent>>,
}

use super::line_editor::LineEditor;
pub(crate) use draft::{
    LocationLabelStatus, LocationLabelTarget, SetupDraftView, SetupEdit, SetupStage, SetupStatus,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand, WorkbenchDirectoryPath,
    WorkbenchLabel, WorkbenchProjectId, WorkbenchRequestId, WorkbenchSnapshotOutcome,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum DirectoryConfigurationUnavailable {
    WorkspaceRequired,
    Configured,
    EmptyRequired,
    Busy,
}

#[derive(Default)]
pub(super) struct SetupState {
    preparation: Option<quick::QuickPreparation>,
    last_token: u64,
    draft: Option<SetupDraft>,
    pending: Option<SetupOperation>,
    known: Option<SetupKnownCommit>,
    settled: Option<SetupKnownCommit>,
    project_quick_entry: Option<quick::ProjectQuickEntry>,
    not_applied: Option<SetupOperation>,
    retained_label: Option<labels::RetainedLabel>,
    uncertain: Option<SetupOperation>,
    status: Option<SetupStatus>,
    deadlines: Option<OperationDeadlines>,
    presentation: PresentationPhase,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct SetupOperation {
    pub(in crate::workbench_client) quick: Option<quick::QuickLaunchPlan>,
    pub(in crate::workbench_client) generation: u64,
    pub(in crate::workbench_client) command: WorkbenchCommand,
    pub(in crate::workbench_client) request: WorkbenchSetupRequest,
    baseline: ClientContextTabs,
    pub(in crate::workbench_client) deadlines: OperationDeadlines,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct SetupKnownCommit {
    pub(in crate::workbench_client) operation: SetupOperation,
    pub(in crate::workbench_client) commit: WorkbenchSetupCommit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum SetupWorkerEvidence {
    Known(SetupKnownCommit),
    Unknown(SetupOperation),
    NotApplied(SetupOperation),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum SetupResult {
    NotApplied,
    /// A definite refusal proving this session's baseline is behind the Server.
    Stale,
    ConfirmationRequired,
    Committed {
        known: Box<SetupKnownCommit>,
        outcome: Box<WorkbenchSnapshotOutcome>,
    },
}
impl SetupOperation {
    pub(in crate::workbench_client) fn known_commit(
        &self,
        commit: WorkbenchSetupCommit,
    ) -> Option<SetupKnownCommit> {
        let unchanged = if matches!(
            self.command.action,
            WorkbenchAction::NavigationConfigure { .. }
        ) {
            self.validate_navigation_receipt(&commit)?
        } else if matches!(self.command.action, WorkbenchAction::TabsConfigure { .. }) {
            self.validate_configuration_receipt(&commit)?
        } else if self.location_label_target().is_some() {
            self.validate_label_receipt(&commit)?
        } else if matches!(
            self.command.action,
            WorkbenchAction::WorkspaceSetProject { .. }
                | WorkbenchAction::WorkspaceCreateProject { .. }
                | WorkbenchAction::ProjectDissolve { .. }
                | WorkbenchAction::ProjectDissolveIfIdle { .. }
        ) {
            self.organization_receipt(&commit)?
        } else {
            // This UI postcondition pins the retained setup action and its own commit kind.
            let compatible = matches!(
                (&self.command.action, &commit),
                (
                    WorkbenchAction::ProjectCreate { .. },
                    WorkbenchSetupCommit::ProjectCreated { .. }
                ) | (
                    WorkbenchAction::WorkspaceCreate { .. },
                    WorkbenchSetupCommit::WorkspaceCreated { .. }
                ) | (
                    WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. },
                    WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted { .. }
                ) | (
                    WorkbenchAction::WorkspaceLaunch { .. },
                    WorkbenchSetupCommit::WorkspaceLaunched(_)
                )
            );
            if !compatible {
                return None;
            }
            false
        };
        let compatible = if unchanged {
            self.command.expected_revision == commit.revision()
        } else {
            self.command.expected_revision.value().checked_add(1) == Some(commit.revision().value())
        };
        compatible.then(|| SetupKnownCommit {
            operation: self.clone(),
            commit,
        })
    }
}
impl WorkbenchClientState {
    pub(super) fn setup_invariant_holds(&self) -> bool {
        let pending = self.setup.pending.as_ref();
        let known = self.setup.known.as_ref();
        if self.setup.draft.is_some()
            && (pending.is_some() || known.is_some() || self.setup.uncertain.is_some())
        {
            return false;
        }
        if pending.is_some() && known.is_some() {
            return false;
        }
        if self.setup_blocks_structure() && self.pending_mutation.is_some() {
            return false;
        }
        if self
            .setup
            .uncertain
            .as_ref()
            .is_some_and(|operation| pending != Some(operation))
        {
            return false;
        }
        if self.setup.draft.as_ref().is_some_and(|draft| {
            draft.baseline != self.cache || draft.token.0 != self.setup.last_token
        }) {
            return false;
        }
        pending
            .or_else(|| known.map(|known| &known.operation))
            .is_none_or(|operation| {
                operation.baseline == self.cache
                    && operation.command.session_id == *self.cache.session_id()
                    && operation.command.expected_revision == self.cache.revision()
                    && operation.request.request_id() == &operation.command.request_id
            })
    }
    pub(in crate::workbench_client) fn setup_authority_available(&self) -> bool {
        self.pending_refresh.is_none()
            && !self.workspace_close_blocks_structure()
            && self.pending_mutation.is_none()
            && self.pending_membership.is_none()
            && !self.recovery_fences(RecoveryFence::Setup)
            && !self.rename_draft_blocks_structure()
            && self.close.permits_retired_membership()
            && self.panel_close.confirmation().is_none()
            && matches!(
                self.phase,
                ClientPhase::Empty
                    | ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
    }
    pub(in crate::workbench_client) fn setup_blocks_structure(&self) -> bool {
        self.setup.draft.is_some()
            || self.setup.preparation.is_some()
            || self.setup_operation_in_progress()
    }
    pub(super) fn setup_operation_in_progress(&self) -> bool {
        self.setup.pending.is_some() || self.setup.known.is_some() || self.setup.uncertain.is_some()
    }
    pub(in crate::workbench_client) fn setup_recovery(&self) -> bool {
        self.setup.known.is_some() && self.setup.pending.is_none()
    }
    pub(in crate::workbench_client) fn setup_pending_or_draft(&self) -> bool {
        self.setup.pending.is_some() || self.setup.draft.is_some() || self.setup.uncertain.is_some()
    }
    pub(in crate::workbench_client) fn workspace_directory_unconfigured(&self) -> bool {
        matches!(
            self.cache.location(),
            ClientContextLocation::Workspace { .. }
        ) && self.cache.directory().is_none()
    }
    /// Read-only projection of the existing first-directory draft admission.
    pub(in crate::workbench_client) fn directory_configuration_unavailable(
        &self,
    ) -> Option<DirectoryConfigurationUnavailable> {
        if !matches!(
            self.cache.location(),
            ClientContextLocation::Workspace { .. }
        ) {
            Some(DirectoryConfigurationUnavailable::WorkspaceRequired)
        } else if self.cache.directory().is_some() {
            Some(DirectoryConfigurationUnavailable::Configured)
        } else if self.setup_blocks_structure()
            || !self.setup_authority_available()
            || !self.invariant_holds()
        {
            Some(DirectoryConfigurationUnavailable::Busy)
        } else if !matches!(self.phase, ClientPhase::Empty) {
            Some(DirectoryConfigurationUnavailable::EmptyRequired)
        } else {
            None
        }
    }
    pub(in crate::workbench_client) fn workspace_directory_path(&self) -> Option<&str> {
        self.cache
            .directory()
            .map(|directory| directory.path.as_str())
    }
    pub(in crate::workbench_client) fn plan_setup(
        &self,
        token: SetupToken,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<SetupOperation> {
        let draft = self.setup_draft()?.draft;
        if draft.token != token
            || ((!draft.shown || !self.setup.presentation.interactive())
                && draft.continuation.is_none())
            || !draft.complete
            || draft.faulted
        {
            return None;
        }
        let action = match draft.stage {
            SetupStage::ChooseWorkspaceProject => {
                let organization::OrganizationTarget::Workspace(workspace) =
                    &draft.organization.as_ref()?.target
                else {
                    return None;
                };
                if draft.selected > draft.baseline.projects().len() {
                    return None;
                }
                WorkbenchAction::WorkspaceSetProject {
                    workspace_id: workspace.workspace_id.clone(),
                    project_id: draft.project.clone(),
                }
            }
            SetupStage::WorkspaceProjectName => {
                let organization::OrganizationTarget::Workspace(workspace) =
                    &draft.organization.as_ref()?.target
                else {
                    return None;
                };
                WorkbenchAction::WorkspaceCreateProject {
                    workspace_id: workspace.workspace_id.clone(),
                    label: WorkbenchLabel::parse(draft.editor.text()).ok()?,
                }
            }
            SetupStage::DissolveProject | SetupStage::ProjectDeleteConfirm => {
                let organization::OrganizationTarget::Project(project) =
                    &draft.organization.as_ref()?.target
                else {
                    return None;
                };
                WorkbenchAction::ProjectDissolve {
                    project_id: project.project_id.clone(),
                }
            }
            SetupStage::TabGroupName => {
                let mut operation = draft.tab_configuration.clone()?;
                match &mut operation {
                    orchbee_workbench_protocol::TabConfiguration::CreateGroup { label, .. }
                    | orchbee_workbench_protocol::TabConfiguration::CreateGroupForTabs {
                        label,
                        ..
                    }
                    | orchbee_workbench_protocol::TabConfiguration::UpdateGroup { label, .. } => {
                        *label = orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse(
                            draft.editor.text(),
                        )
                        .ok()?
                    }
                    orchbee_workbench_protocol::TabConfiguration::SetGroup { .. }
                    | orchbee_workbench_protocol::TabConfiguration::Ungroup { .. }
                    | orchbee_workbench_protocol::TabConfiguration::SetPinned { .. }
                    | orchbee_workbench_protocol::TabConfiguration::SetTabsGroup { .. }
                    | orchbee_workbench_protocol::TabConfiguration::SetTabsPinned { .. }
                    | orchbee_workbench_protocol::TabConfiguration::MoveTab { .. }
                    | orchbee_workbench_protocol::TabConfiguration::MoveGroup { .. }
                    | orchbee_workbench_protocol::TabConfiguration::MoveTabsByDrag { .. } => {
                        return None;
                    }
                }
                WorkbenchAction::TabsConfigure {
                    terminal_context_id: self.cache.terminal_context_id()?.clone(),
                    operation,
                }
            }
            SetupStage::QuickName | SetupStage::QuickDirectory => {
                let plan = draft.quick.as_ref()?.plan()?;
                let placement = match &plan.project {
                    quick::QuickProject::Ungrouped => {
                        orchbee_workbench_protocol::WorkspacePlacement::Ungrouped {}
                    }
                    quick::QuickProject::Existing(_) | quick::QuickProject::Targeted(_) => {
                        orchbee_workbench_protocol::WorkspacePlacement::Project {
                            project_id: plan.project.existing_id()?.clone(),
                        }
                    }
                    quick::QuickProject::Choose => return None,
                };
                WorkbenchAction::WorkspaceLaunch {
                    placement,
                    label: plan.label,
                    geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(
                        self.latest_geometry.columns(),
                        self.latest_geometry.rows(),
                        self.latest_geometry.cell_pixel_width(),
                        self.latest_geometry.cell_pixel_height(),
                    )
                    .ok()?,
                    launch: None,
                    profile: None,
                    options: Box::new(orchbee_workbench_protocol::LaunchOptions {
                        cwd: Some(plan.cwd),
                        ..Default::default()
                    }),
                }
            }
            SetupStage::DirectoryPath => {
                let ClientContextLocation::Workspace { workspace_id, .. } = self.cache.location()
                else {
                    return None;
                };
                if !matches!(self.phase, ClientPhase::Empty)
                    || !self.workspace_directory_unconfigured()
                {
                    return None;
                }
                WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                    workspace_id: workspace_id.clone(),
                    path: WorkbenchDirectoryPath::parse(draft.editor.text()).ok()?,
                }
            }
            SetupStage::ProjectRename => WorkbenchAction::ProjectRename {
                project_id: draft.project.clone()?,
                label: WorkbenchLabel::parse(draft.editor.text()).ok()?,
            },
            SetupStage::WorkspaceRename => {
                let ClientContextLocation::Workspace { workspace_id, .. } =
                    draft.baseline.location()
                else {
                    return None;
                };
                let label = WorkbenchLabel::parse_workspace(draft.editor.text()).ok()?;
                WorkbenchAction::WorkspaceRename {
                    workspace_id: workspace_id.clone(),
                    label,
                }
            }
            SetupStage::QuickProject | SetupStage::PreparingWorkspace => return None,
        };
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: self.cache.session_id().clone(),
            expected_revision: self.cache.revision(),
            action,
        };
        Some(SetupOperation {
            quick: draft
                .quick
                .as_ref()
                .and_then(quick::QuickWorkspaceFields::plan),
            generation,
            request: WorkbenchSetupRequest::try_from(command.clone()).ok()?,
            command,
            baseline: self.cache.clone(),
            deadlines,
        })
    }
    pub(in crate::workbench_client) fn begin_setup(&mut self, operation: SetupOperation) {
        {
            self.setup.project_quick_entry = operation
                .quick
                .as_ref()
                .filter(|_| {
                    matches!(
                        operation.command.action,
                        WorkbenchAction::WorkspaceLaunch { .. }
                    ) && (matches!(
                        self.setup.project_quick_entry,
                        Some(quick::ProjectQuickEntry::Preparing(token))
                            if self.setup.draft.as_ref().is_some_and(|draft| draft.token == token)
                    ) || self.setup.draft.as_ref().is_some_and(|draft| {
                        draft.quick.is_some() && draft.continuation.is_none() && draft.shown
                    }))
                })
                .map(|_| quick::ProjectQuickEntry::Submitted(operation.generation));
        }
        self.remember_current_navigation();
        self.setup.draft = None;
        self.setup.deadlines = Some(operation.deadlines);
        self.pending_refresh = Some(super::PendingRefreshFlight::recovery(operation.generation));
        self.adopt_publication_debt();
        self.refresh_status = Some(NavigatorRefreshStatus::Refreshing);
        self.setup.pending = Some(operation);
    }
    pub(in crate::workbench_client) fn retain_setup_worker(
        &mut self,
        evidence: SetupWorkerEvidence,
    ) -> bool {
        match evidence {
            SetupWorkerEvidence::NotApplied(operation) => self.settle_setup_not_applied(&operation),
            SetupWorkerEvidence::Known(known) => {
                if self.setup.settled.as_ref() == Some(&known) {
                    return true;
                }
                if self.setup.pending.as_ref() != Some(&known.operation)
                    && self.setup.known.as_ref() != Some(&known)
                {
                    return false;
                }
                self.setup.pending = None;
                self.setup.status = Some(known.operation.status(
                    LocationLabelStatus::CommittedNeedRefresh,
                    SetupStatus::CommittedNeedRefresh,
                ));
                self.setup.known = Some(known);
                true
            }
            SetupWorkerEvidence::Unknown(operation) => {
                if self.setup.pending.as_ref() != Some(&operation) {
                    return false;
                }
                self.cancel_project_quick_entry();
                self.setup.status = Some(operation.status(
                    LocationLabelStatus::OutcomeUnknown,
                    SetupStatus::OutcomeUnknown,
                ));
                self.setup.uncertain = Some(operation);
                true
            }
        }
    }
    pub(in crate::workbench_client) fn settle_setup_not_applied(
        &mut self,
        operation: &SetupOperation,
    ) -> bool {
        if self.setup.not_applied.as_ref() == Some(operation) {
            return true;
        }
        if self.setup.pending.as_ref() != Some(operation) {
            return false;
        }
        self.cancel_project_quick_entry();
        self.setup.not_applied = self.setup.pending.take();
        self.setup.deadlines = None;
        self.setup.status =
            Some(operation.status(LocationLabelStatus::NotApplied, SetupStatus::NotApplied));
        true
    }
    /// Settles a definite Server refusal of this setup flight. The flight read no
    /// snapshot, so publication demand it adopted returns unread; a stale refusal is
    /// itself demand, keeps rename text for the same owner and names the concurrent change.
    pub(in crate::workbench_client) fn settle_setup_refusal(
        &mut self,
        operation: &SetupOperation,
        stale: bool,
    ) -> bool {
        if !self.settle_setup_not_applied(operation) {
            return false;
        }
        if self.pending_refresh_generation() == Some(operation.generation) {
            self.release_unread_publication_debt(stale);
        }
        if stale {
            self.setup.retained_label = operation.retained_label();
            self.setup.status =
                Some(operation.status(LocationLabelStatus::Stale, SetupStatus::Stale));
        }
        true
    }
    pub(super) fn setup_target_matches(&self, target: &ClientContextLocation) -> bool {
        self.setup
            .known
            .as_ref()
            .is_some_and(|known| &self.setup_refresh_target(known) == target)
    }

    fn setup_refresh_target(&self, known: &SetupKnownCommit) -> ClientContextLocation {
        if matches!(known.commit, WorkbenchSetupCommit::WorkspaceLaunched(_)) {
            // NoWorkspace ceases to be a valid canonical location after the first launch.
            // Installing its verified member does not restore revoked UI return permission.
            if matches!(self.cache.location(), ClientContextLocation::NoWorkspace) {
                return known.target();
            }
            if matches!(
                self.setup.project_quick_entry,
                Some(quick::ProjectQuickEntry::Submitted(generation))
                    if generation == known.operation.generation
            ) {
                return known.target();
            }
            // Newer focus intent revokes the asynchronous landing, including when
            // the requested action itself was refused while this launch was busy.
            return self.cache.location().clone();
        }
        known.target()
    }

    fn validated_setup_candidate(
        &self,
        known: &SetupKnownCommit,
        snapshot: &Snapshot,
    ) -> Option<ClientContextTabs> {
        // Always prove the committed resources before choosing the Client's landing.
        let launched = known.validate(snapshot)?;
        if launched.location() == &self.setup_refresh_target(known) {
            return Some(launched);
        }
        validate_canonical_refresh(&self.cache, self.cache.project_snapshot(snapshot).ok()?).ok()
    }
    pub(in crate::workbench_client) fn setup_deadlines(&self) -> Option<OperationDeadlines> {
        self.setup.deadlines
    }
    pub(super) fn prepare_setup_refresh(
        &self,
        generation: u64,
        snapshot: &Snapshot,
    ) -> PreparedRefresh {
        let prepared = self.setup.known.as_ref().and_then(|known| {
            let candidate = self.validated_setup_candidate(known, snapshot)?;
            let same = candidate.location() == self.cache.location();
            let mut prepared = self
                .prepare_membership_candidate(
                    candidate,
                    same.then(|| self.desired.as_ref().map(|key| key.anchor().tab_id()))
                        .flatten(),
                    self.active().filter(|_| same).map(SelectedTab::index),
                    None,
                    None,
                )
                .ok()?;
            if let WorkbenchSetupCommit::WorkspaceLaunched(commit) = &known.commit
                && self.setup_refresh_target(known) == known.target()
            {
                let member = crate::workbench_client::ClientMemberTarget {
                    tab: commit.tab_id.clone(),
                    panel: commit.panel_id.clone(),
                    presentation: commit.presentation_id.clone(),
                };
                if !prepared.retarget_selected_member(&known.target(), &member) {
                    return None;
                }
            }
            Some(prepared)
        });
        match prepared {
            Some(prepared) => PreparedRefresh::Membership {
                generation,
                prepared,
            },
            None => PreparedRefresh::Divergent { generation },
        }
    }
    pub(super) fn prepare_setup_completion(
        &self,
        candidate: &ClientContextTabs,
    ) -> Option<Box<PreparedSetupCompletion>> {
        let known = self.setup.known.as_ref()?;
        if &self.validated_setup_candidate(known, candidate.canonical_snapshot())? != candidate {
            return None;
        }
        let status = match &known.commit {
            WorkbenchSetupCommit::WorkspaceProjectChanged { .. } => SetupStatus::WorkspaceOrganized,
            WorkbenchSetupCommit::ProjectDissolved { .. } => SetupStatus::ProjectDissolved,
            WorkbenchSetupCommit::WorkspaceLaunched(_) => SetupStatus::WorkspaceCreated,
            WorkbenchSetupCommit::WorkspaceOrder { .. } => return None,
            WorkbenchSetupCommit::TabsConfigured { .. } => SetupStatus::TabsConfigured,
            WorkbenchSetupCommit::NavigationConfigured { .. } => SetupStatus::NavigationConfigured,
            WorkbenchSetupCommit::ProjectRenamed { unchanged, .. }
            | WorkbenchSetupCommit::WorkspaceRenamed { unchanged, .. } => known.operation.status(
                if *unchanged {
                    LocationLabelStatus::Unchanged
                } else {
                    LocationLabelStatus::Renamed
                },
                SetupStatus::NotApplied,
            ),
            WorkbenchSetupCommit::ProjectCreated { .. } => SetupStatus::ProjectCreated,
            WorkbenchSetupCommit::WorkspaceCreated { .. } => SetupStatus::WorkspaceCreated,
            WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted { .. } => {
                SetupStatus::DirectoryAccepted
            }
        };
        Some(Box::new(PreparedSetupCompletion {
            known: known.clone(),
            status,
        }))
    }
    pub(super) fn complete_setup_refresh(&mut self, prepared: PreparedSetupCompletion) {
        let PreparedSetupCompletion { known, status } = prepared;
        debug_assert_eq!(self.setup.known.as_ref(), Some(&known));
        if matches!(
            self.setup.project_quick_entry,
            Some(quick::ProjectQuickEntry::Submitted(generation))
                if generation == known.operation.generation
        ) {
            self.setup.project_quick_entry =
                self.desired
                    .clone()
                    .map(|key| quick::ProjectQuickEntry::Entering {
                        generation: known.operation.generation,
                        key,
                    });
        }
        self.setup.known = None;
        self.setup.deadlines = None;
        self.setup.status = Some(status);
        self.setup.draft = None;
        self.setup.settled = Some(known);
    }
    pub(in crate::workbench_client) fn reset_setup_recovery_deadlines(
        &mut self,
        deadlines: OperationDeadlines,
    ) {
        self.setup.deadlines = Some(deadlines);
    }
}

#[derive(Clone, Debug)]
pub(super) struct PreparedSetupCompletion {
    known: SetupKnownCommit,
    status: SetupStatus,
}
