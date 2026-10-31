//! Stable-identity drop policy; the shared canonical transition validates every candidate.
use super::{ClientContextLocation, NavigationConfiguration, SidebarReorderSource, SidebarTarget};
use crate::tui_hit_map::sidebar_reorder::Placement;
use orchbee_workbench_protocol::{WorkbenchProjectId, WorkbenchWorkspaceId, configure_navigation};
use ratatui::layout::Rect;

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

pub(super) struct Drop {
    pub(super) operation: NavigationConfiguration,
    pub(super) area: Rect,
    pub(super) placement: Placement,
    pub(super) label: String,
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
        Some(
            self.destination(
                &item,
                Some(ordered.get(neighbor)?),
                !up,
                Rect::new(0, 0, 1, 1),
            )?
            .operation,
        )
    }

    pub(super) fn destination(
        &self,
        source: &Item,
        target: Option<&Item>,
        after: bool,
        area: Rect,
    ) -> Option<Drop> {
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
                let ordered = self.navigation().ordered_projects().ok()?;
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
                    .filter(|workspace| workspace.project_id == target.project_id)
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
        // No-op and invalid drops have no insertion affordance and never submit transport work.
        configure_navigation(self.navigation(), &operation).ok()??;
        Some(Drop {
            operation,
            area,
            placement,
            label,
        })
    }
}
