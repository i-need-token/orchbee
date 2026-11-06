//! Owns Client-local chrome, prefix bindings, overlay decisions, and presentation preferences,
//! emitting explicit backend or shell actions. It does not own canonical Workbench membership,
//! physical terminal state, or Server command settlement.

#[cfg(test)]
pub(crate) use bindings::PREFIX_GROUPS;
/// Wide hosts show the sidebar by default: the default width still docks beside usable content.
pub(crate) const WIDE_VIEWPORT_MINIMUM_COLUMNS: u16 =
    crate::sidebar_width::SidebarWidth::DEFAULT.docked_host_columns();

mod bindings;
mod copy;
pub(crate) mod help;
use crate::terminal_events::keys;
pub(crate) mod navigator;
pub(crate) mod notifications;
mod resize;

pub use bindings::Bindings;
pub(crate) use bindings::{DiscoveryEntry, PrefixAction, PrefixDesc};
pub(crate) use copy::{
    CopyHintLabel, CopyModeKey, CopySearchCommand, decide as copy_mode_key,
    footer_bindings as copy_footer_bindings, search_command as copy_search_command,
};
pub(crate) use keys::{
    Binding, KeyLabelGlyphs, LabelStyle, format_binding, format_binding_with_glyphs,
    join_key_labels,
};
pub use keys::{KeyCode, KeyEvent, KeyKind, Modifiers, format_configuration_binding};

/// Effective Client colors for controller queries. Defaults are complete; only unobserved outer
/// cursor and ANSI slots remain absent. All present channels originate in validated RGB sources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalQueryColors {
    pub(crate) foreground: u32,
    pub(crate) background: u32,
    pub(crate) cursor: Option<u32>,
    pub(crate) palette: [Option<u32>; 16],
}

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
pub enum Overlay {
    BusinessSidebar,
    ShortcutHelp,
    WorkbenchNavigator,
}

pub(crate) use crate::localization::PanelDirection;

pub(crate) use crate::localization::BackendAction;

pub use crate::client_actions::ClientAction;

pub use crate::localization::Notice;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientKey {
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
pub enum PrefixInput {
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
pub enum ClientActionDisposition {
    KeepNavigator,
    DismissNavigator,
    /// The action changed the rendered pane layout — a zoom toggle, or a
    /// focus move while zoomed — so the dispatcher re-observes geometry
    /// and invalidates the pointer hit map. The navigator disposition
    /// stays unchanged (kept).
    LayoutChanged,
    /// Native admission redirected the no-Workspace landing to one fresh
    /// Workspace launch. Preserve its shell entry intent across control ticks.
    WorkspaceCreationStarted,
}

impl ClientActionDisposition {
    pub(crate) fn changes_layout(self) -> bool {
        match self {
            Self::LayoutChanged => true,
            Self::WorkspaceCreationStarted => true,
            Self::KeepNavigator | Self::DismissNavigator => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientTransition {
    next: ClientView,
    action: Option<ClientAction>,
    shell: ShellCommand,
    layout_changed: bool,
}

impl ClientTransition {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidebarSurface {
    Docked(crate::sidebar_width::SidebarWidth),
    Hidden,
    Overlay(crate::sidebar_width::SidebarWidth),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientView {
    /// Compile-time host admission received once at construction; nothing writes it afterwards.
    capabilities: crate::tui_capabilities::Capabilities,
    pub onboarding: crate::onboarding::State,
    /// Confirmed shared preferences. `apply_user_preferences` is the single writer;
    /// `WorkbenchClientUi` receives its copy from here in the same settlement.
    user_preferences: Option<orchbee_workbench_protocol::UserPreferencesValues>,
    pub machines: crate::machine_picker::Picker,
    bindings: std::sync::Arc<Bindings>,
    theme_name: Option<crate::tui_theme::ThemeName>,
    theme: Option<crate::tui_theme::Theme>,
    host_appearance: crate::host_appearance::HostAppearance,
    host_colors: crate::host_appearance::HostColors,
    color_level: crate::tui_theme::ColorLevel,
    no_color: bool,
    pub notifications: notifications::State,
    pub window_title: Option<orchbee_workbench_protocol::WindowTitleSnapshot>,
    notice: Option<Notice>,
    notice_timer: crate::transient_notice::NoticeTimer<Notice>,
    configuration_warning: bool,
    preferences_unreadable: bool,
    // The confirmed snapshot's own source, once one arrives; it supersedes the diagnostic.
    preferences_source_unreadable: Option<bool>,
    overlay: Option<Overlay>,
    // Some(false) still tracks a launch opened without a compact sidebar;
    // None means a newer intent has retired that launch's return permission.
    project_quick_entry_eligible: Option<bool>,
    pub(crate) navigator: navigator::Navigator,
    prefix_active: bool,
    resize_mode: bool,
    pub(crate) resize_pointer_fine: bool,
    /// Mirror of the `WorkbenchClientUi` copy-mode owner, written only by `settle_copy_mode`
    /// from the owner's current answer; it detects the exit edge for discovery.
    copy_mode_active: bool,
    pub(crate) resize_pending: bool,
    viewport: ViewportClass,
    wide_sidebar_visible: bool,
    /// Client-local Help incarnation, filter, requested scroll offset, and
    /// successful-frame proof. It never enters WorkbenchClient business state.
    help: help::State,
    /// The pointer-capture release flag: `Ctrl+B m` and `--mouse off`
    /// release host mouse reporting; while released the status line carries
    /// the persistent indicator and every Client pointer gesture is inert.
    /// The dispatcher settles it only after the terminal mode write
    /// succeeded, so it always mirrors the armed state.
    pointer_released: bool,
}

impl ClientView {
    pub fn new(width: u16) -> Self {
        Self::with_capabilities(width, crate::tui_capabilities::Capabilities::all())
    }
    pub fn with_capabilities(
        width: u16,
        capabilities: crate::tui_capabilities::Capabilities,
    ) -> Self {
        Self {
            capabilities,
            user_preferences: None,
            machines: crate::machine_picker::Picker::default(),
            bindings: std::sync::Arc::new(Bindings::default()),
            theme_name: None,
            theme: None,
            host_appearance: crate::host_appearance::HostAppearance::Unknown,
            host_colors: crate::host_appearance::HostColors::default(),
            color_level: crate::tui_theme::ColorLevel::Ansi16,
            no_color: false,
            notifications: notifications::State::default(),
            window_title: None,
            onboarding: crate::onboarding::State::default(),
            notice: None,
            notice_timer: Default::default(),
            configuration_warning: false,
            preferences_unreadable: false,
            preferences_source_unreadable: None,
            overlay: None,
            project_quick_entry_eligible: None,
            navigator: navigator::Navigator::default(),
            prefix_active: false,
            resize_mode: false,
            resize_pointer_fine: false,
            copy_mode_active: false,
            resize_pending: false,
            viewport: ViewportClass::from_width(width),
            wide_sidebar_visible: true,
            help: help::State::default(),
            pointer_released: false,
        }
    }

    pub fn expire_notice(&mut self, now: std::time::Instant) -> bool {
        if self.notice_timer.expired(self.notice, now) {
            self.notice = None;
            true
        } else {
            false
        }
    }

    /// The navigation hold keeps the last flushed frame only while nothing new must be seen.
    /// Shortcut help, a notification, or a one-shot notice such as dropped input renders now.
    pub fn permits_navigation_hold(&self) -> bool {
        self.overlay() != Some(Overlay::ShortcutHelp)
            && self.notifications.empty()
            && self.notice.is_none()
    }

    pub(crate) const fn notice(&self) -> Option<Notice> {
        if self.notice.is_some() {
            return self.notice;
        }
        // The configuration diagnostic is historical. A confirmed Settings snapshot's source
        // supersedes it, so the notice stays until Reset all replaces the unreadable record.
        let unreadable = match self.preferences_source_unreadable {
            Some(unreadable) => unreadable,
            None => self.preferences_unreadable,
        };
        if unreadable {
            return Some(Notice::PreferencesUnreadable);
        }
        if self.configuration_warning {
            return Some(Notice::ConfigurationWarning);
        }
        None
    }

    /// Host capabilities are sampled by the Shell, never by a render consumer.
    pub fn configure_no_color(&mut self, no_color: bool) {
        self.no_color = no_color;
    }

    pub const fn no_color(&self) -> bool {
        self.no_color
    }

    /// Host capabilities are sampled by the Shell, never by a render consumer.
    pub fn configure_color_level(&mut self, level: crate::tui_theme::ColorLevel) {
        self.color_level = level;
        if let Some(values) = &self.user_preferences {
            self.theme = Some(self.resolve_theme(crate::tui_theme::ColorPreferences::from(values)));
            return;
        }
        if let Some(name) = self.theme_name {
            self.configure_theme(name);
        }
    }

    /// Whether this host can show a named theme; Settings presents theme choices as unavailable
    /// otherwise.
    pub(crate) const fn named_themes_available(&self) -> bool {
        crate::tui_theme::named_themes_available(self.color_level)
    }

    /// System follows the settled host appearance: the outer terminal's reported background, or
    /// the operating system's appearance once the probe expires unanswered, as the GUI follows
    /// the operating system. A named theme paints its own canvas, so the terminal's unreported
    /// colors need not match it.
    fn resolve_theme(
        &self,
        preferences: crate::tui_theme::ColorPreferences<'_>,
    ) -> crate::tui_theme::Theme {
        let host = crate::tui_theme::Polarity::of_host(self.host_appearance);
        crate::tui_theme::Theme::resolve(preferences, host, self.color_level)
            .with_outer_host_colors(self.host_colors)
    }

    /// Resolve once from the original palette so reload cannot accumulate quantization loss.
    pub fn configure_theme(&mut self, name: crate::tui_theme::ThemeName) {
        self.theme_name = Some(name);
        if self.user_preferences.is_none() {
            self.theme = Some(crate::tui_theme::Theme::selected(name, self.color_level));
        }
    }

    pub const fn configured_theme(&self) -> Option<crate::tui_theme::Theme> {
        self.theme
    }

    /// Updates the sampled host polarity and resolves a confirmed System preference if present.
    pub(crate) fn configure_host_appearance(
        &mut self,
        appearance: crate::host_appearance::HostAppearance,
    ) -> bool {
        self.configure_host_terminal(appearance, self.host_colors)
    }

    /// The colors painted by this Client, independent of the Worker's platform appearance.
    pub(crate) fn host_default_colors(&self) -> Option<(u32, u32)> {
        if let Some(colors) = self.theme.and_then(|theme| theme.terminal_rgb_defaults()) {
            return Some(colors);
        }
        match (self.host_colors.foreground, self.host_colors.background) {
            (Some(foreground), Some(background)) => Some((foreground, background)),
            _ => None,
        }
    }

    /// Known query colors accompany each controller grant; unobserved outer ANSI slots stay unknown.
    pub(crate) fn terminal_query_colors(&self) -> Option<TerminalQueryColors> {
        let (foreground, background) = self.host_default_colors()?;
        let (cursor, palette) = self.theme.map_or(
            (self.host_colors.cursor, self.host_colors.palette),
            |theme| theme.terminal_query_palette(self.host_colors),
        );
        Some(TerminalQueryColors {
            foreground,
            background,
            cursor,
            palette,
        })
    }

    pub(crate) fn configure_host_terminal(
        &mut self,
        appearance: crate::host_appearance::HostAppearance,
        colors: crate::host_appearance::HostColors,
    ) -> bool {
        if self.host_appearance == appearance && self.host_colors == colors {
            return false;
        }
        self.host_appearance = appearance;
        self.host_colors = colors;
        if let Some(values) = &self.user_preferences {
            self.theme = Some(self.resolve_theme(crate::tui_theme::ColorPreferences::from(values)));
            return values.application_theme
                == orchbee_workbench_protocol::ApplicationAppearance::System;
        }
        if let Some(name) = self.theme_name {
            self.configure_theme(name);
        }
        false
    }

    /// Resolves only chrome and terminal default colors from a reversible local draft.
    pub fn preview_theme(
        &self,
        preview: crate::native_settings::AppearancePreview<'_>,
    ) -> crate::tui_theme::Theme {
        self.resolve_theme(preview.into())
    }

    pub(crate) fn apply_user_preferences(
        &mut self,
        values: orchbee_workbench_protocol::UserPreferencesValues,
    ) {
        self.theme = Some(self.resolve_theme(crate::tui_theme::ColorPreferences::from(&values)));
        self.user_preferences = Some(values);
    }

    /// The confirmed preferences that `WorkbenchClientUi` receives after each settlement.
    pub(crate) const fn user_preferences(
        &self,
    ) -> Option<&orchbee_workbench_protocol::UserPreferencesValues> {
        self.user_preferences.as_ref()
    }

    pub const fn capabilities(&self) -> crate::tui_capabilities::Capabilities {
        self.capabilities
    }

    pub fn effective_locale(
        &self,
        fallback: crate::localization::Locale,
    ) -> crate::localization::Locale {
        self.presentation_locale(fallback, None)
    }

    /// Resolves authored copy only; terminal bytes and notification policy remain confirmed.
    pub fn presentation_locale(
        &self,
        fallback: crate::localization::Locale,
        preview: Option<orchbee_workbench_protocol::PreferencesLocale>,
    ) -> crate::localization::Locale {
        use orchbee_workbench_protocol::PreferencesLocale;
        match preview.or_else(|| self.user_preferences.as_ref().map(|values| values.locale)) {
            Some(PreferencesLocale::EnUs) => crate::localization::Locale::EnUs,
            Some(PreferencesLocale::ZhCn) => crate::localization::Locale::ZhCn,
            Some(PreferencesLocale::System) | None => fallback,
        }
    }

    pub const fn overlay(&self) -> Option<Overlay> {
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
        // The renderer publishes the maximum scroll offset for its exact
        // viewport; until then input may request beyond it.
        self.help.request_page(page, usize::MAX);
    }

    fn edit_help_filter(&mut self, event: KeyEvent) -> Option<ClientAction> {
        if event.kind == KeyKind::Release {
            return None;
        }
        if Binding::new(KeyCode::Tab, Modifiers::NONE).matches(event) {
            self.help.toggle_all();
            return Some(ClientAction::PageShortcutHelp);
        }
        if Binding::new(KeyCode::PageUp, Modifiers::NONE).matches(event) {
            self.request_help_page(self.help.page().saturating_sub(self.help.page_step()));
            return Some(ClientAction::PageShortcutHelp);
        }
        if Binding::new(KeyCode::PageDown, Modifiers::NONE).matches(event) {
            self.request_help_page(self.help.page().saturating_add(self.help.page_step()));
            return Some(ClientAction::PageShortcutHelp);
        }
        if Binding::new(KeyCode::Up, Modifiers::NONE).matches(event) {
            self.request_help_page(self.help.page().saturating_sub(1));
            return Some(ClientAction::PageShortcutHelp);
        }
        if Binding::new(KeyCode::Down, Modifiers::NONE).matches(event) {
            self.request_help_page(self.help.page().saturating_add(1));
            return Some(ClientAction::PageShortcutHelp);
        }
        if Binding::new(KeyCode::Home, Modifiers::NONE).matches(event) {
            self.request_help_page(0);
            return Some(ClientAction::PageShortcutHelp);
        }
        if Binding::new(KeyCode::End, Modifiers::NONE).matches(event) {
            if let Some(page) = self.help.end_page() {
                self.request_help_page(page);
                return Some(ClientAction::PageShortcutHelp);
            }
            return None;
        }
        let (code, modifiers) = event.normalized();
        if code == KeyCode::Char('u') && modifiers.ctrl && !modifiers.alt && !modifiers.super_ {
            self.help.clear_filter();
        } else if Binding::new(KeyCode::Backspace, Modifiers::NONE).matches(event) {
            self.help.backspace_filter();
        } else if !modifiers.ctrl
            && !modifiers.alt
            && !modifiers.super_
            && let KeyCode::Char(character) = event.code
        {
            self.help.append_filter(character);
        }
        None
    }

    pub(crate) fn navigator_active(&self) -> bool {
        self.overlay == Some(Overlay::WorkbenchNavigator)
    }

    pub(crate) fn close_navigator(&mut self, notice: Option<Notice>) {
        if self.navigator_active() {
            self.replace_overlay(None);
            self.prefix_active = false;
            self.navigator.close();
            self.notice = notice;
        }
    }

    pub(crate) fn close_navigator_for_source(&mut self, source: &navigator::DrawSource) -> bool {
        if !self.navigator_active() || !self.navigator.accepts_source(source) {
            return false;
        }
        self.close_navigator(None);
        true
    }

    pub const fn prefix_active(&self) -> bool {
        self.prefix_active
    }

    pub(crate) fn cancel_prefix(&mut self) {
        self.prefix_active = false;
    }

    /// Whether pane resize mode currently owns ordinary Client input.
    pub const fn resize_mode(&self) -> bool {
        self.resize_mode
    }

    pub(crate) const fn copy_mode_active(&self) -> bool {
        self.copy_mode_active
    }

    pub(crate) fn settle_copy_mode(&mut self, active: bool) {
        if self.copy_mode_active && !active && self.overlay == Some(Overlay::ShortcutHelp) {
            self.replace_overlay(None);
        }
        self.copy_mode_active = active;
    }

    pub(crate) fn copy_help_rows(&self) -> Vec<(String, PrefixDesc)> {
        copy::help_rows()
            .into_iter()
            .filter(|(_, description)| {
                *description != PrefixDesc::CopySubmit
                    || self
                        .capabilities
                        .supports(crate::tui_capabilities::Capability::Clipboard)
            })
            .collect()
    }

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
            _ => false,
        }
    }

    /// Whether the notice shown reports what became of the person's own resize input.
    pub(crate) const fn resize_notice_shown(&self) -> bool {
        matches!(
            self.notice,
            Some(
                Notice::ResizeBusy
                    | Notice::ResizeFailed
                    | Notice::ResizeUnknown
                    | Notice::ResizeContended
                    | Notice::ResizeNoSplit
                    | Notice::ResizeLimit
            )
        )
    }

    pub(crate) fn clear_resize_notice(&mut self) {
        if self.resize_notice_shown() {
            self.notice = None;
        }
    }

    pub(crate) fn exit_resize_mode(&mut self) {
        self.resize_pointer_fine = false;
        if self.resize_mode {
            self.resize_mode = false;
            if self.overlay == Some(Overlay::ShortcutHelp) {
                self.replace_overlay(None);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn resize_help_rows(&self) -> Vec<(String, PrefixDesc)> {
        self.resize_help_rows_with_glyphs(KeyLabelGlyphs::Unicode)
    }

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

    /// First rendered line requested by the Help list.
    pub(crate) const fn help_page(&self) -> usize {
        self.help.page()
    }

    pub(crate) const fn help_expanded(&self) -> bool {
        self.help.expanded()
    }

    pub(crate) fn help_filter(&self) -> &str {
        self.help.filter()
    }

    /// Projects the exact current Help candidate without granting input authority.
    pub(crate) fn help_draw_candidate(&self) -> Option<help::HelpDrawCandidate> {
        (self.overlay == Some(Overlay::ShortcutHelp)).then(|| self.help.candidate())
    }

    /// Settles only the source carried by a successfully published frame map.
    /// `None` revokes a covered, reset, or incomplete candidate without
    /// changing the requested offset or closing the capturing overlay.
    pub fn settle_help_draw(&mut self, source: Option<help::HelpDrawSource>) {
        self.help
            .settle(source, self.overlay == Some(Overlay::ShortcutHelp));
    }

    pub(crate) fn activate_drawn_help(&mut self, target: &help::DrawnHelpControl) -> bool {
        if self.overlay != Some(Overlay::ShortcutHelp) {
            return false;
        }
        match self.help.activate(target) {
            Some(help::Activation::Close) => self.dismiss_overlay(),
            Some(help::Activation::ToggleAll) => self.help.toggle_all(),
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

    pub fn sidebar_surface(&self) -> SidebarSurface {
        let width = self.machines.sidebar_width();

        match self.viewport {
            ViewportClass::Wide if self.wide_sidebar_visible => SidebarSurface::Docked(width),
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

    pub(crate) fn project_quick_entry_pending(&self) -> bool {
        self.project_quick_entry_eligible.is_some()
    }

    fn arm_project_quick_entry(&mut self) {
        self.project_quick_entry_eligible = Some(
            self.viewport == ViewportClass::Narrow
                && self.overlay == Some(Overlay::BusinessSidebar),
        );
        self.machines.leave_sidebar_navigation();
        self.machines.reveal_sidebar_current();
    }

    /// Row/disclosure presses acquire Picker focus without a ClientAction.
    /// Settle that newer intent before Ready, even if navigation exits later.
    pub(crate) fn settle_project_quick_entry_intent(&mut self) {
        if self.machines.sidebar_navigation_active() {
            self.project_quick_entry_eligible = None;
        }
    }

    pub fn resize(&mut self, width: u16) {
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

    /// A requested navigation has not landed; the refused input was sent nowhere.
    pub(crate) fn report_input_held_for_navigation(&mut self) {
        self.notice = Some(Notice::NavigationInputHeld);
    }

    /// The pending-focus queue overflowed or expired — the queued pointer
    /// input was dropped and the user sees why.
    pub(crate) fn report_pointer_input_dropped(&mut self) {
        self.notice = Some(Notice::PointerInputDropped);
    }

    /// Whether host pointer reporting is currently released (`Ctrl+B m` or
    /// `--mouse off`). Read by the status line's persistent indicator.
    pub const fn pointer_released(&self) -> bool {
        self.pointer_released
    }

    /// Settles the released flag after the terminal mode write succeeded.
    pub fn settle_pointer_released(&mut self, released: bool) {
        self.pointer_released = released;
    }

    /// An outside click on an open overlay dismisses it without producing a
    /// semantic action — the gesture is consumed, never forwarded beneath.
    pub(crate) fn dismiss_overlay(&mut self) {
        {
            self.project_quick_entry_eligible = None;
        }
        if self.navigator_active() {
            self.close_navigator(None);
            return;
        }
        if self.overlay == Some(Overlay::ShortcutHelp)
            && self.viewport == ViewportClass::Narrow
            && self.machines.sidebar_navigation_active()
        {
            self.replace_overlay(Some(Overlay::BusinessSidebar));
            return;
        }
        self.replace_overlay(None);
        if self.sidebar_surface() == SidebarSurface::Hidden {
            self.machines.leave_sidebar_navigation();
        }
    }

    pub(crate) fn dismiss_focus_modes(&mut self) {
        self.machines.leave_sidebar_navigation();
        self.dismiss_overlay();
        self.prefix_active = false;
        self.exit_resize_mode();
    }

    /// Help covers the tree without creating another navigation mode.
    pub(crate) fn sidebar_help_key(&mut self, event: KeyEvent) -> bool {
        if self.overlay != Some(Overlay::ShortcutHelp) {
            return false;
        }
        if event.kind == KeyKind::Release {
            return false;
        }
        if crate::machine_picker::SIDEBAR_BINDINGS
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
            true
        } else {
            false
        }
    }

    /// Applies the navigator settlement of a pointer-issued action. Pointer
    /// intents carry no `ClientTransition`; this is the same disposition
    /// effect `commit` applies for keyboard-issued actions.
    pub(crate) fn commit_pointer(&mut self, disposition: ClientActionDisposition) {
        {
            self.project_quick_entry_eligible = None;
            if disposition == ClientActionDisposition::WorkspaceCreationStarted {
                self.arm_project_quick_entry();
            }
        }
        if disposition == ClientActionDisposition::DismissNavigator
            && self.overlay == Some(Overlay::WorkbenchNavigator)
        {
            self.replace_overlay(None);
        }
    }

    pub(crate) fn configuration_notice(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }

    pub(crate) fn preferences_unreadable(&mut self, present: bool) {
        self.preferences_unreadable = present;
    }

    pub(crate) fn preferences_source(&mut self, source: &str) {
        self.preferences_source_unreadable = Some(source == "unreadable");
    }

    pub(crate) fn configuration_warning(&mut self, present: bool) {
        self.configuration_warning = present;
        if !present && self.notice == Some(Notice::ConfigurationWarning) {
            self.notice = None;
        }
    }

    pub fn startup_notice(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }
    pub fn workspace_navigation_unavailable(&mut self) {
        self.notice = Some(Notice::WorkspaceNavigationUnavailable);
    }
    pub fn clear_workspace_navigation_notice(&mut self) {
        if self.notice == Some(Notice::WorkspaceNavigationUnavailable) {
            self.notice = None;
        }
    }

    pub fn notification_unavailable(&mut self, unsupported: bool) {
        self.notice.get_or_insert(if unsupported {
            Notice::NotificationUnsupported
        } else {
            Notice::NotificationUnavailable
        });
    }
    pub fn window_title_unavailable(&mut self, unsupported: bool) {
        self.notice.get_or_insert(if unsupported {
            Notice::WindowTitleUnsupported
        } else {
            Notice::WindowTitleUnavailable
        });
    }

    pub(crate) fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    pub(crate) fn configure_keys(&mut self, keys: &orchbee_local_runtime::KeySettings) {
        self.exit_resize_mode();
        self.bindings = std::sync::Arc::new(Bindings::configured(keys));
        self.prefix_active = false;
    }

    /// Takes each configured command's name from the Server's catalog; see
    /// [`Bindings::name_configured_commands`].
    pub(crate) fn name_configured_commands(
        &mut self,
        catalog: &[orchbee_workbench_protocol::ConfiguredCommandDescriptor],
    ) {
        std::sync::Arc::make_mut(&mut self.bindings).name_configured_commands(catalog);
    }

    pub(crate) fn decide(&self, input: PrefixInput) -> ClientTransition {
        let mut next = self.clone();
        if self.overlay == Some(Overlay::ShortcutHelp) {
            if let PrefixInput::Key(event) = input
                && event.kind != KeyKind::Release
                && Binding::new(KeyCode::Esc, Modifiers::NONE).matches(event)
            {
                next.dismiss_overlay();
                next.prefix_active = false;
                return ClientTransition {
                    next,
                    action: Some(ClientAction::CloseShortcutHelp),
                    shell: ShellCommand::None,
                    layout_changed: false,
                };
            }
            if self.prefix_active
                && matches!(
                    input,
                    PrefixInput::Key(KeyEvent {
                        kind: KeyKind::Release,
                        ..
                    })
                )
            {
                return ClientTransition {
                    next,
                    action: None,
                    shell: ShellCommand::None,
                    layout_changed: false,
                };
            }
            if self.prefix_active {
                let (action, shell) = next.decide_prefix(input);
                if action == ClientAction::ForwardLiteralPrefix {
                    return ClientTransition {
                        next,
                        action: None,
                        shell: ShellCommand::None,
                        layout_changed: false,
                    };
                }
                if next.overlay == Some(Overlay::ShortcutHelp)
                    && !matches!(
                        &action,
                        ClientAction::DismissPrefix
                            | ClientAction::UnknownPrefix
                            | ClientAction::PageShortcutHelp
                    )
                {
                    next.dismiss_overlay();
                }
                return ClientTransition {
                    action: Some(action),
                    shell,
                    layout_changed: matches!(self.sidebar_surface(), SidebarSurface::Docked(_))
                        != matches!(next.sidebar_surface(), SidebarSurface::Docked(_)),
                    next,
                };
            }
            if let PrefixInput::Key(event) = input {
                if event.kind == KeyKind::Release {
                    return ClientTransition {
                        next,
                        action: None,
                        shell: ShellCommand::None,
                        layout_changed: false,
                    };
                }
                if self.bindings.prefix.matches(event) {
                    next.prefix_active = true;
                    return ClientTransition {
                        next,
                        action: None,
                        shell: ShellCommand::None,
                        layout_changed: false,
                    };
                }
                if next.copy_help_key(event) {
                    return ClientTransition {
                        next,
                        action: None,
                        shell: ShellCommand::None,
                        layout_changed: false,
                    };
                }
                if self.resize_mode
                    && matches!(resize::decide(event), Some(resize::ResizeKey::Help))
                {
                    next.dismiss_overlay();
                    return ClientTransition {
                        next,
                        action: Some(ClientAction::CloseShortcutHelp),
                        shell: ShellCommand::None,
                        layout_changed: false,
                    };
                }
                if next.machines.sidebar_navigation_active() && next.sidebar_help_key(event) {
                    return ClientTransition {
                        next,
                        action: None,
                        shell: ShellCommand::None,
                        layout_changed: false,
                    };
                }
                if let Some(member) = self.bindings.find(event, false)
                    && matches!(
                        &member.action,
                        PrefixAction::ShowHelp
                            | PrefixAction::Client(ClientAction::OpenShortcutHelp)
                    )
                {
                    let (action, shell) = next.decide_member(input, Some(member));
                    return ClientTransition {
                        action: Some(action),
                        shell,
                        layout_changed: self.sidebar_surface() != next.sidebar_surface(),
                        next,
                    };
                }
                let action = next.edit_help_filter(event);
                return ClientTransition {
                    next,
                    action,
                    shell: ShellCommand::None,
                    layout_changed: false,
                };
            }
            return ClientTransition {
                next,
                action: None,
                shell: ShellCommand::None,
                layout_changed: false,
            };
        }
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

    pub(crate) fn decide_agent_board_prefix(&self, input: PrefixInput) -> ClientTransition {
        let transition = self.decide(input);
        if matches!(
            transition.action(),
            Some(
                ClientAction::ForwardLiteralPrefix
                    | ClientAction::DismissPrefix
                    | ClientAction::UnknownPrefix
            )
        ) {
            // The native Agent Board owns input while visible. Literal,
            // canceled, and unknown local prefix results stay in that owner;
            // only a recognized global binding leaves through its action.
            ClientTransition {
                next: transition.next,
                action: None,
                shell: ShellCommand::None,
                layout_changed: false,
            }
        } else {
            transition
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
        {
            self.project_quick_entry_eligible = None;
            if matches!(
                action,
                ClientAction::RequestWorkspaceCreate
                    | ClientAction::RequestWorkspaceAtDirectory
                    | ClientAction::RequestQuickWorkspace
                    | ClientAction::RequestProjectQuickWorkspace
            ) {
                // Creation takes keyboard intent from the sidebar. Preserve the
                // visible overlay until the exact native Ready receipt returns;
                // a later sidebar focus will cancel that pending return instead.
                self.arm_project_quick_entry();
            }
        }
        match action {
            ClientAction::EnterCopyMode => {
                self.dismiss_focus_modes();
                self.request_help_page(0);
                (action, ShellCommand::None)
            }
            ClientAction::EnterResizeMode => {
                self.machines.leave_sidebar_navigation();
                self.resize_mode = true;
                self.prefix_active = false;
                self.replace_overlay(None);
                self.request_help_page(0);
                (action, ShellCommand::None)
            }
            ClientAction::ToggleBusinessSidebar => {
                self.machines.leave_sidebar_navigation();
                self.toggle_sidebar();
                (action, ShellCommand::None)
            }
            ClientAction::OpenSettings => {
                self.dismiss_focus_modes();
                (action, ShellCommand::None)
            }
            ClientAction::FocusBusinessSidebar => {
                let was_active = self.machines.sidebar_navigation_active();
                self.dismiss_focus_modes();
                if self.machines.available && !was_active {
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
                    self.navigator.close();
                    (ClientAction::CloseWorkbenchNavigator, ShellCommand::None)
                } else {
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
        disposition: ClientActionDisposition,
    ) {
        let mut next = transition.next;
        if transition.action == Some(ClientAction::CreateTab)
            && disposition == ClientActionDisposition::WorkspaceCreationStarted
        {
            next.arm_project_quick_entry();
        }
        if disposition == ClientActionDisposition::DismissNavigator
            && next.overlay == Some(Overlay::WorkbenchNavigator)
        {
            next.replace_overlay(None);
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

    pub(crate) fn decide_navigator_prefix(&self, input: PrefixInput) -> ClientTransition {
        debug_assert!(self.navigator_active() && self.navigator.prefix);
        let mut next = self.clone();
        next.navigator.prefix = false;
        let (action, shell) = next.decide_prefix(input);
        if action == ClientAction::ForwardLiteralPrefix {
            // The capturing Navigator never forwards a literal prefix to the
            // covered pane.
            next.notice = self.notice;
            return ClientTransition {
                next,
                action: None,
                shell: ShellCommand::None,
                layout_changed: false,
            };
        }
        if matches!(
            &action,
            ClientAction::DismissPrefix | ClientAction::UnknownPrefix
        ) {
            // The Navigator's local prefix preserves its prior behavior:
            // canceling or failing to complete it does not dispatch a shell
            // action or replace the visible owner.
            next.notice = self.notice;
            return ClientTransition {
                next,
                action: None,
                shell: ShellCommand::None,
                layout_changed: false,
            };
        }
        if next.navigator_active() {
            // Global prefix actions replace the capturing Navigator unless
            // they are merely dismissing an uncompleted prefix.
            next.close_navigator(None);
        }
        ClientTransition {
            layout_changed: self.sidebar_surface() != next.sidebar_surface(),
            next,
            action: Some(action),
            shell,
        }
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
            PrefixAction::Client(ClientAction::DismissOnboarding)
                if !self.onboarding.visible || self.overlay.is_some() =>
            {
                (ClientAction::DismissPrefix, ShellCommand::None)
            }
            PrefixAction::Client(action) => {
                let (action, _) = self.apply_action_decision(action.clone());
                (action, member.shell)
            }
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
                    // Only open Help scrolls. The renderer's exact viewport
                    // settles any requested offset beyond its last line.
                    self.request_help_page(match member.action {
                        PrefixAction::HelpPagePrevious => {
                            self.help_page().saturating_sub(self.help.page_step())
                        }
                        _ => self.help_page().saturating_add(self.help.page_step()),
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

/// Crate-internal `ClientView` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait ClientViewTestAccess {
    fn notice(&self) -> Option<Notice>;
    fn configuration_notice(&mut self, notice: Notice);
    fn decide(&self, input: PrefixInput) -> ClientTransition;
    fn decide_action(&self, action: ClientAction) -> ClientTransition;
    fn commit(&mut self, transition: ClientTransition, disposition: ClientActionDisposition);
}

#[cfg(any(test, feature = "test-support"))]
impl ClientViewTestAccess for ClientView {
    fn notice(&self) -> Option<Notice> {
        ClientView::notice(self)
    }
    fn configuration_notice(&mut self, notice: Notice) {
        ClientView::configuration_notice(self, notice)
    }
    fn decide(&self, input: PrefixInput) -> ClientTransition {
        ClientView::decide(self, input)
    }
    fn decide_action(&self, action: ClientAction) -> ClientTransition {
        ClientView::decide_action(self, action)
    }
    fn commit(&mut self, transition: ClientTransition, disposition: ClientActionDisposition) {
        ClientView::commit(self, transition, disposition)
    }
}

#[cfg(test)]
mod tests;
