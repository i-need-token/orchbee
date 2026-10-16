//! One local menu intent; existing membership and action owners retain all execution authority.
use super::{
    ClientAction, Geometry, ModalPresentation, NativeSessionNotice, NativeSessionState,
    NativeSessionView, NativeUi, PaneKey, PaneRef, SessionPhaseKind,
};
use crate::action_menu::{
    ActionMenu, DrawnMenuSource, MenuAction, MenuControl, MenuDraw, MenuInputContext,
    MenuInputReadiness, MenuTarget, MenuToken, MenuUnavailable, MenuView,
};
use crate::action_projection::ActionAvailability;
use crate::client_view::KeyEvent;
use crate::terminal_session::state::TabCloseResumePermit;
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, NativeMenuContextSource, NativeSimpleTab,
};
use crate::tui_presentation::GeometrySettlement;
use std::time::{Duration, Instant};
mod sidebar;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Origin {
    Unavailable,
    Context(Box<ContextOrigin>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContextOrigin {
    selected: Vec<NativeSimpleTab>,
    groups: Vec<orchbee_workbench_protocol::TabGroup>,
    site: orchbee_workbench_protocol::SiteId,
    location: NativeContextLocation,
    desired: Option<PaneKey>,
    focus: Option<PaneRef>,
    tab: Option<NativeSimpleTab>,
    source: Option<NativeMenuContextSource>,
}

impl Origin {
    fn capture(state: Option<&NativeSessionState>) -> Self {
        state.map_or(Self::Unavailable, |state| {
            Self::Context(Box::new(ContextOrigin {
                selected: state
                    .selected_tab_ids()
                    .iter()
                    .filter_map(|id| state.cache().tab_index(id))
                    .filter_map(|index| state.cache().tabs().get(index))
                    .cloned()
                    .collect(),
                groups: state.cache().groups().to_vec(),
                site: state.cache().site_id().clone(),
                location: state.cache().location().clone(),
                desired: state.desired().cloned(),
                focus: state.focus_pane_ref(),
                tab: state
                    .desired()
                    .and_then(|key| state.cache().tab_index(key.anchor().tab_id()))
                    .and_then(|i| state.cache().tabs().get(i))
                    .cloned(),
                source: state.cache().menu_context_source(state.cache().location()),
            }))
        })
    }
}

fn selected_presentation(
    state: &NativeSessionState,
    pane: &PaneRef,
) -> Option<orchbee_workbench_protocol::TerminalPresentationId> {
    let tab = state
        .cache()
        .tabs()
        .get(state.cache().tab_index(pane.anchor().tab_id())?)?;
    let panel = tab.panel(tab.panel_index(pane.pane().panel())?)?;
    Some(state.selected_presentation(panel).presentation_id().clone())
}

#[derive(Clone)]
enum Target {
    Global,
    Pane {
        pane: PaneRef,
        presentation: orchbee_workbench_protocol::TerminalPresentationId,
        source: NativeMenuContextSource,
    },
    Tab {
        location: NativeContextLocation,
        tab: Box<NativeSimpleTab>,
        source: NativeMenuContextSource,
    },
    Workspace {
        location: NativeContextLocation,
        source: NativeMenuContextSource,
    },
    Project {
        project: Box<orchbee_workbench_protocol::Project>,
        context: Option<Box<ProjectContext>>,
    },
}

#[derive(Clone)]
struct ProjectContext {
    location: NativeContextLocation,
    source: NativeMenuContextSource,
}

fn action_availability(
    state: Option<&NativeSessionState>,
    target: &Target,
    action: MenuAction,
    pointer_released: bool,
    resume_permit: Option<&TabCloseResumePermit>,
) -> ActionAvailability<MenuUnavailable> {
    use super::super::state::workspace_setup::DirectoryConfigurationUnavailable;
    if action == MenuAction::MoveToNewWindow {
        return ActionAvailability::Hidden;
    }
    if matches!(
        action,
        MenuAction::NewGroupWithTab | MenuAction::NewTabInRecentGroup
    ) && state.is_some_and(|state| {
        matches!(
            state.cache().location(),
            NativeContextLocation::Standalone(_)
        ) && state.cache().tabs().is_empty()
    }) {
        return ActionAvailability::Disabled(MenuUnavailable::WorkspaceRequired);
    }
    if matches!(
        target,
        Target::Workspace {
            source: NativeMenuContextSource::Standalone,
            ..
        }
    ) && matches!(
        action,
        MenuAction::WorkspaceDirectory
            | MenuAction::RenameWorkspace
            | MenuAction::MoveWorkspace
            | MenuAction::CloseWorkspace
    ) {
        return ActionAvailability::Disabled(MenuUnavailable::LegacyWorkspace);
    }
    if action == MenuAction::NewTabInRecentGroup
        && state
            .and_then(NativeSessionState::recent_tab_group)
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
    if let (Some(state), Target::Tab { tab, .. }) = (state, target)
        && super::tab_groups::close_tab_ids(state, tab.tab_id(), action.clone())
            .is_some_and(|ids| ids.is_empty())
    {
        return ActionAvailability::Disabled(MenuUnavailable::NoMatchingTabs);
    }
    if matches!(target, Target::Project { .. }) && action == MenuAction::QuickWorkspace {
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
        return ActionAvailability::from_disabled_reason(
            state
                .is_none_or(|state| !state.close_scope_available())
                .then_some(MenuUnavailable::OperationBusy),
        );
    }
    if (matches!(target, Target::Project { .. })
        && matches!(
            action,
            MenuAction::RenameProject | MenuAction::DeleteProject
        ))
        || matches!(
            action,
            MenuAction::MoveWorkspace
                | MenuAction::WorkspaceProjects
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
    if matches!(
        action,
        MenuAction::WorkspaceWorktrees | MenuAction::OpenWorktreeAt(_) | MenuAction::NewWorktree
    ) && matches!(target, Target::Workspace { .. } | Target::Project { .. })
    {
        return ActionAvailability::from_disabled_reason(
            state
                .and_then(NativeSessionState::worktree_origin)
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
    if matches!(
        source,
        NativeMenuContextSource::Workspace {
            directory: Some(_),
            ..
        }
    ) {
        return ActionAvailability::Disabled(MenuUnavailable::DirectoryConfigured);
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

enum Stage {
    Open,
    Confirmed(Instant),
    Context {
        generation: u64,
        prepared: bool,
        deadline: Instant,
    },
    Tab {
        generation: u64,
        prepared: bool,
        deadline: Instant,
    },
    Wait(Origin, Instant),
    Offered(Origin, Instant),
}

impl Stage {
    fn deadline(&self) -> Option<Instant> {
        match self {
            Self::Open => None,
            Self::Confirmed(deadline)
            | Self::Context { deadline, .. }
            | Self::Tab { deadline, .. }
            | Self::Wait(_, deadline)
            | Self::Offered(_, deadline) => Some(*deadline),
        }
    }
}

/// Refresh and attachment generations belong to separate monotonic domains.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum NavigationKey {
    Context(u64),
    Tab(u64),
}

struct Intent {
    entry: MenuTarget,
    target: Target,
    origin: Origin,
    stage: Stage,
    zoomed: bool,
    pointer_released: bool,
    resume_permit: Option<TabCloseResumePermit>,
    project_choices: Option<Vec<orchbee_workbench_protocol::Project>>,
}

#[derive(Default)]
pub(super) struct Coordinator {
    pub(super) menu: ActionMenu,
    intent: Option<Intent>,
    geometry: Option<Geometry>,
    parents: Vec<ParentMenu>,
    resume_handoff: Option<ResumeHandoff>,
    sidebar: Option<sidebar::Episode>,
    next_sidebar: u64,
}

struct ParentMenu {
    target: MenuTarget,
    anchor: Option<(u16, u16)>,
    selected: MenuAction,
    resume_permit: Option<TabCloseResumePermit>,
}

enum ResumePermitSeed {
    CaptureDisplayed,
    Inherit(Option<TabCloseResumePermit>),
}

struct ResumeHandoff {
    deadline: Instant,
    location: NativeContextLocation,
    source: NativeMenuContextSource,
    tab: Box<NativeSimpleTab>,
    permit: TabCloseResumePermit,
}

pub(super) enum TabCloseResumeHandoff {
    Direct,
    Menu(TabCloseResumePermit),
    Refused,
}

impl Coordinator {
    pub(super) fn view(&self) -> Option<MenuView<'_>> {
        self.intent.as_ref()?;
        self.menu.view().map(|mut menu| {
            menu.has_parent = !self.parents.is_empty();
            if let Some(parent) = self.parents.first() {
                menu.context_menu = parent.target.context_menu();
            }
            menu
        })
    }

    fn input_context(&self) -> Option<MenuInputContext> {
        let intent = self.intent.as_ref()?;
        let context = self.menu.input_context()?;
        Some(if matches!(intent.stage, Stage::Open) {
            context
        } else {
            context.recovery_only()
        })
    }
}

impl NativeSessionView<'_> {
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
        self.menu.and_then(Coordinator::view)
    }
    pub(crate) fn action_menu_input_context(&self) -> Option<MenuInputContext> {
        self.menu.and_then(Coordinator::input_context)
    }
    pub(crate) fn action_menu_source(&self) -> Option<DrawnMenuSource> {
        if self.retirement.is_some() {
            return None;
        }
        let state = self.state.as_ref()?;
        Some(state.action_menu_source())
    }
}

impl NativeUi {
    /// Revalidate the drawn context before delegating to the existing create admission.
    pub(in crate::terminal_session) fn tab_create_action(
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
            self.command_notice = self
                .clock
                .now()
                .checked_add(Duration::from_secs(5))
                .map(|until| (NativeSessionNotice::MenuUnavailable, until));
            return None;
        }
        Some(ClientAction::CreateTab)
    }
    pub(in crate::terminal_session) fn action_menu_capture_claim(&self) -> bool {
        self.view().action_menu_input_context().is_some()
    }
    pub(in crate::terminal_session) fn action_menu_input_context(
        &self,
    ) -> Option<MenuInputContext> {
        self.view().action_menu_input_context()
    }
    pub(in crate::terminal_session) fn action_menu_active(&self) -> bool {
        self.action_menu_capture_claim()
    }

    pub(super) fn observe_action_menu_geometry(&mut self, geometry: Geometry) {
        if self
            .menus
            .geometry
            .replace(geometry)
            .is_some_and(|old| old != geometry)
        {
            self.menus.menu.not_presented();
        }
    }

    fn menu_carrier_live(&self) -> bool {
        self.retirement_failure.is_none()
            && self.state.as_ref().is_some_and(|state| {
                state.phase_kind() != SessionPhaseKind::Retired && state.retired_failure().is_none()
            })
    }

    pub(in crate::terminal_session) fn cancel_action_menu(&mut self) {
        self.menus.menu.cancel();
        self.menus.intent = None;
        self.menus.parents.clear();
        self.menus.resume_handoff = None;
        self.menus.sidebar = None;
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
        self.command_notice = self
            .clock
            .now()
            .checked_add(std::time::Duration::from_secs(5))
            .map(|until| (NativeSessionNotice::MenuUnavailable, until));
    }

    pub(in crate::terminal_session) fn open_action_menu(
        &mut self,
        target: MenuTarget,
        anchor: Option<(u16, u16)>,
        pointer_released: bool,
    ) -> bool {
        self.open_action_menu_with_resume_permit(
            target,
            anchor,
            pointer_released,
            ResumePermitSeed::CaptureDisplayed,
        )
    }

    fn open_action_menu_with_resume_permit(
        &mut self,
        target: MenuTarget,
        anchor: Option<(u16, u16)>,
        pointer_released: bool,
        resume_seed: ResumePermitSeed,
    ) -> bool {
        self.menus.resume_handoff = None;
        if self.action_menu_active()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
            || self.board.is_open()
        {
            return false;
        }
        if self.menus.geometry.is_none() {
            self.menus.geometry = self.state.as_ref().map(NativeSessionState::latest_geometry);
        }
        if self.menus.geometry.is_none() {
            return false;
        }
        let target = match target {
            MenuTarget::Drawn { source, target } => {
                if !self.menu_carrier_live()
                    || self.state.as_ref().is_none_or(|state| {
                        state.cache().site_id() != &source.site
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
        if let MenuTarget::TabCreate(ref drawn) = target
            && self.view().tab_create_target().as_ref() != Some(drawn)
        {
            return false;
        }
        let target =
            match target {
                MenuTarget::Worktrees(crate::terminal_session::SidebarWorktreeTarget::Project(
                    id,
                )) => MenuTarget::Project(id),
                MenuTarget::Worktrees(
                    crate::terminal_session::SidebarWorktreeTarget::Workspace(id),
                ) => {
                    let Some(workspace) = self.state.as_ref().and_then(|state| {
                        state.cache().canonical_snapshot().navigation.workspace(&id)
                    }) else {
                        return false;
                    };
                    MenuTarget::Workspace(NativeContextLocation::Workspace {
                        workspace_id: id,
                        terminal_context_id: workspace.terminal_context_id.clone(),
                    })
                }
                MenuTarget::Create(Some(project)) => MenuTarget::Project(project),
                MenuTarget::Create(None) => MenuTarget::Global,
                MenuTarget::TabCreate(_) => MenuTarget::Global,
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
            MenuTarget::TabTools(_) => Some(super::tab_groups::MenuPage::Tools),
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
            | MenuTarget::TabGroupChoices(id)
            | MenuTarget::TabTools(id) => {
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
                    },
                    match &entry {
                        MenuTarget::TabGroups { .. } => tab.group().map_or_else(
                            || tab.display_label(index + 1).into_owned(),
                            |group| group.label.clone(),
                        ),
                        _ => tab.display_label(index + 1).into_owned(),
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
                    location @ NativeContextLocation::Workspace { .. }
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
                let actions = if matches!(source, NativeMenuContextSource::Standalone) {
                    crate::action_menu::LEGACY_WORKSPACE_ACTIONS
                } else {
                    crate::action_menu::WORKSPACE_ACTIONS
                };
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
            | MenuTarget::Worktrees(_) => {
                return false;
            }
        };
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
        actions = sidebar::actions(&entry, actions, self.menus.sidebar.as_ref());
        if tab_creation {
            actions = vec![
                MenuAction::NewTab,
                MenuAction::NewTabInRecentGroup,
                MenuAction::NewGroupWithTab,
            ];
        }
        actions.retain(|action| {
            !action_availability(
                state,
                &target,
                action.clone(),
                pointer_released,
                resume_permit.as_ref(),
            )
            .is_hidden()
        });
        let intent = Intent {
            entry,
            target,
            origin: Origin::capture(state),
            stage: Stage::Open,
            zoomed: state.is_some_and(NativeSessionState::zoom_active),
            pointer_released,
            resume_permit,
            project_choices,
        };
        if !self
            .menus
            .menu
            .open_for(intent.entry.clone(), title, actions, anchor)
        {
            return false;
        }
        self.menus.menu.set_labels(|action| match action {
            MenuAction::JoinWorkspaceProject(id) => intent
                .project_choices
                .as_ref()?
                .iter()
                .find(|project| project.project_id == id)
                .map(|project| project.label.to_string()),
            MenuAction::OpenWorktreeAt(path) => Some(path.as_str().to_owned()),
            MenuAction::NewTabInRecentGroup => state
                .and_then(NativeSessionState::recent_tab_group)
                .map(|group| group.label.clone()),
            MenuAction::JoinGroup(index) => state
                .and_then(|state| state.cache().groups().get(usize::from(index)))
                .map(|group| group.label.clone()),
            MenuAction::GroupMenu => match &intent.target {
                Target::Tab { tab, .. } => tab.group().map(|group| group.label.clone()),
                _ => None,
            },
            _ => None,
        });
        if let (Some(state), Target::Tab { tab, .. }) = (state, &intent.target) {
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
                self.menus
                    .menu
                    .select_action(MenuAction::GroupColor(group.color));
            }
        }
        self.menus.menu.refresh_availability(|action| {
            visible_unavailable(action_availability(
                state,
                &intent.target,
                action,
                intent.pointer_released,
                intent.resume_permit.as_ref(),
            ))
        });
        if !self.menus.menu.active() {
            return false;
        }
        self.menus.intent = Some(intent);
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
    pub(in crate::terminal_session) fn capture_action_menu_key_for(
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
            && matches!(
                self.menus.intent.as_ref().map(|intent| &intent.stage),
                Some(Stage::Open)
            )
            && let Some(ParentMenu {
                target,
                anchor,
                selected,
                resume_permit,
            }) = self.menus.parents.pop()
        {
            let released = self
                .menus
                .intent
                .as_ref()
                .is_some_and(|intent| intent.pointer_released);
            self.menus.menu.cancel();
            self.menus.intent = None;
            if self.open_action_menu_with_resume_permit(
                target,
                anchor,
                released,
                ResumePermitSeed::Inherit(resume_permit),
            ) {
                self.menus.menu.select_action(selected);
            } else {
                self.menus.parents.clear();
            }
            return true;
        }

        // Escape is always a local exit, including while the retained surface is pending,
        // suspended, or transferring an already confirmed action.
        if is_press && event.code == crate::client_view::KeyCode::Esc {
            let _ = self.menus.menu.key(event);
            if !self.action_menu_active() {
                self.menus.intent = None;
                self.menus.parents.clear();
                self.menus.sidebar = None;
            }
            return true;
        }

        // Recovery retains ownership without an ordinary editing lease. Pending, suspended,
        // covered, reopened-parent and transfer states therefore swallow everything except Esc.
        if current.readiness() == MenuInputReadiness::Recovery
            || !matches!(
                self.menus.intent.as_ref().map(|intent| &intent.stage),
                Some(Stage::Open)
            )
        {
            return true;
        }

        if self.menus.menu.key(event) {
            self.confirm_action_menu();
        }
        if !self.action_menu_active() {
            self.menus.intent = None;
            self.menus.parents.clear();
            self.menus.sidebar = None;
        }
        true
    }

    #[cfg(test)]
    pub(in crate::terminal_session) fn capture_action_menu_key(&mut self, event: KeyEvent) {
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

    pub(in crate::terminal_session) fn activate_action_menu_control(
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

    pub(in crate::terminal_session) fn hover_action_menu_row(
        &mut self,
        source: &MenuTarget,
        token: MenuToken,
        row: usize,
    ) -> bool {
        self.drive_action_menu();
        self.action_menu_source_matches(source, token) && self.menus.menu.hover(token, row)
    }

    pub(in crate::terminal_session) fn activate_action_menu_row(
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

    pub(in crate::terminal_session) fn scroll_action_menu(
        &mut self,
        token: MenuToken,
        up: bool,
        lines: usize,
    ) -> Option<MenuToken> {
        self.drive_action_menu();
        self.menus.menu.scroll(token, up, lines)
    }

    fn confirm_action_menu(&mut self) {
        let Some(intent) = &mut self.menus.intent else {
            return;
        };
        if !matches!(intent.stage, Stage::Open) {
            return;
        }
        let Some(deadline) = self.clock.now().checked_add(Duration::from_secs(30)) else {
            self.fail_action_menu();
            return;
        };
        intent.stage = Stage::Confirmed(deadline);
        self.drive_action_menu();
    }

    fn settle_blocked_action_menu_geometry(&mut self) {
        match self.menus.menu.geometry_blocked() {
            Some(GeometrySettlement::Refused) => {
                self.menus.intent = None;
                self.menus.parents.clear();
                self.menus.sidebar = None;
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
            } => {
                state.cache().location() == location
                    && state.cache().menu_context_source(location).as_ref() == Some(source)
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
                        | MenuAction::DeleteProject
                        | MenuAction::WorkspaceWorktrees
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
                    MenuAction::MoveWorkspace
                        | MenuAction::CloseWorkspace
                        | MenuAction::WorkspaceProjects
                        | MenuAction::WorkspaceWorktrees
                        | MenuAction::CreateWorkspaceProject
                        | MenuAction::LeaveWorkspaceProject
                        | MenuAction::JoinWorkspaceProject(_)
                        | MenuAction::DeleteWorkspace
                        | MenuAction::NewWorktree
                        | MenuAction::OpenWorktreeAt(_)
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
        let Some(mut intent) = self.menus.intent.take() else {
            return false;
        };
        let current = Origin::capture(self.state.as_ref());
        let open = matches!(intent.stage, Stage::Open | Stage::Confirmed(_));
        if intent
            .stage
            .deadline()
            .is_some_and(|deadline| self.clock.now() >= deadline)
            || !self.target_valid(&intent.target)
            || !self.sidebar_worktree_source_current()
            || intent.project_choices.as_ref().is_some_and(|choices| {
                self.state
                    .as_ref()
                    .and_then(|state| sidebar::project_choices(state, &intent.target))
                    .is_none_or(|current| &current.0 != choices)
            })
            || (open && current != intent.origin)
        {
            self.fail_action_menu();
            return true;
        }
        let mut changed = false;
        match intent.stage {
            Stage::Open => {
                if self.menus.menu.view().is_some_and(|menu| {
                    menu.actions.iter().any(|action| {
                        action_availability(
                            self.state.as_ref(),
                            &intent.target,
                            action.clone(),
                            intent.pointer_released,
                            intent.resume_permit.as_ref(),
                        )
                        .is_hidden()
                    })
                }) {
                    self.fail_action_menu();
                    return true;
                }
                changed = self.menus.menu.refresh_availability(|action| {
                    visible_unavailable(action_availability(
                        self.state.as_ref(),
                        &intent.target,
                        action,
                        intent.pointer_released,
                        intent.resume_permit.as_ref(),
                    ))
                });
            }
            Stage::Confirmed(deadline) => {
                if self.menus.menu.confirmed().is_some_and(|action| {
                    !action_availability(
                        self.state.as_ref(),
                        &intent.target,
                        action,
                        intent.pointer_released,
                        intent.resume_permit.as_ref(),
                    )
                    .is_enabled()
                }) {
                    self.fail_action_menu();
                    return true;
                }
                let next = match &intent.target {
                    Target::Global => Some(Stage::Offered(current.clone(), deadline)),
                    Target::Project { .. } => self
                        .menus
                        .menu
                        .confirmed()
                        .filter(|action| self.menu_target_selected(&intent.target, action.clone()))
                        .is_some()
                        .then(|| Stage::Offered(current.clone(), deadline)),
                    Target::Workspace { location, .. }
                        if matches!(
                            self.menus.menu.confirmed(),
                            Some(
                                MenuAction::MoveWorkspace
                                    | MenuAction::CloseWorkspace
                                    | MenuAction::WorkspaceProjects
                                    | MenuAction::WorkspaceWorktrees
                                    | MenuAction::CreateWorkspaceProject
                                    | MenuAction::LeaveWorkspaceProject
                                    | MenuAction::JoinWorkspaceProject(_)
                                    | MenuAction::DeleteWorkspace
                                    | MenuAction::NewWorktree
                                    | MenuAction::OpenWorktreeAt(_)
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
                intent.stage = next;
                changed = true;
            }
            Stage::Wait(ref expected, deadline) => {
                if &current != expected {
                    self.fail_action_menu();
                    return true;
                }
                if self.state.as_ref().is_none_or(|s| {
                    matches!(
                        s.phase_kind(),
                        SessionPhaseKind::Empty
                            | SessionPhaseKind::Ready
                            | SessionPhaseKind::SelectedWithoutAttachment
                            | SessionPhaseKind::RetryableAttachUnavailable
                    )
                }) {
                    intent.stage = Stage::Offered(current.clone(), deadline);
                    changed = true;
                }
            }
            Stage::Offered(ref expected, _) if &current != expected => {
                self.fail_action_menu();
                return true;
            }
            _ => {}
        }
        if self.menus.menu.active() {
            self.menus.intent = Some(intent);
        }
        changed
    }

    pub(in crate::terminal_session) fn take_action_menu_action(
        &mut self,
        pointer_released: bool,
    ) -> Option<ClientAction> {
        self.drive_action_menu();
        let intent = self.menus.intent.as_ref()?;
        if !matches!(intent.stage, Stage::Offered(_, _)) {
            return None;
        }
        let action = self.menus.menu.confirmed()?;
        if !self.target_valid(&intent.target)
            || !self.menu_target_selected(&intent.target, action.clone())
            || !action_availability(
                self.state.as_ref(),
                &intent.target,
                action.clone(),
                pointer_released,
                intent.resume_permit.as_ref(),
            )
            .is_enabled()
            || (action == MenuAction::Zoom
                && self
                    .state
                    .as_ref()
                    .is_some_and(NativeSessionState::zoom_active)
                    != intent.zoomed)
            || (action == MenuAction::ReleasePointer
                && (pointer_released || pointer_released != intent.pointer_released))
        {
            self.fail_action_menu();
            return None;
        }
        if action == MenuAction::TabCreation {
            let child = MenuTarget::TabCreate(self.view().tab_create_target()?);
            let parent = ParentMenu {
                target: intent.entry.clone(),
                anchor: self.menus.menu.view()?.anchor,
                selected: action,
                resume_permit: intent.resume_permit.clone(),
            };
            self.menus.menu.take()?;
            self.menus.intent = None;
            if self.open_action_menu(child, parent.anchor, pointer_released) {
                self.menus.parents.push(parent);
            } else {
                self.menus.parents.clear();
            }
            return None;
        }
        if action == MenuAction::WorkspaceProjects {
            let Target::Workspace { location, .. } = &intent.target else {
                return None;
            };
            let child = MenuTarget::WorkspaceProjects(location.clone());
            let parent = ParentMenu {
                target: intent.entry.clone(),
                anchor: self.menus.menu.view()?.anchor,
                selected: action,
                resume_permit: intent.resume_permit.clone(),
            };
            self.menus.menu.take()?;
            self.menus.intent = None;
            if self.open_action_menu(child, parent.anchor, pointer_released) {
                self.menus.parents.push(parent);
            } else {
                self.menus.parents.clear();
            }
            return None;
        }
        if action == MenuAction::WorkspaceWorktrees {
            let child = MenuTarget::Worktrees(sidebar::target(
                &intent.entry,
                &intent.target,
                self.state.as_ref()?,
            )?);
            let parent = ParentMenu {
                target: intent.entry.clone(),
                anchor: self.menus.menu.view()?.anchor,
                selected: action,
                resume_permit: intent.resume_permit.clone(),
            };
            self.menus.menu.take()?;
            self.menus.intent = None;
            if self.open_action_menu(child, parent.anchor, pointer_released) {
                self.menus.parents.push(parent);
            } else {
                self.menus.parents.clear();
            }
            return None;
        }
        if matches!(
            action,
            MenuAction::NewWorktree | MenuAction::OpenWorktreeAt(_)
        ) && matches!(
            intent.entry,
            MenuTarget::Worktrees(_) | MenuTarget::Create(_)
        ) {
            let discovery = self.menus.sidebar.as_ref()?.ready()?.clone();
            let checkout = match &action {
                MenuAction::OpenWorktreeAt(path) => Some(path.clone()),
                _ => None,
            };
            self.menus.menu.take()?;
            self.menus.intent = None;
            self.menus.parents.clear();
            self.menus.sidebar = None;
            self.open_sidebar_worktree(discovery, checkout);
            return None;
        }
        if let Target::Tab { tab, .. } = &intent.target {
            let child = match action {
                MenuAction::ManageGroups => Some(MenuTarget::TabGroupChoices(tab.tab_id().clone())),
                MenuAction::TabTools => Some(MenuTarget::TabTools(tab.tab_id().clone())),
                MenuAction::GroupMenu | MenuAction::ChooseGroupColor => {
                    Some(MenuTarget::TabGroups {
                        tab_id: tab.tab_id().clone(),
                        colors: action == MenuAction::ChooseGroupColor,
                    })
                }
                _ => None,
            };
            if let Some(child) = child {
                let parent = ParentMenu {
                    target: intent.entry.clone(),
                    anchor: self.menus.menu.view()?.anchor,
                    selected: action,
                    resume_permit: intent.resume_permit.clone(),
                };
                self.menus.menu.take()?;
                self.menus.intent = None;
                if self.open_action_menu(child, parent.anchor, pointer_released) {
                    self.menus.parents.push(parent);
                } else {
                    self.menus.parents.clear();
                }
                return None;
            }
        }
        let resume_handoff = match (
            &intent.target,
            action.clone(),
            intent.resume_permit.as_ref(),
            intent.stage.deadline(),
        ) {
            (
                Target::Tab {
                    location,
                    tab,
                    source,
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
        let action = match (&intent.target, action) {
            (
                Target::Workspace {
                    location: NativeContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                MenuAction::DeleteWorkspace,
            ) => ClientAction::RequestSidebarWorkspaceDelete(Box::new(
                crate::terminal_session::WorkspaceDeleteRequest::capture(
                    self.state.as_ref()?,
                    workspace_id,
                    intent.stage.deadline()?,
                )?,
            )),
            (
                Target::Workspace {
                    location: NativeContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                choice @ (MenuAction::CreateWorkspaceProject
                | MenuAction::LeaveWorkspaceProject
                | MenuAction::JoinWorkspaceProject(_)),
            ) => {
                use crate::terminal_session::state::workspace_setup::OrganizationChoice;
                let choice = match choice {
                    MenuAction::CreateWorkspaceProject => OrganizationChoice::NewProject,
                    MenuAction::LeaveWorkspaceProject => OrganizationChoice::SetProject(None),
                    MenuAction::JoinWorkspaceProject(id) => {
                        OrganizationChoice::SetProject(Some(id))
                    }
                    _ => unreachable!(),
                };
                let request = crate::terminal_session::WorkspaceOrganizationRequest::workspace(
                    self.state.as_ref()?,
                    workspace_id,
                    intent.stage.deadline()?,
                )?
                .with_choice(choice)?;
                ClientAction::RequestWorkspaceOrganization(Box::new(request))
            }
            (Target::Project { project, .. }, MenuAction::DeleteProject) => {
                let request = crate::terminal_session::WorkspaceOrganizationRequest::project(
                    self.state.as_ref()?, project, intent.stage.deadline()?)?
                    .with_choice(crate::terminal_session::state::workspace_setup::OrganizationChoice::DeleteIfIdle)?;
                ClientAction::RequestWorkspaceOrganization(Box::new(request))
            }
            (
                Target::Workspace {
                    location: NativeContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                MenuAction::CloseWorkspace,
            ) => ClientAction::RequestWorkspaceCloseAt(workspace_id.clone()),
            (
                Target::Workspace {
                    location: NativeContextLocation::Workspace { workspace_id, .. },
                    ..
                },
                MenuAction::MoveWorkspace,
            ) => {
                let request = crate::terminal_session::WorkspaceOrganizationRequest::workspace(
                    self.state.as_ref()?,
                    workspace_id,
                    intent.stage.deadline()?,
                )?;
                ClientAction::RequestWorkspaceOrganization(Box::new(request))
            }
            (Target::Project { project, .. }, MenuAction::CloseProjectWorkspaces) => {
                ClientAction::RequestProjectCloseAll(Some(project.project_id.clone()))
            }
            (Target::Tab { tab, .. }, action)
                if matches!(action.client_action(), Some(ClientAction::TabMenu { .. })) =>
            {
                ClientAction::TabMenu {
                    action,
                    tab_id: Some(tab.tab_id().clone()),
                }
            }
            (Target::Project { project, .. }, MenuAction::QuickWorkspace) => {
                let request = crate::terminal_session::ProjectQuickWorkspaceRequest::capture(
                    self.state.as_ref()?,
                    project,
                    intent.stage.deadline()?,
                )?;
                ClientAction::RequestProjectQuickWorkspace(Box::new(request))
            }
            (Target::Project { project, .. }, MenuAction::RenameProject) => {
                let request = crate::terminal_session::ProjectRenameRequest::capture(
                    self.state.as_ref()?,
                    project,
                    intent.stage.deadline()?,
                )?;
                ClientAction::RequestTargetedProjectRename(Box::new(request))
            }
            (_, action) => action.client_action()?,
        };
        self.menus.menu.take()?;
        self.menus.intent = None;
        self.menus.parents.clear();
        self.menus.sidebar = None;
        self.menus.resume_handoff = resume_handoff;
        Some(action)
    }

    pub(super) fn take_action_menu_resume_handoff(
        &mut self,
        action: &ClientAction,
    ) -> TabCloseResumeHandoff {
        let Some(handoff) = self.menus.resume_handoff.take() else {
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
        let Some(intent) = &mut self.menus.intent else {
            return;
        };
        match (&mut intent.stage, navigation) {
            (
                Stage::Context {
                    generation: owned,
                    prepared,
                    ..
                },
                NavigationKey::Context(generation),
            )
            | (
                Stage::Tab {
                    generation: owned,
                    prepared,
                    ..
                },
                NavigationKey::Tab(generation),
            ) if *owned == generation => {
                if accepted {
                    *prepared = true;
                } else {
                    self.fail_action_menu();
                }
            }
            _ => {}
        }
    }

    pub(super) fn menu_navigation_installed(&mut self, navigation: NavigationKey) {
        let Some(intent) = &self.menus.intent else {
            return;
        };
        let owned = matches!((&intent.stage,navigation), (Stage::Context { generation: g, prepared:true, .. },NavigationKey::Context(generation)) | (Stage::Tab { generation:g, prepared:true, .. },NavigationKey::Tab(generation)) if *g == generation);
        if !owned {
            return;
        }
        let selected = self
            .state
            .as_ref()
            .is_some_and(|state| match &intent.target {
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
        if let Some(intent) = &mut self.menus.intent {
            let Some(deadline) = intent.stage.deadline() else {
                return;
            };
            intent.stage = Stage::Wait(expected, deadline);
        }
    }

    pub(super) fn fence_menu_context_candidate(
        &self,
        prepared: super::super::state::PreparedRefresh,
        generation: u64,
        baseline_revision: orchbee_workbench_protocol::WorkbenchRevision,
    ) -> super::super::state::PreparedRefresh {
        let Some(Intent {
            target: Target::Workspace { location, source },
            stage: Stage::Context {
                generation: owned, ..
            },
            ..
        }) = &self.menus.intent
        else {
            return prepared;
        };
        if *owned != generation {
            return prepared;
        }
        match &prepared {
            super::super::state::PreparedRefresh::Membership {
                prepared: candidate,
                ..
            } if candidate.context_location() == location
                && candidate.menu_context_source().as_ref() == Some(source) =>
            {
                prepared
            }
            super::super::state::PreparedRefresh::Membership { .. } => {
                super::super::state::PreparedRefresh::DefiniteFailure {
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
        candidate: &super::super::state::PreparedMembershipCandidate,
    ) -> bool {
        let Some(Intent {
            target:
                Target::Tab {
                    location,
                    tab,
                    source,
                },
            stage: Stage::Tab {
                generation: owned, ..
            },
            ..
        }) = &self.menus.intent
        else {
            return true;
        };
        *owned != generation
            || (candidate.context_location() == location
                && candidate.menu_selected_tab_matches(tab)
                && candidate.menu_context_source().as_ref() == Some(source))
    }
}
