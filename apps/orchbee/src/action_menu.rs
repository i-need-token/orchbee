//! Deterministic menu capture, selection and successful-draw authority.
//! NativeUi alone owns target validation and navigation; this unit cannot execute actions.
use crate::client_view::{
    Binding, ClientAction, KeyCode, KeyEvent, KeyKind, Modifiers, PanelDirection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MenuTarget {
    Global,
    Pane(crate::terminal_session::state::PaneRef),
    Tab(orchbee_workbench_protocol::WorkbenchTabId),
    /// Project actions require an explicitly current Workspace in that exact Project.
    Project(orchbee_workbench_protocol::WorkbenchProjectId),
    Workspace(crate::terminal_session::NativeContextLocation),
    /// An object entry must still belong to the exact Site snapshot that painted it.
    Drawn {
        source: DrawnMenuSource,
        target: Box<Self>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DrawnMenuSource {
    pub(crate) site: orchbee_workbench_protocol::SiteId,
    pub(crate) revision: orchbee_workbench_protocol::WorkbenchRevision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuAction {
    SplitVertical,
    SplitHorizontal,
    Zoom,
    Resize,
    Copy,
    Takeover,
    ClosePane,
    NewTab,
    RenameTab,
    CloseTab,
    QuickWorkspace,
    WorkspaceDirectory,
    RenameWorkspace,
    RenameProject,
    CloseWorkspace,
    NewWorkspace,
    NewWorktree,
    OpenWorktree,
    Reload,
    Settings,
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
    ProjectWorkspaceRequired,
    OperationBusy,
    PointerReleased,
}

impl MenuAction {
    pub(crate) fn client_action(self) -> ClientAction {
        match self {
            Self::SplitVertical => ClientAction::SplitPanel(PanelDirection::Right),
            Self::SplitHorizontal => ClientAction::SplitPanel(PanelDirection::Down),
            Self::Zoom => ClientAction::ToggleZoom,
            Self::Resize => ClientAction::EnterResizeMode,
            Self::Copy => ClientAction::EnterCopyMode,
            Self::Takeover => ClientAction::RequestControlTakeover,
            Self::ClosePane => ClientAction::ClosePanel,
            Self::NewTab => ClientAction::CreateTab,
            Self::RenameTab => ClientAction::RequestRenameTab,
            Self::CloseTab => ClientAction::RequestCloseTab,
            Self::QuickWorkspace => ClientAction::RequestQuickWorkspace,
            Self::WorkspaceDirectory => ClientAction::RequestWorkspaceDirectory,
            Self::RenameWorkspace => ClientAction::RequestWorkspaceRename,
            Self::RenameProject => ClientAction::RequestProjectRename,
            Self::CloseWorkspace => ClientAction::RequestWorkspaceClose,
            Self::NewWorkspace => ClientAction::RequestWorkspaceCreate,
            Self::NewWorktree => ClientAction::RequestWorktreeCreate,
            Self::OpenWorktree => ClientAction::RequestWorktreeOpen,
            Self::Reload => ClientAction::ReloadConfiguration,
            Self::Settings => ClientAction::OpenSettings,
            Self::Help => ClientAction::OpenShortcutHelp,
            Self::Sidebar => ClientAction::ToggleBusinessSidebar,
            Self::Navigator => ClientAction::OpenWorkbenchNavigator,
            Self::AgentBoard => ClientAction::ToggleAgentBoard,
            Self::ReleasePointer => ClientAction::TogglePointerCapture,
            Self::Detach => ClientAction::Exit,
        }
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
    MenuAction::NewTab,
    MenuAction::RenameTab,
    MenuAction::CloseTab,
];
pub(crate) const WORKSPACE_ACTIONS: &[MenuAction] = &[
    MenuAction::QuickWorkspace,
    MenuAction::NewTab,
    MenuAction::WorkspaceDirectory,
    MenuAction::RenameWorkspace,
    MenuAction::NewWorktree,
    MenuAction::OpenWorktree,
    MenuAction::CloseWorkspace,
];
pub(crate) const PROJECT_ACTIONS: &[MenuAction] = &[
    MenuAction::NewWorkspace,
    MenuAction::QuickWorkspace,
    MenuAction::RenameProject,
    MenuAction::NewWorktree,
    MenuAction::OpenWorktree,
];
pub(crate) const GLOBAL_ACTIONS: &[MenuAction] = &[
    MenuAction::NewWorkspace,
    MenuAction::QuickWorkspace,
    MenuAction::NewWorktree,
    MenuAction::OpenWorktree,
    MenuAction::Reload,
    MenuAction::Settings,
    MenuAction::Help,
    MenuAction::Sidebar,
    MenuAction::Navigator,
    MenuAction::AgentBoard,
    MenuAction::ReleasePointer,
    MenuAction::Detach,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuToken(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuDraw {
    pub(crate) token: MenuToken,
    pub(crate) rows: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuSelection {
    Previous,
    Next,
    First,
    Last,
}

pub(crate) struct MenuView<'a> {
    pub(crate) title: &'a str,
    pub(crate) actions: &'a [MenuAction],
    pub(crate) unavailable: &'a [Option<MenuUnavailable>],
    pub(crate) selected: usize,
    pub(crate) first: usize,
    pub(crate) token: MenuToken,
    pub(crate) anchor: Option<(u16, u16)>,
    pub(crate) transferring: bool,
}

impl MenuView<'_> {
    pub(crate) fn first_visible_row(&self, rows: usize) -> usize {
        window_first(self.first, self.selected, rows, self.actions.len())
    }
}

fn window_first(first: usize, selected: usize, rows: usize, action_count: usize) -> usize {
    first
        .min(selected)
        .min(action_count.saturating_sub(rows))
        .max((selected + 1).saturating_sub(rows))
}

struct Menu {
    title: String,
    actions: Vec<MenuAction>,
    unavailable: Vec<Option<MenuUnavailable>>,
    selected: usize,
    first: usize,
    rows: usize,
    token: MenuToken,
    drawn: Option<MenuToken>,
    anchor: Option<(u16, u16)>,
}

#[derive(Default)]
pub(crate) struct ActionMenu {
    next: u64,
    menu: Option<Menu>,
    confirmed: Option<MenuAction>,
}

impl ActionMenu {
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

    pub(crate) fn open(
        &mut self,
        title: String,
        actions: Vec<MenuAction>,
        anchor: Option<(u16, u16)>,
    ) -> bool {
        if self.active() || actions.is_empty() || actions.len() > 64 {
            return false;
        }
        let Some(next) = self.next.checked_add(1) else {
            return false;
        };
        self.next = next;
        self.menu = Some(Menu {
            title,
            unavailable: vec![None; actions.len()],
            actions,
            selected: 0,
            first: 0,
            rows: 1,
            token: MenuToken(next),
            drawn: None,
            anchor,
        });
        self.confirmed = None;
        true
    }

    pub(crate) fn view(&self) -> Option<MenuView<'_>> {
        self.menu.as_ref().map(|menu| MenuView {
            title: &menu.title,
            actions: &menu.actions,
            unavailable: &menu.unavailable,
            selected: menu.selected,
            first: menu.first,
            token: menu.token,
            anchor: menu.anchor,
            transferring: self.confirmed.is_some(),
        })
    }

    pub(crate) fn draw(&mut self, draw: Option<MenuDraw>) {
        let Some(draw) = draw else {
            self.cancel();
            return;
        };
        if let Some(menu) = &mut self.menu
            && menu.token == draw.token
            && draw.rows > 0
        {
            menu.rows = draw.rows;
            menu.first = window_first(menu.first, menu.selected, draw.rows, menu.actions.len());
            menu.drawn = Some(draw.token);
        }
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

    fn select_row(&mut self, selected: usize) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        if selected == menu.selected {
            return;
        }
        let Some(next) = self.next.checked_add(1) else {
            self.cancel();
            return;
        };
        self.next = next;
        menu.selected = selected;
        menu.first = window_first(menu.first, selected, menu.rows, menu.actions.len());
        menu.token = MenuToken(next);
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
        let unavailable: Vec<_> = menu.actions.iter().map(|action| query(*action)).collect();
        if menu.unavailable == unavailable {
            return false;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return true;
        }
        let Some(next) = self.next.checked_add(1) else {
            self.cancel();
            return true;
        };
        self.next = next;
        menu.unavailable = unavailable;
        menu.token = MenuToken(next);
        menu.drawn = None;
        true
    }

    pub(crate) fn confirm(&mut self) -> bool {
        if self.confirmed.is_some() {
            return false;
        }
        let Some(menu) = &self.menu else {
            return false;
        };
        if menu.drawn != Some(menu.token) || menu.unavailable[menu.selected].is_some() {
            return false;
        }
        self.confirmed = Some(menu.actions[menu.selected]);
        true
    }

    /// Hover uses only the last painted window and never grants execution authority.
    pub(crate) fn hover(&mut self, token: MenuToken, row: usize) {
        let Some(menu) = &self.menu else {
            return;
        };
        if self.confirmed.is_some()
            || menu.drawn != Some(token)
            || row >= menu.actions.len()
            || row < menu.first
            || row >= menu.first + menu.rows
        {
            return;
        }
        self.select_row(row);
        if let Some(menu) = &mut self.menu {
            // Hover stays within this painted window, so subsequent motion may
            // reuse its geometry. Confirmation still requires the new token to
            // be drawn; keyboard/availability changes discard this proof.
            menu.drawn = Some(token);
        }
    }

    pub(crate) fn click(&mut self, token: MenuToken, row: usize) -> bool {
        let Some(menu) = &mut self.menu else {
            return false;
        };
        if self.confirmed.is_some()
            || menu.drawn != Some(token)
            || menu.token != token
            || row >= menu.actions.len()
            || row < menu.first
            || row >= menu.first + menu.rows
        {
            return false;
        }
        if menu.unavailable[row].is_some() {
            self.select_row(row);
            return false;
        }
        menu.selected = row;
        self.confirmed = Some(menu.actions[row]);
        true
    }

    pub(crate) fn confirmed(&self) -> Option<MenuAction> {
        self.confirmed
    }

    pub(crate) fn take(&mut self) -> Option<MenuAction> {
        let action = self.confirmed.take()?;
        self.menu = None;
        Some(action)
    }

    pub(crate) fn cancel(&mut self) {
        self.menu = None;
        self.confirmed = None;
    }
}

#[cfg(test)]
mod tests;
