pub(crate) const WIDE_VIEWPORT_MINIMUM_COLUMNS: u16 = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ViewportClass {
    Narrow,
    Wide,
}

impl ViewportClass {
    pub(crate) const fn from_width(width: u16) -> Self {
        if width >= WIDE_VIEWPORT_MINIMUM_COLUMNS {
            Self::Wide
        } else {
            Self::Narrow
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Overlay {
    BusinessSidebar,
    ShortcutHelp,
    WorkbenchNavigator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PanelDirection {
    Down,
    Left,
    Right,
    Up,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackendAction {
    CreateWorkspace,
    ConfigureWorkspaceDirectory,
    RenameWorkspace,
    RenameProject,
    RenameTab,
    CloseTab,
    MovePreviousTab,
    MoveNextTab,
    CreateTab,
    SelectNextTab,
    SelectPreviousTab,
    SelectTab(u8),
    SelectPreviousContext,
    SelectNextContext,
    SplitPanel,
    ClosePanel,
    MovePanel,
}

// This value is shared with the native Session reducer while remaining protocol-free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientAction {
    RequestWorkspaceCreate,
    RequestWorkspaceDirectory,
    RequestWorkspaceRename,
    RequestProjectRename,
    RequestRenameTab,
    RequestCloseTab,
    ConfirmClose,
    CancelClose,
    MovePreviousTab,
    MoveNextTab,
    ToggleBusinessSidebar,
    OpenShortcutHelp,
    OpenWorkbenchNavigator,
    CloseWorkbenchNavigator,
    CreateTab,
    SelectNextTab,
    SelectPreviousTab,
    SelectTab(u8),
    SelectPreviousContext,
    SelectNextContext,
    MovePanelFocus(PanelDirection),
    SplitPanel(PanelDirection),
    ClosePanel,
    MovePanel(PanelDirection),
    ForwardLiteralPrefix,
    DismissPrefix,
    UnknownPrefix,
    Exit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    BackendUnavailable(BackendAction),
    TerminalInputUnavailable,
    UnknownPrefix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientKey {
    ControlB,
    Escape,
    Plain(char),
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellCommand {
    Exit,
    ForwardTerminal,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionActionDisposition {
    KeepNavigator,
    DismissNavigator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClientTransition {
    next: ClientView,
    action: Option<ClientAction>,
    shell: ShellCommand,
    layout_changed: bool,
}

impl ClientTransition {
    pub(crate) const fn action(self) -> Option<ClientAction> {
        self.action
    }

    pub(crate) const fn shell(self) -> ShellCommand {
        self.shell
    }

    pub(crate) const fn layout_changed(self) -> bool {
        self.layout_changed
    }
}

impl ClientAction {
    pub(crate) const fn backend_action(self) -> Option<BackendAction> {
        match self {
            Self::RequestWorkspaceCreate => Some(BackendAction::CreateWorkspace),
            Self::RequestWorkspaceDirectory => Some(BackendAction::ConfigureWorkspaceDirectory),
            Self::RequestWorkspaceRename => Some(BackendAction::RenameWorkspace),
            Self::RequestProjectRename => Some(BackendAction::RenameProject),
            Self::RequestRenameTab => Some(BackendAction::RenameTab),
            Self::RequestCloseTab => Some(BackendAction::CloseTab),
            Self::CreateTab => Some(BackendAction::CreateTab),
            Self::MovePreviousTab => Some(BackendAction::MovePreviousTab),
            Self::MoveNextTab => Some(BackendAction::MoveNextTab),
            Self::SelectNextTab => Some(BackendAction::SelectNextTab),
            Self::SelectPreviousTab => Some(BackendAction::SelectPreviousTab),
            Self::SelectTab(position) => Some(BackendAction::SelectTab(position)),
            Self::SelectPreviousContext => Some(BackendAction::SelectPreviousContext),
            Self::SelectNextContext => Some(BackendAction::SelectNextContext),
            Self::MovePanelFocus(_) => None,
            Self::SplitPanel(_) => Some(BackendAction::SplitPanel),
            Self::ClosePanel => Some(BackendAction::ClosePanel),
            Self::MovePanel(_) => Some(BackendAction::MovePanel),
            Self::ToggleBusinessSidebar
            | Self::ConfirmClose
            | Self::CancelClose
            | Self::OpenShortcutHelp
            | Self::OpenWorkbenchNavigator
            | Self::CloseWorkbenchNavigator
            | Self::ForwardLiteralPrefix
            | Self::DismissPrefix
            | Self::UnknownPrefix
            | Self::Exit => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarSurface {
    Docked,
    Hidden,
    Overlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClientView {
    notice: Option<Notice>,
    overlay: Option<Overlay>,
    prefix_active: bool,
    viewport: ViewportClass,
    wide_sidebar_visible: bool,
}

impl ClientView {
    pub(crate) const fn new(width: u16) -> Self {
        Self {
            notice: None,
            overlay: None,
            prefix_active: false,
            viewport: ViewportClass::from_width(width),
            wide_sidebar_visible: true,
        }
    }

    pub(crate) const fn notice(self) -> Option<Notice> {
        self.notice
    }

    pub(crate) const fn overlay(self) -> Option<Overlay> {
        self.overlay
    }

    pub(crate) const fn prefix_active(self) -> bool {
        self.prefix_active
    }

    pub(crate) const fn sidebar_surface(self) -> SidebarSurface {
        match self.viewport {
            ViewportClass::Wide if self.wide_sidebar_visible => SidebarSurface::Docked,
            ViewportClass::Narrow if matches!(self.overlay, Some(Overlay::BusinessSidebar)) => {
                SidebarSurface::Overlay
            }
            ViewportClass::Narrow | ViewportClass::Wide => SidebarSurface::Hidden,
        }
    }

    pub(crate) fn resize(&mut self, width: u16) {
        let next = ViewportClass::from_width(width);
        if next == ViewportClass::Wide && self.overlay == Some(Overlay::BusinessSidebar) {
            self.overlay = None;
        }
        self.viewport = next;
    }

    pub(crate) fn report_backend_unavailable(&mut self, action: BackendAction) {
        self.notice = Some(Notice::BackendUnavailable(action));
    }

    pub(crate) fn report_terminal_input_unavailable(&mut self) {
        self.notice = Some(Notice::TerminalInputUnavailable);
    }

    pub(crate) fn decide(&self, key: ClientKey) -> ClientTransition {
        let mut next = *self;
        if self.prefix_active {
            let (action, shell) = next.decide_prefix(key);
            return ClientTransition {
                next,
                action: Some(action),
                shell,
                layout_changed: matches!(self.sidebar_surface(), SidebarSurface::Docked)
                    != matches!(next.sidebar_surface(), SidebarSurface::Docked),
            };
        }

        if key == ClientKey::ControlB {
            next.prefix_active = true;
            return ClientTransition {
                next,
                action: None,
                shell: ShellCommand::None,
                layout_changed: false,
            };
        }

        ClientTransition {
            next,
            action: None,
            shell: ShellCommand::ForwardTerminal,
            layout_changed: false,
        }
    }

    pub(crate) fn commit(
        &mut self,
        transition: ClientTransition,
        disposition: SessionActionDisposition,
    ) {
        let mut next = transition.next;
        if disposition == SessionActionDisposition::DismissNavigator
            && next.overlay == Some(Overlay::WorkbenchNavigator)
        {
            next.overlay = None;
        }
        *self = next;
    }

    fn decide_prefix(&mut self, key: ClientKey) -> (ClientAction, ShellCommand) {
        self.prefix_active = false;
        self.notice = None;

        match key {
            ClientKey::ControlB => (
                ClientAction::ForwardLiteralPrefix,
                ShellCommand::ForwardTerminal,
            ),
            ClientKey::Escape => (ClientAction::DismissPrefix, ShellCommand::None),
            ClientKey::Plain('b') => {
                self.toggle_sidebar();
                (ClientAction::ToggleBusinessSidebar, ShellCommand::None)
            }
            ClientKey::Plain('w') => {
                if self.overlay == Some(Overlay::WorkbenchNavigator) {
                    self.overlay = None;
                    (ClientAction::CloseWorkbenchNavigator, ShellCommand::None)
                } else {
                    self.overlay = Some(Overlay::WorkbenchNavigator);
                    (ClientAction::OpenWorkbenchNavigator, ShellCommand::None)
                }
            }
            ClientKey::Plain('?') => {
                self.overlay = Some(Overlay::ShortcutHelp);
                (ClientAction::OpenShortcutHelp, ShellCommand::None)
            }
            ClientKey::Plain('q' | 'd') => (ClientAction::Exit, ShellCommand::Exit),
            ClientKey::Plain('a') => (ClientAction::RequestWorkspaceCreate, ShellCommand::None),
            ClientKey::Plain('e') => (ClientAction::RequestWorkspaceDirectory, ShellCommand::None),
            ClientKey::Plain('r') => (ClientAction::RequestWorkspaceRename, ShellCommand::None),
            ClientKey::Plain('R') => (ClientAction::RequestProjectRename, ShellCommand::None),
            ClientKey::Plain('c') => (ClientAction::CreateTab, ShellCommand::None),
            ClientKey::Plain('x') => (ClientAction::RequestCloseTab, ShellCommand::None),
            ClientKey::Plain(',') => (ClientAction::RequestRenameTab, ShellCommand::None),
            ClientKey::Plain('<') => (ClientAction::MovePreviousTab, ShellCommand::None),
            ClientKey::Plain('>') => (ClientAction::MoveNextTab, ShellCommand::None),
            ClientKey::Plain('n') => (ClientAction::SelectNextTab, ShellCommand::None),
            ClientKey::Plain('p') => (ClientAction::SelectPreviousTab, ShellCommand::None),
            ClientKey::Plain('[') => (ClientAction::SelectPreviousContext, ShellCommand::None),
            ClientKey::Plain(']') => (ClientAction::SelectNextContext, ShellCommand::None),
            ClientKey::Plain(character @ '1'..='9') => {
                let number = character as u8 - b'0';
                (ClientAction::SelectTab(number), ShellCommand::None)
            }
            ClientKey::Plain('h') => (
                ClientAction::MovePanelFocus(PanelDirection::Left),
                ShellCommand::None,
            ),
            ClientKey::Plain('j') => (
                ClientAction::MovePanelFocus(PanelDirection::Down),
                ShellCommand::None,
            ),
            ClientKey::Plain('k') => (
                ClientAction::MovePanelFocus(PanelDirection::Up),
                ShellCommand::None,
            ),
            ClientKey::Plain('l') => (
                ClientAction::MovePanelFocus(PanelDirection::Right),
                ShellCommand::None,
            ),
            // Provisional panel-operation bindings aligned with the input-vocabulary
            // draft table; see the README shortcut table.
            ClientKey::Plain('-') => (
                ClientAction::SplitPanel(PanelDirection::Down),
                ShellCommand::None,
            ),
            ClientKey::Plain('_') => (
                ClientAction::SplitPanel(PanelDirection::Up),
                ShellCommand::None,
            ),
            ClientKey::Plain('\\') => (
                ClientAction::SplitPanel(PanelDirection::Left),
                ShellCommand::None,
            ),
            ClientKey::Plain('|') => (
                ClientAction::SplitPanel(PanelDirection::Right),
                ShellCommand::None,
            ),
            ClientKey::Plain('X') => (ClientAction::ClosePanel, ShellCommand::None),
            ClientKey::Plain('H') => (
                ClientAction::MovePanel(PanelDirection::Left),
                ShellCommand::None,
            ),
            ClientKey::Plain('J') => (
                ClientAction::MovePanel(PanelDirection::Down),
                ShellCommand::None,
            ),
            ClientKey::Plain('K') => (
                ClientAction::MovePanel(PanelDirection::Up),
                ShellCommand::None,
            ),
            ClientKey::Plain('L') => (
                ClientAction::MovePanel(PanelDirection::Right),
                ShellCommand::None,
            ),
            ClientKey::Other | ClientKey::Plain(_) => {
                self.notice = Some(Notice::UnknownPrefix);
                (ClientAction::UnknownPrefix, ShellCommand::None)
            }
        }
    }

    fn toggle_sidebar(&mut self) {
        match self.viewport {
            ViewportClass::Wide => {
                self.wide_sidebar_visible = !self.wide_sidebar_visible;
                self.overlay = None;
            }
            ViewportClass::Narrow => {
                self.overlay = if self.overlay == Some(Overlay::BusinessSidebar) {
                    None
                } else {
                    Some(Overlay::BusinessSidebar)
                };
            }
        }
    }
}

#[cfg(test)]
mod tests;
