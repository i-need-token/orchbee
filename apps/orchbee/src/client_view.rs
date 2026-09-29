#[cfg(test)]
pub(crate) use bindings::{PREFIX_CHORD, PrefixGroup};
pub(crate) const WIDE_VIEWPORT_MINIMUM_COLUMNS: u16 = 96;

mod bindings;
mod keys;

pub(crate) use bindings::{Bindings, PREFIX_GROUPS, PrefixAction, PrefixDesc};
pub(crate) use keys::{Binding, KeyCode, KeyEvent, KeyKind, LabelStyle, Modifiers, format_binding};

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
    ReloadConfiguration,
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
// Not `Copy`: `FocusPanel` carries a `WorkbenchPanelId` (heap identity).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClientAction {
    ReloadConfiguration,
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
    CloseShortcutHelp,
    PageShortcutHelp,
    OpenWorkbenchNavigator,
    CloseWorkbenchNavigator,
    /// `Ctrl+B g` — toggles the session-owned Agent board overlay.
    ToggleAgentBoard,
    CreateTab,
    SelectNextTab,
    SelectPreviousTab,
    SelectTab(u8),
    SelectPreviousContext,
    SelectNextContext,
    /// Pointer gesture: select the navigation entry at this 0-based index.
    SelectContextAt(usize),
    PreviousPresentation,
    NextPresentation,
    /// Pointer destination resolved from the drawn member selector, never a cached ordinal.
    SelectPresentation {
        panel: orchbee_workbench_protocol::WorkbenchPanelId,
        presentation: orchbee_workbench_protocol::TerminalPresentationId,
    },
    MovePanelFocus(PanelDirection),
    /// Pointer gesture: focus the panel with this stable identity in the
    /// active tab.
    FocusPanel(orchbee_workbench_protocol::WorkbenchPanelId),
    /// Client-local zoom on the active tab: draw the focused leaf across
    /// the whole pane area while hidden leaves stay attached. Volatile
    /// session view state — never a backend action.
    ToggleZoom,
    SplitPanel(PanelDirection),
    ClosePanel,
    MovePanel(PanelDirection),
    TogglePointerCapture,
    /// `Ctrl+B PageUp` while help is closed: enter retained-history mode on
    /// the focus pane. The session owns admission; an unenterable pane
    /// simply stays Live.
    EnterHistory,
    ForwardLiteralPrefix,
    DismissPrefix,
    UnknownPrefix,
    Exit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    ConfigurationPublished,
    ConfigurationRestart,
    ConfigurationFailed,
    ConfigurationUnknown,
    BackendUnavailable(BackendAction),
    TerminalInputUnavailable,
    UnknownPrefix,
    /// A pointer gesture's queued input was dropped: the pending-focus
    /// queue overflowed or its bounded wait expired.
    PointerInputDropped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientKey {
    ControlB,
    Escape,
    Plain(char),
    Other,
}

/// What the decision layer consumes: one typed key event, or a non-key
/// complete sequence (`NonKey`, e.g. an unknown CSI run under an armed
/// prefix). Owners never see raw bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrefixInput {
    Key(KeyEvent),
    NonKey,
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
    /// The action changed the rendered pane layout — a zoom toggle, or a
    /// focus move while zoomed — so the dispatcher re-observes geometry
    /// and invalidates the pointer hit map. The navigator disposition
    /// stays unchanged (kept).
    LayoutChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientTransition {
    next: ClientView,
    action: Option<ClientAction>,
    shell: ShellCommand,
    layout_changed: bool,
}

impl ClientTransition {
    pub(crate) fn action(&self) -> Option<ClientAction> {
        self.action.clone()
    }

    pub(crate) fn shell(&self) -> ShellCommand {
        self.shell
    }

    pub(crate) fn layout_changed(&self) -> bool {
        self.layout_changed
    }
}

impl ClientAction {
    pub(crate) fn backend_action(&self) -> Option<BackendAction> {
        match self {
            Self::ReloadConfiguration => Some(BackendAction::ReloadConfiguration),
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
            Self::SelectTab(position) => Some(BackendAction::SelectTab(*position)),
            Self::SelectPreviousContext => Some(BackendAction::SelectPreviousContext),
            Self::SelectNextContext => Some(BackendAction::SelectNextContext),
            Self::PreviousPresentation
            | Self::NextPresentation
            | Self::SelectPresentation { .. }
            | Self::MovePanelFocus(_)
            | Self::FocusPanel(_)
            | Self::ToggleZoom
            | Self::SelectContextAt(_) => None,
            Self::SplitPanel(_) => Some(BackendAction::SplitPanel),
            Self::ClosePanel => Some(BackendAction::ClosePanel),
            Self::MovePanel(_) => Some(BackendAction::MovePanel),
            Self::ToggleBusinessSidebar
            | Self::ConfirmClose
            | Self::CancelClose
            | Self::OpenShortcutHelp
            | Self::CloseShortcutHelp
            | Self::PageShortcutHelp
            | Self::OpenWorkbenchNavigator
            | Self::CloseWorkbenchNavigator
            | Self::ToggleAgentBoard
            | Self::TogglePointerCapture
            | Self::EnterHistory
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientView {
    bindings: std::sync::Arc<Bindings>,
    notice: Option<Notice>,
    overlay: Option<Overlay>,
    prefix_active: bool,
    viewport: ViewportClass,
    wide_sidebar_visible: bool,
    /// The shortcut-help page the renderer was asked for. Client-local
    /// presentation state only — it is never put on the wire or into
    /// NativeSession business. The renderer clamps it against the real
    /// page count on every draw and the UI loop writes the effective page
    /// back through `settle_help_page`; render never mutates it.
    help_page: usize,
    /// The pointer-capture release flag: `Ctrl+B m` and `--mouse off`
    /// release host mouse reporting; while released the status line carries
    /// the persistent indicator and every Client pointer gesture is inert.
    /// The dispatcher settles it only after the terminal mode write
    /// succeeded, so it always mirrors the armed state.
    pointer_released: bool,
}

impl ClientView {
    /// The largest page index the dispatcher may ask for. The registry's
    /// row count bounds the real page count (every page shows at least one
    /// row), so a held PageDown can never grow this counter past the
    /// number of binding groups — no allocation scales with input.
    const HELP_PAGE_BOUND: usize = PREFIX_GROUPS.len();

    pub(crate) fn new(width: u16) -> Self {
        Self {
            bindings: std::sync::Arc::new(Bindings::default()),
            notice: None,
            overlay: None,
            prefix_active: false,
            viewport: ViewportClass::from_width(width),
            wide_sidebar_visible: true,
            help_page: 0,
            pointer_released: false,
        }
    }

    pub(crate) const fn notice(&self) -> Option<Notice> {
        self.notice
    }

    pub(crate) const fn overlay(&self) -> Option<Overlay> {
        self.overlay
    }

    pub(crate) const fn prefix_active(&self) -> bool {
        self.prefix_active
    }

    /// The help page the renderer should draw. Consumed by the paged
    /// `render_shortcut_help` seam; always a bounded counter.
    pub(crate) const fn help_page(&self) -> usize {
        self.help_page
    }

    /// Settles the renderer-clamped effective page after a draw. The UI
    /// loop is the only writer: the renderer reports which page it
    /// actually painted (or clamped to after a resize shrank the page
    /// count) and this state follows that receipt.
    pub(crate) fn settle_help_page(&mut self, effective_page: usize) {
        self.help_page = effective_page.min(Self::HELP_PAGE_BOUND);
    }

    pub(crate) const fn sidebar_surface(&self) -> SidebarSurface {
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

    /// The pending-focus queue overflowed or expired — the queued pointer
    /// input was dropped and the user sees why.
    pub(crate) fn report_pointer_input_dropped(&mut self) {
        self.notice = Some(Notice::PointerInputDropped);
    }

    /// Whether host pointer reporting is currently released (`Ctrl+B m` or
    /// `--mouse off`). Read by the status line's persistent indicator.
    pub(crate) const fn pointer_released(&self) -> bool {
        self.pointer_released
    }

    /// Settles the released flag after the terminal mode write succeeded.
    pub(crate) fn settle_pointer_released(&mut self, released: bool) {
        self.pointer_released = released;
    }

    /// An outside click on an open overlay dismisses it without producing a
    /// semantic action — the gesture is consumed, never forwarded beneath.
    pub(crate) fn dismiss_overlay(&mut self) {
        self.overlay = None;
    }

    #[cfg(windows)]
    pub(crate) fn dismiss_focus_modes(&mut self) {
        self.dismiss_overlay();
        self.prefix_active = false;
    }

    /// Applies the navigator settlement of a pointer-issued action. Pointer
    /// intents carry no `ClientTransition`; this is the same disposition
    /// effect `commit` applies for keyboard-issued actions.
    pub(crate) fn commit_pointer(&mut self, disposition: SessionActionDisposition) {
        if disposition == SessionActionDisposition::DismissNavigator
            && self.overlay == Some(Overlay::WorkbenchNavigator)
        {
            self.overlay = None;
        }
    }

    pub(crate) fn configuration_notice(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }

    pub(crate) fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    #[cfg(windows)]
    pub(crate) fn configure_keys(&mut self, keys: &orchbee_local_site_runtime::WindowsKeySettings) {
        self.bindings = std::sync::Arc::new(Bindings::configured(keys));
        self.prefix_active = false;
    }

    pub(crate) fn decide(&self, input: PrefixInput) -> ClientTransition {
        let mut next = self.clone();
        if self.prefix_active {
            let (action, shell) = next.decide_prefix(input);
            return ClientTransition {
                action: Some(action),
                shell,
                layout_changed: matches!(self.sidebar_surface(), SidebarSurface::Docked)
                    != matches!(next.sidebar_surface(), SidebarSurface::Docked),
                next,
            };
        }

        if let PrefixInput::Key(event) = input
            && self.bindings.prefix.matches(event)
        {
            next.prefix_active = true;
            return ClientTransition {
                next,
                action: None,
                shell: ShellCommand::None,
                layout_changed: false,
            };
        }

        if let PrefixInput::Key(event) = input
            && let Some(member) = self.bindings.find(event, false)
        {
            let (action, shell) = next.decide_member(input, Some(member));
            return ClientTransition {
                layout_changed: self.sidebar_surface() != next.sidebar_surface(),
                next,
                action: Some(action),
                shell,
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

    fn decide_prefix(&mut self, input: PrefixInput) -> (ClientAction, ShellCommand) {
        self.prefix_active = false;
        self.notice = None;

        let member = match input {
            PrefixInput::Key(event) => self.bindings.find(event, true).cloned(),
            PrefixInput::NonKey => None,
        };
        self.decide_member(input, member.as_ref())
    }

    fn decide_member(
        &mut self,
        input: PrefixInput,
        member: Option<&bindings::PrefixMember>,
    ) -> (ClientAction, ShellCommand) {
        self.notice = None;
        let Some(member) = member else {
            self.notice = Some(Notice::UnknownPrefix);
            return (ClientAction::UnknownPrefix, ShellCommand::None);
        };
        match &member.action {
            PrefixAction::Client(action) => (action.clone(), member.shell),
            PrefixAction::SelectTabDigit => {
                let number = match input {
                    PrefixInput::Key(event) => match event.normalized() {
                        (KeyCode::Char(c), _) => c as u8 - b'0',
                        _ => 1,
                    },
                    PrefixInput::NonKey => 1,
                };
                (ClientAction::SelectTab(number), member.shell)
            }
            PrefixAction::SidebarToggle => {
                self.toggle_sidebar();
                (ClientAction::ToggleBusinessSidebar, ShellCommand::None)
            }
            PrefixAction::NavigatorToggle => {
                if self.overlay == Some(Overlay::WorkbenchNavigator) {
                    self.overlay = None;
                    (ClientAction::CloseWorkbenchNavigator, ShellCommand::None)
                } else {
                    self.overlay = Some(Overlay::WorkbenchNavigator);
                    (ClientAction::OpenWorkbenchNavigator, ShellCommand::None)
                }
            }
            PrefixAction::BoardToggle => {
                // The board lives session-side and is mutually exclusive
                // with every client overlay: toggling it always takes the
                // overlay slot — the session decides open vs. close.
                self.overlay = None;
                (ClientAction::ToggleAgentBoard, ShellCommand::None)
            }
            PrefixAction::ShowHelp => {
                // The same binding opens and closes the help overlay so the
                // overlay is always exit-discoverable; ordinary keys keep
                // reaching the pane because nothing captures input here.
                // Either way the page resets: a reopened help starts at
                // page 0.
                self.settle_help_page(0);
                if self.overlay == Some(Overlay::ShortcutHelp) {
                    self.overlay = None;
                    (ClientAction::CloseShortcutHelp, ShellCommand::None)
                } else {
                    self.overlay = Some(Overlay::ShortcutHelp);
                    (ClientAction::OpenShortcutHelp, ShellCommand::None)
                }
            }
            PrefixAction::HelpPagePrevious | PrefixAction::HelpPageNext => {
                if self.overlay == Some(Overlay::ShortcutHelp) {
                    // Only an open help pages: the page key is consumed
                    // while the overlay is up, and the renderer's clamped
                    // page settles the upper bound on the next draw.
                    self.settle_help_page(match member.action {
                        PrefixAction::HelpPagePrevious => self.help_page().saturating_sub(1),
                        _ => self.help_page() + 1,
                    });
                    (ClientAction::PageShortcutHelp, ShellCommand::None)
                } else if member.action == PrefixAction::HelpPagePrevious {
                    // With help closed, `Ctrl+B PageUp` is the history
                    // entry gesture; the session owns admission.
                    (ClientAction::EnterHistory, ShellCommand::None)
                } else {
                    // With help closed the chord is inert — the same
                    // unknown-prefix outcome any unbound member key gets.
                    self.notice = Some(Notice::UnknownPrefix);
                    (ClientAction::UnknownPrefix, ShellCommand::None)
                }
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
