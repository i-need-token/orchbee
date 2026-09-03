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
    CreateTab,
    MovePanelFocus(PanelDirection),
    SelectNextTab,
    SelectPreviousTab,
    SelectTab(u8),
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
    Backend(BackendAction),
    Exit,
    ForwardTerminal,
    None,
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

    pub(crate) fn transition(&mut self, key: ClientKey) -> ShellCommand {
        if self.prefix_active {
            return self.transition_prefix(key);
        }

        if key == ClientKey::ControlB {
            self.prefix_active = true;
            self.notice = None;
            return ShellCommand::None;
        }

        ShellCommand::ForwardTerminal
    }

    fn transition_prefix(&mut self, key: ClientKey) -> ShellCommand {
        self.prefix_active = false;
        self.notice = None;

        match key {
            ClientKey::ControlB => ShellCommand::ForwardTerminal,
            ClientKey::Escape => ShellCommand::None,
            ClientKey::Plain('b') => {
                self.toggle_sidebar();
                ShellCommand::None
            }
            ClientKey::Plain('w') => {
                self.overlay = Some(Overlay::WorkbenchNavigator);
                ShellCommand::None
            }
            ClientKey::Plain('?') => {
                self.overlay = Some(Overlay::ShortcutHelp);
                ShellCommand::None
            }
            ClientKey::Plain('q' | 'd') => ShellCommand::Exit,
            ClientKey::Plain('c') => ShellCommand::Backend(BackendAction::CreateTab),
            ClientKey::Plain('n') => ShellCommand::Backend(BackendAction::SelectNextTab),
            ClientKey::Plain('p') => ShellCommand::Backend(BackendAction::SelectPreviousTab),
            ClientKey::Plain(character @ '1'..='9') => {
                let number = character as u8 - b'0';
                ShellCommand::Backend(BackendAction::SelectTab(number))
            }
            ClientKey::Plain('h') => {
                ShellCommand::Backend(BackendAction::MovePanelFocus(PanelDirection::Left))
            }
            ClientKey::Plain('j') => {
                ShellCommand::Backend(BackendAction::MovePanelFocus(PanelDirection::Down))
            }
            ClientKey::Plain('k') => {
                ShellCommand::Backend(BackendAction::MovePanelFocus(PanelDirection::Up))
            }
            ClientKey::Plain('l') => {
                ShellCommand::Backend(BackendAction::MovePanelFocus(PanelDirection::Right))
            }
            ClientKey::Other | ClientKey::Plain(_) => {
                self.notice = Some(Notice::UnknownPrefix);
                ShellCommand::None
            }
        }
    }

    fn toggle_sidebar(&mut self) {
        match self.viewport {
            ViewportClass::Wide => {
                self.wide_sidebar_visible = !self.wide_sidebar_visible;
                if self.overlay == Some(Overlay::BusinessSidebar) {
                    self.overlay = None;
                }
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
