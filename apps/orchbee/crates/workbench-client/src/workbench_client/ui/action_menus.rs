//! One local menu intent; existing membership and action owners retain all execution authority.
//! The episode itself belongs to `episode`; this module evaluates its rules against the other
//! owners and executes what it confirms.
use super::{
    ClientAction, ClientPhaseKind, Geometry, ModalPresentation, PaneRef, WorkbenchClientNotice,
    WorkbenchClientState, WorkbenchClientUi, WorkbenchClientView,
};
use crate::action_menu::{
    DrawnMenuSource, MenuAction, MenuAnchor, MenuControl, MenuDraw, MenuInputContext,
    MenuInputReadiness, MenuSearchChild, MenuTarget, MenuToken, MenuUnavailable, MenuView,
};
use crate::action_projection::ActionAvailability;
use crate::client_view::KeyEvent;
use crate::tui_presentation::GeometrySettlement;
use crate::workbench_projection::{ClientContextLocation, MenuContextSource};
use crate::workbench_state::TabCloseResumePermit;
use episode::{
    Discovery, Intent, Origin, ParentMenu, ProjectContext, ResumeHandoff, Stage, TabResolverSource,
    Target,
};
pub(super) use episode::{MenuEpisode, NavigationKey};
use std::time::Duration;
mod episode;
mod sidebar;
#[cfg(test)]
mod tests;

fn selected_presentation(
    state: &WorkbenchClientState,
    pane: &PaneRef,
) -> Option<orchbee_workbench_protocol::TerminalPresentationId> {
    let tab = state
        .cache()
        .tabs()
        .get(state.cache().tab_index(pane.anchor().tab_id())?)?;
    let panel = tab.panel(tab.panel_index(pane.pane().panel())?)?;
    Some(state.selected_presentation(panel).presentation_id().clone())
}

fn action_availability(
    capabilities: crate::tui_capabilities::Capabilities,
    state: Option<&WorkbenchClientState>,
    target: &Target,
    action: MenuAction,
    pointer_released: bool,
    resume_permit: Option<&TabCloseResumePermit>,
) -> ActionAvailability<MenuUnavailable> {
    use crate::workbench_state::workspace_setup::DirectoryConfigurationUnavailable;
    if action
        .client_action()
        .as_ref()
        .and_then(crate::tui_capabilities::for_action)
        .is_some_and(|capability| !capabilities.supports(capability))
    {
        return ActionAvailability::Hidden;
    }
    if action == MenuAction::NewTab
        && !capabilities.supports(crate::tui_capabilities::Capability::DirectoryInput)
        && state.is_some_and(|state| {
            matches!(state.cache().location(), ClientContextLocation::NoWorkspace)
        })
    {
        return ActionAvailability::Hidden;
    }
    if let (Some(state), Target::Pane { pane, .. }) = (state, target) {
        // Zoom needs another panel to hide; Take control applies only while another Client
        // holds the panel's size.
        let hidden = match action {
            MenuAction::Zoom => state
                .cache()
                .tab_index(pane.anchor().tab_id())
                .and_then(|index| state.cache().tabs().get(index))
                .is_none_or(|tab| tab.panel_count() < 2),
            MenuAction::Takeover => !state.pane_facts().any(|(record, _, mode)| {
                record == *pane
                    && matches!(mode, Some(crate::workbench_state::ControllerMode::ReadOnly))
            }),
            _ => false,
        };
        if hidden {
            return ActionAvailability::Hidden;
        }
    }
    if matches!(target, Target::Global)
        && matches!(
            action,
            MenuAction::RenameTab
                | MenuAction::NewTabToRight
                | MenuAction::DuplicateTab
                | MenuAction::MoveTabPrevious
                | MenuAction::MoveTabNext
                | MenuAction::MoveTabFirst
                | MenuAction::MoveTabLast
                | MenuAction::SelectAllTabs
                | MenuAction::ClearTabSelection
                | MenuAction::CloseTrailingTabs
                | MenuAction::SplitPanel(_)
                | MenuAction::Zoom
                | MenuAction::Resize
                | MenuAction::Copy
                | MenuAction::ClosePane
                | MenuAction::MovePanel(_)
        )
        && state.is_none_or(|state| state.focus_pane_ref().is_none())
    {
        return ActionAvailability::Disabled(MenuUnavailable::FocusedPanelRequired);
    }
    if matches!(
        action,
        MenuAction::NewGroupWithTab
            | MenuAction::NewTabInRecentGroup
            | MenuAction::NewTabWithProfile(_)
    ) && state.is_some_and(|state| {
        matches!(state.cache().location(), ClientContextLocation::NoWorkspace)
            && state.cache().tabs().is_empty()
    }) {
        return ActionAvailability::Disabled(MenuUnavailable::WorkspaceRequired);
    }
    if action == MenuAction::NewTabInRecentGroup
        && state
            .and_then(WorkbenchClientState::recent_tab_group)
            .is_none()
    {
        return ActionAvailability::Disabled(MenuUnavailable::NoRecentGroup);
    }
    if action == MenuAction::ResumeTabClose {
        return match (state, target, resume_permit) {
            (Some(state), Target::Tab { tab, .. }, Some(permit))
                if state.validate_tab_close_resume_permit(tab.tab_id(), permit) =>
            {
                ActionAvailability::Enabled
            }
            _ => ActionAvailability::Disabled(MenuUnavailable::OperationBusy),
        };
    }
    let addressed = match (state, target) {
        (Some(state), Target::Tab { tab, .. }) => Some((state, &**tab)),
        // The global menu's Tab rows address the current Tab.
        (Some(state), Target::Global) => {
            super::tab_groups::current_tab(state).map(|tab| (state, tab))
        }
        _ => None,
    };
    if let Some((state, tab)) = addressed
        && super::tab_groups::close_tab_ids(state, tab.tab_id(), action.clone())
            .is_some_and(|ids| ids.is_empty())
    {
        return ActionAvailability::Disabled(MenuUnavailable::NoMatchingTabs);
    }
    if matches!(
        action,
        MenuAction::WorkspaceAtDirectory | MenuAction::WorkspaceInFolder(_)
    ) || matches!(target, Target::Project { .. }) && action == MenuAction::QuickWorkspace
    {
        return ActionAvailability::from_disabled_reason(
            state
                .is_none_or(|state| !state.quick_workspace_available())
                .then_some(MenuUnavailable::OperationBusy),
        );
    }
    if (matches!(target, Target::Project { .. }) && action == MenuAction::CloseProjectWorkspaces)
        || (matches!(target, Target::Workspace { .. })
            && matches!(
                action,
                MenuAction::CloseWorkspace | MenuAction::DeleteWorkspace
            ))
    {
        let available = state.is_some_and(|state| {
            state.close_scope_available()
                || matches!(target,
                    Target::Workspace {
                        location: ClientContextLocation::Workspace { workspace_id, .. }, ..
                    } if state.can_confirm_guarded_workspace_close(workspace_id))
        });
        return ActionAvailability::from_disabled_reason(
            (!available).then_some(MenuUnavailable::OperationBusy),
        );
    }
    if (matches!(target, Target::Project { .. })
        && matches!(
            action,
            MenuAction::RenameProject | MenuAction::DeleteProject
        ))
        || matches!(
            action,
            MenuAction::WorkspaceProjects
                | MenuAction::CreateWorkspaceProject
                | MenuAction::LeaveWorkspaceProject
                | MenuAction::JoinWorkspaceProject(_)
        )
    {
        return ActionAvailability::from_disabled_reason(
            state
                .is_none_or(|state| !state.project_rename_available())
                .then_some(MenuUnavailable::OperationBusy),
        );
    }
    if matches!(target, Target::Project { .. }) && action == MenuAction::WorkspaceCreation {
        // The submenu reports each choice's own availability.
        return ActionAvailability::Enabled;
    }
    if matches!(
        action,
        MenuAction::OpenWorktreeAt(_) | MenuAction::NewWorktree | MenuAction::RemoveWorktree
    ) && matches!(target, Target::Workspace { .. } | Target::Project { .. })
    {
        return ActionAvailability::from_disabled_reason(
            state
                .and_then(WorkbenchClientState::worktree_origin)
                .is_none()
                .then_some(MenuUnavailable::OperationBusy),
        );
    }
    if matches!(target, Target::Project { context: None, .. }) {
        return ActionAvailability::Disabled(MenuUnavailable::ProjectWorkspaceRequired);
    }
    if action == MenuAction::ReleasePointer && pointer_released {
        return ActionAvailability::Disabled(MenuUnavailable::PointerReleased);
    }
    if action != MenuAction::WorkspaceDirectory {
        return ActionAvailability::Enabled;
    }
    let Target::Workspace { location, source } = target else {
        return ActionAvailability::Enabled;
    };
    // The row applies only while the Workspace has no directory.
    if matches!(
        source,
        MenuContextSource::Workspace {
            directory: Some(_),
            ..
        }
    ) {
        return ActionAvailability::Hidden;
    }
    let Some(state) = state.filter(|state| state.cache().location() == location) else {
        return ActionAvailability::Enabled;
    };
    ActionAvailability::from_disabled_reason(state.directory_configuration_unavailable().map(
        |reason| match reason {
            DirectoryConfigurationUnavailable::Configured => MenuUnavailable::DirectoryConfigured,
            DirectoryConfigurationUnavailable::EmptyRequired => {
                MenuUnavailable::EmptyWorkspaceRequired
            }
            DirectoryConfigurationUnavailable::WorkspaceRequired => {
                MenuUnavailable::WorkspaceRequired
            }
            DirectoryConfigurationUnavailable::Busy => MenuUnavailable::OperationBusy,
        },
    ))
}

fn visible_unavailable(
    availability: ActionAvailability<MenuUnavailable>,
) -> Option<MenuUnavailable> {
    match availability {
        ActionAvailability::Enabled => None,
        ActionAvailability::Disabled(reason) => Some(reason),
        // Callers remove hidden rows or cancel the episode before asking for a visible reason.
        ActionAvailability::Hidden => None,
    }
}

fn intent_availability(
    intent: &Intent,
    state: Option<&WorkbenchClientState>,
    discovery: Option<&Discovery>,
    action: MenuAction,
) -> ActionAvailability<MenuUnavailable> {
    let availability = action_availability(
        intent.capabilities(),
        state,
        intent.target(),
        action.clone(),
        intent.pointer_released(),
        intent.resume_permit(),
    );
    if availability.is_enabled() {
        ActionAvailability::from_disabled_reason(sidebar::unavailable(
            intent.entry(),
            discovery,
            &action,
        ))
    } else {
        availability
    }
}

enum ResumePermitSeed {
    CaptureDisplayed,
    Inherit(Option<TabCloseResumePermit>),
}

pub(super) enum TabCloseResumeHandoff {
    Direct,
    Menu(TabCloseResumePermit),
    Refused,
}

impl WorkbenchClientView<'_> {
    /// Creation uses the painted context even when it contains no Tabs.
    pub(crate) fn tab_create_target(&self) -> Option<crate::action_menu::DrawnTabCreate> {
        Some(crate::action_menu::DrawnTabCreate {
            source: self.action_menu_source()?,
            location: self
                .context_at(self.active_context_position()?.checked_sub(1)?)?
                .location()
                .clone(),
        })
    }
    pub(crate) fn action_menu(&self) -> Option<MenuView<'_>> {
        self.menu.and_then(MenuEpisode::view)
    }
    pub(crate) fn action_menu_cascade(&self) -> Vec<MenuView<'_>> {
        self.menu.map(MenuEpisode::cascade).unwrap_or_default()
    }
    pub(crate) fn action_menu_root(&self) -> Option<(&MenuTarget, Option<MenuAnchor>)> {
        self.menu.and_then(MenuEpisode::root)
    }
    pub(crate) fn action_menu_input_context(&self) -> Option<MenuInputContext> {
        self.menu.and_then(MenuEpisode::input_context)
    }
    pub(crate) fn action_menu_source(&self) -> Option<DrawnMenuSource> {
        if self.retirement.is_some() {
            return None;
        }
        let state = self.state.as_ref()?;
        Some(DrawnMenuSource {
            session: state.session_id().clone(),
            revision: state.revision(),
        })
    }
}

impl WorkbenchClientUi {
    /// Revalidate the drawn context before delegating to the existing create admission.
    pub(in crate::workbench_client) fn tab_create_action(
        &mut self,
        target: &crate::action_menu::DrawnTabCreate,
    ) -> Option<ClientAction> {
        if self.action_menu_active()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
            || self.board.is_open()
        {
            return None;
        }
        if !self.menu_carrier_live() || self.view().tab_create_target().as_ref() != Some(target) {
            self.command_notice.set(
                self.clock
                    .now()
                    .checked_add(Duration::from_secs(5))
                    .map(|until| (WorkbenchClientNotice::MenuUnavailable, until)),
            );
            return None;
        }
        Some(ClientAction::CreateTab)
    }
    pub(in crate::workbench_client) fn action_menu_capture_claim(&self) -> bool {
        self.view().action_menu_input_context().is_some()
    }
    pub(in crate::workbench_client) fn action_menu_input_context(
        &self,
    ) -> Option<MenuInputContext> {
        self.view().action_menu_input_context()
    }
    pub(in crate::workbench_client) fn action_menu_active(&self) -> bool {
        self.action_menu_capture_claim()
    }

    pub(super) fn observe_action_menu_geometry(&mut self, geometry: Geometry) {
        self.menus.observe_geometry(geometry);
    }

    fn menu_carrier_live(&self) -> bool {
        self.retirement_failure.is_none()
            && self.state.as_ref().is_some_and(|state| {
                state.phase_kind() != ClientPhaseKind::Retired && state.retired_failure().is_none()
            })
    }

    pub(in crate::workbench_client) fn cancel_action_menu(&mut self) {
        self.menus.cancel();
    }

    pub(super) fn fail_action_menu(&mut self) {
        // `drive_action_menu` temporarily moves the intent out while it validates the episode.
        // The retained model remains the failure authority during that interval, even though the
        // public view projection deliberately has no claim without its coordinator intent.
        if !self.menus.menu.active() {
            return;
        }
        self.cancel_action_menu();
        self.note_action_menu_unavailable();
    }

    fn note_action_menu_unavailable(&mut self) {
        self.command_notice.set(
            self.clock
                .now()
                .checked_add(std::time::Duration::from_secs(5))
                .map(|until| (WorkbenchClientNotice::MenuUnavailable, until)),
        );
    }

    pub(in crate::workbench_client) fn open_action_menu(
        &mut self,
        target: MenuTarget,
        anchor: Option<MenuAnchor>,
        pointer_released: bool,
    ) -> bool {
        // A pointer-opened menu waits for the pointer or a key to choose its candidate.
        let hidden = anchor.is_some_and(MenuAnchor::pointer);
        self.open_action_menu_with_resume_permit(
            target,
            anchor,
            pointer_released,
            ResumePermitSeed::CaptureDisplayed,
            hidden,
        )
    }

    fn open_action_menu_with_resume_permit(
        &mut self,
        target: MenuTarget,
        anchor: Option<MenuAnchor>,
        pointer_released: bool,
        resume_seed: ResumePermitSeed,
        hidden: bool,
    ) -> bool {
        self.menus.clear_resume_handoff();
        if self.action_menu_active()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
            || self.board.is_open()
        {
            return false;
        }
        if !self.menus.ensure_geometry(|| {
            self.state
                .as_ref()
                .map(WorkbenchClientState::latest_geometry)
        }) {
            return false;
        }
        let target = match target {
            MenuTarget::Drawn { source, target } => {
                if !self.menu_carrier_live()
                    || self.state.as_ref().is_none_or(|state| {
                        state.cache().session_id() != &source.session
                            || state.cache().revision() != source.revision
                    })
                {
                    return false;
                }
                *target
            }
            target => target,
        };
        let creation_only = matches!(target, MenuTarget::Create(_));
        let entry = target.clone();
        let tab_creation = matches!(target, MenuTarget::TabCreate(_));
        let tab_profiles = matches!(target, MenuTarget::TabProfiles(_));
        if let MenuTarget::TabCreate(ref drawn) | MenuTarget::TabProfiles(ref drawn) = target
            && self.view().tab_create_target().as_ref() != Some(drawn)
        {
            return false;
        }
        let target = match target {
            MenuTarget::WorkspaceCreation(project) | MenuTarget::Create(Some(project)) => {
                MenuTarget::Project(project)
            }
            MenuTarget::Create(None) => MenuTarget::Global,
            MenuTarget::TabCreate(_) | MenuTarget::TabProfiles(_) => MenuTarget::Global,
            target => target,
        };
        let live = self.menu_carrier_live();
        if creation_only && !live {
            return false;
        }
        let state = self.state.as_ref();
        let group_menu = match &target {
            MenuTarget::TabGroups { colors: true, .. } => Some(super::tab_groups::MenuPage::Colors),
            MenuTarget::TabGroups { colors: false, .. } => Some(super::tab_groups::MenuPage::Group),
            MenuTarget::TabGroupChoices(_) => Some(super::tab_groups::MenuPage::Choices),
            _ => None,
        };
        let (target, title, mut actions) = match target {
            MenuTarget::Global => (
                Target::Global,
                String::new(),
                crate::action_menu::GLOBAL_ACTIONS.to_vec(),
            ),
            MenuTarget::Pane(pane) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                let Some(source) = state.cache().menu_context_source(state.cache().location())
                else {
                    return false;
                };
                if state
                    .pane_by_panel(pane.pane().panel())
                    .is_none_or(|record| record.pane_ref() != pane)
                {
                    return false;
                }
                let Some(presentation) = selected_presentation(state, &pane) else {
                    return false;
                };
                let Some(tab) = state
                    .cache()
                    .tab_index(pane.anchor().tab_id())
                    .and_then(|i| state.cache().tabs().get(i))
                else {
                    return false;
                };
                let Some(panel) = tab
                    .panel_index(pane.pane().panel())
                    .and_then(|i| tab.panel(i))
                else {
                    return false;
                };
                let title = state
                    .selected_presentation(panel)
                    .display_label()
                    .to_owned();
                (
                    Target::Pane {
                        pane: pane.clone(),
                        presentation,
                        source,
                    },
                    title,
                    crate::action_menu::PANE_ACTIONS.to_vec(),
                )
            }
            MenuTarget::Tab(id)
            | MenuTarget::TabGroups { tab_id: id, .. }
            | MenuTarget::TabGroupChoices(id) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                let Some(source) = state.cache().menu_context_source(state.cache().location())
                else {
                    return false;
                };
                let Some(index) = state.cache().tab_index(&id) else {
                    return false;
                };
                let tab = &state.cache().tabs()[index];
                (
                    Target::Tab {
                        location: state.cache().location().clone(),
                        tab: Box::new(tab.clone()),
                        source,
                        resolver: TabResolverSource::capture(state.cache().tabs()),
                    },
                    match &entry {
                        MenuTarget::TabGroups { .. } => tab.group().map_or_else(
                            || tab.display_label(index).into_owned(),
                            |group| group.label.clone(),
                        ),
                        _ => tab.display_label(index).into_owned(),
                    },
                    super::tab_groups::menu_actions(state, tab, group_menu),
                )
            }
            MenuTarget::Project(id) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                let Some(project) = state.cache().projects().iter().find(|p| p.project_id == id)
                else {
                    return false;
                };
                let context = match state.cache().location() {
                    location @ ClientContextLocation::Workspace { .. }
                        if state.cache().project_id() == Some(&id) =>
                    {
                        let Some(source) = state.cache().menu_context_source(location) else {
                            return false;
                        };
                        Some(Box::new(ProjectContext {
                            location: location.clone(),
                            source,
                        }))
                    }
                    _ => None,
                };
                (
                    Target::Project {
                        project: Box::new(project.clone()),
                        context,
                    },
                    project.label.clone(),
                    crate::action_menu::PROJECT_ACTIONS.to_vec(),
                )
            }
            MenuTarget::Workspace(location) | MenuTarget::WorkspaceProjects(location) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                let Some(context) = state
                    .cache()
                    .navigation()
                    .iter()
                    .find(|c| c.location == location)
                else {
                    return false;
                };
                let Some(source) = state.cache().menu_context_source(&location) else {
                    return false;
                };
                let actions = crate::action_menu::WORKSPACE_ACTIONS;
                (
                    Target::Workspace { location, source },
                    context
                        .workspace_label
                        .as_ref()
                        .map_or(String::new(), |l| l.as_str().to_owned()),
                    actions.to_vec(),
                )
            }
            MenuTarget::Drawn { .. }
            | MenuTarget::Create(_)
            | MenuTarget::TabCreate(_)
            | MenuTarget::TabProfiles(_)
            | MenuTarget::WorkspaceCreation(_) => {
                return false;
            }
        };
        if let (MenuTarget::WorkspaceCreation(project), Some(state)) = (&entry, state) {
            actions = vec![MenuAction::NewWorktree];
            actions.extend(
                sidebar::project_folders(state, project)
                    .into_iter()
                    .map(MenuAction::WorkspaceInFolder),
            );
        }
        let project_choices = if matches!(entry, MenuTarget::WorkspaceProjects(_)) {
            let Some(choices) = state.and_then(|state| sidebar::project_choices(state, &target))
            else {
                return false;
            };
            actions = choices.1;
            Some(choices.0)
        } else {
            None
        };
        let resume_permit = match (state, &target, resume_seed) {
            (Some(state), Target::Tab { tab, .. }, ResumePermitSeed::CaptureDisplayed)
                if matches!(&entry, MenuTarget::Tab(_)) =>
            {
                state.capture_tab_close_resume_permit(tab.tab_id())
            }
            (Some(state), Target::Tab { tab, .. }, ResumePermitSeed::Inherit(Some(permit)))
                if matches!(&entry, MenuTarget::Tab(_))
                    && state.validate_tab_close_resume_permit(tab.tab_id(), &permit) =>
            {
                Some(permit)
            }
            _ => None,
        };
        actions.retain(|action| *action != MenuAction::ResumeTabClose || resume_permit.is_some());
        if resume_permit.is_some()
            && !actions.contains(&MenuAction::ResumeTabClose)
            && let Some(index) = actions
                .iter()
                .position(|action| *action == MenuAction::CloseSelectedTabs)
        {
            actions.insert(index, MenuAction::ResumeTabClose);
        }
        if creation_only {
            actions.retain(|action| {
                matches!(
                    action,
                    MenuAction::NewWorkspace
                        | MenuAction::WorkspaceAtDirectory
                        | MenuAction::QuickWorkspace
                        | MenuAction::NewWorktree
                        | MenuAction::OpenWorktree
                )
            });
            if matches!(target, Target::Project { .. }) {
                // The exact-Project path also works for an empty Project.
                actions.sort_by_key(|action| *action != MenuAction::QuickWorkspace);
            }
        }
        if !live {
            actions.retain(|a| {
                matches!(
                    a,
                    MenuAction::Help
                        | MenuAction::Machines
                        | MenuAction::Sidebar
                        | MenuAction::ReleasePointer
                        | MenuAction::Detach
                )
            });
        }
        actions = sidebar::actions(&entry, actions, self.menus.discovery());
        if tab_creation {
            actions = vec![
                MenuAction::NewTab,
                MenuAction::NewTabInRecentGroup,
                MenuAction::NewGroupWithTab,
            ];
            actions.extend(
                self.preferences
                    .launch_profiles
                    .profiles
                    .iter()
                    .map(|profile| MenuAction::NewTabWithProfile(Box::new(profile.clone()))),
            );
        }
        let profiles = &self.preferences.launch_profiles;
        if tab_profiles {
            // A click on the button already opens a Tab with the default, which leads the list.
            let mut saved: Vec<_> = profiles.profiles.iter().collect();
            saved.sort_by_key(|profile| profile.id != profiles.default_profile_id);
            actions = saved
                .into_iter()
                .map(|profile| MenuAction::NewTabWithProfile(Box::new(profile.clone())))
                .collect();
        }
        actions.retain(|action| {
            !action_availability(
                self.capabilities,
                state,
                &target,
                action.clone(),
                pointer_released,
                resume_permit.as_ref(),
            )
            .is_hidden()
        });
        // A single profile is the button's own click, so it opens no menu.
        if tab_profiles && actions.len() < 2 {
            return false;
        }
        let search_children = if matches!(&entry, MenuTarget::Global)
            && let Some(drawn) = self.view().tab_create_target()
        {
            let child_actions = [
                MenuAction::NewTab,
                MenuAction::NewTabInRecentGroup,
                MenuAction::NewGroupWithTab,
            ]
            .into_iter()
            .chain(
                self.preferences
                    .launch_profiles
                    .profiles
                    .iter()
                    .map(|profile| MenuAction::NewTabWithProfile(Box::new(profile.clone()))),
            )
            .filter(|action| {
                !action_availability(
                    self.capabilities,
                    state,
                    &target,
                    action.clone(),
                    pointer_released,
                    resume_permit.as_ref(),
                )
                .is_hidden()
            })
            .collect::<Vec<_>>();
            let labels = child_actions
                .iter()
                .map(|action| match action {
                    MenuAction::NewTabInRecentGroup => state
                        .and_then(WorkbenchClientState::recent_tab_group)
                        .map(|group| group.label.clone()),
                    _ => None,
                })
                .collect();
            vec![MenuSearchChild {
                parent: MenuAction::TabCreation,
                source: MenuTarget::TabCreate(drawn),
                actions: child_actions,
                labels,
            }]
        } else {
            Vec::new()
        };
        let intent = Intent::open(
            self.capabilities,
            entry,
            target,
            state,
            pointer_released,
            resume_permit,
            project_choices,
        );
        if !self
            .menus
            .menu
            .open_for(intent.entry().clone(), title, actions, anchor)
        {
            return false;
        }
        self.menus.menu.set_search_children(search_children);
        let choices = sidebar::ChoiceLabels::new(
            self.menus.menu.view().map_or(&[][..], |menu| menu.actions),
            self.menus.discovery(),
        );
        self.menus.menu.set_labels(|action| match action {
            MenuAction::JoinWorkspaceProject(id) => intent
                .project_choices()?
                .iter()
                .find(|project| project.project_id == id)
                .map(|project| project.label.to_string()),
            action @ (MenuAction::OpenWorktreeAt(_) | MenuAction::WorkspaceInFolder(_)) => {
                choices.label(&action)
            }
            MenuAction::NewTabInRecentGroup => state
                .and_then(WorkbenchClientState::recent_tab_group)
                .map(|group| group.label.clone()),
            MenuAction::JoinGroup(index) => state
                .and_then(|state| state.cache().groups().get(usize::from(index)))
                .map(|group| group.label.clone()),
            _ => None,
        });
        if tab_profiles {
            self.menus.menu.set_notes(|action| match action {
                MenuAction::NewTabWithProfile(profile)
                    if profile.id == profiles.default_profile_id =>
                {
                    Some(crate::action_menu::MenuRowNote::DefaultProfile)
                }
                _ => None,
            });
        }
        if let (Some(state), Target::Tab { tab, .. }) = (state, intent.target()) {
            let Some(scope) = super::tab_groups::menu_scope(state, tab, group_menu) else {
                self.menus.menu.cancel();
                return false;
            };
            self.menus.menu.set_tab_presentation(
                scope,
                tab.group().map(|group| group.color),
                |action| super::tab_groups::menu_row(state, tab, action),
            );
            if matches!(group_menu, Some(super::tab_groups::MenuPage::Colors))
                && let Some(group) = tab.group()
            {
                // The applied color holds the candidate however the palette was opened.
                self.menus
                    .menu
                    .select_action(MenuAction::GroupColor(group.color));
            } else if hidden {
                self.menus.menu.hide_candidate();
            }
        } else {
            if let (Some(state), MenuTarget::Global) = (state, intent.entry())
                && let Some(tab) = super::tab_groups::current_tab(state)
            {
                // The Tab section names the selected set that the current Tab belongs to.
                self.menus.menu.set_tab_presentation(
                    crate::action_menu::TabMenuScope::Addressed,
                    None,
                    |action| {
                        Some(
                            super::tab_groups::menu_row(state, tab, action)
                                .unwrap_or(crate::action_menu::TabMenuScope::Addressed),
                        )
                    },
                );
            }
            if hidden {
                self.menus.menu.hide_candidate();
            }
        }
        let (menu, _, discovery) = self.menus.menu_with_facts();
        menu.refresh_availability(|action| {
            visible_unavailable(intent_availability(&intent, state, discovery, action))
        });
        if !self.menus.menu.active() {
            return false;
        }
        self.menus.begin(intent);
        self.prepare_sidebar_worktrees();
        if let Some(state) = self.state.as_mut() {
            state.cancel_project_quick_entry();
        }
        true
    }

    /// Routes one key only if the central input owner captured this exact menu snapshot.
    ///
    /// A false result means that driving current state retired or revised the supplied snapshot.
    /// The central owner still consumes that stale key instead of falling through to another
    /// feature. A true result means this menu snapshot owned and consumed the key, even when its
    /// current phase deliberately admits no state change.
    pub(in crate::workbench_client) fn capture_action_menu_key_for(
        &mut self,
        expected: MenuInputContext,
        event: KeyEvent,
    ) -> bool {
        self.drive_action_menu();
        let Some(current) = self.action_menu_input_context() else {
            return false;
        };
        if current != expected {
            return false;
        }

        let is_press = event.kind != crate::client_view::KeyKind::Release
            && event.mods == crate::client_view::Modifiers::NONE;
        if is_press
            && (event.code == crate::client_view::KeyCode::Esc
                || (current.readiness() != MenuInputReadiness::Recovery
                    && event.code == crate::client_view::KeyCode::Left))
            && matches!(self.menus.intent().map(Intent::stage), Some(Stage::Open))
            && let Some(ParentMenu {
                target,
                anchor,
                selected,
                query,
                resume_permit,
                retained: _,
            }) = self.menus.pop_parent()
        {
            let released = self.menus.intent().is_some_and(Intent::pointer_released);
            self.menus.menu.cancel();
            self.menus.release_intent();
            // Returning names the row that opened the submenu as the candidate.
            if self.open_action_menu_with_resume_permit(
                target,
                anchor,
                released,
                ResumePermitSeed::Inherit(resume_permit),
                false,
            ) {
                self.menus.menu.set_query(query.as_deref());
                self.menus.menu.select_action(selected);
            } else {
                self.menus.clear_parents();
            }
            return true;
        }

        // Escape is always a local exit, including while the retained surface is pending,
        // suspended, or transferring an already confirmed action.
        if is_press && event.code == crate::client_view::KeyCode::Esc {
            let _ = self.menus.menu.key(event);
            if !self.action_menu_active() {
                self.menus.end();
            }
            return true;
        }

        // Recovery retains ownership without an ordinary editing lease. Pending, suspended,
        // covered, reopened-parent and transfer states therefore swallow everything except Esc.
        if current.readiness() == MenuInputReadiness::Recovery
            || !matches!(self.menus.intent().map(Intent::stage), Some(Stage::Open))
        {
            return true;
        }

        if self.menus.menu.key(event) {
            self.confirm_action_menu();
        }
        if !self.action_menu_active() {
            self.menus.end();
        }
        true
    }

    #[cfg(test)]
    pub(in crate::workbench_client) fn capture_action_menu_key(&mut self, event: KeyEvent) {
        let Some(expected) = self.action_menu_input_context() else {
            return;
        };
        let _ = self.capture_action_menu_key_for(expected, event);
    }

    fn action_menu_source_matches(&self, source: &MenuTarget, token: MenuToken) -> bool {
        self.view()
            .action_menu()
            .is_some_and(|menu| menu.source == source && menu.token == token)
    }

    pub(in crate::workbench_client) fn activate_action_menu_control(
        &mut self,
        source: &MenuTarget,
        token: MenuToken,
        control: MenuControl,
    ) {
        self.drive_action_menu();
        if !self.action_menu_source_matches(source, token) {
            return;
        }
        match control {
            MenuControl::Back => {
                let Some(expected) = self.action_menu_input_context() else {
                    return;
                };
                let _ = self.capture_action_menu_key_for(
                    expected,
                    KeyEvent::press(
                        crate::client_view::KeyCode::Left,
                        crate::client_view::Modifiers::NONE,
                    ),
                );
            }
            MenuControl::Close => self.cancel_action_menu(),
        }
    }

    pub(in crate::workbench_client) fn hover_action_menu_row(
        &mut self,
        source: &MenuTarget,
        token: MenuToken,
        row: usize,
    ) -> bool {
        self.drive_action_menu();
        self.action_menu_source_matches(source, token) && self.menus.menu.hover(token, row)
    }

    pub(in crate::workbench_client) fn activate_action_menu_row(
        &mut self,
        source: &MenuTarget,
        token: MenuToken,
        row: usize,
    ) {
        self.drive_action_menu();
        if self.action_menu_source_matches(source, token) && self.menus.menu.click(token, row) {
            self.confirm_action_menu();
        }
    }

    pub(in crate::workbench_client) fn scroll_action_menu(
        &mut self,
        token: MenuToken,
        up: bool,
        lines: usize,
    ) -> Option<MenuToken> {
        self.drive_action_menu();
        self.menus.menu.scroll(token, up, lines)
    }

    fn confirm_action_menu(&mut self) {
        if !matches!(self.menus.intent().map(Intent::stage), Some(Stage::Open)) {
            return;
        }
        let Some(deadline) = self.clock.now().checked_add(Duration::from_secs(30)) else {
            self.fail_action_menu();
            return;
        };
        self.menus.confirm(deadline);
        self.drive_action_menu();
    }

    fn settle_blocked_action_menu_geometry(&mut self) {
        match self.menus.menu.geometry_blocked() {
            Some(GeometrySettlement::Refused) => {
                self.menus.end();
                self.note_action_menu_unavailable();
            }
            Some(GeometrySettlement::Suspended) | None => {}
        }
    }

    pub(super) fn after_action_menu_draw(&mut self, draw: Option<ModalPresentation<MenuDraw>>) {
        if !self.action_menu_capture_claim() {
            return;
        }
        match draw {
            Some(ModalPresentation::Presented(draw)) => {
                let _ = self.menus.menu.presented(draw);
            }
            Some(ModalPresentation::NotPresented) => self.settle_blocked_action_menu_geometry(),
            // No verdict means that another owner covered this surface. Revoke exact activation
            // proof without reclassifying the retained menu's geometry lifecycle.
            None => self.menus.menu.not_presented(),
        }
    }

    fn target_valid(&self, target: &Target) -> bool {
        let Some(state) = self.state.as_ref() else {
            return matches!(target, Target::Global);
        };
        if !matches!(target, Target::Global) && !self.menu_carrier_live() {
            return false;
        }
        match target {
            Target::Global => true,
            Target::Pane {
                pane,
                presentation,
                source,
            } => {
                state
                    .pane_by_panel(pane.pane().panel())
                    .is_some_and(|record| record.pane_ref() == *pane)
                    && selected_presentation(state, pane).as_ref() == Some(presentation)
                    && state
                        .cache()
                        .menu_context_source(state.cache().location())
                        .as_ref()
                        == Some(source)
            }
            Target::Tab {
                location,
                tab,
                source,
                resolver,
            } => {
                state.cache().location() == location
                    && state.cache().menu_context_source(location).as_ref() == Some(source)
                    && resolver.matches(state.cache().tabs())
                    && state
                        .cache()
                        .tab_index(tab.tab_id())
                        .and_then(|i| state.cache().tabs().get(i))
                        == Some(tab.as_ref())
            }
            Target::Workspace { location, source } => {
                state.cache().menu_context_source(location).as_ref() == Some(source)
            }
            Target::Project { project, context } => {
                state
                    .cache()
                    .projects()
                    .iter()
                    .find(|candidate| candidate.project_id == project.project_id)
                    == Some(project.as_ref())
                    && context.as_ref().is_none_or(|context| {
                        state
                            .cache()
                            .menu_context_source(&context.location)
                            .as_ref()
                            == Some(&context.source)
                    })
            }
        }
    }

    fn menu_target_selected(&self, target: &Target, action: MenuAction) -> bool {
        match target {
            Target::Global => true,
            Target::Tab { .. }
                if matches!(action.client_action(), Some(ClientAction::TabMenu { .. })) =>
            {
                true
            }
            Target::Project { .. }
                if matches!(
                    action,
                    MenuAction::QuickWorkspace
                        | MenuAction::WorkspaceAtDirectory
                        | MenuAction::DeleteProject
                        | MenuAction::WorkspaceCreation
                        | MenuAction::WorkspaceInFolder(_)
                        | MenuAction::NewWorktree
                        | MenuAction::OpenWorktreeAt(_)
                        | MenuAction::RenameProject
                        | MenuAction::CloseProjectWorkspaces
                ) =>
            {
                true
            }
            Target::Project { context, .. } => context.as_ref().is_some_and(|context| {
                self.state
                    .as_ref()
                    .is_some_and(|state| state.cache().location() == &context.location)
            }),
            Target::Workspace { .. }
                if matches!(
                    action,
                    MenuAction::CloseWorkspace
                        | MenuAction::WorkspaceProjects
                        | MenuAction::CreateWorkspaceProject
                        | MenuAction::LeaveWorkspaceProject
                        | MenuAction::JoinWorkspaceProject(_)
                        | MenuAction::DeleteWorkspace
                        | MenuAction::RemoveWorktree
                ) =>
            {
                true
            }
            Target::Workspace { location, .. } => self
                .state
                .as_ref()
                .is_some_and(|state| state.cache().location() == location),
            Target::Tab { location, tab, .. } => self.state.as_ref().is_some_and(|state| {
                state.cache().location() == location
                    && state
                        .desired()
                        .is_some_and(|key| key.anchor().tab_id() == tab.tab_id())
            }),
            Target::Pane { pane, .. } => self.state.as_ref().is_some_and(|state| {
                state.focus_pane_ref().as_ref() == Some(pane)
                    && state.desired() == Some(pane.pane())
            }),
        }
    }

    pub(super) fn drive_action_menu(&mut self) -> bool {
        if self.action_menu_active()
            && (self.modal_owns_input()
                || self.copy_mode_active()
                || self.popup.captured()
                || self.board.is_open())
        {
            self.fail_action_menu();
            return true;
        }
        let Some(mut intent) = self.menus.take_intent() else {
            return false;
        };
        let current = Origin::capture(self.state.as_ref());
        let open = matches!(intent.stage(), Stage::Open | Stage::Confirmed(_));
        if intent
            .stage()
            .deadline()
            .is_some_and(|deadline| self.clock.now() >= deadline)
            || !self.target_valid(intent.target())
            || !self.sidebar_worktree_source_current()
            || intent.project_choices().is_some_and(|choices| {
                self.state
                    .as_ref()
                    .and_then(|state| sidebar::project_choices(state, intent.target()))
                    .is_none_or(|current| current.0.as_slice() != choices)
            })
            || (open && current != *intent.origin())
        {
            self.fail_action_menu();
            return true;
        }
        let mut changed = false;
        match intent.stage() {
            Stage::Open => {
                if self.menus.menu.view().is_some_and(|menu| {
                    menu.actions.iter().any(|action| {
                        intent_availability(
                            &intent,
                            self.state.as_ref(),
                            self.menus.discovery(),
                            action.clone(),
                        )
                        .is_hidden()
                    })
                }) {
                    self.fail_action_menu();
                    return true;
                }
                let state = self.state.as_ref();
                let (menu, _, discovery) = self.menus.menu_with_facts();
                changed = menu.refresh_availability(|action| {
                    visible_unavailable(intent_availability(&intent, state, discovery, action))
                });
            }
            Stage::Confirmed(deadline) => {
                let deadline = *deadline;
                if self.menus.menu.confirmed().is_some_and(|action| {
                    !intent_availability(
                        &intent,
                        self.state.as_ref(),
                        self.menus.discovery(),
                        action,
                    )
                    .is_enabled()
                }) {
                    self.fail_action_menu();
                    return true;
                }
                let next = match intent.target() {
                    Target::Global => Some(Stage::Offered(current.clone(), deadline)),
                    Target::Project { .. } => self
                        .menus
                        .menu
                        .confirmed()
                        .filter(|action| self.menu_target_selected(intent.target(), action.clone()))
                        .is_some()
                        .then(|| Stage::Offered(current.clone(), deadline)),
                    Target::Workspace { location, .. }
                        if matches!(
                            self.menus.menu.confirmed(),
                            Some(
                                MenuAction::CloseWorkspace
                                    | MenuAction::WorkspaceProjects
                                    | MenuAction::CreateWorkspaceProject
                                    | MenuAction::LeaveWorkspaceProject
                                    | MenuAction::JoinWorkspaceProject(_)
                                    | MenuAction::DeleteWorkspace
                                    | MenuAction::RemoveWorktree
                            )
                        ) || self
                            .state
                            .as_ref()
                            .is_some_and(|s| s.cache().location() == location) =>
                    {
                        Some(Stage::Offered(current.clone(), deadline))
                    }
                    Target::Workspace { location, .. } => {
                        let index = self.state.as_ref().and_then(|s| {
                            s.cache()
                                .navigation()
                                .iter()
                                .position(|c| &c.location == location)
                        });
                        index
                            .and_then(|i| self.select_context_at_generation(i))
                            .map(|generation| Stage::Context {
                                generation,
                                prepared: false,
                                deadline,
                            })
                    }
                    Target::Tab { .. }
                        if self.menus.menu.confirmed().is_some_and(|action| {
                            matches!(action.client_action(), Some(ClientAction::TabMenu { .. }))
                        }) =>
                    {
                        Some(Stage::Offered(current.clone(), deadline))
                    }
                    Target::Tab { tab, .. }
                        if self
                            .state
                            .as_ref()
                            .and_then(|s| s.desired())
                            .is_some_and(|d| d.anchor().tab_id() == tab.tab_id()) =>
                    {
                        Some(Stage::Offered(current.clone(), deadline))
                    }
                    Target::Tab { tab, .. } => {
                        let index = self
                            .state
                            .as_ref()
                            .and_then(|s| s.cache().tab_index(tab.tab_id()));
                        index
                            .and_then(|i| u8::try_from(i + 1).ok())
                            .and_then(|p| self.select_tab_generation(ClientAction::SelectTab(p)))
                            .map(|generation| Stage::Tab {
                                generation,
                                prepared: false,
                                deadline,
                            })
                    }
                    Target::Pane { pane, .. } => {
                        let exact = self.state.as_ref().and_then(|s| s.focus_pane_ref())
                            == Some(pane.clone());
                        (exact
                            || self
                                .move_panel_focus_admitted(pane.pane().panel())
                                .is_some())
                        .then(|| Stage::Offered(Origin::capture(self.state.as_ref()), deadline))
                    }
                };
                let Some(next) = next else {
                    self.fail_action_menu();
                    return true;
                };
                intent.advance(next);
                changed = true;
            }
            Stage::Wait(expected, deadline) => {
                let deadline = *deadline;
                if current != *expected {
                    self.fail_action_menu();
                    return true;
                }
                if self.state.as_ref().is_none_or(|s| {
                    matches!(
                        s.phase_kind(),
                        ClientPhaseKind::Empty
                            | ClientPhaseKind::Ready
                            | ClientPhaseKind::SelectedWithoutAttachment
                            | ClientPhaseKind::RetryableAttachUnavailable
                    )
                }) {
                    intent.advance(Stage::Offered(current.clone(), deadline));
                    changed = true;
                }
            }
            Stage::Offered(expected, _) if current != *expected => {
                self.fail_action_menu();
                return true;
            }
            _ => {}
        }
        self.menus.restore_intent(intent);
        changed
    }

    pub(in crate::workbench_client) fn take_action_menu_action(
        &mut self,
        pointer_released: bool,
    ) -> Option<crate::workbench_client::InputAction> {
        self.drive_action_menu();
        let intent = self.menus.intent()?;
        if !matches!(intent.stage(), Stage::Offered(_, _)) {
            return None;
        }
        let action = self.menus.menu.confirmed()?;
        if !self.target_valid(intent.target())
            || !self.menu_target_selected(intent.target(), action.clone())
            || sidebar::unavailable(intent.entry(), self.menus.discovery(), &action).is_some()
            || !action_availability(
                self.capabilities,
                self.state.as_ref(),
                intent.target(),
                action.clone(),
                pointer_released,
                intent.resume_permit(),
            )
            .is_enabled()
            || (action == MenuAction::Zoom
                && self
                    .state
                    .as_ref()
                    .is_some_and(WorkbenchClientState::zoom_active)
                    != intent.zoomed())
            || (action == MenuAction::ReleasePointer
                && (pointer_released || pointer_released != intent.pointer_released()))
        {
            self.fail_action_menu();
            return None;
        }
        let child = match (intent.target(), &action) {
            (_, MenuAction::TabCreation) => {
                Some(MenuTarget::TabCreate(self.view().tab_create_target()?))
            }
            (Target::Workspace { location, .. }, MenuAction::WorkspaceProjects) => {
                Some(MenuTarget::WorkspaceProjects(location.clone()))
            }
            (Target::Project { project, .. }, MenuAction::WorkspaceCreation) => {
                Some(MenuTarget::WorkspaceCreation(project.project_id.clone()))
            }
            (Target::Tab { tab, .. }, MenuAction::ManageGroups) => {
                Some(MenuTarget::TabGroupChoices(tab.tab_id().clone()))
            }
            (Target::Tab { tab, .. }, MenuAction::ChooseGroupColor) => {
                Some(MenuTarget::TabGroups {
                    tab_id: tab.tab_id().clone(),
                    colors: true,
                })
            }
            _ => None,
        };
        if let Some(child) = child {
            self.open_child_action_menu(child, action, pointer_released);
            return None;
        }
        if matches!(
            action,
            MenuAction::NewWorktree | MenuAction::OpenWorktreeAt(_) | MenuAction::RemoveWorktree
        ) && matches!(
            intent.entry(),
            MenuTarget::WorkspaceCreation(_) | MenuTarget::Create(_) | MenuTarget::Workspace(_)
        ) {
            let discovery = self.menus.discovery()?.ready()?.clone();
            let checkout = match &action {
                MenuAction::OpenWorktreeAt(path) => Some(path.clone()),
                _ => None,
            };
            self.menus.menu.take()?;
            self.menus.end();
            if action == MenuAction::RemoveWorktree {
                self.remove_sidebar_worktree(discovery);
            } else {
                self.open_sidebar_worktree(discovery, checkout);
            }
            return None;
        }
        let resume_handoff = match (
            intent.target(),
            action.clone(),
            intent.resume_permit(),
            intent.stage().deadline(),
        ) {
            (
                Target::Tab {
                    location,
                    tab,
                    source,
                    ..
                },
                MenuAction::ResumeTabClose,
                Some(permit),
                Some(deadline),
            ) => Some(ResumeHandoff {
                deadline,
                location: location.clone(),
                source: source.clone(),
                tab: tab.clone(),
                permit: permit.clone(),
            }),
            _ => None,
        };
        let action = match (intent.target(), action) {
            (
                Target::Workspace {
                    location: ClientContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                MenuAction::DeleteWorkspace,
            ) if self
                .state
                .as_ref()?
                .can_confirm_guarded_workspace_close(workspace_id) =>
            {
                ClientAction::RequestWorkspaceCloseAt(workspace_id.clone()).into()
            }
            (
                Target::Workspace {
                    location: ClientContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                MenuAction::DeleteWorkspace,
            ) => crate::workbench_client::InputAction::sidebar_workspace_delete(Box::new(
                crate::workbench_state::workspace_close::WorkspaceDeleteRequest::capture(
                    self.state.as_ref()?,
                    workspace_id,
                    intent.stage().deadline()?,
                )?,
            )),
            (
                Target::Workspace {
                    location: ClientContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                choice @ (MenuAction::CreateWorkspaceProject
                | MenuAction::LeaveWorkspaceProject
                | MenuAction::JoinWorkspaceProject(_)),
            ) => {
                use crate::workbench_state::workspace_setup::OrganizationChoice;
                let choice = match choice {
                    MenuAction::CreateWorkspaceProject => OrganizationChoice::NewProject,
                    MenuAction::LeaveWorkspaceProject => OrganizationChoice::SetProject(None),
                    MenuAction::JoinWorkspaceProject(id) => {
                        OrganizationChoice::SetProject(Some(id))
                    }
                    _ => unreachable!(),
                };
                let request = crate::workbench_state::workspace_setup::WorkspaceOrganizationRequest::workspace(
                    self.state.as_ref()?,
                    workspace_id,
                    intent.stage().deadline()?,
                )?
                .with_choice(choice)?;
                crate::workbench_client::InputAction::workspace_organization(Box::new(request))
            }
            (Target::Project { project, .. }, MenuAction::DeleteProject) => {
                let request =
                    crate::workbench_state::workspace_setup::WorkspaceOrganizationRequest::project(
                        self.state.as_ref()?,
                        project,
                        intent.stage().deadline()?,
                    )?
                    .with_choice(
                        crate::workbench_state::workspace_setup::OrganizationChoice::Dissolve,
                    )?;
                crate::workbench_client::InputAction::workspace_organization(Box::new(request))
            }
            (
                Target::Workspace {
                    location: ClientContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                MenuAction::CloseWorkspace,
            ) => ClientAction::RequestWorkspaceCloseAt(workspace_id.clone()).into(),
            (Target::Project { project, .. }, MenuAction::CloseProjectWorkspaces) => {
                ClientAction::RequestProjectCloseAll(Some(project.project_id.clone())).into()
            }
            (Target::Tab { tab, .. }, action)
                if matches!(action.client_action(), Some(ClientAction::TabMenu { .. })) =>
            {
                ClientAction::TabMenu {
                    action,
                    tab_id: Some(tab.tab_id().clone()),
                }
                .into()
            }
            (
                Target::Project { project, .. },
                action @ (MenuAction::QuickWorkspace
                | MenuAction::WorkspaceAtDirectory
                | MenuAction::WorkspaceInFolder(_)),
            ) => {
                let directory = match &action {
                    MenuAction::WorkspaceInFolder(directory) => Some(directory.clone()),
                    _ => None,
                };
                let request = crate::workbench_state::workspace_setup::quick::ProjectQuickWorkspaceRequest::capture(
                    self.state.as_ref()?,
                    project,
                    directory,
                    intent.stage().deadline()?,
                )?;
                if action == MenuAction::WorkspaceAtDirectory {
                    crate::workbench_client::InputAction::workspace_at_directory(Box::new(request))
                } else {
                    crate::workbench_client::InputAction::project_quick_workspace(Box::new(request))
                }
            }
            (Target::Project { project, .. }, MenuAction::RenameProject) => {
                let request =
                    crate::workbench_state::workspace_setup::ProjectRenameRequest::capture(
                        self.state.as_ref()?,
                        project,
                        intent.stage().deadline()?,
                    )?;
                crate::workbench_client::InputAction::targeted_project_rename(Box::new(request))
            }
            (_, action) => action.client_action()?.into(),
        };
        self.menus.menu.take()?;
        self.menus.finish(resume_handoff);
        Some(action)
    }

    /// Opens a submenu in place of its confirmed row. A context menu stays painted beside it;
    /// the global menu's page is replaced and keeps its query for the return.
    fn open_child_action_menu(
        &mut self,
        child: MenuTarget,
        selected: MenuAction,
        pointer_released: bool,
    ) {
        let Some(intent) = self.menus.intent() else {
            return;
        };
        let Some(menu) = self.menus.menu.view() else {
            return;
        };
        let cascade = intent.entry().context_menu();
        let parent = ParentMenu {
            target: intent.entry().clone(),
            anchor: menu.anchor,
            selected,
            query: self.menus.menu.query().map(str::to_owned),
            resume_permit: intent.resume_permit().cloned(),
            retained: if cascade {
                self.menus.menu.retain()
            } else {
                None
            },
        };
        // A click leaves the candidate to the pointer; a key starts on the first row.
        let hidden = self.menus.menu.confirmed_by_pointer();
        if self.menus.menu.take().is_none() {
            return;
        }
        self.menus.release_intent();
        if self.open_action_menu_with_resume_permit(
            child,
            parent.anchor,
            pointer_released,
            ResumePermitSeed::CaptureDisplayed,
            hidden,
        ) {
            if !cascade {
                self.menus.menu.set_query(parent.query.as_deref());
            }
            self.menus.push_parent(parent);
        } else {
            self.menus.clear_parents();
        }
    }

    pub(super) fn take_action_menu_resume_handoff(
        &mut self,
        action: &ClientAction,
    ) -> TabCloseResumeHandoff {
        let Some(handoff) = self.menus.take_resume_handoff() else {
            return TabCloseResumeHandoff::Direct;
        };
        let ClientAction::TabMenu {
            action: MenuAction::ResumeTabClose,
            tab_id,
        } = action
        else {
            return TabCloseResumeHandoff::Direct;
        };
        let Some(tab_id) = tab_id else {
            return TabCloseResumeHandoff::Refused;
        };
        let Some(state) = self.state.as_ref() else {
            return TabCloseResumeHandoff::Refused;
        };
        if self.clock.now() >= handoff.deadline
            || !self.menu_carrier_live()
            || state.cache().location() != &handoff.location
            || state
                .cache()
                .menu_context_source(&handoff.location)
                .as_ref()
                != Some(&handoff.source)
            || state
                .cache()
                .tab_index(tab_id)
                .and_then(|index| state.cache().tabs().get(index))
                != Some(handoff.tab.as_ref())
            || !state.validate_tab_close_resume_permit(tab_id, &handoff.permit)
        {
            return TabCloseResumeHandoff::Refused;
        }
        TabCloseResumeHandoff::Menu(handoff.permit)
    }

    pub(super) fn menu_navigation_prepared(&mut self, navigation: NavigationKey, accepted: bool) {
        if !self
            .menus
            .intent()
            .is_some_and(|intent| intent.owns_navigation(navigation))
        {
            return;
        }
        if accepted {
            self.menus.mark_navigation_prepared();
        } else {
            self.fail_action_menu();
        }
    }

    pub(super) fn menu_navigation_installed(&mut self, navigation: NavigationKey) {
        let Some(intent) = self.menus.intent() else {
            return;
        };
        if !intent.navigation_prepared(navigation) {
            return;
        }
        let selected = self
            .state
            .as_ref()
            .is_some_and(|state| match intent.target() {
                Target::Workspace { location, .. } => state.cache().location() == location,
                Target::Tab { location, tab, .. } => {
                    state.cache().location() == location
                        && state
                            .desired()
                            .is_some_and(|d| d.anchor().tab_id() == tab.tab_id())
                }
                _ => false,
            });
        if !selected {
            self.fail_action_menu();
            return;
        }
        let expected = Origin::capture(self.state.as_ref());
        self.menus.await_origin(expected);
    }

    pub(super) fn fence_menu_context_candidate(
        &self,
        prepared: crate::workbench_state::PreparedRefresh,
        generation: u64,
        baseline_revision: orchbee_workbench_protocol::WorkbenchRevision,
    ) -> crate::workbench_state::PreparedRefresh {
        let Some(intent) = self.menus.intent() else {
            return prepared;
        };
        let (
            Target::Workspace { location, source },
            Stage::Context {
                generation: owned, ..
            },
        ) = (intent.target(), intent.stage())
        else {
            return prepared;
        };
        if *owned != generation {
            return prepared;
        }
        match &prepared {
            crate::workbench_state::PreparedRefresh::Membership {
                prepared: candidate,
                ..
            } if candidate.context_location() == location
                && candidate.menu_context_source().as_ref() == Some(source) =>
            {
                prepared
            }
            crate::workbench_state::PreparedRefresh::Membership { .. } => {
                crate::workbench_state::PreparedRefresh::DefiniteFailure {
                    generation,
                    baseline_revision,
                }
            }
            _ => prepared,
        }
    }

    pub(super) fn menu_switch_candidate_valid(
        &self,
        generation: u64,
        candidate: &crate::workbench_state::PreparedMembershipCandidate,
    ) -> bool {
        let Some(intent) = self.menus.intent() else {
            return true;
        };
        let (
            Target::Tab {
                location,
                tab,
                source,
                ..
            },
            Stage::Tab {
                generation: owned, ..
            },
        ) = (intent.target(), intent.stage())
        else {
            return true;
        };
        *owned != generation
            || (candidate.context_location() == location
                && candidate.menu_selected_tab_matches(tab)
                && candidate.menu_context_source().as_ref() == Some(source))
    }
}
