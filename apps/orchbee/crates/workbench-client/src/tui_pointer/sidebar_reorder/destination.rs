//! Stable-identity drop policy; the shared canonical transition validates every candidate.
use super::{ClientContextLocation, NavigationConfiguration, SidebarReorderSource, SidebarTarget};
use crate::tui_hit_map::sidebar_reorder::Placement;
use orchbee_workbench_protocol::{WorkbenchProjectId, WorkbenchWorkspaceId, configure_navigation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Item {
    Project(WorkbenchProjectId),
    Workspace(WorkbenchWorkspaceId),
}
impl Item {
    pub(super) fn from_target(target: &SidebarTarget) -> Option<Self> {
        Some(match target {
            SidebarTarget::Project { project_id, .. } => Self::Project(project_id.clone()),
            SidebarTarget::Context { target, .. } => match &target.location {
                ClientContextLocation::Workspace { workspace_id, .. } => {
                    Self::Workspace(workspace_id.clone())
                }
                ClientContextLocation::NoWorkspace => return None,
            },
            SidebarTarget::Recents { .. } | SidebarTarget::Agent { .. } => return None,
        })
    }
}

/// One validated move off the canonical baseline, with the words the status line names it by.
#[derive(Clone, Debug)]
pub(super) struct Drop {
    pub(super) operation: NavigationConfiguration,
    pub(super) placement: Placement,
    pub(super) label: String,
}

/// Where a candidate destination leaves the dragged entry.
pub(super) enum Landing {
    /// The baseline refuses this destination.
    Unavailable,
    /// The destination is the slot the entry already holds in the baseline.
    Origin,
    Moved(Drop),
}

impl SidebarReorderSource {
    /// Keyboard sorting has the same destination semantics as a drag release.
    pub(crate) fn step(&self, target: &SidebarTarget, up: bool) -> Option<NavigationConfiguration> {
        if target.session_id() != self.session_id() {
            return None;
        }
        let item = Item::from_target(target)?;
        let (ordered, index) = match &item {
            Item::Project(id) => {
                let ordered = self
                    .navigation()
                    .ordered_projects()
                    .ok()?
                    .into_iter()
                    .map(|project| Item::Project(project.project_id.clone()))
                    .collect::<Vec<_>>();
                let index = ordered
                    .iter()
                    .position(|item| item == &Item::Project(id.clone()))?;
                (ordered, index)
            }
            Item::Workspace(id) => {
                let source = self.navigation().workspace(id)?;
                let ordered = self
                    .navigation()
                    .ordered_workspaces()
                    .ok()?
                    .into_iter()
                    .map(|(_, workspace)| workspace)
                    .filter(|workspace| workspace.project_id == source.project_id)
                    .map(|workspace| Item::Workspace(workspace.workspace_id.clone()))
                    .collect::<Vec<_>>();
                let index = ordered
                    .iter()
                    .position(|item| item == &Item::Workspace(id.clone()))?;
                (ordered, index)
            }
        };
        let neighbor = if up {
            index.checked_sub(1)?
        } else {
            index.checked_add(1)?
        };
        match self.destination(&item, Some(ordered.get(neighbor)?), !up) {
            Landing::Moved(drop) => Some(drop.operation),
            Landing::Unavailable | Landing::Origin => None,
        }
    }

    /// The move that places `source` before or after `target`, into a Project `target`, or, with
    /// no target, at the end: outside any Project for a Workspace, last for a Project.
    pub(super) fn destination(&self, source: &Item, target: Option<&Item>, after: bool) -> Landing {
        self.candidate(source, target, after)
            .map_or(Landing::Unavailable, |drop| self.validate(drop))
    }

    /// A Workspace dragged onto a Project heading enters it as its first Workspace.
    pub(super) fn first_in(
        &self,
        workspace_id: &WorkbenchWorkspaceId,
        project_id: &WorkbenchProjectId,
    ) -> Landing {
        let Ok(ordered) = self.navigation().ordered_workspaces() else {
            return Landing::Unavailable;
        };
        let first = ordered
            .into_iter()
            .map(|(_, workspace)| workspace)
            .find(|workspace| {
                workspace.project_id.as_ref() == Some(project_id)
                    && &workspace.workspace_id != workspace_id
            });
        let Some(first) = first else {
            return self.destination(
                &Item::Workspace(workspace_id.clone()),
                Some(&Item::Project(project_id.clone())),
                false,
            );
        };
        self.validate(Drop {
            operation: NavigationConfiguration::MoveWorkspace {
                workspace_id: workspace_id.clone(),
                project_id: Some(project_id.clone()),
                before_workspace_id: Some(first.workspace_id.clone()),
            },
            placement: Placement::Before,
            label: first.label.clone(),
        })
    }

    fn candidate(&self, source: &Item, target: Option<&Item>, after: bool) -> Option<Drop> {
        use NavigationConfiguration::{MoveProject, MoveWorkspace};
        let (operation, placement, label) = match (source, target) {
            (Item::Project(project_id), None) => (
                MoveProject {
                    project_id: project_id.clone(),
                    before_project_id: None,
                },
                Placement::End,
                String::new(),
            ),
            (Item::Project(project_id), Some(Item::Project(anchor))) => {
                // The dragged entry leaves its slot, so the slot after an anchor skips it.
                let ordered = self
                    .navigation()
                    .ordered_projects()
                    .ok()?
                    .into_iter()
                    .filter(|project| &project.project_id != project_id)
                    .collect::<Vec<_>>();
                let index = ordered
                    .iter()
                    .position(|project| &project.project_id == anchor)?;
                let before_project_id = ordered
                    .get(index + usize::from(after))
                    .map(|project| project.project_id.clone());
                (
                    MoveProject {
                        project_id: project_id.clone(),
                        before_project_id,
                    },
                    if after {
                        Placement::After
                    } else {
                        Placement::Before
                    },
                    ordered[index].label.clone(),
                )
            }
            (Item::Project(_), Some(Item::Workspace(_))) => return None,
            (Item::Workspace(workspace_id), None) => (
                MoveWorkspace {
                    workspace_id: workspace_id.clone(),
                    project_id: None,
                    before_workspace_id: None,
                },
                Placement::Ungrouped,
                String::new(),
            ),
            (Item::Workspace(workspace_id), Some(Item::Project(project_id))) => (
                MoveWorkspace {
                    workspace_id: workspace_id.clone(),
                    project_id: Some(project_id.clone()),
                    before_workspace_id: None,
                },
                Placement::Into,
                self.navigation().project(project_id)?.label.clone(),
            ),
            (Item::Workspace(workspace_id), Some(Item::Workspace(anchor))) => {
                let target = self.navigation().workspace(anchor)?;
                let ordered = self
                    .navigation()
                    .ordered_workspaces()
                    .ok()?
                    .into_iter()
                    .map(|(_, workspace)| workspace)
                    .filter(|workspace| {
                        workspace.project_id == target.project_id
                            && &workspace.workspace_id != workspace_id
                    })
                    .collect::<Vec<_>>();
                let index = ordered
                    .iter()
                    .position(|workspace| &workspace.workspace_id == anchor)?;
                let before_workspace_id = ordered
                    .get(index + usize::from(after))
                    .map(|workspace| workspace.workspace_id.clone());
                (
                    MoveWorkspace {
                        workspace_id: workspace_id.clone(),
                        project_id: target.project_id.clone(),
                        before_workspace_id,
                    },
                    if after {
                        Placement::After
                    } else {
                        Placement::Before
                    },
                    target.label.clone(),
                )
            }
        };
        Some(Drop {
            operation,
            placement,
            label,
        })
    }

    /// Invalid moves land nowhere; a move that changes nothing returns the entry to its origin.
    /// Neither ever submits transport work.
    fn validate(&self, drop: Drop) -> Landing {
        match configure_navigation(self.navigation(), &drop.operation) {
            Ok(Some(_)) => Landing::Moved(drop),
            Ok(None) => Landing::Origin,
            Err(_) => Landing::Unavailable,
        }
    }
}
