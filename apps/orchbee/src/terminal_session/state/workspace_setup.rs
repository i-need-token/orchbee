mod draft;
mod labels;
mod projection;
use super::line_editor::{LengthLimit, LineEditor};
use super::{
    AttachmentKey, NativeContextTabs, NativeSessionState, NavigatorRefreshStatus,
    OperationDeadlines, PreparedRefresh, SelectedTab, SessionNotice, SessionPhase,
    validate_canonical_refresh,
};
use crate::local_client::workbench::workspace_setup::{
    WorkbenchSetupCommit, WorkbenchSetupRequest,
};
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, project_native_context_tabs,
};
pub(in crate::terminal_session) use draft::NativeSetupIntent;
use draft::SetupDraft;
pub(crate) use draft::{
    NativeLocationLabelStatus, NativeLocationLabelTarget, NativeSetupDraftView, NativeSetupEdit,
    NativeSetupStage, NativeSetupStatus, NativeSetupToken,
};
use orchbee_workbench_protocol::{
    MAXIMUM_LABEL_LENGTH, ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand,
    WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchProjectId, WorkbenchRequestId,
    WorkbenchSnapshotOutcome,
};

#[derive(Default)]
pub(super) struct SetupState {
    last_token: u64,
    draft: Option<SetupDraft>,
    pending: Option<SetupOperation>,
    known: Option<SetupKnownCommit>,
    settled: Option<SetupKnownCommit>,
    not_applied: Option<SetupOperation>,
    uncertain: Option<SetupOperation>,
    status: Option<NativeSetupStatus>,
    deadlines: Option<OperationDeadlines>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct SetupOperation {
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
        let unchanged = if self.location_label_target().is_some() {
            self.validate_label_receipt(&commit)?
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
    fn setup_authority_available(&self) -> bool {
        self.pending_refresh_generation.is_none()
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
            || self.setup.pending.is_some()
            || self.setup.known.is_some()
            || self.setup.uncertain.is_some()
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
        if draft.token != token || !draft.shown || !draft.complete || draft.faulted {
            return None;
        }
        let action = match draft.stage {
            NativeSetupStage::ProjectLabel => WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse(draft.editor.text()).ok()?,
            },
            NativeSetupStage::WorkspaceLabel => WorkbenchAction::WorkspaceCreate {
                project_id: draft.project.clone()?,
                label: WorkbenchLabel::parse(draft.editor.text()).ok()?,
            },
            NativeSetupStage::DirectoryPath => {
                let NativeContextLocation::Workspace {
                    project_id,
                    workspace_id,
                    ..
                } = self.cache.location()
                else {
                    return None;
                };
                if !matches!(self.phase, SessionPhase::Empty)
                    || !self.workspace_directory_unconfigured()
                {
                    return None;
                }
                WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                    project_id: project_id.clone(),
                    workspace_id: workspace_id.clone(),
                    path: WorkbenchDirectoryPath::parse(draft.editor.text()).ok()?,
                }
            }
            NativeSetupStage::ProjectRename | NativeSetupStage::WorkspaceRename => {
                let NativeContextLocation::Workspace {
                    project_id,
                    workspace_id,
                    ..
                } = draft.baseline.location()
                else {
                    return None;
                };
                let label = WorkbenchLabel::parse(draft.editor.text()).ok()?;
                if draft.stage == NativeSetupStage::ProjectRename {
                    WorkbenchAction::ProjectRename {
                        project_id: project_id.clone(),
                        label,
                    }
                } else {
                    WorkbenchAction::WorkspaceRename {
                        project_id: project_id.clone(),
                        workspace_id: workspace_id.clone(),
                        label,
                    }
                }
            }
            NativeSetupStage::ChooseProject => return None,
        };
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            site_id: self.cache.site_id().clone(),
            expected_revision: self.cache.revision(),
            action,
        };
        Some(SetupOperation {
            generation,
            request: WorkbenchSetupRequest::try_from(command.clone()).ok()?,
            command,
            baseline: self.cache.clone(),
            deadlines,
        })
    }
    pub(in crate::terminal_session) fn begin_setup(&mut self, operation: SetupOperation) {
        self.setup.draft = None;
        self.setup.deadlines = Some(operation.deadlines);
        self.pending_refresh_generation = Some(operation.generation);
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
            .is_some_and(|known| &known.target() == target)
    }
    pub(in crate::terminal_session) fn setup_deadlines(&self) -> Option<OperationDeadlines> {
        self.setup.deadlines
    }
    pub(super) fn prepare_setup_refresh(
        &self,
        generation: u64,
        snapshot: &Snapshot,
    ) -> PreparedRefresh {
        let prepared = self
            .setup
            .known
            .as_ref()
            .and_then(|known| known.validate(snapshot))
            .and_then(|candidate| {
                let same = candidate.location() == self.cache.location();
                self.prepare_membership_candidate(
                    candidate,
                    same.then(|| self.desired.as_ref().map(AttachmentKey::tab_id))
                        .flatten(),
                    self.active().filter(|_| same).map(SelectedTab::index),
                    None,
                )
                .ok()
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
        let status = match &known.commit {
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
        let draft = match &known.commit {
            WorkbenchSetupCommit::ProjectCreated { project_id, .. } => {
                self.setup.last_token.checked_add(1).map(|next| SetupDraft {
                    token: NativeSetupToken(next),
                    baseline: candidate.clone(),
                    stage: NativeSetupStage::WorkspaceLabel,
                    selected: candidate
                        .projects()
                        .iter()
                        .position(|p| &p.project_id == project_id)
                        .unwrap_or(0),
                    project: Some(project_id.clone()),
                    known_project_created: true,
                    editor: LineEditor::new(
                        String::new(),
                        LengthLimit::Scalars(MAXIMUM_LABEL_LENGTH),
                    ),
                    shown: false,
                    complete: true,
                    faulted: false,
                })
            }
            _ => None,
        };
        Some(Box::new(PreparedSetupCompletion {
            known: known.clone(),
            status,
            draft,
        }))
    }
    pub(super) fn complete_setup_refresh(&mut self, prepared: PreparedSetupCompletion) {
        let PreparedSetupCompletion {
            known,
            status,
            draft,
        } = prepared;
        debug_assert_eq!(self.setup.known.as_ref(), Some(&known));
        self.setup.known = None;
        self.setup.deadlines = None;
        self.setup.status = Some(status);
        if let Some(draft) = &draft {
            self.setup.last_token = draft.token.0;
        }
        self.setup.draft = draft;
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
    draft: Option<SetupDraft>,
}
