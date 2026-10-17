//! Deterministic menu capture, selection and successful-draw authority.
//! NativeUi alone owns target validation and navigation; this unit cannot execute actions.
use crate::client_view::{
    Binding, ClientAction, KeyCode, KeyEvent, KeyKind, Modifiers, PanelDirection,
};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};

mod tab_presentation;
pub(crate) use tab_presentation::{TabMenuPresentation, TabMenuScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MenuTarget {
    Global,
    /// Creation-only entry over the existing global or exact Project actions.
    Create(Option<orchbee_workbench_protocol::WorkbenchProjectId>),
    TabGroups {
        tab_id: orchbee_workbench_protocol::WorkbenchTabId,
        colors: bool,
    },
    TabGroupChoices(orchbee_workbench_protocol::WorkbenchTabId),
    TabTools(orchbee_workbench_protocol::WorkbenchTabId),
    TabCreate(DrawnTabCreate),
    Pane(crate::terminal_session::state::PaneRef),
    Tab(orchbee_workbench_protocol::WorkbenchTabId),
    /// Organization actions address the Project directly; terminal actions require a member.
    Project(orchbee_workbench_protocol::WorkbenchProjectId),
    Workspace(crate::terminal_session::NativeContextLocation),
    WorkspaceProjects(crate::terminal_session::NativeContextLocation),
    Worktrees(crate::terminal_session::SidebarWorktreeTarget),
    /// An object entry must still belong to the exact Site snapshot that painted it.
    Drawn {
        source: DrawnMenuSource,
        target: Box<Self>,
    },
}

impl MenuTarget {
    /// Object and creation menus retain context chrome even when entered with the keyboard.
    pub(crate) fn context_menu(&self) -> bool {
        match self {
            Self::Global => false,
            Self::Drawn { target, .. } => target.context_menu(),
            _ => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DrawnMenuSource {
    pub(crate) site: orchbee_workbench_protocol::SiteId,
    pub(crate) revision: orchbee_workbench_protocol::WorkbenchRevision,
}

/// The exact context and snapshot behind a painted tab-strip create control.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DrawnTabCreate {
    pub(crate) source: DrawnMenuSource,
    pub(crate) location: crate::terminal_session::NativeContextLocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MenuAction {
    SplitVertical,
    SplitHorizontal,
    Zoom,
    Resize,
    Copy,
    Takeover,
    ClosePane,
    NewTab,
    TabCreation,
    NewTabToRight,
    NewTabInGroup,
    NewTabInRecentGroup,
    NewGroupWithTab,
    DuplicateTab,
    MoveTabPrevious,
    MoveTabNext,
    MoveTabFirst,
    MoveTabLast,
    SelectTab,
    DeselectTab,
    SelectTabRange,
    SelectAllTabs,
    ClearTabSelection,
    PinTab,
    UnpinTab,
    ManageGroups,
    TabTools,
    GroupMenu,
    MoveToNewWindow,
    CreateGroup,
    RenameGroup,
    ChooseGroupColor,
    GroupColor(orchbee_workbench_protocol::TabGroupColor),
    JoinGroup(u16),
    RemoveFromGroup,
    Ungroup,
    CollapseGroup,
    ExpandGroup,
    MoveGroupPrevious,
    MoveGroupNext,
    RenameTab,
    ResumeTabClose,
    CloseSelectedTabs,
    CloseOtherTabs,
    CloseTrailingTabs,
    CloseGroup,
    QuickWorkspace,
    WorkspaceAtDirectory,
    WorkspaceDirectory,
    RenameWorkspace,
    RenameProject,
    MoveWorkspace,
    WorkspaceProjects,
    WorkspaceWorktrees,
    CreateWorkspaceProject,
    LeaveWorkspaceProject,
    JoinWorkspaceProject(orchbee_workbench_protocol::WorkbenchProjectId),
    OpenWorktreeAt(orchbee_workbench_protocol::WorkbenchDirectoryPath),
    DeleteWorkspace,
    DeleteProject,
    CloseProjectWorkspaces,
    CloseWorkspace,
    NewWorkspace,
    NewWorktree,
    OpenWorktree,
    Reload,
    Settings,
    Machines,
    Help,
    Sidebar,
    Navigator,
    AgentBoard,
    ReleasePointer,
    Detach,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuUnavailable {
    DirectoryConfigured,
    EmptyWorkspaceRequired,
    WorkspaceRequired,
    LegacyWorkspace,
    ProjectWorkspaceRequired,
    OperationBusy,
    PointerReleased,
    NoMatchingTabs,
    NoRecentGroup,
}

impl MenuAction {
    pub(crate) const fn submenu(&self) -> bool {
        matches!(
            self,
            Self::ManageGroups
                | Self::TabTools
                | Self::GroupMenu
                | Self::ChooseGroupColor
                | Self::TabCreation
                | Self::WorkspaceProjects
                | Self::WorkspaceWorktrees
        )
    }
    /// Addressed sidebar choices require their menu's frozen source owner.
    /// They cannot fall back to an action on the current Workspace.
    pub(crate) fn client_action(&self) -> Option<ClientAction> {
        Some(match self {
            Self::SplitVertical => ClientAction::SplitPanel(PanelDirection::Right),
            Self::SplitHorizontal => ClientAction::SplitPanel(PanelDirection::Down),
            Self::Zoom => ClientAction::ToggleZoom,
            Self::Resize => ClientAction::EnterResizeMode,
            Self::Copy => ClientAction::EnterCopyMode,
            Self::Takeover => ClientAction::RequestControlTakeover,
            Self::ClosePane => ClientAction::ClosePanel,
            Self::MoveTabPrevious
            | Self::ResumeTabClose
            | Self::CloseSelectedTabs
            | Self::CloseOtherTabs
            | Self::CloseTrailingTabs
            | Self::CloseGroup
            | Self::MoveTabNext
            | Self::MoveTabFirst
            | Self::MoveTabLast
            | Self::SelectTab
            | Self::DeselectTab
            | Self::SelectTabRange
            | Self::SelectAllTabs
            | Self::ClearTabSelection
            | Self::PinTab
            | Self::NewTabInGroup
            | Self::NewTabInRecentGroup
            | Self::TabCreation
            | Self::NewTabToRight
            | Self::NewGroupWithTab
            | Self::DuplicateTab
            | Self::UnpinTab
            | Self::ManageGroups
            | Self::TabTools
            | Self::GroupMenu
            | Self::MoveToNewWindow
            | Self::CreateGroup
            | Self::RenameGroup
            | Self::ChooseGroupColor
            | Self::GroupColor(_)
            | Self::JoinGroup(_)
            | Self::RemoveFromGroup
            | Self::Ungroup
            | Self::CollapseGroup
            | Self::ExpandGroup
            | Self::MoveGroupPrevious
            | Self::MoveGroupNext => ClientAction::TabMenu {
                action: self.clone(),
                tab_id: None,
            },
            Self::NewTab => ClientAction::CreateTab,
            Self::RenameTab => ClientAction::RequestRenameTab,
            Self::QuickWorkspace => ClientAction::RequestQuickWorkspace,
            Self::WorkspaceAtDirectory => ClientAction::RequestWorkspaceAtDirectory(None),
            Self::WorkspaceDirectory => ClientAction::RequestWorkspaceDirectory,
            Self::RenameWorkspace => ClientAction::RequestWorkspaceRename,
            Self::RenameProject => ClientAction::RequestProjectRename,
            Self::MoveWorkspace => ClientAction::RequestWorkspaceMove,
            Self::CloseProjectWorkspaces => ClientAction::RequestProjectCloseAll(None),
            Self::CloseWorkspace => ClientAction::RequestWorkspaceClose,
            Self::NewWorkspace => ClientAction::RequestWorkspaceCreate,
            Self::NewWorktree => ClientAction::RequestWorktreeCreate,
            Self::OpenWorktree => ClientAction::RequestWorktreeOpen,
            Self::Reload => ClientAction::ReloadConfiguration,
            Self::Settings => ClientAction::OpenSettings,
            Self::Machines => ClientAction::OpenMachines,
            Self::Help => ClientAction::OpenShortcutHelp,
            Self::Sidebar => ClientAction::ToggleBusinessSidebar,
            Self::Navigator => ClientAction::OpenWorkbenchNavigator,
            Self::AgentBoard => ClientAction::ToggleAgentBoard,
            Self::ReleasePointer => ClientAction::TogglePointerCapture,
            Self::Detach => ClientAction::Exit,
            Self::WorkspaceProjects
            | Self::WorkspaceWorktrees
            | Self::CreateWorkspaceProject
            | Self::LeaveWorkspaceProject
            | Self::JoinWorkspaceProject(_)
            | Self::OpenWorktreeAt(_)
            | Self::DeleteWorkspace
            | Self::DeleteProject => return None,
        })
    }
}

pub(crate) const PANE_ACTIONS: &[MenuAction] = &[
    MenuAction::SplitVertical,
    MenuAction::SplitHorizontal,
    MenuAction::Zoom,
    MenuAction::Resize,
    MenuAction::Copy,
    MenuAction::Takeover,
    MenuAction::ClosePane,
];
pub(crate) const TAB_ACTIONS: &[MenuAction] = &[
    MenuAction::NewTabToRight,
    MenuAction::RenameTab,
    MenuAction::ManageGroups,
    MenuAction::MoveToNewWindow,
    MenuAction::DuplicateTab,
    MenuAction::PinTab,
    MenuAction::CloseSelectedTabs,
    MenuAction::CloseOtherTabs,
    MenuAction::CloseTrailingTabs,
    MenuAction::TabTools,
];
pub(crate) const WORKSPACE_ACTIONS: &[MenuAction] = &[
    MenuAction::WorkspaceProjects,
    MenuAction::WorkspaceWorktrees,
    MenuAction::RenameWorkspace,
    MenuAction::DeleteWorkspace,
];
/// Retained compatibility contexts have no Workspace ID for canonical sidebar operations.
pub(crate) const LEGACY_WORKSPACE_ACTIONS: &[MenuAction] = &[
    MenuAction::QuickWorkspace,
    MenuAction::NewTab,
    MenuAction::WorkspaceDirectory,
    MenuAction::RenameWorkspace,
    MenuAction::MoveWorkspace,
    MenuAction::NewWorktree,
    MenuAction::OpenWorktree,
    MenuAction::CloseWorkspace,
];
pub(crate) const PROJECT_ACTIONS: &[MenuAction] = &[
    MenuAction::QuickWorkspace,
    MenuAction::RenameProject,
    MenuAction::WorkspaceWorktrees,
    MenuAction::DeleteProject,
];
pub(crate) const GLOBAL_ACTIONS: &[MenuAction] = &[
    MenuAction::TabCreation,
    MenuAction::NewWorkspace,
    MenuAction::WorkspaceAtDirectory,
    MenuAction::NewWorktree,
    MenuAction::OpenWorktree,
    MenuAction::Reload,
    MenuAction::Settings,
    MenuAction::Machines,
    MenuAction::Help,
    MenuAction::Sidebar,
    MenuAction::Navigator,
    MenuAction::AgentBoard,
    MenuAction::ReleasePointer,
    MenuAction::Detach,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuToken(u64);

/// The local chrome actions owned by an open menu surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuControl {
    Back,
    Close,
}

/// What input the exact retained menu snapshot currently admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuInputReadiness {
    /// The owner retains capture, but no ordinary key may mutate this surface.
    Recovery,
    /// The whole surface is still the admitted owner while an exact row proof is renewed.
    Continuation,
    /// The selected row has a fresh exact draw and may activate.
    ActivationReady,
}

/// One exact input snapshot for the retained menu surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuInputContext {
    token: MenuToken,
    phase: PresentationPhase,
    readiness: MenuInputReadiness,
}

impl MenuInputContext {
    #[cfg(test)]
    pub(crate) const fn token(self) -> MenuToken {
        self.token
    }

    pub(crate) const fn phase(self) -> PresentationPhase {
        self.phase
    }

    pub(crate) const fn readiness(self) -> MenuInputReadiness {
        self.readiness
    }

    pub(crate) const fn activation_ready(self) -> bool {
        matches!(self.readiness, MenuInputReadiness::ActivationReady)
    }

    pub(crate) const fn recovery_only(mut self) -> Self {
        self.readiness = MenuInputReadiness::Recovery;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuDraw {
    pub(crate) token: MenuToken,
    pub(crate) first: usize,
    pub(crate) visible_rows: usize,
    pub(crate) selected: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuSelection {
    Previous,
    Next,
    First,
    Last,
}

#[derive(Clone, Copy)]
pub(crate) struct MenuView<'a> {
    pub(crate) source: &'a MenuTarget,
    pub(crate) context_menu: bool,
    pub(crate) has_parent: bool,
    pub(crate) title: &'a str,
    pub(crate) labels: &'a [Option<String>],
    pub(crate) tabs: Option<&'a TabMenuPresentation>,
    pub(crate) actions: &'a [MenuAction],
    pub(crate) unavailable: &'a [Option<MenuUnavailable>],
    pub(crate) selected: usize,
    pub(crate) first: usize,
    pub(crate) follow_selection: bool,
    pub(crate) token: MenuToken,
    pub(crate) anchor: Option<(u16, u16)>,
    pub(crate) transferring: bool,
}

impl MenuView<'_> {
    pub(crate) fn first_visible_row(&self, rows: usize) -> usize {
        let rows = rows.max(1);
        let mut maximum = self.actions.len().saturating_sub(1);
        while maximum > 0 && self.row_height(maximum - 1, self.actions.len()) <= rows {
            maximum -= 1;
        }
        let mut first = self.first.min(maximum);
        if !self.follow_selection {
            return first;
        }
        if self.selected < first {
            first = self.selected;
        }
        while first < self.selected && self.row_height(first, self.selected + 1) > rows {
            first += 1;
        }
        first.min(maximum)
    }

    pub(crate) fn separator_before(&self, index: usize) -> bool {
        separator_before(self.source, self.actions, index)
    }

    pub(crate) fn row_height(&self, first: usize, end: usize) -> usize {
        row_height(self.source, self.actions, first, end)
    }
}

fn separator_before(source: &MenuTarget, actions: &[MenuAction], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    let action = &actions[index];
    match action {
        MenuAction::JoinWorkspaceProject(_) => {
            return !matches!(actions[index - 1], MenuAction::JoinWorkspaceProject(_));
        }
        MenuAction::OpenWorktreeAt(_) => {
            return !matches!(actions[index - 1], MenuAction::OpenWorktreeAt(_));
        }
        MenuAction::WorkspaceWorktrees => {
            return matches!(source, MenuTarget::Workspace(_));
        }
        MenuAction::RenameWorkspace | MenuAction::DeleteWorkspace | MenuAction::DeleteProject => {
            return true;
        }
        _ => {}
    }
    crate::action_projection::menu_metadata(action.clone()).starts_section(action.clone())
}

fn row_height(source: &MenuTarget, actions: &[MenuAction], first: usize, end: usize) -> usize {
    end - first
        + ((first + 1)..end)
            .filter(|index| separator_before(source, actions, *index))
            .count()
}

struct Menu {
    source: MenuTarget,
    labels: Vec<Option<String>>,
    tabs: Option<TabMenuPresentation>,
    title: String,
    actions: Vec<MenuAction>,
    unavailable: Vec<Option<MenuUnavailable>>,
    selected: usize,
    first: usize,
    visible_rows: usize,
    follow_selection: bool,
    token: MenuToken,
    drawn: Option<MenuDraw>,
    surface_presented: bool,
    anchor: Option<(u16, u16)>,
}

#[derive(Default)]
pub(crate) struct ActionMenu {
    next: u64,
    menu: Option<Menu>,
    confirmed: Option<MenuAction>,
    presentation: PresentationPhase,
}

impl ActionMenu {
    fn allocate(&mut self) -> Option<MenuToken> {
        self.next = self.next.checked_add(1)?;
        Some(MenuToken(self.next))
    }

    pub(crate) fn select_action(&mut self, action: MenuAction) {
        if let Some(index) = self
            .menu
            .as_ref()
            .and_then(|menu| menu.actions.iter().position(|item| *item == action))
        {
            self.select_row(index);
        }
    }
    pub(crate) fn key(&mut self, event: KeyEvent) -> bool {
        if event.kind == KeyKind::Release {
            return false;
        }
        if Binding::new(KeyCode::Enter, Modifiers::NONE).matches(event) {
            return self.confirm();
        }
        if event.mods != Modifiers::NONE {
            return false;
        }
        match event.code {
            KeyCode::Right
                if self
                    .menu
                    .as_ref()
                    .is_some_and(|menu| menu.actions[menu.selected].submenu()) =>
            {
                return self.confirm();
            }
            KeyCode::Esc => self.cancel(),
            KeyCode::Up | KeyCode::Char('k') => self.select(MenuSelection::Previous),
            KeyCode::Down | KeyCode::Char('j') => self.select(MenuSelection::Next),
            KeyCode::Home => self.select(MenuSelection::First),
            KeyCode::End => self.select(MenuSelection::Last),
            _ => {}
        }
        false
    }
    pub(crate) fn active(&self) -> bool {
        self.menu.is_some()
    }

    pub(crate) fn input_context(&self) -> Option<MenuInputContext> {
        let menu = self.menu.as_ref()?;
        let exact_selected = self.confirmed.is_none()
            && menu.drawn.is_some_and(|draw| {
                draw.token == menu.token
                    && draw.selected == menu.selected
                    && (draw.first..draw.first.saturating_add(draw.visible_rows))
                        .contains(&menu.selected)
            });
        let readiness = if !self.presentation.interactive() || !menu.surface_presented {
            MenuInputReadiness::Recovery
        } else if exact_selected {
            MenuInputReadiness::ActivationReady
        } else {
            MenuInputReadiness::Continuation
        };
        Some(MenuInputContext {
            token: menu.token,
            phase: self.presentation,
            readiness,
        })
    }

    #[cfg(test)]
    pub(crate) fn open(
        &mut self,
        title: String,
        actions: Vec<MenuAction>,
        anchor: Option<(u16, u16)>,
    ) -> bool {
        self.open_for(MenuTarget::Global, title, actions, anchor)
    }

    pub(crate) fn open_for(
        &mut self,
        source: MenuTarget,
        title: String,
        actions: Vec<MenuAction>,
        anchor: Option<(u16, u16)>,
    ) -> bool {
        if self.active()
            || actions.is_empty()
            || actions.len() > orchbee_workbench_protocol::MAXIMUM_TAB_GROUPS_PER_CONTEXT + 16
        {
            return false;
        }
        let Some(token) = self.allocate() else {
            return false;
        };
        self.menu = Some(Menu {
            source,
            title,
            labels: vec![None; actions.len()],
            tabs: None,
            unavailable: vec![None; actions.len()],
            actions,
            selected: 0,
            first: 0,
            visible_rows: 0,
            follow_selection: true,
            token,
            drawn: None,
            surface_presented: false,
            anchor,
        });
        self.confirmed = None;
        self.presentation = PresentationPhase::Pending;
        true
    }

    pub(crate) fn set_labels(&mut self, label: impl Fn(MenuAction) -> Option<String>) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        let labels: Vec<_> = menu
            .actions
            .iter()
            .map(|action| label(action.clone()))
            .collect();
        if menu.labels == labels {
            return;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.labels = labels;
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
    }

    /// Replaces captured display facts under the same successful-draw fence as row copy.
    pub(crate) fn set_tab_presentation(
        &mut self,
        scope: TabMenuScope,
        applied_color: Option<orchbee_workbench_protocol::TabGroupColor>,
        row: impl Fn(&MenuAction) -> Option<TabMenuScope>,
    ) {
        let Some(menu) = &self.menu else {
            return;
        };
        let Some(rows) = menu.actions.iter().map(row).collect() else {
            self.cancel();
            return;
        };
        let tabs = TabMenuPresentation {
            scope,
            applied_color,
            rows,
        };
        if menu.tabs.as_ref() == Some(&tabs) {
            return;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.tabs = Some(tabs);
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
    }

    pub(crate) fn view(&self) -> Option<MenuView<'_>> {
        self.menu.as_ref().map(|menu| MenuView {
            source: &menu.source,
            context_menu: menu.source.context_menu(),
            has_parent: false,
            title: &menu.title,
            labels: &menu.labels,
            tabs: menu.tabs.as_ref(),
            actions: &menu.actions,
            unavailable: &menu.unavailable,
            selected: menu.selected,
            first: menu.first,
            follow_selection: menu.follow_selection,
            token: menu.token,
            anchor: menu.anchor,
            transferring: self.confirmed.is_some(),
        })
    }

    /// Commits the exact row window from one completed menu frame.
    pub(crate) fn presented(&mut self, draw: MenuDraw) -> bool {
        let Some(menu) = &self.menu else {
            return false;
        };
        if menu.token != draw.token
            || draw.visible_rows == 0
            || draw.first >= menu.actions.len()
            || draw.selected != menu.selected
            || draw.first.saturating_add(draw.visible_rows) > menu.actions.len()
        {
            self.revoke_exact_draw();
            return false;
        }
        let Some(menu) = &mut self.menu else {
            return false;
        };
        menu.first = draw.first;
        menu.visible_rows = draw.visible_rows;
        menu.drawn = Some(draw);
        menu.surface_presented = true;
        self.presentation.presented();
        true
    }

    /// Revokes row-level activation while preserving the admitted whole-surface lease.
    fn revoke_exact_draw(&mut self) {
        if let Some(menu) = &mut self.menu {
            menu.drawn = None;
            menu.visible_rows = 0;
        }
    }

    /// Revokes both exact activation and proof that this owner occupied the latest whole frame.
    pub(crate) fn not_presented(&mut self) {
        let rekey = self
            .menu
            .as_ref()
            .is_some_and(|menu| menu.surface_presented || menu.drawn.is_some());
        let token = if rekey {
            let Some(token) = self.allocate() else {
                self.cancel();
                return;
            };
            Some(token)
        } else {
            None
        };
        if let Some(menu) = &mut self.menu {
            if let Some(token) = token {
                menu.token = token;
            }
            menu.drawn = None;
            menu.visible_rows = 0;
            menu.surface_presented = false;
        }
    }

    /// Settles an evaluated geometry block for this retained menu incarnation.
    pub(crate) fn geometry_blocked(&mut self) -> Option<GeometrySettlement> {
        self.menu.as_ref()?;
        self.not_presented();
        let settlement = self.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.cancel();
        }
        Some(settlement)
    }

    pub(crate) fn select(&mut self, selection: MenuSelection) {
        if self.confirmed.is_some() {
            return;
        }
        let Some(menu) = &mut self.menu else {
            return;
        };
        let selected = match selection {
            MenuSelection::Previous => menu.selected.saturating_sub(1),
            MenuSelection::Next => (menu.selected + 1).min(menu.actions.len() - 1),
            MenuSelection::First => 0,
            MenuSelection::Last => menu.actions.len() - 1,
        };
        self.select_row(selected);
    }

    pub(crate) fn scroll(&mut self, token: MenuToken, up: bool, lines: usize) -> Option<MenuToken> {
        let menu = self.menu.as_ref()?;
        let draw = menu.drawn?;
        if self.confirmed.is_some()
            || !self.presentation.interactive()
            || menu.token != token
            || draw.token != token
        {
            return None;
        }
        // Derive the actual terminal-row budget from the admitted window. Separators mean the same
        // budget can contain a different number of actions at the tail.
        let rows = row_height(
            &menu.source,
            &menu.actions,
            draw.first,
            draw.first.saturating_add(draw.visible_rows),
        );
        let mut maximum = menu.actions.len().saturating_sub(1);
        while maximum > 0
            && row_height(&menu.source, &menu.actions, maximum - 1, menu.actions.len()) <= rows
        {
            maximum -= 1;
        }
        let first = if up {
            draw.first.saturating_sub(lines)
        } else {
            draw.first.saturating_add(lines).min(maximum)
        };
        if first == draw.first {
            return Some(token);
        }
        let Some(next) = self.allocate() else {
            self.cancel();
            return None;
        };
        let menu = self.menu.as_mut()?;
        menu.first = first;
        menu.follow_selection = false;
        menu.visible_rows = 0;
        menu.token = next;
        menu.drawn = None;
        Some(next)
    }

    fn select_row(&mut self, selected: usize) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        if selected == menu.selected && menu.follow_selection {
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.selected = selected;
        menu.follow_selection = true;
        menu.visible_rows = 0;
        menu.token = token;
        menu.drawn = None;
    }

    /// Reasons come from the operation owner; changing them revokes the painted proof.
    pub(crate) fn refresh_availability(
        &mut self,
        mut query: impl FnMut(MenuAction) -> Option<MenuUnavailable>,
    ) -> bool {
        let Some(menu) = &mut self.menu else {
            return false;
        };
        let unavailable: Vec<_> = menu
            .actions
            .iter()
            .map(|action| query(action.clone()))
            .collect();
        if menu.unavailable == unavailable {
            return false;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return true;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return true;
        };
        let Some(menu) = &mut self.menu else {
            return false;
        };
        menu.unavailable = unavailable;
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
        true
    }

    pub(crate) fn confirm(&mut self) -> bool {
        if self.confirmed.is_some() {
            return false;
        }
        let Some(menu) = &self.menu else {
            return false;
        };
        if !self
            .input_context()
            .is_some_and(MenuInputContext::activation_ready)
            || menu.unavailable[menu.selected].is_some()
        {
            return false;
        }
        self.confirmed = Some(menu.actions[menu.selected].clone());
        true
    }

    fn drawn_enabled_row(&self, token: MenuToken, row: usize) -> bool {
        let Some(menu) = &self.menu else {
            return false;
        };
        let Some(draw) = menu.drawn else {
            return false;
        };
        self.confirmed.is_none()
            && self.presentation.interactive()
            && draw.token == token
            && menu.token == token
            && row < menu.actions.len()
            && row >= draw.first
            && row < draw.first.saturating_add(draw.visible_rows)
            && menu.unavailable[row].is_none()
    }

    /// Movement changes only the provisional menu candidate, never confirms an action.
    /// The painted window remains anchored until explicit keyboard navigation resumes.
    pub(crate) fn hover(&mut self, token: MenuToken, row: usize) -> bool {
        if !self.drawn_enabled_row(token, row) {
            return false;
        }
        let Some(menu) = &mut self.menu else {
            return false;
        };
        if menu.selected == row {
            return false;
        }
        menu.selected = row;
        menu.follow_selection = false;
        // Hover preserves the exact row window so every motion in a packet can update the
        // candidate. Enter still requires a draw whose selected row matches this candidate.
        true
    }

    pub(crate) fn click(&mut self, token: MenuToken, row: usize) -> bool {
        if !self.drawn_enabled_row(token, row) {
            return false;
        }
        let Some(menu) = &mut self.menu else {
            return false;
        };
        menu.selected = row;
        self.confirmed = Some(menu.actions[row].clone());
        true
    }

    pub(crate) fn confirmed(&self) -> Option<MenuAction> {
        self.confirmed.clone()
    }

    pub(crate) fn take(&mut self) -> Option<MenuAction> {
        let action = self.confirmed.take()?;
        self.menu = None;
        self.presentation = PresentationPhase::Pending;
        Some(action)
    }

    pub(crate) fn cancel(&mut self) {
        self.menu = None;
        self.confirmed = None;
        self.presentation = PresentationPhase::Pending;
    }
}

#[cfg(test)]
mod tests;
