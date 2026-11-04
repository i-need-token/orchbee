//! Derived sidebar sources: organization never acquires a canonical directory.
use super::Discovery;
use crate::workbench_projection::{
    ClientContextLocation, ClientContextTabs, ClientMemberTarget, ClientTerminalBinding,
    MenuContextSource, project_native_context_tabs, settled_workspace,
};
use orchbee_workbench_protocol::{
    Project, Snapshot, TerminalContext, WorkbenchDirectoryPath, WorkbenchProjectId,
    WorkbenchWorkspaceId, Workspace, WorktreeEntryText, WorktreeQueryEntry, WorktreeSource,
};

#[derive(Clone, Debug)]
pub(crate) struct SidebarDiscovery {
    pub(crate) addressed: AddressedWorktreeSource,
    /// The Server's current-directory answer the source was resolved from; `None` when it gave none.
    pub(crate) observed: Option<WorkbenchDirectoryPath>,
    pub(crate) source: WorktreeSource,
    pub(crate) discovery: Discovery,
}

impl SidebarDiscovery {
    /// Whether `entry` is the addressed Workspace's own checkout, which opening would only reuse.
    pub(crate) fn own_checkout(&self, entry: &WorktreeQueryEntry) -> bool {
        let SidebarWorktreeTarget::Workspace(workspace_id) = &self.addressed.target else {
            return false;
        };
        let WorktreeEntryText::Utf8(path) = &entry.checkout.path else {
            return false;
        };
        entry
            .workspaces
            .iter()
            .any(|workspace| &workspace.workspace_id == workspace_id)
            || orchbee_workbench_protocol::same_directory_spelling(
                self.discovery.source.requested_path.as_str(),
                path,
            )
    }

    pub(crate) fn valid(&self, snapshot: &Snapshot) -> bool {
        self.addressed.unchanged(snapshot)
            && self.addressed.resolve(self.observed.as_ref()).as_ref() == Some(&self.source)
            && self
                .discovery
                .matches_addressed(&self.addressed, &self.source)
    }
}

#[cfg(test)]
pub(crate) mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SidebarWorktreeTarget {
    Workspace(WorkbenchWorkspaceId),
    Project(WorkbenchProjectId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceOwner {
    Workspace,
    Project {
        project: Project,
        members: Vec<Workspace>,
    },
}

/// The immutable addressed source is independent of the Client's selected
/// Workspace, Tab, leaf or child Agent. Revision records discovery provenance;
/// later checks compare the complete source scope rather than unrelated writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AddressedWorktreeSource {
    pub(crate) target: SidebarWorktreeTarget,
    pub(crate) cache: ClientContextTabs,
    pub(crate) member: Option<ClientMemberTarget>,
    pub(crate) primary: Option<ClientTerminalBinding>,
    owner: SourceOwner,
    source: MenuContextSource,
    context: TerminalContext,
    /// The first terminal's directory the Server follows, which the sidebar's location line shows.
    /// A change to it restarts discovery; the directory Git reads is the Server's fresh answer.
    followed: Option<WorkbenchDirectoryPath>,
}

impl AddressedWorktreeSource {
    pub(crate) fn capture(snapshot: &Snapshot, target: &SidebarWorktreeTarget) -> Option<Self> {
        let (owner, workspace) = match target {
            SidebarWorktreeTarget::Workspace(id) => {
                (SourceOwner::Workspace, snapshot.navigation.workspace(id)?)
            }
            SidebarWorktreeTarget::Project(id) => {
                let project = snapshot.navigation.project(id)?.clone();
                // workspaces_in_project iterates the identity registry. Source
                // policy follows actual canonical display order instead.
                let members: Vec<_> = snapshot
                    .navigation
                    .ordered_workspaces()
                    .ok()?
                    .into_iter()
                    .filter(|(_, workspace)| workspace.project_id.as_ref() == Some(id))
                    .map(|(_, workspace)| settled_workspace(workspace))
                    .collect();
                let first = members.first()?.workspace_id.clone();
                (
                    SourceOwner::Project { project, members },
                    snapshot.navigation.workspace(&first)?,
                )
            }
        };
        let location = ClientContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        };
        let cache = project_native_context_tabs(snapshot, &location).ok()?;
        let source = cache.menu_context_source(&location)?;
        let first = cache
            .tabs()
            .first()
            .and_then(|tab| tab.panel(0).map(|panel| (tab, panel)));
        let member = first.map(|(tab, panel)| ClientMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: panel.primary().presentation_id().clone(),
        });
        let primary = first
            .and_then(|(_, panel)| panel.primary().binding())
            .cloned();
        let context = cache.canonical_context()?.clone();
        let followed = workspace.terminal_directory.clone();
        Some(Self {
            target: target.clone(),
            cache,
            member,
            primary,
            owner,
            source,
            context,
            followed,
        })
    }

    /// The directory the sidebar's row describes: the first terminal's, otherwise the Workspace's.
    pub(crate) fn stands_in(&self) -> Option<&WorkbenchDirectoryPath> {
        let MenuContextSource::Workspace { directory, .. } = &self.source;
        self.followed
            .as_ref()
            .or(directory.as_ref().map(|directory| &directory.path))
    }

    /// Whether this source stands in `directory` or below it.
    pub(crate) fn stands_under(&self, directory: &WorkbenchDirectoryPath) -> bool {
        self.stands_in()
            .is_some_and(|here| super::removal::inside_checkout(here.as_str(), directory.as_str()))
    }

    /// The same source, standing in the same directory.
    pub(crate) fn unchanged(&self, snapshot: &Snapshot) -> bool {
        Self::capture(snapshot, &self.target)
            .is_some_and(|current| self.same_owner(&current) && self.followed == current.followed)
    }

    /// Whether `snapshot` still holds this source, wherever its first terminal stands now. A menu
    /// outlives a `cd`; only its discovery starts over.
    pub(crate) fn current(&self, snapshot: &Snapshot) -> bool {
        Self::capture(snapshot, &self.target).is_some_and(|current| self.same_owner(&current))
    }

    /// Whether `next` is this source, wherever its first terminal stands now.
    pub(crate) fn moved_to(&self, next: &Self) -> bool {
        self.target == next.target && self.same_owner(next)
    }

    fn same_owner(&self, current: &Self) -> bool {
        self.cache.session_id() == current.cache.session_id()
            && self.owner == current.owner
            && self.source == current.source
            && self.context == current.context
            && self.member == current.member
            && self.primary == current.primary
    }

    /// The Workspace whose current directory the Server answers: the addressed Workspace, or a
    /// Project's first member.
    pub(crate) fn first_workspace(&self) -> Option<&WorkbenchWorkspaceId> {
        match self.cache.location() {
            ClientContextLocation::Workspace { workspace_id, .. } => Some(workspace_id),
            _ => None,
        }
    }

    /// One source from the Server's `workbench.current-directory` answer for the first Workspace,
    /// `None` when it gave none. The Server answers where the first terminal stands now, else the
    /// Workspace's own directory, and the Web and Desktop Clients read the same answer. An addressed
    /// Workspace stays the owner the Server groups alongside. A Project source carries its Project
    /// in the Directory form and needs a bound first terminal, unless its first member has no
    /// terminal at all; it never selects another member or the Client's own context.
    pub(crate) fn resolve(
        &self,
        observed: Option<&WorkbenchDirectoryPath>,
    ) -> Option<WorktreeSource> {
        match &self.target {
            SidebarWorktreeTarget::Workspace(workspace_id) => {
                let workspace_id = workspace_id.clone();
                match observed {
                    Some(path) if !self.own(path) => Some(WorktreeSource::WorkspaceDirectory {
                        workspace_id,
                        path: path.clone(),
                    }),
                    Some(_) => Some(WorktreeSource::Workspace { workspace_id }),
                    None => self
                        .cache
                        .directory()
                        .map(|_| WorktreeSource::Workspace { workspace_id }),
                }
            }
            SidebarWorktreeTarget::Project(_) => {
                let path = observed?;
                if self.primary.is_some() {
                    return Some(WorktreeSource::Directory { path: path.clone() });
                }
                // A first member without any terminal reads its own directory.
                let workspace_id = self.first_workspace()?.clone();
                (self.member.is_none() && self.own(path))
                    .then_some(WorktreeSource::Workspace { workspace_id })
            }
        }
    }

    /// Whether `path` names the first Workspace's own directory on its first terminal's Worker.
    fn own(&self, path: &WorkbenchDirectoryPath) -> bool {
        self.cache.directory().is_some_and(|directory| {
            self.primary
                .as_ref()
                .is_none_or(|primary| directory.worker_id == *primary.worker_id())
                && orchbee_workbench_protocol::same_directory_spelling(
                    directory.path.as_str(),
                    path.as_str(),
                )
        })
    }
}

impl Discovery {
    pub(super) fn matches_addressed(
        &self,
        addressed: &AddressedWorktreeSource,
        source: &WorktreeSource,
    ) -> bool {
        match source {
            WorktreeSource::Directory { path } => {
                self.source.requested_path == *path
                    && self.source.workspace_id.is_none()
                    && addressed
                        .primary
                        .as_ref()
                        .is_some_and(|binding| binding.worker_id() == &self.worker)
            }
            WorktreeSource::WorkspaceDirectory { workspace_id, path } => {
                addressed.target == SidebarWorktreeTarget::Workspace(workspace_id.clone())
                    && self.source.workspace_id.as_ref() == Some(workspace_id)
                    && self.source.requested_path == *path
                    && addressed
                        .primary
                        .as_ref()
                        .is_none_or(|binding| binding.worker_id() == &self.worker)
            }
            WorktreeSource::Workspace { workspace_id } => {
                matches!(addressed.cache.location(), ClientContextLocation::Workspace { workspace_id: expected, .. } if expected == workspace_id)
                    && self.source.workspace_id.as_ref() == Some(workspace_id)
                    && addressed.cache.directory().is_some_and(|directory| {
                        directory.path == self.source.requested_path
                            && directory.worker_id == self.worker
                    })
            }
        }
    }
}
