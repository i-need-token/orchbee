//! Sidebar composition projects exact typed organization choices, never labels as identity.
use super::episode::{Answer, DiscoveryState};
use super::{
    ClientContextLocation, Discovery, MenuAction, MenuContextSource, MenuTarget, Stage, Target,
    WorkbenchClientState, WorkbenchClientUi, intent_availability, visible_unavailable,
};
use crate::workbench_client::worktree_lane::Action;
use crate::workbench_state::worktree_launch::addressed::SidebarWorktreeTarget;
use crate::workbench_state::worktree_launch::addressed::{
    AddressedWorktreeSource, SidebarDiscovery,
};
use crate::workbench_state::worktree_launch::{Removal, WorktreeNotice};

#[cfg(test)]
mod tests;

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

fn belongs(entry: &MenuTarget, episode: &Discovery) -> bool {
    match (entry, &episode.source().target) {
        (MenuTarget::WorkspaceCreation(project), SidebarWorktreeTarget::Project(id)) => {
            project == id
        }
        (
            MenuTarget::Workspace(ClientContextLocation::Workspace { workspace_id, .. })
            | MenuTarget::WorkspaceWorktrees(ClientContextLocation::Workspace {
                workspace_id, ..
            }),
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
        // Spellings of one Windows directory, such as by case or separator, share one row.
        if !folders.iter().any(|folder| {
            orchbee_workbench_protocol::same_directory_spelling(
                folder.as_str(),
                directory.path.as_str(),
            )
        }) {
            folders.push(directory.path.clone());
        }
    }
    folders
}

/// Display text for folder and checkout rows, which name a path rather than an action.
pub(super) struct ChoiceLabels(Vec<(MenuAction, String)>);

impl ChoiceLabels {
    pub(super) fn new(actions: &[MenuAction], episode: Option<&Discovery>) -> Self {
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
            .and_then(Discovery::recognized)
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
    checkouts(discovery, |_| true)
}

/// A Workspace's Open Worktree submenu lists every other openable checkout.
fn other_checkout_actions(discovery: &SidebarDiscovery) -> Vec<MenuAction> {
    checkouts(discovery, |entry| !discovery.own_checkout(entry))
}

fn checkouts(
    discovery: &SidebarDiscovery,
    keep: impl Fn(&orchbee_workbench_protocol::WorktreeQueryEntry) -> bool,
) -> Vec<MenuAction> {
    discovery
        .discovery
        .entries
        .iter()
        .filter(|entry| !entry.checkout.bare && keep(entry))
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
    episode: Option<&Discovery>,
) -> Vec<MenuAction> {
    let episode = episode.filter(|episode| belongs(entry, episode));
    let ready = episode.and_then(Discovery::recognized);
    // A menu that opened knowing its source is outside Git has no Worktree rows to offer.
    let outside_git = episode.is_some_and(Discovery::outside_git);
    match entry {
        // Checkouts join the submenu when discovery proves them; New Worktree… waits disabled.
        MenuTarget::WorkspaceCreation(_) => {
            let mut actions: Vec<_> = actions
                .into_iter()
                .filter(|action| {
                    !matches!(action, MenuAction::OpenWorktreeAt(_))
                        && !(outside_git && *action == MenuAction::NewWorktree)
                })
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
                .filter(|action| {
                    *action != MenuAction::RemoveWorktree
                        && !(outside_git
                            && matches!(
                                action,
                                MenuAction::NewWorktree | MenuAction::WorkspaceWorktrees
                            ))
                })
                .collect();
            if ready.is_some_and(|ready| Removal::capture(ready).is_some()) {
                actions.push(MenuAction::RemoveWorktree);
            }
            actions
        }
        MenuTarget::WorkspaceWorktrees(_) => ready.map(other_checkout_actions).unwrap_or_default(),
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
    episode: Option<&Discovery>,
    action: &MenuAction,
) -> Option<crate::action_menu::MenuUnavailable> {
    use crate::action_menu::MenuUnavailable;
    // New Worktree… and Open Worktree ▸ wait for the addressed source's Git discovery. A checkout or
    // removal row exists only by an earlier answer, so one chosen while the Workspace leaves it, or
    // Git no longer answers, must not wait out the menu's deadline.
    let waits = match action {
        MenuAction::NewWorktree => {
            matches!(
                entry,
                MenuTarget::WorkspaceCreation(_) | MenuTarget::Workspace(_)
            )
        }
        MenuAction::WorkspaceWorktrees => matches!(entry, MenuTarget::Workspace(_)),
        MenuAction::OpenWorktreeAt(_) | MenuAction::RemoveWorktree => matches!(
            entry,
            MenuTarget::WorkspaceCreation(_)
                | MenuTarget::Create(_)
                | MenuTarget::Workspace(_)
                | MenuTarget::WorkspaceWorktrees(_)
        ),
        _ => false,
    };
    if !waits {
        return None;
    }
    let Some(episode) = episode.filter(|episode| belongs(entry, episode)) else {
        return Some(MenuUnavailable::WorktreeUnavailable);
    };
    match episode.shown() {
        DiscoveryState::Ready(ready) => (action == &MenuAction::WorkspaceWorktrees
            && other_checkout_actions(ready).is_empty())
        .then_some(MenuUnavailable::NoOtherWorktrees),
        // Git facts prove New Worktree…; the other rows depend on the checkout list.
        DiscoveryState::Waiting | DiscoveryState::Querying => (action != &MenuAction::NewWorktree
            || !episode.known_git())
        .then_some(MenuUnavailable::WorktreeChecking),
        DiscoveryState::NotGit => Some(MenuUnavailable::NotGitRepository),
        DiscoveryState::Absent => Some(MenuUnavailable::WorktreeUnavailable),
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
    /// The shell reports where the sidebar row of the menu about to open stands, while that row
    /// shows a branch for it. The menu's New Worktree… then starts enabled instead of waiting.
    pub(in crate::workbench_client) fn observe_sidebar_git(
        &mut self,
        directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    ) {
        self.menus.observe_sidebar_git(directory);
    }

    pub(super) fn prepare_sidebar_worktrees(&mut self) {
        let Some(intent) = self.menus.intent() else {
            return;
        };
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let Some(target) = target(intent.entry(), intent.target(), state) else {
            return;
        };
        if self.menus.discovery().is_some_and(|episode| {
            episode.source().target == target
                && episode
                    .source()
                    .unchanged(state.cache().canonical_snapshot())
                && (matches!(
                    episode.state(),
                    DiscoveryState::Ready(_) | DiscoveryState::NotGit | DiscoveryState::Absent
                ) || self.clock.now() < episode.deadline())
        }) {
            return;
        }
        let Some(source) =
            AddressedWorktreeSource::capture(state.cache().canonical_snapshot(), &target)
        else {
            self.menus.clear_discovery();
            return;
        };
        if !self.menus.begin_discovery(source, || self.clock.now()) {
            return;
        }
        // Opening must publish the pending reason before its first paint or input snapshot.
        // Otherwise the first input revises availability and invalidates its own menu proof.
        self.recompose_sidebar_menu();
    }

    pub(super) fn sidebar_worktree_source_current(&self) -> bool {
        self.menus.discovery().is_none_or(|episode| {
            self.state
                .as_ref()
                .is_some_and(|state| episode.source().current(state.cache().canonical_snapshot()))
        })
    }

    pub(in crate::workbench_client::ui) fn drive_sidebar_worktrees(&mut self) -> bool {
        if !self.action_menu_active() || self.menus.discovery().is_none() {
            return false;
        }
        if !self.sidebar_worktree_source_current() {
            self.fail_action_menu();
            return true;
        }
        // The first terminal moved under the open menu: ask Git about where it stands now.
        if self.menus.discovery().is_some_and(|episode| {
            self.state.as_ref().is_some_and(|state| {
                !episode
                    .source()
                    .unchanged(state.cache().canonical_snapshot())
            })
        }) {
            self.prepare_sidebar_worktrees();
            return true;
        }
        if self.menus.expire_discovery(self.clock.now()) {
            return self.recompose_sidebar_menu();
        }
        let Some((id, source, deadline)) = self
            .menus
            .discovery()
            .filter(|episode| matches!(episode.state(), DiscoveryState::Waiting))
            .map(|episode| (episode.id(), episode.source().clone(), episode.deadline()))
        else {
            return false;
        };
        if self.issue_worktree_before(
            Action::SidebarQuery {
                episode: id,
                source: Box::new(source),
            },
            deadline,
        ) {
            self.menus.mark_discovery_querying();
        }
        false
    }

    pub(in crate::workbench_client::ui) fn settle_sidebar_worktrees(
        &mut self,
        id: u64,
        result: Result<Box<SidebarDiscovery>, WorktreeNotice>,
    ) {
        let state = self.state.as_ref();
        let answer = match result {
            Ok(found) => Answer::Found(found),
            Err(WorktreeNotice::NotRepository) => Answer::NotGit,
            Err(_) => Answer::Failed,
        };
        if self.menus.settle_discovery(
            id,
            answer,
            || self.clock.now(),
            |discovery| {
                state.is_some_and(|state| discovery.valid(state.cache().canonical_snapshot()))
            },
            |source| {
                state.is_some_and(|state| source.unchanged(state.cache().canonical_snapshot()))
            },
        ) {
            self.recompose_sidebar_menu();
        }
    }

    fn recompose_sidebar_menu(&mut self) -> bool {
        let state = self.state.as_ref();
        let (menu, intent, discovery) = self.menus.menu_with_facts();
        let Some(intent) = intent.filter(|intent| matches!(intent.stage(), Stage::Open)) else {
            return false;
        };
        if !matches!(
            intent.entry(),
            MenuTarget::Workspace(_)
                | MenuTarget::WorkspaceWorktrees(_)
                | MenuTarget::Project(_)
                | MenuTarget::Create(_)
                | MenuTarget::WorkspaceCreation(_)
        ) {
            return false;
        }
        let Some(view) = menu.view() else {
            return false;
        };
        // The open rows already omit hidden actions; only discovery rows are recomposed.
        let actions = actions(intent.entry(), view.actions.to_vec(), discovery);
        let labels = ChoiceLabels::new(&actions, discovery);
        if actions == view.actions {
            menu.set_labels(|action| labels.label(&action));
            return menu.refresh_availability(|action| {
                visible_unavailable(intent_availability(intent, state, discovery, action))
            });
        }
        if actions.is_empty() {
            self.fail_action_menu();
            return true;
        }
        let (entry, title, anchor, selected, hidden) = (
            intent.entry().clone(),
            view.title.to_owned(),
            view.anchor,
            view.actions[view.selected].clone(),
            view.candidate_hidden,
        );
        menu.cancel();
        if !menu.open_for(entry, title, actions, anchor) {
            self.fail_action_menu();
            return true;
        }
        menu.set_labels(|action| labels.label(&action));
        if hidden {
            menu.hide_candidate();
        } else {
            menu.select_action(selected);
        }
        menu.refresh_availability(|action| {
            visible_unavailable(intent_availability(intent, state, discovery, action))
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
