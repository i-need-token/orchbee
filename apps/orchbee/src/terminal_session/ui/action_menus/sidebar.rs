//! Sidebar composition projects exact typed organization choices, never labels as identity.
use super::{
    Duration, Instant, MenuAction, MenuTarget, NativeContextLocation, NativeMenuContextSource,
    NativeSessionState, NativeUi, Stage, Target, action_availability, visible_unavailable,
};
use crate::terminal_session::{
    SidebarWorktreeTarget,
    state::worktree_launch::{
        WorktreeNotice,
        addressed::{AddressedWorktreeSource, SidebarDiscovery},
    },
    worktree_lane::Action,
};

#[cfg(test)]
mod tests;

pub(super) struct Episode {
    id: u64,
    source: AddressedWorktreeSource,
    deadline: Instant,
    state: DiscoveryState,
}
enum DiscoveryState {
    Waiting,
    Querying,
    Ready(Box<SidebarDiscovery>),
    Absent,
}
impl Episode {
    pub(super) fn ready(&self) -> Option<&SidebarDiscovery> {
        match &self.state {
            DiscoveryState::Ready(discovery) => Some(discovery),
            _ => None,
        }
    }
}

pub(super) fn target(
    entry: &MenuTarget,
    target: &Target,
    state: &NativeSessionState,
) -> Option<SidebarWorktreeTarget> {
    match (entry, target) {
        (MenuTarget::Global | MenuTarget::TabCreate(_), _) => None,
        (_, Target::Project { project, .. }) => {
            Some(SidebarWorktreeTarget::Project(project.project_id.clone()))
        }
        (
            _,
            Target::Workspace {
                location: NativeContextLocation::Workspace { workspace_id, .. },
                ..
            },
        ) => Some(SidebarWorktreeTarget::Workspace(workspace_id.clone())),
        (MenuTarget::Create(None), Target::Global) => match state.cache().location() {
            NativeContextLocation::Workspace { workspace_id, .. } => {
                Some(SidebarWorktreeTarget::Workspace(workspace_id.clone()))
            }
            _ => None,
        },
        _ => None,
    }
}

fn belongs(entry: &MenuTarget, episode: &Episode) -> bool {
    match (entry, &episode.source.target) {
        (MenuTarget::Worktrees(target), source) => target == source,
        (
            MenuTarget::Workspace(NativeContextLocation::Workspace { workspace_id, .. }),
            SidebarWorktreeTarget::Workspace(id),
        ) => workspace_id == id,
        (
            MenuTarget::Project(project) | MenuTarget::Create(Some(project)),
            SidebarWorktreeTarget::Project(id),
        ) => project == id,
        (MenuTarget::Create(None), SidebarWorktreeTarget::Workspace(_)) => true,
        _ => false,
    }
}

fn checkout_actions(discovery: &SidebarDiscovery) -> Vec<MenuAction> {
    discovery
        .discovery
        .entries
        .iter()
        .filter(|entry| !entry.checkout.bare)
        .filter_map(|entry| {
            let orchbee_workbench_protocol::WorktreeEntryText::Utf8(path) = &entry.checkout.path
            else {
                return None;
            };
            Some(MenuAction::OpenWorktreeAt(
                orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(path).ok()?,
            ))
        })
        .collect()
}

pub(super) fn actions(
    entry: &MenuTarget,
    mut actions: Vec<MenuAction>,
    episode: Option<&Episode>,
) -> Vec<MenuAction> {
    let ready = episode
        .filter(|episode| belongs(entry, episode))
        .and_then(Episode::ready);
    match entry {
        MenuTarget::Worktrees(_) => {
            let Some(ready) = ready else {
                return Vec::new();
            };
            let mut actions = vec![MenuAction::NewWorktree];
            actions.extend(checkout_actions(ready));
            actions
        }
        MenuTarget::Workspace(_) | MenuTarget::Project(_) => {
            if ready.is_none() {
                actions.retain(|action| action != &MenuAction::WorkspaceWorktrees);
            }
            actions
        }
        MenuTarget::Create(project) => {
            let mut actions = vec![if project.is_some() {
                MenuAction::QuickWorkspace
            } else {
                MenuAction::NewWorkspace
            }];
            // Creation's conditional list is independent of the Worktree submenu's New action.
            // No entries means no list and no preceding separator.
            if let Some(ready) = ready {
                actions.extend(checkout_actions(ready));
            }
            actions
        }
        _ => actions,
    }
}

pub(super) fn project_choices(
    state: &NativeSessionState,
    target: &Target,
) -> Option<(Vec<orchbee_workbench_protocol::Project>, Vec<MenuAction>)> {
    let Target::Workspace {
        source: NativeMenuContextSource::Workspace { workspace, .. },
        ..
    } = target
    else {
        return None;
    };
    let projects: Vec<_> = state
        .cache()
        .canonical_snapshot()
        .navigation
        .ordered_projects()
        .ok()?
        .into_iter()
        .cloned()
        .collect();
    let mut actions = vec![if workspace.project_id.is_some() {
        MenuAction::LeaveWorkspaceProject
    } else {
        MenuAction::CreateWorkspaceProject
    }];
    actions.extend(
        projects
            .iter()
            .map(|project| MenuAction::JoinWorkspaceProject(project.project_id.clone())),
    );
    Some((projects, actions))
}

impl NativeUi {
    pub(super) fn prepare_sidebar_worktrees(&mut self) {
        let Some(intent) = self.menus.intent.as_ref() else {
            return;
        };
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let Some(target) = target(&intent.entry, &intent.target, state) else {
            return;
        };
        if self.menus.sidebar.as_ref().is_some_and(|episode| {
            episode.source.target == target
                && episode.source.unchanged(state.cache().canonical_snapshot())
                && (matches!(
                    episode.state,
                    DiscoveryState::Ready(_) | DiscoveryState::Absent
                ) || self.clock.now() < episode.deadline)
        }) {
            return;
        }
        let Some(source) =
            AddressedWorktreeSource::capture(state.cache().canonical_snapshot(), &target)
        else {
            self.menus.sidebar = None;
            return;
        };
        let Some(id) = self.menus.next_sidebar.checked_add(1) else {
            return;
        };
        let Some(deadline) = self.clock.now().checked_add(Duration::from_secs(10)) else {
            return;
        };
        self.menus.next_sidebar = id;
        self.menus.sidebar = Some(Episode {
            id,
            source,
            deadline,
            state: DiscoveryState::Waiting,
        });
    }

    pub(super) fn sidebar_worktree_source_current(&self) -> bool {
        self.menus.sidebar.as_ref().is_none_or(|episode| {
            self.state
                .as_ref()
                .is_some_and(|state| episode.source.unchanged(state.cache().canonical_snapshot()))
        })
    }

    pub(in crate::terminal_session::ui) fn drive_sidebar_worktrees(&mut self) -> bool {
        if !self.action_menu_active() {
            return false;
        }
        let Some(episode) = self.menus.sidebar.as_ref() else {
            return false;
        };
        if !self.sidebar_worktree_source_current() {
            self.fail_action_menu();
            return true;
        }
        if self.clock.now() >= episode.deadline
            && matches!(
                episode.state,
                DiscoveryState::Waiting | DiscoveryState::Querying
            )
        {
            if let Some(episode) = self.menus.sidebar.as_mut() {
                episode.state = DiscoveryState::Absent;
            }
            return self.recompose_sidebar_menu();
        }
        if !matches!(episode.state, DiscoveryState::Waiting) {
            return false;
        }
        let (id, source, deadline) = (episode.id, episode.source.clone(), episode.deadline);
        if self.issue_worktree_before(
            Action::SidebarQuery {
                episode: id,
                source: Box::new(source),
            },
            deadline,
        ) && let Some(episode) = self.menus.sidebar.as_mut()
        {
            episode.state = DiscoveryState::Querying;
        }
        false
    }

    pub(in crate::terminal_session::ui) fn settle_sidebar_worktrees(
        &mut self,
        id: u64,
        result: Result<Box<SidebarDiscovery>, WorktreeNotice>,
    ) {
        let Some(episode) = self.menus.sidebar.as_mut().filter(|episode| {
            episode.id == id && matches!(episode.state, DiscoveryState::Querying)
        }) else {
            return;
        };
        episode.state = match result {
            Ok(discovery)
                if self.clock.now() < episode.deadline
                    && discovery.addressed == episode.source
                    && self.state.as_ref().is_some_and(|state| {
                        discovery.valid(state.cache().canonical_snapshot())
                    }) =>
            {
                DiscoveryState::Ready(discovery)
            }
            _ => DiscoveryState::Absent,
        };
        self.recompose_sidebar_menu();
    }

    fn recompose_sidebar_menu(&mut self) -> bool {
        let Some(intent) = self
            .menus
            .intent
            .as_ref()
            .filter(|intent| matches!(intent.stage, Stage::Open))
        else {
            return false;
        };
        if !matches!(
            intent.entry,
            MenuTarget::Workspace(_)
                | MenuTarget::Project(_)
                | MenuTarget::Create(_)
                | MenuTarget::Worktrees(_)
        ) {
            return false;
        }
        let Some(menu) = self.menus.menu.view() else {
            return false;
        };
        let base = match intent.entry {
            MenuTarget::Workspace(_) => crate::action_menu::WORKSPACE_ACTIONS.to_vec(),
            MenuTarget::Project(_) => crate::action_menu::PROJECT_ACTIONS.to_vec(),
            _ => menu.actions.to_vec(),
        };
        let actions = actions(&intent.entry, base, self.menus.sidebar.as_ref());
        if actions == menu.actions {
            return false;
        }
        if actions.is_empty() {
            self.fail_action_menu();
            return true;
        }
        let (entry, title, anchor, selected) = (
            intent.entry.clone(),
            menu.title.to_owned(),
            menu.anchor,
            menu.actions[menu.selected].clone(),
        );
        self.menus.menu.cancel();
        if !self.menus.menu.open_for(entry, title, actions, anchor) {
            self.fail_action_menu();
            return true;
        }
        self.menus.menu.set_labels(|action| match action {
            MenuAction::OpenWorktreeAt(path) => Some(path.as_str().to_owned()),
            _ => None,
        });
        self.menus.menu.select_action(selected);
        self.menus.menu.refresh_availability(|action| {
            visible_unavailable(action_availability(
                self.state.as_ref(),
                &intent.target,
                action,
                intent.pointer_released,
                intent.resume_permit.as_ref(),
            ))
        });
        true
    }

    pub(super) fn open_sidebar_worktree(
        &mut self,
        discovery: SidebarDiscovery,
        checkout: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    ) -> bool {
        if self.modal_owns_input() || self.retirement_failure.is_some() {
            return false;
        }
        let Some(origin) = self
            .state
            .as_ref()
            .and_then(NativeSessionState::worktree_origin)
        else {
            return false;
        };
        self.worktrees
            .open_sidebar(origin, discovery, checkout.as_ref())
    }
}
