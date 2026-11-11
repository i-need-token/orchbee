//! Exact native origin and the source the Server's current-directory answer names, independent
//! of Git execution.
use super::{WorktreeMode, WorktreeToken};
use crate::workbench_projection::{
    ClientContextLocation, ClientContextTabs, ClientMemberTarget, ClientTerminalBinding,
};
use crate::workbench_state::{
    ClientNotice, ClientPhaseKind, OperationDeadlines, PaneRef, WorkbenchClientState,
};
use orchbee_client_server_protocol::Geometry;
use orchbee_workbench_protocol::{WorkbenchDirectoryPath, WorktreeSource, same_directory_spelling};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Origin {
    pub(crate) cache: ClientContextTabs,
    pub(crate) member: Option<ClientMemberTarget>,
    pub(crate) zoom: bool,
    pub(crate) geometry: Geometry,
    pub(crate) primary_pane: Option<PaneRef>,
}
impl Origin {
    pub(crate) fn unchanged(&self, current: &Self) -> bool {
        self.cache.session_id() == current.cache.session_id()
            && self.cache.location() == current.cache.location()
            && self.cache.directory() == current.cache.directory()
            && self.member == current.member
            && self.zoom == current.zoom
            && self.geometry == current.geometry
            && self.primary_pane == current.primary_pane
            && self.primary_binding() == current.primary_binding()
    }
    /// Whether `projected`, this Context read again later, still names the same source. A commit
    /// elsewhere in the Workbench advances only the revision and leaves the source current.
    pub(crate) fn source_current_in(&self, projected: &ClientContextTabs) -> bool {
        self.unchanged(&Self {
            cache: projected.clone(),
            ..self.clone()
        })
    }
    /// The source the Server's `workbench.current-directory` answer for the focused Panel names, as
    /// the Web and Desktop Clients resolve it: Git runs where that terminal stands when that differs
    /// from the Workspace's own directory, else in the Workspace itself. Without an answer only a
    /// Workspace with a directory has a source.
    pub(crate) fn source(
        &self,
        observed: Option<WorkbenchDirectoryPath>,
    ) -> Option<WorktreeSource> {
        let ClientContextLocation::Workspace { workspace_id, .. } = self.cache.location() else {
            return None;
        };
        let workspace_id = workspace_id.clone();
        let directory = self.cache.directory();
        match observed {
            Some(path)
                if directory.is_none_or(|directory| {
                    !same_directory_spelling(directory.path.as_str(), path.as_str())
                }) =>
            {
                Some(WorktreeSource::WorkspaceDirectory { workspace_id, path })
            }
            Some(_) => Some(WorktreeSource::Workspace { workspace_id }),
            None => directory.map(|_| WorktreeSource::Workspace { workspace_id }),
        }
    }
    pub(crate) fn primary_binding(&self) -> Option<&ClientTerminalBinding> {
        let member = self.member.as_ref()?;
        let tab = self.cache.tabs().get(self.cache.tab_index(&member.tab)?)?;
        tab.panel(tab.panel_index(&member.panel)?)?
            .primary()
            .binding()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Preparation {
    pub(crate) generation: u64,
    pub(crate) token: WorktreeToken,
    pub(crate) mode: WorktreeMode,
    pub(crate) origin: Origin,
    pub(crate) deadlines: OperationDeadlines,
    /// The Agent a create from the Start Agent picker starts in its new Workspace's first shell.
    pub(crate) agent: Option<&'static str>,
}
impl Preparation {
    #[must_use]
    pub(crate) const fn starting(mut self, agent: Option<&'static str>) -> Self {
        self.agent = agent;
        self
    }
}
impl WorkbenchClientState {
    /// Why W and O cannot start from this Client state, such as a Workspace whose
    /// terminal is unavailable while its close waits on a failed removal.
    pub(crate) fn worktree_refusal(&self) -> Option<ClientNotice> {
        if !matches!(
            self.phase_kind(),
            ClientPhaseKind::Ready | ClientPhaseKind::Empty
        ) {
            return Some(ClientNotice::WorkspaceNotReady);
        }
        // A background publication read is not a refusal; the request waits for it.
        (self.mutation_blocks_structure() || self.own_refresh_pending())
            .then_some(ClientNotice::LocalBusy)
    }
    pub(crate) fn worktree_origin(&self) -> Option<Origin> {
        if self.worktree_refusal().is_some() {
            return None;
        }
        let member = self.focused_member();
        let primary = self.active().and_then(|active| {
            self.cache()
                .tabs()
                .get(active.index())?
                .panel(active.panel_index())?
                .primary()
                .binding()
        });
        let primary_pane = self
            .focus_record()
            .filter(|record| Some(record.binding()) == primary)
            .map(super::super::PaneRecord::pane_ref);
        Some(Origin {
            cache: self.cache().clone(),
            member,
            zoom: self.zoom_active(),
            geometry: self.latest_geometry(),
            primary_pane,
        })
    }
    pub(crate) fn worktree_preparation(
        &self,
        mode: WorktreeMode,
        generation: u64,
        token: WorktreeToken,
        deadlines: OperationDeadlines,
    ) -> Option<Preparation> {
        Some(Preparation {
            generation,
            token,
            mode,
            origin: self.worktree_origin()?,
            deadlines,
            agent: None,
        })
    }
}
