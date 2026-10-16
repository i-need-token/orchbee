#[cfg(test)]
pub(crate) use bindings::{PREFIX_CHORD, PREFIX_GROUPS, PrefixGroup};
pub(crate) const WIDE_VIEWPORT_MINIMUM_COLUMNS: u16 = 96;

mod bindings;
#[cfg(windows)]
mod copy;
pub(crate) mod help;
mod keys;
#[cfg(windows)]
pub(crate) mod navigator;
#[cfg(windows)]
mod resize;

pub(crate) use bindings::{Bindings, PrefixAction, PrefixDesc};
#[cfg(windows)]
pub(crate) use copy::{
    CopyHintLabel, CopyModeKey, CopySearchCommand, decide as copy_mode_key,
    footer_bindings as copy_footer_bindings, search_command as copy_search_command,
};
#[cfg(windows)]
pub(crate) use keys::format_configuration_binding;
pub(crate) use keys::{
    Binding, KeyCode, KeyEvent, KeyKind, KeyLabelGlyphs, LabelStyle, Modifiers, format_binding,
    format_binding_with_glyphs, join_key_labels,
};

#[cfg(windows)]
const SIDEBAR_HELP_PAGES: [Binding; 2] = [
    Binding::new(KeyCode::PageUp, Modifiers::NONE),
    Binding::new(KeyCode::PageDown, Modifiers::NONE),
];

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
    QuickCreateWorkspace,
    #[cfg(windows)]
    CreateWorktree,
    #[cfg(windows)]
    OpenWorktree,
    CloseWorkspace,
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
    #[cfg(windows)]
    TabMenu {
        action: crate::action_menu::MenuAction,
        tab_id: Option<orchbee_workbench_protocol::WorkbenchTabId>,
    },
    #[cfg(windows)]
    ToggleTabGroup(orchbee_workbench_protocol::WorkbenchTabGroupId),
    #[cfg(windows)]
    OpenActionMenu,
    OpenSettings,
    #[cfg(windows)]
    ConfiguredCommand(String),
    #[cfg(windows)]
    EnterResizeMode,
    #[cfg(windows)]
    EnterCopyMode,
    #[cfg(windows)]
    ExitResizeMode,
    #[cfg(windows)]
    ResizePane {
        direction: PanelDirection,
        fine: bool,
    },
    ReloadConfiguration,
    RequestWorkspaceCreate,
    RequestQuickWorkspace,
    RequestProjectQuickWorkspace(Box<crate::terminal_session::ProjectQuickWorkspaceRequest>),
    #[cfg(windows)]
    RequestWorktreeCreate,
    #[cfg(windows)]
    RequestWorktreeOpen,
    RequestWorkspaceClose,
    RequestWorkspaceCloseAt(orchbee_workbench_protocol::WorkbenchWorkspaceId),
    RequestControlTakeover,
    RequestWorkspaceDirectory,
    RequestWorkspaceRename,
    RequestProjectRename,
    RequestTargetedProjectRename(Box<crate::terminal_session::ProjectRenameRequest>),
    RequestWorkspaceMove,
    RequestProjectDissolve,
    RequestWorkspaceOrganization(Box<crate::terminal_session::WorkspaceOrganizationRequest>),
    RequestProjectCloseAll(Option<orchbee_workbench_protocol::WorkbenchProjectId>),
    RequestRenameTab,
    RequestCloseTab,
    ConfirmClose,
    CancelClose,
    MovePreviousTab,
    MoveNextTab,
    ToggleBusinessSidebar,
    #[cfg(windows)]
    FocusBusinessSidebar,
    OpenShortcutHelp,
    CloseShortcutHelp,
    PageShortcutHelp,
    OpenWorkbenchNavigator,
    #[cfg(windows)]
    OpenMachines,
    #[cfg(windows)]
    PreviousAgent,
    #[cfg(windows)]
    NextAgent,
    #[cfg(windows)]
    FocusAgent(u8),
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
    #[cfg(any(not(windows), test))]
    SelectContextAt(usize),
    PreviousPresentation,
    NextPresentation,
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
    #[cfg(windows)]
    WorkspaceNavigationUnavailable,
    #[cfg(windows)]
    NavigatorChanged,
    #[cfg(windows)]
    NavigatorTooSmall,
    #[cfg(windows)]
    ResizeBusy,
    #[cfg(windows)]
    ResizeFailed,
    #[cfg(windows)]
    ResizeUnknown,
    #[cfg(windows)]
    ResizeNoSplit,
    #[cfg(windows)]
    ResizeLimit,
    CheckpointRecovered,
    CheckpointEmpty,
    ConfigurationPublished,
    ConfigurationWarning,
    ConfigurationRestart,
    ConfigurationFailed,
    ConfigurationUnknown,
    WindowTitleUnavailable,
    NotificationUnavailable,
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
    Enter,
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
    /// Native admission redirected empty compatibility content to one fresh
    /// Workspace launch. Preserve its shell entry intent across control ticks.
    #[cfg(windows)]
    WorkspaceCreationStarted,
}

impl SessionActionDisposition {
    pub(crate) fn changes_layout(self) -> bool {
        match self {
            Self::LayoutChanged => true,
            #[cfg(windows)]
            Self::WorkspaceCreationStarted => true,
            Self::KeepNavigator | Self::DismissNavigator => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientTransition {
    next: ClientView,
    action: Option<ClientAction>,
    shell: ShellCommand,
    layout_changed: bool,
}

impl ClientTransition {
    #[cfg(windows)]
    pub(crate) const fn prefix_active(&self) -> bool {
        self.next.prefix_active
    }

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
            #[cfg(windows)]
            Self::OpenActionMenu | Self::FocusBusinessSidebar => None,
            Self::OpenSettings => None,
            #[cfg(windows)]
            Self::EnterResizeMode
            | Self::EnterCopyMode
            | Self::ExitResizeMode
            | Self::ResizePane { .. } => None,
            #[cfg(windows)]
            Self::ConfiguredCommand(_) => None,
            #[cfg(windows)]
            Self::OpenMachines | Self::PreviousAgent | Self::NextAgent | Self::FocusAgent(_) => {
                None
            }
            #[cfg(windows)]
            Self::TabMenu { .. } | Self::ToggleTabGroup(_) => None,
            Self::ReloadConfiguration => Some(BackendAction::ReloadConfiguration),
            Self::RequestWorkspaceCreate => Some(BackendAction::CreateWorkspace),
            Self::RequestQuickWorkspace | Self::RequestProjectQuickWorkspace(_) => {
                Some(BackendAction::QuickCreateWorkspace)
            }
            #[cfg(windows)]
            Self::RequestWorktreeCreate => Some(BackendAction::CreateWorktree),
            #[cfg(windows)]
            Self::RequestWorktreeOpen => Some(BackendAction::OpenWorktree),
            Self::RequestWorkspaceClose => Some(BackendAction::CloseWorkspace),
            Self::RequestControlTakeover => None,
            Self::RequestWorkspaceDirectory => Some(BackendAction::ConfigureWorkspaceDirectory),
            Self::RequestWorkspaceRename => Some(BackendAction::RenameWorkspace),
            Self::RequestProjectRename | Self::RequestTargetedProjectRename(_) => {
                Some(BackendAction::RenameProject)
            }
            Self::RequestWorkspaceMove
            | Self::RequestWorkspaceCloseAt(_)
            | Self::RequestProjectDissolve
            | Self::RequestWorkspaceOrganization(_)
            | Self::RequestProjectCloseAll(_) => None,
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
            | Self::MovePanelFocus(_)
            | Self::FocusPanel(_)
            | Self::ToggleZoom => None,
            #[cfg(any(not(windows), test))]
            Self::SelectContextAt(_) => None,
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
    Docked(crate::tui_layout::SidebarWidth),
    Hidden,
    Overlay(crate::tui_layout::SidebarWidth),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientView {
    pub(crate) user_preferences: Option<orchbee_workbench_protocol::UserPreferencesValues>,
    #[cfg(windows)]
    pub(crate) machines: crate::machine_picker::Picker,
    bindings: std::sync::Arc<Bindings>,
    #[cfg(windows)]
    theme_name: Option<orchbee_local_site_runtime::WindowsThemeName>,
    theme: Option<crate::tui_theme::Theme>,
    color_level: crate::tui_theme::ColorLevel,
    pub(crate) notifications: crate::notifications::State,
    #[cfg(windows)]
    pub(crate) window_title: Option<orchbee_workbench_protocol::WindowTitleSnapshot>,
    notice: Option<Notice>,
    #[cfg(windows)]
    configuration_warning: bool,
    overlay: Option<Overlay>,
    // Some(false) still tracks a launch opened without a compact sidebar;
    // None means a newer intent has retired that launch's return permission.
    #[cfg(windows)]
    project_quick_entry_eligible: Option<bool>,
    #[cfg(windows)]
    pub(crate) navigator: navigator::Navigator,
    prefix_active: bool,
    #[cfg(windows)]
    resize_mode: bool,
    #[cfg(windows)]
    pub(crate) resize_pointer_fine: bool,
    /// Read-only projection of the session-owned copy episode, used only for discovery.
    #[cfg(windows)]
    copy_mode_active: bool,
    #[cfg(windows)]
    pub(crate) resize_pending: bool,
    viewport: ViewportClass,
    wide_sidebar_visible: bool,
    /// Client-local Help incarnation, requested page, and successful-frame
    /// proof. It never enters NativeSession business state.
    help: help::State,
    /// The pointer-capture release flag: `Ctrl+B m` and `--mouse off`
    /// release host mouse reporting; while released the status line carries
    /// the persistent indicator and every Client pointer gesture is inert.
    /// The dispatcher settles it only after the terminal mode write
    /// succeeded, so it always mirrors the armed state.
    pointer_released: bool,
}

impl ClientView {
    // The resolved registry, including configured-only actions, bounds held paging input.
    fn help_page_bound(&self) -> usize {
        #[cfg(windows)]
        if self.machines.sidebar_navigation_active() {
            return crate::machine_picker::SIDEBAR_BINDINGS.len() + 1;
        }
        #[cfg(windows)]
        if self.copy_mode_active {
            return self.copy_help_rows().len();
        }
        #[cfg(windows)]
        if self.resize_mode {
            return self.resize_help_rows().len();
        }
        self.bindings.descriptions().len()
    }

    pub(crate) fn new(width: u16) -> Self {
        Self {
            user_preferences: None,
            #[cfg(windows)]
            machines: crate::machine_picker::Picker::default(),
            bindings: std::sync::Arc::new(Bindings::default()),
            #[cfg(windows)]
            theme_name: None,
            theme: None,
            color_level: crate::tui_theme::ColorLevel::Ansi16,
            notifications: crate::notifications::State::default(),
            #[cfg(windows)]
            window_title: None,
            notice: None,
            #[cfg(windows)]
            configuration_warning: false,
            overlay: None,
            #[cfg(windows)]
            project_quick_entry_eligible: None,
            #[cfg(windows)]
            navigator: navigator::Navigator::default(),
            prefix_active: false,
            #[cfg(windows)]
            resize_mode: false,
            #[cfg(windows)]
            resize_pointer_fine: false,
            #[cfg(windows)]
            copy_mode_active: false,
            #[cfg(windows)]
            resize_pending: false,
            viewport: ViewportClass::from_width(width),
            wide_sidebar_visible: true,
            help: help::State::default(),
            pointer_released: false,
        }
    }

    pub(crate) const fn notice(&self) -> Option<Notice> {
        if self.notice.is_some() {
            return self.notice;
        }
        #[cfg(windows)]
        if self.configuration_warning {
            return Some(Notice::ConfigurationWarning);
        }
        None
    }

    /// Host capabilities are sampled by the Shell, never by a render consumer.
    pub(crate) fn configure_color_level(&mut self, level: crate::tui_theme::ColorLevel) {
        self.color_level = level;
        if let Some(values) = &self.user_preferences {
            self.theme = Some(
                crate::tui_theme::Theme::application(values.application_theme, level)
                    .with_terminal_preferences(values),
            );
            #[cfg(windows)]
            return;
        }
        #[cfg(windows)]
        if let Some(name) = self.theme_name {
            self.configure_theme(name);
        }
    }

    /// Resolve once from the original palette so reload cannot accumulate quantization loss.
    #[cfg(windows)]
    pub(crate) fn configure_theme(&mut self, name: orchbee_local_site_runtime::WindowsThemeName) {
        self.theme_name = Some(name);
        if self.user_preferences.is_none() {
            self.theme = Some(crate::tui_theme::Theme::selected(name, self.color_level));
        }
    }

    pub(crate) const fn configured_theme(&self) -> Option<crate::tui_theme::Theme> {
        self.theme
    }

    pub(crate) fn apply_user_preferences(
        &mut self,
        values: orchbee_workbench_protocol::UserPreferencesValues,
    ) {
        self.theme = Some(
            crate::tui_theme::Theme::application(values.application_theme, self.color_level)
                .with_terminal_preferences(&values),
        );
        self.user_preferences = Some(values);
    }

    pub(crate) fn effective_locale(
        &self,
        fallback: crate::localization::Locale,
    ) -> crate::localization::Locale {
        use orchbee_workbench_protocol::PreferencesLocale;
        match self.user_preferences.as_ref().map(|values| values.locale) {
            Some(PreferencesLocale::EnUs) => crate::localization::Locale::EnUs,
            Some(PreferencesLocale::ZhCn) => crate::localization::Locale::ZhCn,
            _ => fallback,
        }
    }

    /// Persistent configuration health does not suppress transient Agent notifications. A
    /// higher-priority notice still does, while the warning returns to the status line afterward.
    pub(crate) const fn has_transient_notice(&self) -> bool {
        self.notice.is_some()
    }

    pub(crate) const fn overlay(&self) -> Option<Overlay> {
        self.overlay
    }

    fn replace_overlay(&mut self, overlay: Option<Overlay>) {
        if self.overlay == Some(Overlay::ShortcutHelp) && overlay != Some(Overlay::ShortcutHelp) {
            self.help.close();
        }
        self.overlay = overlay;
    }

    fn open_help(&mut self) -> bool {
        if !self.help.open() {
            return false;
        }
        self.replace_overlay(Some(Overlay::ShortcutHelp));
        true
    }

    fn request_help_page(&mut self, page: usize) {
        self.help.request_page(page, self.help_page_bound());
    }

    #[cfg(windows)]
    pub(crate) fn navigator_active(&self) -> bool {
        self.overlay == Some(Overlay::WorkbenchNavigator)
    }

    #[cfg(windows)]
    pub(crate) fn close_navigator(&mut self, notice: Option<Notice>) {
        if self.navigator_active() {
            self.replace_overlay(None);
            self.prefix_active = false;
            self.navigator.close();
            self.notice = notice;
        }
    }

    #[cfg(windows)]
    pub(crate) fn close_navigator_for_source(&mut self, source: &navigator::DrawSource) -> bool {
        if !self.navigator_active() || !self.navigator.accepts_source(source) {
            return false;
        }
        self.close_navigator(None);
        true
    }

    pub(crate) const fn prefix_active(&self) -> bool {
        self.prefix_active
    }

    /// The help page the renderer should draw. Consumed by the paged
    /// `render_shortcut_help` seam; always a bounded counter.
    pub(crate) const fn resize_mode(&self) -> bool {
        #[cfg(windows)]
        {
            self.resize_mode
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    #[cfg(windows)]
    pub(crate) const fn copy_mode_active(&self) -> bool {
        self.copy_mode_active
    }

    #[cfg(windows)]
    pub(crate) fn settle_copy_mode(&mut self, active: bool) {
        if self.copy_mode_active && !active && self.overlay == Some(Overlay::ShortcutHelp) {
            self.replace_overlay(None);
        }
        self.copy_mode_active = active;
    }

    #[cfg(windows)]
    pub(crate) fn copy_help_rows(&self) -> Vec<(String, PrefixDesc)> {
        copy::help_rows()
    }

    #[cfg(windows)]
    pub(crate) fn copy_help_key(&mut self, key: KeyEvent) -> bool {
        if !self.copy_mode_active {
            return false;
        }
        match copy::decide(key) {
            Some(CopyModeKey::Help) => {
                if self.overlay == Some(Overlay::ShortcutHelp) {
                    self.dismiss_overlay();
                } else {
                    let _ = self.open_help();
                }
                true
            }
            Some(CopyModeKey::Page { older }) if self.overlay == Some(Overlay::ShortcutHelp) => {
                self.request_help_page(if older {
                    self.help.page().saturating_sub(1)
                } else {
                    self.help.page() + 1
                });
                true
            }
            _ => false,
        }
    }

    #[cfg(windows)]
    pub(crate) fn clear_resize_notice(&mut self) {
        if matches!(
            self.notice,
            Some(
                Notice::ResizeBusy
                    | Notice::ResizeFailed
                    | Notice::ResizeUnknown
                    | Notice::ResizeNoSplit
                    | Notice::ResizeLimit
            )
        ) {
            self.notice = None;
        }
    }

    #[cfg(windows)]
    pub(crate) fn exit_resize_mode(&mut self) {
        self.resize_pointer_fine = false;
        if self.resize_mode {
            self.resize_mode = false;
            if self.overlay == Some(Overlay::ShortcutHelp) {
                self.replace_overlay(None);
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn resize_help_rows(&self) -> Vec<(String, PrefixDesc)> {
        self.resize_help_rows_with_glyphs(KeyLabelGlyphs::Unicode)
    }

    #[cfg(windows)]
    pub(crate) fn resize_help_rows_with_glyphs(
        &self,
        glyphs: KeyLabelGlyphs,
    ) -> Vec<(String, PrefixDesc)> {
        vec![
            (
                resize::labels_with_glyphs(false, glyphs),
                PrefixDesc::ResizeCoarse,
            ),
            (
                resize::labels_with_glyphs(true, glyphs),
                PrefixDesc::ResizeFine,
            ),
            (resize::help_label_with_glyphs(glyphs), PrefixDesc::ShowHelp),
            (resize::page_label_with_glyphs(glyphs), PrefixDesc::HelpPage),
            (
                resize::exit_label_with_glyphs(glyphs),
                PrefixDesc::ResizeExit,
            ),
        ]
    }

    pub(crate) const fn help_page(&self) -> usize {
        self.help.page()
    }

    /// Projects the exact current Help candidate without granting input authority.
    pub(crate) fn help_draw_candidate(&self) -> Option<help::HelpDrawCandidate> {
        (self.overlay == Some(Overlay::ShortcutHelp)).then(|| self.help.candidate())
    }

    /// Settles only the source carried by a successfully published frame map.
    /// `None` revokes a covered, reset, or incomplete candidate without
    /// changing the requested page or closing the noncapturing overlay.
    pub(crate) fn settle_help_draw(&mut self, source: Option<help::HelpDrawSource>) {
        self.help
            .settle(source, self.overlay == Some(Overlay::ShortcutHelp));
    }

    pub(crate) fn activate_drawn_help(&mut self, target: &help::DrawnHelpControl) -> bool {
        if self.overlay != Some(Overlay::ShortcutHelp) {
            return false;
        }
        match self.help.activate(target) {
            Some(help::Activation::Close) => self.dismiss_overlay(),
            Some(help::Activation::Page(page)) => self.request_help_page(page),
            None => return false,
        }
        true
    }

    pub(crate) fn scroll_drawn_help(&mut self, source: &help::HelpDrawSource, up: bool) -> bool {
        if self.overlay != Some(Overlay::ShortcutHelp) {
            return false;
        }
        let Some(page) = self.help.scroll(source, up) else {
            return false;
        };
        self.request_help_page(page);
        true
    }

    pub(crate) fn sidebar_surface(&self) -> SidebarSurface {
        #[cfg(windows)]
        let width = self.machines.sidebar_width();
        #[cfg(not(windows))]
        let width = crate::tui_layout::SidebarWidth::DEFAULT;
        match self.viewport {
            ViewportClass::Wide if self.wide_sidebar_visible => SidebarSurface::Docked(width),
            #[cfg(windows)]
            ViewportClass::Narrow
                if self.overlay == Some(Overlay::ShortcutHelp)
                    && self.machines.sidebar_navigation_active() =>
            {
                SidebarSurface::Overlay(width)
            }
            ViewportClass::Narrow if matches!(self.overlay, Some(Overlay::BusinessSidebar)) => {
                SidebarSurface::Overlay(width)
            }
            ViewportClass::Narrow | ViewportClass::Wide => SidebarSurface::Hidden,
        }
    }

    /// Consumes the original sidebar intent before applying an exact Ready return.
    #[cfg(windows)]
    pub(crate) fn complete_project_quick_entry(&mut self) -> bool {
        let eligible = self.project_quick_entry_eligible.take();
        if eligible != Some(true)
            || self.viewport != ViewportClass::Narrow
            || self.overlay != Some(Overlay::BusinessSidebar)
            || self.prefix_active
            || self.resize_mode
            || self.copy_mode_active
            || self.machines.open
        {
            return false;
        }
        self.replace_overlay(None);
        self.machines.leave_sidebar_navigation();
        true
    }

    #[cfg(windows)]
    pub(crate) fn project_quick_entry_pending(&self) -> bool {
        self.project_quick_entry_eligible.is_some()
    }

    #[cfg(windows)]
    fn arm_project_quick_entry(&mut self) {
        self.project_quick_entry_eligible = Some(
            self.viewport == ViewportClass::Narrow
                && self.overlay == Some(Overlay::BusinessSidebar),
        );
        self.machines.leave_sidebar_navigation();
    }

    /// Row/disclosure presses acquire Picker focus without a ClientAction.
    /// Settle that newer intent before Ready, even if navigation exits later.
    #[cfg(windows)]
    pub(crate) fn settle_project_quick_entry_intent(&mut self) {
        if self.machines.sidebar_navigation_active() {
            self.project_quick_entry_eligible = None;
        }
    }

    pub(crate) fn resize(&mut self, width: u16) {
        let next = ViewportClass::from_width(width);
        if next == ViewportClass::Wide && self.overlay == Some(Overlay::BusinessSidebar) {
            self.replace_overlay(None);
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
        #[cfg(windows)]
        {
            self.project_quick_entry_eligible = None;
        }
        #[cfg(windows)]
        if self.navigator_active() {
            self.close_navigator(None);
            return;
        }
        #[cfg(windows)]
        if self.overlay == Some(Overlay::ShortcutHelp)
            && self.viewport == ViewportClass::Narrow
            && self.machines.sidebar_navigation_active()
        {
            self.replace_overlay(Some(Overlay::BusinessSidebar));
            return;
        }
        self.replace_overlay(None);
        #[cfg(windows)]
        if self.sidebar_surface() == SidebarSurface::Hidden {
            self.machines.leave_sidebar_navigation();
        }
    }

    pub(crate) fn dismiss_focus_modes(&mut self) {
        #[cfg(windows)]
        self.machines.leave_sidebar_navigation();
        self.dismiss_overlay();
        self.prefix_active = false;
        #[cfg(windows)]
        self.exit_resize_mode();
    }

    #[cfg(windows)]
    pub(crate) fn sidebar_help_page_label() -> String {
        SIDEBAR_HELP_PAGES
            .iter()
            .map(|binding| format_binding(binding, LabelStyle::for_host()))
            .collect::<Vec<_>>()
            .join(" / ")
    }

    /// Help covers the tree without creating another navigation mode.
    #[cfg(windows)]
    pub(crate) fn sidebar_help_key(&mut self, event: KeyEvent) -> bool {
        if self.overlay != Some(Overlay::ShortcutHelp) {
            return false;
        }
        if event.kind == KeyKind::Release {
            return true;
        }
        if let Some(index) = SIDEBAR_HELP_PAGES
            .iter()
            .position(|binding| binding.matches(event))
        {
            self.request_help_page(if index == 0 {
                self.help_page().saturating_sub(1)
            } else {
                self.help_page() + 1
            });
        } else if crate::machine_picker::SIDEBAR_BINDINGS
            .iter()
            .any(|(binding, command)| {
                binding.matches(event)
                    && matches!(
                        command,
                        crate::machine_picker::SidebarCommand::Help
                            | crate::machine_picker::SidebarCommand::Exit
                    )
            })
        {
            self.dismiss_overlay();
        }
        true
    }

    /// Applies the navigator settlement of a pointer-issued action. Pointer
    /// intents carry no `ClientTransition`; this is the same disposition
    /// effect `commit` applies for keyboard-issued actions.
    pub(crate) fn commit_pointer(&mut self, disposition: SessionActionDisposition) {
        #[cfg(windows)]
        {
            self.project_quick_entry_eligible = None;
            if disposition == SessionActionDisposition::WorkspaceCreationStarted {
                self.arm_project_quick_entry();
            }
        }
        if disposition == SessionActionDisposition::DismissNavigator
            && self.overlay == Some(Overlay::WorkbenchNavigator)
        {
            self.replace_overlay(None);
        }
    }

    pub(crate) fn configuration_notice(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }

    #[cfg(windows)]
    pub(crate) fn configuration_warning(&mut self, present: bool) {
        self.configuration_warning = present;
        if !present && self.notice == Some(Notice::ConfigurationWarning) {
            self.notice = None;
        }
    }

    pub(crate) fn startup_notice(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }
    #[cfg(windows)]
    pub(crate) fn workspace_navigation_unavailable(&mut self) {
        self.notice = Some(Notice::WorkspaceNavigationUnavailable);
    }
    #[cfg(windows)]
    pub(crate) fn clear_workspace_navigation_notice(&mut self) {
        if self.notice == Some(Notice::WorkspaceNavigationUnavailable) {
            self.notice = None;
        }
    }

    #[cfg(windows)]
    pub(crate) fn notification_unavailable(&mut self) {
        self.notice.get_or_insert(Notice::NotificationUnavailable);
    }
    #[cfg(windows)]
    pub(crate) fn window_title_unavailable(&mut self) {
        self.notice.get_or_insert(Notice::WindowTitleUnavailable);
    }

    pub(crate) fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    #[cfg(windows)]
    pub(crate) fn configure_keys(&mut self, keys: &orchbee_local_site_runtime::WindowsKeySettings) {
        self.exit_resize_mode();
        self.bindings = std::sync::Arc::new(Bindings::configured(keys));
        self.prefix_active = false;
    }

    pub(crate) fn decide(&self, input: PrefixInput) -> ClientTransition {
        let mut next = self.clone();
        #[cfg(windows)]
        if self.resize_mode {
            let key = match input {
                PrefixInput::Key(key) => resize::decide(key),
                _ => None,
            };
            let action = match key {
                Some(resize::ResizeKey::Adjust { direction, fine }) => {
                    next.notice = None;
                    Some(ClientAction::ResizePane { direction, fine })
                }
                Some(resize::ResizeKey::Exit) => {
                    next.exit_resize_mode();
                    Some(ClientAction::ExitResizeMode)
                }
                Some(resize::ResizeKey::Help) => {
                    if next.overlay == Some(Overlay::ShortcutHelp) {
                        next.dismiss_overlay();
                        Some(ClientAction::CloseShortcutHelp)
                    } else {
                        next.open_help().then_some(ClientAction::OpenShortcutHelp)
                    }
                }
                Some(resize::ResizeKey::Page { previous }) => {
                    if next.overlay == Some(Overlay::ShortcutHelp) {
                        next.request_help_page(if previous {
                            next.help_page().saturating_sub(1)
                        } else {
                            next.help_page() + 1
                        });
                        Some(ClientAction::PageShortcutHelp)
                    } else {
                        None
                    }
                }
                None => None,
            };
            return ClientTransition {
                next,
                action,
                shell: ShellCommand::None,
                layout_changed: false,
            };
        }
        if self.prefix_active {
            let (action, shell) = next.decide_prefix(input);
            return ClientTransition {
                action: Some(action),
                shell,
                layout_changed: matches!(self.sidebar_surface(), SidebarSurface::Docked(_))
                    != matches!(next.sidebar_surface(), SidebarSurface::Docked(_)),
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

    /// Semantic entry shared by menu activation and the binding decision layer.
    pub(crate) fn decide_action(&self, action: ClientAction) -> ClientTransition {
        let mut next = self.clone();
        next.prefix_active = false;
        next.notice = None;
        let (action, shell) = next.apply_action_decision(action);
        ClientTransition {
            layout_changed: self.sidebar_surface() != next.sidebar_surface(),
            next,
            action: Some(action),
            shell,
        }
    }

    fn apply_action_decision(&mut self, action: ClientAction) -> (ClientAction, ShellCommand) {
        #[cfg(windows)]
        {
            self.project_quick_entry_eligible = None;
            if matches!(
                action,
                ClientAction::RequestWorkspaceCreate
                    | ClientAction::RequestQuickWorkspace
                    | ClientAction::RequestProjectQuickWorkspace(_)
            ) {
                // Creation takes keyboard intent from the sidebar. Preserve the
                // visible overlay until the exact native Ready receipt returns;
                // a later sidebar focus will cancel that pending return instead.
                self.arm_project_quick_entry();
            }
        }
        match action {
            #[cfg(windows)]
            ClientAction::EnterCopyMode => {
                self.dismiss_focus_modes();
                self.request_help_page(0);
                (action, ShellCommand::None)
            }
            #[cfg(windows)]
            ClientAction::EnterResizeMode => {
                self.machines.leave_sidebar_navigation();
                self.resize_mode = true;
                self.prefix_active = false;
                self.replace_overlay(None);
                self.request_help_page(0);
                (action, ShellCommand::None)
            }
            ClientAction::ToggleBusinessSidebar => {
                #[cfg(windows)]
                self.machines.leave_sidebar_navigation();
                self.toggle_sidebar();
                (action, ShellCommand::None)
            }
            ClientAction::OpenSettings => {
                self.dismiss_focus_modes();
                (action, ShellCommand::None)
            }
            #[cfg(windows)]
            ClientAction::FocusBusinessSidebar => {
                self.dismiss_focus_modes();
                if self.machines.available {
                    match self.viewport {
                        ViewportClass::Wide => self.wide_sidebar_visible = true,
                        ViewportClass::Narrow => {
                            self.replace_overlay(Some(Overlay::BusinessSidebar));
                        }
                    }
                    self.machines.enter_sidebar_navigation();
                }
                (action, ShellCommand::None)
            }
            ClientAction::OpenWorkbenchNavigator => {
                if self.overlay == Some(Overlay::WorkbenchNavigator) {
                    self.replace_overlay(None);
                    #[cfg(windows)]
                    self.navigator.close();
                    (ClientAction::CloseWorkbenchNavigator, ShellCommand::None)
                } else {
                    #[cfg(windows)]
                    {
                        self.dismiss_focus_modes();
                        self.prefix_active = false;
                        if !self.navigator.open() {
                            self.notice = Some(Notice::NavigatorChanged);
                            return (ClientAction::CloseWorkbenchNavigator, ShellCommand::None);
                        }
                    }
                    self.replace_overlay(Some(Overlay::WorkbenchNavigator));
                    (action, ShellCommand::None)
                }
            }
            ClientAction::ToggleAgentBoard => {
                self.replace_overlay(None);
                (action, ShellCommand::None)
            }
            ClientAction::OpenShortcutHelp => {
                if self.overlay == Some(Overlay::ShortcutHelp) {
                    self.dismiss_overlay();
                    (ClientAction::CloseShortcutHelp, ShellCommand::None)
                } else if self.open_help() {
                    (action, ShellCommand::None)
                } else {
                    (ClientAction::CloseShortcutHelp, ShellCommand::None)
                }
            }
            ClientAction::Exit => (action, ShellCommand::Exit),
            _ => (action, ShellCommand::None),
        }
    }

    pub(crate) fn commit(
        &mut self,
        transition: ClientTransition,
        disposition: SessionActionDisposition,
    ) {
        let mut next = transition.next;
        #[cfg(windows)]
        if transition.action == Some(ClientAction::CreateTab)
            && disposition == SessionActionDisposition::WorkspaceCreationStarted
        {
            next.arm_project_quick_entry();
        }
        if disposition == SessionActionDisposition::DismissNavigator
            && next.overlay == Some(Overlay::WorkbenchNavigator)
        {
            next.replace_overlay(None);
            #[cfg(windows)]
            next.navigator.close();
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
            PrefixAction::Client(action) => {
                let (action, _) = self.apply_action_decision(action.clone());
                (action, member.shell)
            }
            #[cfg(windows)]
            PrefixAction::SelectAgentDigit => {
                // The binding owns its digit even when Shift changes the input symbol.
                let bindings::KeySet::One(Binding {
                    code: KeyCode::Char(digit @ '1'..='9'),
                    ..
                }) = member.keys
                else {
                    return (ClientAction::UnknownPrefix, ShellCommand::None);
                };
                (ClientAction::FocusAgent(digit as u8 - b'1'), member.shell)
            }
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
                self.apply_action_decision(ClientAction::ToggleBusinessSidebar)
            }
            PrefixAction::NavigatorToggle => {
                self.apply_action_decision(ClientAction::OpenWorkbenchNavigator)
            }
            PrefixAction::BoardToggle => self.apply_action_decision(ClientAction::ToggleAgentBoard),
            PrefixAction::ShowHelp => self.apply_action_decision(ClientAction::OpenShortcutHelp),
            PrefixAction::HelpPagePrevious | PrefixAction::HelpPageNext => {
                if self.overlay == Some(Overlay::ShortcutHelp) {
                    // Only an open help pages: the page key is consumed
                    // while the overlay is up, and the renderer's clamped
                    // page settles the upper bound on the next draw.
                    self.request_help_page(match member.action {
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
                self.replace_overlay(None);
            }
            ViewportClass::Narrow => {
                let overlay = if self.overlay == Some(Overlay::BusinessSidebar) {
                    None
                } else {
                    Some(Overlay::BusinessSidebar)
                };
                self.replace_overlay(overlay);
            }
        }
    }
}

#[cfg(test)]
mod tests;
