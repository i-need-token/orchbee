//! Sidebar composition projects exact typed organization choices, never labels as identity.
use super::{
    ClientContextLocation, Duration, Instant, MenuAction, MenuContextSource, MenuTarget, Stage,
    Target, WorkbenchClientState, WorkbenchClientUi, visible_unavailable,
};
use crate::workbench_client::{
    SidebarWorktreeTarget,
    state::worktree_launch::{
        Removal, WorktreeNotice,
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
    state: &WorkbenchClientState,
) -> Option<SidebarWorktreeTarget> {
    match (entry, target) {
        (MenuTarget::Global | MenuTarget::TabCreate(_) | MenuTarget::TabProfiles(_), _) => None,
        (_, Target::Project { project, .. }) => {
            Some(SidebarWorktreeTarget::Project(project.project_id.clone()))
        }
        (
            _,
            Target::Workspace {
                location: ClientContextLocation::Workspace { workspace_id, .. },
                ..
            },
        ) => Some(SidebarWorktreeTarget::Workspace(workspace_id.clone())),
        (MenuTarget::Create(None), Target::Global) => match state.cache().location() {
            ClientContextLocation::Workspace { workspace_id, .. } => {
                Some(SidebarWorktreeTarget::Workspace(workspace_id.clone()))
            }
            _ => None,
        },
        _ => None,
    }
}

fn belongs(entry: &MenuTarget, episode: &Episode) -> bool {
    match (entry, &episode.source.target) {
        (MenuTarget::WorkspaceCreation(project), SidebarWorktreeTarget::Project(id)) => {
            project == id
        }
        (
            MenuTarget::Workspace(ClientContextLocation::Workspace { workspace_id, .. }),
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

/// The recorded directories of a Project's Workspaces in navigation order, each once.
pub(super) fn project_folders(
    state: &WorkbenchClientState,
    project: &orchbee_workbench_protocol::WorkbenchProjectId,
) -> Vec<orchbee_workbench_protocol::WorkbenchDirectoryPath> {
    let navigation = &state.cache().canonical_snapshot().navigation;
    let Ok(ordered) = navigation.ordered_workspaces() else {
        return Vec::new();
    };
    let mut folders: Vec<orchbee_workbench_protocol::WorkbenchDirectoryPath> = Vec::new();
    for (_, workspace) in ordered
        .into_iter()
        .filter(|(member, _)| member.is_some_and(|member| &member.project_id == project))
    {
        let Some(directory) = navigation.workspace_directory(&workspace.workspace_id) else {
            continue;
        };
        // Recorded paths differ only in case on case-insensitive hosts; one row serves both.
        if !folders.iter().any(|folder| {
            folder
                .as_str()
                .eq_ignore_ascii_case(directory.path.as_str())
        }) {
            folders.push(directory.path.clone());
        }
    }
    folders
}

/// Display text for folder and checkout rows, which name a path rather than an action.
pub(super) struct ChoiceLabels(Vec<(MenuAction, String)>);

impl ChoiceLabels {
    pub(super) fn new(actions: &[MenuAction], episode: Option<&Episode>) -> Self {
        let folders: Vec<_> = actions
            .iter()
            .filter_map(|action| match action {
                MenuAction::WorkspaceInFolder(path) => Some(path),
                _ => None,
            })
            .collect();
        let base = |path: &orchbee_workbench_protocol::WorkbenchDirectoryPath| {
            path.as_str()
                .trim_end_matches(['/', '\\'])
                .rsplit(['/', '\\'])
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or(path.as_str())
                .to_owned()
        };
        let mut labels = Vec::new();
        for path in &folders {
            let name = base(path);
            // A shared folder name is ambiguous, so those rows show their whole path.
            let label = if folders
                .iter()
                .filter(|other| base(other).eq_ignore_ascii_case(&name))
                .count()
                > 1
            {
                path.as_str().to_owned()
            } else {
                name
            };
            labels.push((MenuAction::WorkspaceInFolder((*path).clone()), label));
        }
        let entries = episode
            .and_then(Episode::ready)
            .map_or(&[][..], |ready| &ready.discovery.entries[..]);
        for action in actions {
            let MenuAction::OpenWorktreeAt(path) = action else {
                continue;
            };
            let branch = entries.iter().find_map(|entry| {
                let orchbee_workbench_protocol::WorktreeEntryText::Utf8(entry_path) =
                    &entry.checkout.path
                else {
                    return None;
                };
                if entry_path != path.as_str() {
                    return None;
                }
                match &entry.checkout.branch {
                    Some(orchbee_workbench_protocol::WorktreeEntryText::Utf8(branch)) => Some(
                        branch
                            .strip_prefix("refs/heads/")
                            .unwrap_or(branch)
                            .to_owned(),
                    ),
                    _ => None,
                }
            });
            labels.push((
                action.clone(),
                branch.unwrap_or_else(|| path.as_str().to_owned()),
            ));
        }
        Self(labels)
    }

    pub(super) fn label(&self, action: &MenuAction) -> Option<String> {
        self.0
            .iter()
            .find(|(candidate, _)| candidate == action)
            .map(|(_, label)| label.clone())
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
    actions: Vec<MenuAction>,
    episode: Option<&Episode>,
) -> Vec<MenuAction> {
    let ready = episode
        .filter(|episode| belongs(entry, episode))
        .and_then(Episode::ready);
    match entry {
        // Checkouts join the submenu when discovery proves them; New Worktree… waits disabled.
        MenuTarget::WorkspaceCreation(_) => {
            let mut actions: Vec<_> = actions
                .into_iter()
                .filter(|action| !matches!(action, MenuAction::OpenWorktreeAt(_)))
                .collect();
            if let Some(ready) = ready {
                actions.extend(checkout_actions(ready));
            }
            actions
        }
        // Removal is offered only for a proven linked checkout. It follows the close row, so its
        // late arrival leaves every row above it in place.
        MenuTarget::Workspace(_) => {
            let mut actions: Vec<_> = actions
                .into_iter()
                .filter(|action| *action != MenuAction::RemoveWorktree)
                .collect();
            if ready.is_some_and(|ready| Removal::capture(ready).is_some()) {
                actions.push(MenuAction::RemoveWorktree);
            }
            actions
        }
        MenuTarget::Project(_) => actions,
        MenuTarget::Create(project) => {
            let mut actions = vec![if project.is_some() {
                MenuAction::QuickWorkspace
            } else {
                MenuAction::NewWorkspace
            }];
            actions.push(MenuAction::WorkspaceAtDirectory);
            // Creation's checkout list is independent of New Workspace's New Worktree… row.
            // No entries means no list and no preceding separator.
            if let Some(ready) = ready {
                actions.extend(checkout_actions(ready));
            }
            actions
        }
        _ => actions,
    }
}

pub(super) fn unavailable(
    entry: &MenuTarget,
    episode: Option<&Episode>,
    action: &MenuAction,
) -> Option<crate::action_menu::MenuUnavailable> {
    use crate::action_menu::MenuUnavailable;
    if action != &MenuAction::NewWorktree || !matches!(entry, MenuTarget::WorkspaceCreation(_)) {
        return None;
    }
    match episode
        .filter(|episode| belongs(entry, episode))
        .map(|episode| &episode.state)
    {
        Some(DiscoveryState::Ready(_)) => None,
        Some(DiscoveryState::Waiting | DiscoveryState::Querying) => {
            Some(MenuUnavailable::WorktreeChecking)
        }
        Some(DiscoveryState::Absent) | None => Some(MenuUnavailable::WorktreeUnavailable),
    }
}

pub(super) fn project_choices(
    state: &WorkbenchClientState,
    target: &Target,
) -> Option<(Vec<orchbee_workbench_protocol::Project>, Vec<MenuAction>)> {
    let Target::Workspace {
        source: MenuContextSource::Workspace { workspace, .. },
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
    // The other Projects first, then a new Project and, for a member, leaving its Project.
    let mut actions: Vec<_> = projects
        .iter()
        .filter(|project| workspace.project_id.as_ref() != Some(&project.project_id))
        .map(|project| MenuAction::JoinWorkspaceProject(project.project_id.clone()))
        .collect();
    actions.push(MenuAction::CreateWorkspaceProject);
    if workspace.project_id.is_some() {
        actions.push(MenuAction::LeaveWorkspaceProject);
    }
    Some((projects, actions))
}

impl WorkbenchClientUi {
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
        // Opening must publish the pending reason before its first paint or input snapshot.
        // Otherwise the first input revises availability and invalidates its own menu proof.
        self.recompose_sidebar_menu();
    }

    pub(super) fn sidebar_worktree_source_current(&self) -> bool {
        self.menus.sidebar.as_ref().is_none_or(|episode| {
            self.state
                .as_ref()
                .is_some_and(|state| episode.source.unchanged(state.cache().canonical_snapshot()))
        })
    }

    pub(in crate::workbench_client::ui) fn drive_sidebar_worktrees(&mut self) -> bool {
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

    pub(in crate::workbench_client::ui) fn settle_sidebar_worktrees(
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
                | MenuTarget::WorkspaceCreation(_)
        ) {
            return false;
        }
        let Some(menu) = self.menus.menu.view() else {
            return false;
        };
        // The open rows already omit hidden actions; only discovery rows are recomposed.
        let actions = actions(
            &intent.entry,
            menu.actions.to_vec(),
            self.menus.sidebar.as_ref(),
        );
        let labels = ChoiceLabels::new(&actions, self.menus.sidebar.as_ref());
        if actions == menu.actions {
            self.menus.menu.set_labels(|action| labels.label(&action));
            return self.menus.menu.refresh_availability(|action| {
                visible_unavailable(intent.availability(
                    self.state.as_ref(),
                    self.menus.sidebar.as_ref(),
                    action,
                ))
            });
        }
        if actions.is_empty() {
            self.fail_action_menu();
            return true;
        }
        let (entry, title, anchor, selected, hidden) = (
            intent.entry.clone(),
            menu.title.to_owned(),
            menu.anchor,
            menu.actions[menu.selected].clone(),
            menu.candidate_hidden,
        );
        self.menus.menu.cancel();
        if !self.menus.menu.open_for(entry, title, actions, anchor) {
            self.fail_action_menu();
            return true;
        }
        self.menus.menu.set_labels(|action| labels.label(&action));
        if hidden {
            self.menus.menu.hide_candidate();
        } else {
            self.menus.menu.select_action(selected);
        }
        self.menus.menu.refresh_availability(|action| {
            visible_unavailable(intent.availability(
                self.state.as_ref(),
                self.menus.sidebar.as_ref(),
                action,
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
            .and_then(WorkbenchClientState::worktree_origin)
        else {
            return false;
        };
        self.worktrees
            .open_sidebar(origin, discovery, checkout.as_ref())
    }

    pub(super) fn remove_sidebar_worktree(&mut self, discovery: SidebarDiscovery) -> bool {
        if self.modal_owns_input() || self.retirement_failure.is_some() {
            return false;
        }
        let Some(origin) = self
            .state
            .as_ref()
            .and_then(WorkbenchClientState::worktree_origin)
        else {
            return false;
        };
        self.worktrees.remove_sidebar(origin, discovery)
    }
}
