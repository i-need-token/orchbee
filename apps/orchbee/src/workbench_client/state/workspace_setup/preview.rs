//! Immediate Client-local creation feedback; only retained receipts supply canonical identities.

use super::{WorkbenchClientState, WorkbenchSetupCommit, quick::ProjectQuickEntry};
use orchbee_workbench_protocol::{
    SessionId, WorkbenchProjectId, WorkbenchRevision, WorkbenchWorkspaceId,
};

/// The creation phase projected without waiting for preparation or terminal startup I/O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCreationPhase {
    /// The source directory and launch command are being prepared.
    Preparing,
    /// The accepted launch or its canonical snapshot is still being reconciled.
    Starting,
    /// Canonical creation has settled; the sidebar observer may still be catching up.
    Committed,
}

/// One admitted launch's display, separate from canonical navigation and terminal selection.
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCreationPreview<'a> {
    pub(crate) session: &'a SessionId,
    pub(crate) label: Option<&'a str>,
    pub(crate) project_id: Option<&'a WorkbenchProjectId>,
    pub(crate) committed: Option<(&'a WorkbenchWorkspaceId, WorkbenchRevision)>,
    pub(crate) phase: WorkspaceCreationPhase,
    pub(crate) current: bool,
}

impl WorkspaceCreationPreview<'_> {
    /// Only an unfinished foreground creation replaces the retained terminal's content.
    pub(crate) fn replaces_content(self) -> bool {
        self.current && self.phase != WorkspaceCreationPhase::Committed
    }
}

impl WorkbenchClientState {
    pub(in crate::workbench_client) fn workspace_creation_preview(
        &self,
    ) -> Option<WorkspaceCreationPreview<'_>> {
        if self.setup.uncertain.is_some()
            || self
                .setup
                .preparation
                .as_ref()
                .is_some_and(|preparation| preparation.review_directory)
        {
            return None;
        }
        if self.setup.preparation.is_some() || self.setup.draft.is_some() {
            let draft = self.setup.draft.as_ref()?;
            let label = if let Some(preparation) = &self.setup.preparation {
                if draft.token != preparation.token {
                    return None;
                }
                None
            } else {
                draft.continuation?;
                Some(draft.quick.as_ref()?.name())
            };
            let current = matches!(self.setup.project_quick_entry,
                Some(ProjectQuickEntry::Preparing(token)) if token == draft.token);
            return Some(WorkspaceCreationPreview {
                session: self.cache.session_id(),
                label,
                project_id: draft.project.as_ref(),
                committed: None,
                phase: WorkspaceCreationPhase::Preparing,
                current,
            });
        }
        // A prior receipt cannot supply identities for a newer in-flight launch.
        let known = self.setup.known.as_ref().or_else(|| {
            self.setup
                .pending
                .is_none()
                .then_some(self.setup.settled.as_ref())
                .flatten()
        });
        let operation = self
            .setup
            .pending
            .as_ref()
            .or_else(|| known.map(|known| &known.operation))?;
        let plan = operation.quick.as_ref()?;
        let committed = known.and_then(|known| match &known.commit {
            WorkbenchSetupCommit::WorkspaceLaunched(commit) => {
                Some((&commit.workspace_id, commit.revision))
            }
            _ => None,
        });
        let settled = self.setup.pending.is_none() && self.setup.known.is_none();
        let workspace =
            committed.and_then(|(id, _)| self.cache.canonical_snapshot().navigation.workspace(id));
        if settled && workspace.is_none() {
            return None;
        }
        let current = if settled {
            known.is_some_and(|known| self.cache.location() == &known.target())
        } else {
            matches!(self.setup.project_quick_entry,
                Some(ProjectQuickEntry::Submitted(generation)) if generation == operation.generation)
        };
        Some(WorkspaceCreationPreview {
            session: self.cache.session_id(),
            label: Some(
                workspace.map_or(plan.label.as_str(), |workspace| workspace.label.as_str()),
            ),
            project_id: plan.project.existing_id(),
            committed,
            phase: if settled {
                WorkspaceCreationPhase::Committed
            } else {
                WorkspaceCreationPhase::Starting
            },
            current,
        })
    }
}

impl<'a> crate::workbench_client::state::WorkbenchClientView<'a> {
    pub(in crate::workbench_client) fn workspace_creation_preview(
        &self,
    ) -> Option<WorkspaceCreationPreview<'a>> {
        self.state.workspace_creation_preview()
    }
}
