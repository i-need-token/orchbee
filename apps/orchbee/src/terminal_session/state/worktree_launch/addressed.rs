//! Derived sidebar sources: organization never acquires a canonical directory.
use super::Discovery;
use crate::terminal_session::{
    primary_directory::RootDirectory,
    workbench_projection::{
        NativeContextLocation, NativeContextTabs, NativeMemberTarget, NativeMenuContextSource,
        NativeTerminalBinding, project_native_context_tabs,
    },
};
use orchbee_workbench_protocol::{
    Project, Snapshot, TerminalContext, WorkbenchProjectId, WorkbenchWorkspaceId, Workspace,
    WorktreeSource,
};

#[derive(Clone, Debug)]
pub(in crate::terminal_session) struct SidebarDiscovery {
    pub addressed: AddressedWorktreeSource,
    pub sample: RootDirectory,
    pub source: WorktreeSource,
    pub discovery: Discovery,
}

impl SidebarDiscovery {
    pub fn valid(&self, snapshot: &Snapshot) -> bool {
        self.addressed.unchanged(snapshot)
            && self.addressed.resolve_sample(self.sample.clone()).as_ref() == Some(&self.source)
            && self
                .discovery
                .matches_addressed(&self.addressed, &self.source)
    }
}

#[cfg(test)]
pub(in crate::terminal_session) mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SidebarWorktreeTarget {
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
pub(in crate::terminal_session) struct AddressedWorktreeSource {
    pub target: SidebarWorktreeTarget,
    pub cache: NativeContextTabs,
    pub member: Option<NativeMemberTarget>,
    pub primary: Option<NativeTerminalBinding>,
    owner: SourceOwner,
    source: NativeMenuContextSource,
    context: TerminalContext,
}

impl AddressedWorktreeSource {
    pub fn capture(snapshot: &Snapshot, target: &SidebarWorktreeTarget) -> Option<Self> {
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
                    .map(|(_, workspace)| workspace.clone())
                    .collect();
                let first = members.first()?.workspace_id.clone();
                (
                    SourceOwner::Project { project, members },
                    snapshot.navigation.workspace(&first)?,
                )
            }
        };
        let location = NativeContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        };
        let cache = project_native_context_tabs(snapshot, &location).ok()?;
        let source = cache.menu_context_source(&location)?;
        let first = cache
            .tabs()
            .first()
            .and_then(|tab| tab.panel(0).map(|panel| (tab, panel)));
        let member = first.map(|(tab, panel)| NativeMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: panel.primary().presentation_id().clone(),
        });
        let primary = first
            .and_then(|(_, panel)| panel.primary().binding())
            .cloned();
        let context = cache.canonical_context().clone();
        Some(Self {
            target: target.clone(),
            cache,
            member,
            primary,
            owner,
            source,
            context,
        })
    }

    pub fn unchanged(&self, snapshot: &Snapshot) -> bool {
        Self::capture(snapshot, &self.target).is_some_and(|current| {
            self.cache.site_id() == current.cache.site_id()
                && self.owner == current.owner
                && self.source == current.source
                && self.context == current.context
                && self.member == current.member
                && self.primary == current.primary
        })
    }

    /// Only an admitted absent cwd (or no first terminal at all)
    /// permits the same Workspace's canonical directory. An unavailable sample
    /// can never select another Project member, Agent cwd or Client cwd.
    pub fn resolve_sample(&self, sample: RootDirectory) -> Option<WorktreeSource> {
        match sample {
            RootDirectory::Available(Some(path)) if self.primary.is_some() => {
                Some(WorktreeSource::Directory { path })
            }
            RootDirectory::Available(None)
                if self.cache.directory().is_some()
                    && (self.primary.is_some() || self.member.is_none()) =>
            {
                let NativeContextLocation::Workspace { workspace_id, .. } = self.cache.location()
                else {
                    return None;
                };
                Some(WorktreeSource::Workspace {
                    workspace_id: workspace_id.clone(),
                })
            }
            RootDirectory::Available(_) | RootDirectory::Unavailable => None,
        }
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
            WorktreeSource::Workspace { workspace_id } => {
                matches!(addressed.cache.location(), NativeContextLocation::Workspace { workspace_id: expected, .. } if expected == workspace_id)
                    && self.source.workspace_id.as_ref() == Some(workspace_id)
                    && addressed.cache.directory().is_some_and(|directory| {
                        directory.path == self.source.requested_path
                            && directory.worker_id == self.worker
                    })
            }
        }
    }
}
