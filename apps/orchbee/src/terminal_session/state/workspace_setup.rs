mod draft;
mod labels;
mod organization;
pub(crate) use organization::WorkspaceOrganizationRequest;
mod navigation_configuration;
mod tab_configuration;
pub(crate) use labels::ProjectRenameRequest;
#[cfg(windows)]
mod preview;
mod projection;
#[cfg(windows)]
pub(crate) use preview::{WorkspaceCreationPhase, WorkspaceCreationPreview};
pub(in crate::terminal_session) mod quick;
use super::{
    NativeContextTabs, NativeSessionState, NavigatorRefreshStatus, OperationDeadlines,
    PreparedRefresh, SelectedTab, SessionPhase, validate_canonical_refresh,
};
use crate::local_client::workbench::workspace_setup::{
    WorkbenchSetupCommit, WorkbenchSetupRequest,
};
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, project_native_context_tabs,
};
use crate::tui_presentation::PresentationPhase;
pub(in crate::terminal_session) use draft::NativeSetupIntent;
use draft::SetupDraft;
pub(crate) use draft::{
    NativeLocationLabelStatus, NativeLocationLabelTarget, NativeSetupDraftView, NativeSetupEdit,
    NativeSetupStage, NativeSetupStatus, NativeSetupToken,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand, WorkbenchDirectoryPath,
    WorkbenchLabel, WorkbenchProjectId, WorkbenchRequestId, WorkbenchSnapshotOutcome,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum DirectoryConfigurationUnavailable {
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
    #[cfg(any(windows, test))]
    project_quick_entry: Option<quick::ProjectQuickEntry>,
    not_applied: Option<SetupOperation>,
    uncertain: Option<SetupOperation>,
    status: Option<NativeSetupStatus>,
    deadlines: Option<OperationDeadlines>,
    presentation: PresentationPhase,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct SetupOperation {
    pub(in crate::terminal_session) quick: Option<quick::QuickLaunchPlan>,
    pub(in crate::terminal_session) generation: u64,
    pub(in crate::terminal_session) command: WorkbenchCommand,
    pub(in crate::terminal_session) request: WorkbenchSetupRequest,
    baseline: NativeContextTabs,
    pub(in crate::terminal_session) deadlines: OperationDeadlines,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct SetupKnownCommit {
    pub(in crate::terminal_session) operation: SetupOperation,
    pub(in crate::terminal_session) commit: WorkbenchSetupCommit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum SetupWorkerEvidence {
    Known(SetupKnownCommit),
    Unknown(SetupOperation),
    NotApplied(SetupOperation),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum SetupResult {
    NotApplied,
    Committed {
        known: Box<SetupKnownCommit>,
        outcome: Box<WorkbenchSnapshotOutcome>,
    },
}
impl SetupOperation {
    pub(in crate::terminal_session) fn known_commit(
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
        ) {
            self.organization_receipt(&commit)?
        } else {
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
impl NativeSessionState {
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
                    && operation.command.site_id == *self.cache.site_id()
                    && operation.command.expected_revision == self.cache.revision()
                    && operation.request.request_id() == &operation.command.request_id
            })
    }
    pub(in crate::terminal_session) fn setup_authority_available(&self) -> bool {
        self.pending_refresh.is_none()
            && !self.workspace_close_blocks_structure()
            && self.pending_mutation.is_none()
            && self.pending_membership.is_none()
            && self.move_recovery.is_none()
            && self.rename_recovery.is_none()
            && !self.rename_draft_blocks_structure()
            && self.close.permits_retired_membership()
            && self.panel_close.confirmation().is_none()
            && matches!(
                self.phase,
                SessionPhase::Empty
                    | SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
    }
    pub(in crate::terminal_session) fn setup_blocks_structure(&self) -> bool {
        self.setup.draft.is_some()
            || self.setup.preparation.is_some()
            || self.setup_operation_in_progress()
    }
    pub(super) fn setup_operation_in_progress(&self) -> bool {
        self.setup.pending.is_some() || self.setup.known.is_some() || self.setup.uncertain.is_some()
    }
    pub(in crate::terminal_session) fn setup_recovery(&self) -> bool {
        self.setup.known.is_some() && self.setup.pending.is_none()
    }
    pub(in crate::terminal_session) fn setup_pending_or_draft(&self) -> bool {
        self.setup.pending.is_some() || self.setup.draft.is_some() || self.setup.uncertain.is_some()
    }
    pub(in crate::terminal_session) fn workspace_directory_unconfigured(&self) -> bool {
        matches!(
            self.cache.location(),
            NativeContextLocation::Workspace { .. }
        ) && self.cache.directory().is_none()
    }
    /// Read-only projection of the existing first-directory draft admission.
    pub(in crate::terminal_session) fn directory_configuration_unavailable(
        &self,
    ) -> Option<DirectoryConfigurationUnavailable> {
        if !matches!(
            self.cache.location(),
            NativeContextLocation::Workspace { .. }
        ) {
            Some(DirectoryConfigurationUnavailable::WorkspaceRequired)
        } else if self.cache.directory().is_some() {
            Some(DirectoryConfigurationUnavailable::Configured)
        } else if self.setup_blocks_structure()
            || !self.setup_authority_available()
            || !self.invariant_holds()
        {
            Some(DirectoryConfigurationUnavailable::Busy)
        } else if !matches!(self.phase, SessionPhase::Empty) {
            Some(DirectoryConfigurationUnavailable::EmptyRequired)
        } else {
            None
        }
    }
    pub(in crate::terminal_session) fn workspace_directory_path(&self) -> Option<&str> {
        self.cache
            .directory()
            .map(|directory| directory.path.as_str())
    }
    pub(in crate::terminal_session) fn plan_setup(
        &self,
        token: NativeSetupToken,
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
            NativeSetupStage::ChooseWorkspaceProject => {
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
            NativeSetupStage::WorkspaceProjectName => {
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
            NativeSetupStage::DissolveProject => {
                let organization::OrganizationTarget::Project(project) =
                    &draft.organization.as_ref()?.target
                else {
                    return None;
                };
                WorkbenchAction::ProjectDissolve {
                    project_id: project.project_id.clone(),
                }
            }
            NativeSetupStage::TabGroupName => {
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
                    _ => return None,
                }
                WorkbenchAction::TabsConfigure {
                    terminal_context_id: self.cache.terminal_context_id().clone(),
                    operation,
                }
            }
            NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory => {
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
            NativeSetupStage::DirectoryPath => {
                let NativeContextLocation::Workspace { workspace_id, .. } = self.cache.location()
                else {
                    return None;
                };
                if !matches!(self.phase, SessionPhase::Empty)
                    || !self.workspace_directory_unconfigured()
                {
                    return None;
                }
                WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                    workspace_id: workspace_id.clone(),
                    path: WorkbenchDirectoryPath::parse(draft.editor.text()).ok()?,
                }
            }
            NativeSetupStage::ProjectRename => WorkbenchAction::ProjectRename {
                project_id: draft.project.clone()?,
                label: WorkbenchLabel::parse(draft.editor.text()).ok()?,
            },
            NativeSetupStage::WorkspaceRename => {
                let NativeContextLocation::Workspace { workspace_id, .. } =
                    draft.baseline.location()
                else {
                    return None;
                };
                let label = WorkbenchLabel::parse(draft.editor.text()).ok()?;
                WorkbenchAction::WorkspaceRename {
                    workspace_id: workspace_id.clone(),
                    label,
                }
            }
            NativeSetupStage::QuickProject | NativeSetupStage::PreparingWorkspace => return None,
        };
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            site_id: self.cache.site_id().clone(),
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
    pub(in crate::terminal_session) fn begin_setup(&mut self, operation: SetupOperation) {
        #[cfg(any(windows, test))]
        {
            self.setup.project_quick_entry = operation
                .quick
                .as_ref()
                .filter(|_| {
                    matches!(
                        operation.command.action,
                        WorkbenchAction::WorkspaceLaunch { .. }
                    ) && matches!(
                        self.setup.project_quick_entry,
                        Some(quick::ProjectQuickEntry::Preparing(token))
                            if self.setup.draft.as_ref().is_some_and(|draft| draft.token == token)
                    )
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
    pub(in crate::terminal_session) fn retain_setup_worker(
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
                    NativeLocationLabelStatus::CommittedNeedRefresh,
                    NativeSetupStatus::CommittedNeedRefresh,
                ));
                self.setup.known = Some(known);
                true
            }
            SetupWorkerEvidence::Unknown(operation) => {
                if self.setup.pending.as_ref() != Some(&operation) {
                    return false;
                }
                #[cfg(any(windows, test))]
                self.cancel_project_quick_entry();
                self.setup.status = Some(operation.status(
                    NativeLocationLabelStatus::OutcomeUnknown,
                    NativeSetupStatus::OutcomeUnknown,
                ));
                self.setup.uncertain = Some(operation);
                true
            }
        }
    }
    pub(in crate::terminal_session) fn settle_setup_not_applied(
        &mut self,
        operation: &SetupOperation,
    ) -> bool {
        if self.setup.not_applied.as_ref() == Some(operation) {
            return true;
        }
        if self.setup.pending.as_ref() != Some(operation) {
            return false;
        }
        #[cfg(any(windows, test))]
        self.cancel_project_quick_entry();
        self.setup.not_applied = self.setup.pending.take();
        self.setup.deadlines = None;
        self.setup.status = Some(operation.status(
            NativeLocationLabelStatus::NotApplied,
            NativeSetupStatus::NotApplied,
        ));
        true
    }
    pub(super) fn setup_target_matches(&self, target: &NativeContextLocation) -> bool {
        self.setup
            .known
            .as_ref()
            .is_some_and(|known| &self.setup_refresh_target(known) == target)
    }

    fn setup_refresh_target(&self, known: &SetupKnownCommit) -> NativeContextLocation {
        if matches!(known.commit, WorkbenchSetupCommit::WorkspaceLaunched(_)) {
            #[cfg(any(windows, test))]
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
    ) -> Option<NativeContextTabs> {
        // Always prove the committed resources before choosing the Client's landing.
        let launched = known.validate(snapshot)?;
        if launched.location() == &self.setup_refresh_target(known) {
            return Some(launched);
        }
        validate_canonical_refresh(&self.cache, self.cache.project_snapshot(snapshot).ok()?).ok()
    }
    pub(in crate::terminal_session) fn setup_deadlines(&self) -> Option<OperationDeadlines> {
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
                let member = crate::terminal_session::NativeMemberTarget {
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
        candidate: &NativeContextTabs,
    ) -> Option<Box<PreparedSetupCompletion>> {
        let known = self.setup.known.as_ref()?;
        if &self.validated_setup_candidate(known, candidate.canonical_snapshot())? != candidate {
            return None;
        }
        let status = match &known.commit {
            WorkbenchSetupCommit::WorkspaceProjectChanged { .. } => {
                NativeSetupStatus::WorkspaceOrganized
            }
            WorkbenchSetupCommit::ProjectDissolved { .. } => NativeSetupStatus::ProjectDissolved,
            WorkbenchSetupCommit::WorkspaceLaunched(_) => NativeSetupStatus::WorkspaceCreated,
            WorkbenchSetupCommit::WorkspaceOrder { .. } => return None,
            WorkbenchSetupCommit::TabsConfigured { .. } => NativeSetupStatus::TabsConfigured,
            WorkbenchSetupCommit::NavigationConfigured { .. } => {
                NativeSetupStatus::NavigationConfigured
            }
            WorkbenchSetupCommit::ProjectRenamed { unchanged, .. }
            | WorkbenchSetupCommit::WorkspaceRenamed { unchanged, .. } => known.operation.status(
                if *unchanged {
                    NativeLocationLabelStatus::Unchanged
                } else {
                    NativeLocationLabelStatus::Renamed
                },
                NativeSetupStatus::NotApplied,
            ),
            WorkbenchSetupCommit::ProjectCreated { .. } => NativeSetupStatus::ProjectCreated,
            WorkbenchSetupCommit::WorkspaceCreated { .. } => NativeSetupStatus::WorkspaceCreated,
            WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted { .. } => {
                NativeSetupStatus::DirectoryAccepted
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
        #[cfg(any(windows, test))]
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
    pub(in crate::terminal_session) fn reset_setup_recovery_deadlines(
        &mut self,
        deadlines: OperationDeadlines,
    ) {
        self.setup.deadlines = Some(deadlines);
    }
}

#[derive(Clone, Debug)]
pub(super) struct PreparedSetupCompletion {
    known: SetupKnownCommit,
    status: NativeSetupStatus,
}
