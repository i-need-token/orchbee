//! Semantic theme roles and shipped themes for Orchbee-drawn chrome.
//!
//! Pure data: no environment reads, no clock, no I/O. Host detection and theme
//! selection live with the terminal-environment owner; this module maps the
//! semantic roles of `docs/tui/visual-foundations.md` to concrete values and
//! quantizes them once at startup to a detected color level.

use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
mod host_colors;
mod shipped;
pub(crate) use host_colors::resolve_color_level;

/// One semantic color role. Rendering code references roles only;
/// `Color::Rgb`, `Color::Indexed`, and named ANSI variants appear only here
/// and in shipped theme values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    TextPrimary,
    TextSecondary,
    TextDisabled,
    TextInverse,
    AccentAction,
    AccentFocus,
    AccentSelection,
    #[cfg(test)]
    AccentLink,
    StatusSuccess,
    StatusWarning,
    StatusError,
    StatusInfo,
    StatusPending,
    StatusWorking,
    StatusUnread,
    FrameFocused,
    FrameUnfocused,
    FrameDivider,
    SurfaceCanvas,
    SurfaceChrome,
    SurfaceOverlay,
    SurfaceHover,
    SurfacePressed,
    SurfaceSelection,
    SurfaceCurrent,
    SurfaceInactive,
    HintKey,
    HintLabel,
}

impl Role {
    /// Test/validator helper for totality checks; production consumes roles
    /// individually.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 28] = [
        Self::TextPrimary,
        Self::TextSecondary,
        Self::TextDisabled,
        Self::TextInverse,
        Self::AccentAction,
        Self::AccentFocus,
        Self::AccentSelection,
        Self::AccentLink,
        Self::StatusSuccess,
        Self::StatusWarning,
        Self::StatusError,
        Self::StatusInfo,
        Self::StatusPending,
        Self::StatusWorking,
        Self::StatusUnread,
        Self::FrameFocused,
        Self::FrameUnfocused,
        Self::FrameDivider,
        Self::SurfaceCanvas,
        Self::SurfaceChrome,
        Self::SurfaceOverlay,
        Self::SurfaceHover,
        Self::SurfacePressed,
        Self::SurfaceSelection,
        Self::SurfaceCurrent,
        Self::SurfaceInactive,
        Self::HintKey,
        Self::HintLabel,
    ];
}

/// The detected host color level, supplied by the environment owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColorLevel {
    Monochrome,
    Truecolor,
    Ansi256,
    Ansi16,
}

/// Background polarity an RGB theme is designed for. The terminal-native theme
/// carries none: it must work on an unknown host appearance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Polarity {
    Dark,
    Light,
}

/// The host-safe vocabulary selected for authored chrome glyphs and key labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GlyphMode {
    Unicode,
    #[cfg(test)]
    Simplified,
}

/// Non-ASCII chrome glyphs with a declared ASCII fallback, resolved once for a
/// UTF-8 or simplified-glyph host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GlyphSet {
    mode: GlyphMode,
    /// Truncation marker. `…` is East Asian ambiguous width; `..` is the
    /// ASCII form.
    pub ellipsis: &'static str,
    /// Separator inside frame titles and list rows.
    pub middle_dot: &'static str,
    /// Status glyph used in the sidebar site section.
    pub status_bullet: &'static str,
    pub git_ahead: &'static str,
    pub git_behind: &'static str,
    /// Active marker in navigator and context lists.
    pub active_marker: &'static str,
    /// Reversible keyboard-navigation anchor within a collection.
    pub navigation_marker: &'static str,
    /// One-cell scrollbar chrome, including host-safe ASCII alternatives.
    pub scrollbar_track: &'static str,
    pub scrollbar_thumb: &'static str,
    /// Project disclosure, with one-cell ASCII fallbacks.
    pub disclosure_open: &'static str,
    pub disclosure_closed: &'static str,
}

impl GlyphSet {
    pub(crate) const UTF8: Self = Self {
        mode: GlyphMode::Unicode,
        ellipsis: "…",
        middle_dot: "·",
        status_bullet: "●",
        git_ahead: "↑",
        git_behind: "↓",
        active_marker: "*",
        navigation_marker: "›",
        scrollbar_track: "│",
        scrollbar_thumb: "┃",
        disclosure_open: "▾",
        disclosure_closed: "▸",
    };

    #[cfg(test)]
    pub(crate) const ASCII: Self = Self {
        mode: GlyphMode::Simplified,
        ellipsis: "..",
        middle_dot: "-",
        status_bullet: "*",
        git_ahead: "+",
        git_behind: "-",
        active_marker: "*",
        navigation_marker: ">",
        scrollbar_track: "|",
        scrollbar_thumb: "#",
        disclosure_open: "-",
        disclosure_closed: "+",
    };

    pub(crate) const fn mode(self) -> GlyphMode {
        self.mode
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn resolve(utf8: bool) -> Self {
        if utf8 { Self::UTF8 } else { Self::ASCII }
    }
}

/// One concrete value per semantic role. A field per role makes a theme with a
/// missing role invalid at compile time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RoleStyles {
    pub text_primary: Style,
    pub text_secondary: Style,
    pub text_disabled: Style,
    pub text_inverse: Style,
    pub accent_action: Style,
    pub accent_focus: Style,
    pub accent_selection: Style,
    #[cfg(test)]
    pub accent_link: Style,
    pub status_success: Style,
    pub status_warning: Style,
    pub status_error: Style,
    pub status_info: Style,
    pub status_pending: Style,
    pub status_working: Style,
    pub status_unread: Style,
    pub frame_focused: Style,
    pub frame_unfocused: Style,
    pub frame_divider: Style,
    pub surface_canvas: Style,
    pub surface_chrome: Style,
    pub surface_overlay: Style,
    pub surface_hover: Style,
    pub surface_pressed: Style,
    pub surface_selection: Style,
    pub surface_current: Style,
    pub surface_inactive: Style,
    pub hint_key: Style,
    pub hint_label: Style,
}

impl RoleStyles {
    fn style(&self, role: Role) -> Style {
        match role {
            Role::TextPrimary => self.text_primary,
            Role::TextSecondary => self.text_secondary,
            Role::TextDisabled => self.text_disabled,
            Role::TextInverse => self.text_inverse,
            Role::AccentAction => self.accent_action,
            Role::AccentFocus => self.accent_focus,
            Role::AccentSelection => self.accent_selection,
            #[cfg(test)]
            Role::AccentLink => self.accent_link,
            Role::StatusSuccess => self.status_success,
            Role::StatusWarning => self.status_warning,
            Role::StatusError => self.status_error,
            Role::StatusInfo => self.status_info,
            Role::StatusPending => self.status_pending,
            Role::StatusWorking => self.status_working,
            Role::StatusUnread => self.status_unread,
            Role::FrameFocused => self.frame_focused,
            Role::FrameUnfocused => self.frame_unfocused,
            Role::FrameDivider => self.frame_divider,
            Role::SurfaceCanvas => self.surface_canvas,
            Role::SurfaceChrome => self.surface_chrome,
            Role::SurfaceOverlay => self.surface_overlay,
            Role::SurfaceHover => self.surface_hover,
            Role::SurfacePressed => self.surface_pressed,
            Role::SurfaceSelection => self.surface_selection,
            Role::SurfaceCurrent => self.surface_current,
            Role::SurfaceInactive => self.surface_inactive,
            Role::HintKey => self.hint_key,
            Role::HintLabel => self.hint_label,
        }
    }
    fn map(&self, apply: impl Fn(Style) -> Style) -> Self {
        Self {
            text_primary: apply(self.text_primary),
            text_secondary: apply(self.text_secondary),
            text_disabled: apply(self.text_disabled),
            text_inverse: apply(self.text_inverse),
            accent_action: apply(self.accent_action),
            accent_focus: apply(self.accent_focus),
            accent_selection: apply(self.accent_selection),
            #[cfg(test)]
            accent_link: apply(self.accent_link),
            status_success: apply(self.status_success),
            status_warning: apply(self.status_warning),
            status_error: apply(self.status_error),
            status_info: apply(self.status_info),
            status_pending: apply(self.status_pending),
            status_working: apply(self.status_working),
            status_unread: apply(self.status_unread),
            frame_focused: apply(self.frame_focused),
            frame_unfocused: apply(self.frame_unfocused),
            frame_divider: apply(self.frame_divider),
            surface_canvas: apply(self.surface_canvas),
            surface_chrome: apply(self.surface_chrome),
            surface_overlay: apply(self.surface_overlay),
            surface_hover: apply(self.surface_hover),
            surface_pressed: apply(self.surface_pressed),
            surface_selection: apply(self.surface_selection),
            surface_current: apply(self.surface_current),
            surface_inactive: apply(self.surface_inactive),
            hint_key: apply(self.hint_key),
            hint_label: apply(self.hint_label),
        }
    }
}

/// A complete mapping of roles to concrete styles plus the uniform frame
/// corner option and the resolved glyph set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Theme {
    #[cfg(test)]
    pub name: &'static str,
    pub polarity: Option<Polarity>,
    pub corners: BorderType,
    pub glyphs: GlyphSet,
    color_level: ColorLevel,
    terminal_colors: Option<(Color, Color)>,
    terminal_uses_host_colors: bool,
    styles: RoleStyles,
}

impl Theme {
    pub(crate) fn agent_glyph(
        &self,
        state: orchbee_workbench_protocol::MetadataState,
    ) -> &'static str {
        use orchbee_workbench_protocol::MetadataState::*;
        let ascii = self.glyphs.ellipsis == "..";
        match state {
            Working => {
                if ascii {
                    "~"
                } else {
                    "◐"
                }
            }
            Blocked => "!",
            Done => {
                if ascii {
                    "+"
                } else {
                    "✓"
                }
            }
            Error => {
                if ascii {
                    "x"
                } else {
                    "×"
                }
            }
            Idle => {
                if ascii {
                    "o"
                } else {
                    "○"
                }
            }
            Unknown => "?",
        }
    }
    pub(crate) fn agent_role(state: orchbee_workbench_protocol::MetadataState) -> Role {
        use orchbee_workbench_protocol::MetadataState::*;
        match state {
            Working => Role::StatusWorking,
            Blocked => Role::StatusWarning,
            Done => Role::StatusSuccess,
            Error => Role::StatusError,
            Idle | Unknown => Role::TextSecondary,
        }
    }

    /// User-selected group colors are paired chips, preserving contrast on an unknown host palette.
    /// Monochrome and reduced-color hosts retain the disclosure/count and active marker.
    #[cfg(windows)]
    pub(crate) fn tab_group_style(
        &self,
        color: orchbee_workbench_protocol::TabGroupColor,
    ) -> Style {
        use orchbee_workbench_protocol::TabGroupColor::*;
        let (foreground, background) = match color {
            Blue => (Color::White, Color::Blue),
            Cyan => (Color::Black, Color::Cyan),
            Green => (Color::Black, Color::Green),
            Grey => (Color::Black, Color::Gray),
            Orange => (Color::Black, Color::Rgb(255, 165, 0)),
            Pink => (Color::Black, Color::LightMagenta),
            Purple => (Color::White, Color::Magenta),
            Red => (Color::White, Color::Red),
            Yellow => (Color::Black, Color::Yellow),
        };
        Style::new()
            .fg(quantize_color(foreground, self.color_level))
            .bg(quantize_color(background, self.color_level))
    }
    /// The default theme for an unknown host appearance or color level.
    /// Text and surface roles sit on `Color::Reset`; accent, status, and frame
    /// roles use named ANSI-16 colors; `text-secondary` uses `DIM`, not a gray.
    #[must_use]
    pub(crate) fn terminal_native() -> Self {
        let reset = Style::new().fg(Color::Reset);
        let dim = Style::new().fg(Color::Reset).add_modifier(Modifier::DIM);
        Self {
            #[cfg(test)]
            name: "terminal-native",
            polarity: None,
            corners: BorderType::Plain,
            glyphs: GlyphSet::UTF8,
            color_level: ColorLevel::Truecolor,
            terminal_colors: None,
            terminal_uses_host_colors: false,
            styles: RoleStyles {
                text_primary: reset,
                text_secondary: dim,
                text_disabled: dim,
                text_inverse: reset,
                accent_action: Style::new().fg(Color::Cyan),
                accent_focus: Style::new().fg(Color::Cyan),
                accent_selection: Style::new().fg(Color::White).bg(Color::Blue),
                #[cfg(test)]
                accent_link: Style::new()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
                status_success: Style::new().fg(Color::Green),
                status_warning: Style::new().fg(Color::Magenta),
                status_error: Style::new().fg(Color::Red),
                status_info: Style::new().fg(Color::Cyan),
                status_pending: Style::new().fg(Color::Cyan).add_modifier(Modifier::DIM),
                status_working: Style::new().fg(Color::Magenta),
                status_unread: Style::new().fg(Color::LightBlue),
                frame_focused: Style::new().fg(Color::Cyan),
                frame_unfocused: Style::new().fg(Color::DarkGray),
                frame_divider: Style::new().fg(Color::DarkGray),
                surface_canvas: Style::new().bg(Color::Reset),
                surface_chrome: Style::new().bg(Color::Reset),
                surface_overlay: Style::new().bg(Color::Reset),
                surface_hover: interaction_surface_fallback(),
                surface_pressed: interaction_surface_fallback(),
                surface_selection: Style::new().bg(Color::Blue),
                surface_current: Style::new().fg(Color::Black).bg(Color::Gray),
                surface_inactive: Style::new().bg(Color::Reset),
                hint_key: Style::new().fg(Color::Reset).add_modifier(Modifier::BOLD),
                hint_label: dim,
            },
        }
    }

    /// The RGB theme the render evidence requires alongside `terminal-native`.
    /// Declared dark polarity; a production host-facing RGB theme arrives with
    /// the environment seam — this constructor is test evidence only.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn rgb_test_dark() -> Self {
        Self {
            name: "rgb-test-dark",
            polarity: Some(Polarity::Dark),
            corners: BorderType::Rounded,
            glyphs: GlyphSet::UTF8,
            color_level: ColorLevel::Truecolor,
            terminal_colors: None,
            terminal_uses_host_colors: false,
            styles: RoleStyles {
                text_primary: Style::new().fg(Color::Rgb(211, 218, 232)),
                text_secondary: Style::new().fg(Color::Rgb(133, 146, 171)),
                text_disabled: Style::new().fg(Color::Rgb(91, 101, 119)),
                text_inverse: Style::new().fg(Color::Rgb(11, 14, 20)),
                accent_action: Style::new().fg(Color::Rgb(245, 245, 245)),
                accent_focus: Style::new().fg(Color::Rgb(122, 162, 247)),
                accent_selection: Style::new()
                    .fg(Color::Rgb(211, 218, 232))
                    .bg(Color::Rgb(45, 63, 99)),
                accent_link: Style::new()
                    .fg(Color::Rgb(122, 162, 247))
                    .add_modifier(Modifier::UNDERLINED),
                status_success: Style::new().fg(Color::Rgb(158, 206, 106)),
                status_warning: Style::new().fg(Color::Rgb(232, 179, 95)),
                status_error: Style::new().fg(Color::Rgb(247, 118, 142)),
                status_info: Style::new().fg(Color::Rgb(125, 207, 255)),
                status_working: Style::new().fg(Color::Rgb(192, 132, 252)),
                status_unread: Style::new().fg(Color::Rgb(96, 165, 250)),
                status_pending: Style::new()
                    .fg(Color::Rgb(133, 146, 171))
                    .add_modifier(Modifier::DIM),
                frame_focused: Style::new().fg(Color::Rgb(122, 162, 247)),
                frame_unfocused: Style::new().fg(Color::Rgb(52, 61, 78)),
                frame_divider: Style::new().fg(Color::Rgb(52, 61, 78)),
                surface_canvas: Style::new().bg(Color::Rgb(11, 14, 20)),
                surface_chrome: Style::new().bg(Color::Rgb(11, 14, 20)),
                surface_overlay: Style::new().bg(Color::Rgb(18, 23, 32)),
                surface_hover: Style::new().bg(Color::Rgb(38, 38, 38)),
                surface_pressed: Style::new().bg(Color::Rgb(64, 64, 64)),
                surface_selection: Style::new().bg(Color::Rgb(45, 63, 99)),
                surface_current: Style::new().bg(Color::Rgb(33, 39, 49)),
                surface_inactive: Style::new().bg(Color::Rgb(11, 14, 20)),
                hint_key: Style::new()
                    .fg(Color::Rgb(211, 218, 232))
                    .add_modifier(Modifier::BOLD),
                hint_label: Style::new().fg(Color::Rgb(133, 146, 171)),
            },
        }
    }

    #[must_use]
    pub(crate) fn style(&self, role: Role) -> Style {
        self.styles.style(role)
    }

    /// Composes semantic content onto one base and its winning interaction surface.
    ///
    /// Surface fills and every surface modifier replace or augment the incoming background while
    /// semantic foregrounds and content modifiers remain intact. Host-relative current and
    /// provisional surfaces apply their paired foreground only to ordinary semantic text that has
    /// not received an explicit foreground override. Passing `None` preserves explicit or unknown
    /// content without guessing its meaning. A checked accent keeps its contrast pair beneath a
    /// modifier-only interaction surface.
    pub(crate) fn content_on_surface(
        &self,
        base: Role,
        interaction: Option<Role>,
        content_role: Option<Role>,
        mut content: Style,
    ) -> Style {
        debug_assert!(matches!(
            base,
            Role::SurfaceCanvas
                | Role::SurfaceChrome
                | Role::SurfaceOverlay
                | Role::SurfaceInactive
        ));
        debug_assert!(interaction.is_none_or(|role| matches!(
            role,
            Role::SurfaceHover
                | Role::SurfacePressed
                | Role::SurfaceSelection
                | Role::SurfaceCurrent
        )));
        let base_style = self.style(base);
        let interaction_style = interaction.map(|role| self.style(role));
        let content_foreground = content.fg;
        let preserve_checked_pair = content_role == Some(Role::AccentSelection)
            && interaction_style.and_then(|style| style.bg).is_none()
            && content.bg.is_some();
        let mut base_layer = base_style;
        // A surface foreground is a contrast pair, not a replacement for every semantic content
        // role. Pairing ordinary host-relative text is handled explicitly below.
        base_layer.fg = None;
        if preserve_checked_pair {
            // A checked glyph keeps its safe contrast pair until an interaction contributes a
            // concrete replacement fill. Modifier-only interaction still layers above the pair.
            base_layer.bg = None;
        }
        content = content.patch(base_layer);
        if let Some(mut layer) = interaction_style {
            layer.fg = None;
            content = content.patch(layer);
        }

        let paired = match interaction {
            Some(Role::SurfaceCurrent) => interaction_style.and_then(|style| style.fg),
            Some(Role::SurfaceSelection) => self.style(Role::AccentSelection).fg,
            _ => None,
        };
        let ordinary = matches!(
            content_role,
            Some(
                Role::TextPrimary
                    | Role::TextSecondary
                    | Role::TextDisabled
                    | Role::HintKey
                    | Role::HintLabel
            )
        );
        if self.color_level != ColorLevel::Monochrome
            && self.polarity.is_none()
            && ordinary
            && let Some(role) = content_role
            && content_foreground == self.style(role).fg
            && let Some(foreground) = paired
            && foreground != Color::Reset
        {
            content = content.fg(foreground);
            if matches!(role, Role::TextSecondary | Role::HintLabel) {
                content = content.remove_modifier(Modifier::DIM);
            }
        }
        content
    }

    /// Persistent context uses a neutral fill. Host-relative text needs an explicit
    /// foreground on that fill; named themes keep their primary/secondary hierarchy.
    pub(crate) fn current_context_style(&self, mut style: Style) -> Style {
        let surface = self.style(Role::SurfaceCurrent);
        if let Some(foreground) = surface.fg
            && (style.fg == self.styles.text_primary.fg
                || style.fg == self.styles.text_secondary.fg)
        {
            style = style.fg(foreground).remove_modifier(Modifier::DIM);
        }
        style
            .bg(self.color(Role::SurfaceCurrent))
            .add_modifier(surface.add_modifier)
    }

    /// Quantization can collapse the context fill onto chrome. Retain a text
    /// marker in that case, including when token overrides suppress bold text.
    pub(crate) fn current_context_marker(&self) -> &'static str {
        if self.styles.surface_current.bg == self.styles.surface_chrome.bg {
            self.glyphs.active_marker
        } else {
            " "
        }
    }

    /// Explicit user token styles remain data owned by the theme boundary.
    #[cfg(windows)]
    pub(crate) fn sidebar_token_style(
        &self,
        role: Role,
        token: orchbee_local_site_runtime::WindowsSidebarTokenStyle,
        current: bool,
        focused: bool,
    ) -> Style {
        let mut style = if current {
            self.current_context_style(self.style(role))
        } else {
            self.style(role)
        };
        if focused && !current {
            let surface = self.style(Role::AccentSelection);
            if matches!(
                role,
                Role::TextPrimary
                    | Role::TextSecondary
                    | Role::TextDisabled
                    | Role::HintKey
                    | Role::HintLabel
            ) && let Some(foreground) = surface.fg
            {
                style = style.fg(foreground).remove_modifier(Modifier::DIM);
            }
            style = style
                .bg(self.color(Role::SurfaceSelection))
                .add_modifier(surface.add_modifier);
        }
        if let Some([r, g, b]) = token.fg {
            let color = quantize_color(Color::Rgb(r, g, b), self.color_level);
            style = style.fg(if color == Color::DarkGray {
                nearest_text_ansi16(r, g, b)
            } else {
                color
            });
        }
        for (enabled, modifier) in [(token.bold, Modifier::BOLD), (token.dim, Modifier::DIM)] {
            style = match enabled {
                Some(true) => style.add_modifier(modifier),
                Some(false) => style.remove_modifier(modifier),
                None => style,
            };
        }
        style
    }

    /// Workspace selection spans metadata as well as its identity. Named palettes
    /// keep secondary text muted; host-relative text retains the paired foreground.
    #[cfg(windows)]
    pub(crate) fn workspace_token_style(
        &self,
        role: Role,
        token: orchbee_local_site_runtime::WindowsSidebarTokenStyle,
        current: bool,
        focused: bool,
    ) -> Style {
        let mut style = self.sidebar_token_style(role, token, current, focused);
        if focused
            && !current
            && role == Role::TextSecondary
            && token.fg.is_none()
            && self.polarity.is_some()
        {
            style.fg = self.style(Role::TextSecondary).fg;
        }
        style
    }

    /// The role's color: the background for `surface-*` roles, the foreground
    /// for every other family. Interactive-surface renderers compose the full
    /// [`Style`] so reduced-color fallback modifiers are not discarded; this
    /// accessor only exposes a concrete fill when one exists.
    #[must_use]
    pub(crate) fn color(&self, role: Role) -> Color {
        let style = self.style(role);
        match role {
            Role::SurfaceCanvas
            | Role::SurfaceChrome
            | Role::SurfaceOverlay
            | Role::SurfaceHover
            | Role::SurfacePressed
            | Role::SurfaceSelection
            | Role::SurfaceCurrent
            | Role::SurfaceInactive => style.bg,
            _ => style.fg,
        }
        .unwrap_or(Color::Reset)
    }

    /// Default-marked cells follow the theme, including host-relative Reset.
    /// Frame defaults describe the retained program, not the Client's host.
    /// Explicit program colors bypass this mapping.
    pub(crate) fn terminal_defaults(&self) -> (Color, Color) {
        self.terminal_colors.unwrap_or((
            self.color(Role::TextPrimary),
            self.color(Role::SurfaceCanvas),
        ))
    }

    /// Client terminal colors are independent of application chrome and explicit program SGR.
    pub(crate) fn with_terminal_preferences(
        mut self,
        values: &orchbee_workbench_protocol::UserPreferencesValues,
    ) -> Self {
        use orchbee_workbench_protocol::TerminalDisplayAppearance;
        self.terminal_uses_host_colors =
            values.terminal_display_theme == TerminalDisplayAppearance::Host;
        let colors = match values.terminal_display_theme {
            TerminalDisplayAppearance::Host | TerminalDisplayAppearance::Light => {
                (0x141414, 0xfcfcfc)
            }
            TerminalDisplayAppearance::Dark => (0xd4d4d4, 0x1e1e1e),
            TerminalDisplayAppearance::Custom => (
                u32::from_str_radix(&values.custom_foreground[1..], 16)
                    .expect("validated preference color"),
                u32::from_str_radix(&values.custom_background[1..], 16)
                    .expect("validated preference color"),
            ),
        };
        self.set_terminal_colors(colors.0, colors.1);
        self
    }

    pub(crate) fn with_host_terminal_colors(mut self, foreground: u32, background: u32) -> Self {
        if self.terminal_uses_host_colors {
            self.set_terminal_colors(foreground, background);
        }
        self
    }

    fn set_terminal_colors(&mut self, foreground: u32, background: u32) {
        let color = |value: u32| {
            quantize_color(
                Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8),
                self.color_level,
            )
        };
        self.terminal_colors = Some((color(foreground), color(background)));
    }

    /// Swap the resolved glyph set (UTF-8 host or declared ASCII fallbacks).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_glyphs(mut self, glyphs: GlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Quantize every role value once at startup to the detected color level.
    /// `Reset` and named colors pass through color-capable levels; monochrome
    /// resets all colors. At `Ansi16` no `Rgb` or `Indexed` value remains.
    #[must_use]
    pub(crate) fn quantize(self, level: ColorLevel) -> Self {
        let mut styles = self.styles.map(|style| quantize_style(style, level));
        if level == ColorLevel::Ansi16 && self.polarity.is_some() {
            for (target, source) in [
                (&mut styles.text_primary, self.styles.text_primary),
                (&mut styles.text_secondary, self.styles.text_secondary),
                (&mut styles.text_disabled, self.styles.text_disabled),
                (&mut styles.text_inverse, self.styles.text_inverse),
                (&mut styles.accent_action, self.styles.accent_action),
                (&mut styles.accent_focus, self.styles.accent_focus),
                (&mut styles.accent_selection, self.styles.accent_selection),
                #[cfg(test)]
                (&mut styles.accent_link, self.styles.accent_link),
                (&mut styles.status_success, self.styles.status_success),
                (&mut styles.status_warning, self.styles.status_warning),
                (&mut styles.status_error, self.styles.status_error),
                (&mut styles.status_info, self.styles.status_info),
                (&mut styles.status_pending, self.styles.status_pending),
                (&mut styles.status_working, self.styles.status_working),
                (&mut styles.status_unread, self.styles.status_unread),
                (&mut styles.surface_canvas, self.styles.surface_canvas),
                (&mut styles.surface_chrome, self.styles.surface_chrome),
                (&mut styles.surface_overlay, self.styles.surface_overlay),
                (&mut styles.surface_hover, self.styles.surface_hover),
                (&mut styles.surface_pressed, self.styles.surface_pressed),
                (&mut styles.surface_selection, self.styles.surface_selection),
                (&mut styles.surface_current, self.styles.surface_current),
                (&mut styles.surface_inactive, self.styles.surface_inactive),
                (&mut styles.hint_key, self.styles.hint_key),
                (&mut styles.hint_label, self.styles.hint_label),
            ] {
                if target.fg == Some(Color::DarkGray)
                    && let Some(Color::Rgb(r, g, b)) = source.fg
                {
                    *target = target.fg(nearest_text_ansi16(r, g, b));
                }
            }
            // A dark divider can collapse onto the painted background at 16 colors.
            // Keep the nearest eligible frame color rather than hiding a boundary.
            for (target, source) in [
                (&mut styles.frame_focused, self.styles.frame_focused),
                (&mut styles.frame_unfocused, self.styles.frame_unfocused),
                (&mut styles.frame_divider, self.styles.frame_divider),
            ] {
                if target.fg == styles.surface_canvas.bg
                    && let Some(Color::Rgb(r, g, b)) = source.fg
                {
                    *target = target.fg(nearest_visible_ansi16(r, g, b, styles.surface_canvas.bg));
                }
            }
        }
        if level == ColorLevel::Monochrome
            || (matches!(level, ColorLevel::Ansi16 | ColorLevel::Ansi256)
                && interaction_surfaces_collapse(&styles))
        {
            styles.surface_hover = interaction_surface_fallback();
            styles.surface_pressed = interaction_surface_fallback();
        }
        if matches!(level, ColorLevel::Ansi16 | ColorLevel::Monochrome) {
            let primary = styles.text_primary.fg;
            for secondary in [
                &mut styles.text_secondary,
                &mut styles.text_disabled,
                &mut styles.hint_label,
            ] {
                if secondary.fg == primary {
                    *secondary = secondary.add_modifier(Modifier::DIM);
                }
            }
        }
        if level == ColorLevel::Monochrome {
            styles.frame_focused = styles.frame_focused.add_modifier(Modifier::BOLD);
            styles.accent_focus = styles.accent_focus.add_modifier(Modifier::BOLD);
            styles.surface_selection = styles.surface_selection.add_modifier(Modifier::BOLD);
            styles.surface_current = styles.surface_current.add_modifier(Modifier::BOLD);
            styles.accent_selection = styles.accent_selection.add_modifier(Modifier::BOLD);
        }
        Self {
            styles,
            color_level: level,
            terminal_colors: self.terminal_colors.map(|(foreground, background)| {
                (
                    quantize_color(foreground, level),
                    quantize_color(background, level),
                )
            }),
            ..self
        }
    }
}

/// Terminal-native and collapsed reduced-color interaction bands use the
/// resolver-owned reverse-video fallback. Components add pressed-label
/// emphasis separately; keeping it out of the surface style prevents an
/// entire row from becoming bold.
fn interaction_surface_fallback() -> Style {
    Style::new().add_modifier(Modifier::REVERSED)
}

fn interaction_surfaces_collapse(styles: &RoleStyles) -> bool {
    let hover = styles.surface_hover.bg;
    let pressed = styles.surface_pressed.bg;
    let non_interaction = [
        styles.surface_canvas.bg,
        styles.surface_chrome.bg,
        styles.surface_overlay.bg,
        styles.surface_selection.bg,
        styles.surface_current.bg,
        styles.surface_inactive.bg,
    ];
    hover == pressed
        || [hover, pressed]
            .into_iter()
            .any(|surface| surface.is_none() || non_interaction.contains(&surface))
}

fn nearest_visible_ansi16(r: u8, g: u8, b: u8, background: Option<Color>) -> Color {
    let target = srgb_to_lab(r, g, b);
    ANSI16_SRGB
        .iter()
        .filter(|(color, _)| Some(*color) != background)
        .min_by(|(_, left), (_, right)| {
            let distance = |&(r, g, b)| squared_distance(target, srgb_to_lab(r, g, b));
            distance(left).total_cmp(&distance(right))
        })
        .map(|(color, _)| *color)
        .expect("a painted background cannot exclude every ANSI frame color")
}
fn quantize_style(style: Style, level: ColorLevel) -> Style {
    let mut out = style;
    if let Some(fg) = style.fg {
        out = out.fg(quantize_color(fg, level));
    }
    if let Some(bg) = style.bg {
        out = out.bg(quantize_color(bg, level));
    }
    // `Style::underline_color` exists only under ratatui's `underline-color`
    // feature, which this crate does not enable; if it is ever enabled the
    // field must be quantized at the same level here.
    out
}
fn quantize_color(color: Color, level: ColorLevel) -> Color {
    match level {
        ColorLevel::Monochrome => Color::Reset,
        ColorLevel::Truecolor => color,
        ColorLevel::Ansi256 => match color {
            Color::Rgb(r, g, b) => Color::Indexed(nearest_indexed(r, g, b)),
            color => color,
        },
        ColorLevel::Ansi16 => match color {
            Color::Rgb(r, g, b) => nearest_ansi16(r, g, b),
            Color::Indexed(index) => {
                let (r, g, b) = INDEXED_SRGB[usize::from(index)];
                nearest_ansi16(r, g, b)
            }
            color => color,
        },
    }
}

/// Canonical xterm sRGB values for the ANSI-16 names. Host palettes vary;
/// quantization targets these documented values, not a live palette read.
const ANSI16_SRGB: [(Color, (u8, u8, u8)); 16] = [
    (Color::Black, (0, 0, 0)),
    (Color::Red, (205, 0, 0)),
    (Color::Green, (0, 205, 0)),
    (Color::Yellow, (205, 205, 0)),
    (Color::Blue, (0, 0, 238)),
    (Color::Magenta, (205, 0, 205)),
    (Color::Cyan, (0, 205, 205)),
    (Color::Gray, (229, 229, 229)),
    (Color::DarkGray, (127, 127, 127)),
    (Color::LightRed, (255, 0, 0)),
    (Color::LightGreen, (0, 255, 0)),
    (Color::LightYellow, (255, 255, 0)),
    (Color::LightBlue, (92, 92, 255)),
    (Color::LightMagenta, (255, 0, 255)),
    (Color::LightCyan, (0, 255, 255)),
    (Color::White, (255, 255, 255)),
];
const INDEXED_SRGB: [(u8, u8, u8); 256] = build_indexed_srgb();
const fn build_indexed_srgb() -> [(u8, u8, u8); 256] {
    let mut table = [(0_u8, 0_u8, 0_u8); 256];
    let mut index = 0;
    while index < 16 {
        table[index] = ANSI16_SRGB[index].1;
        index += 1;
    }
    let levels = [0_u8, 95, 135, 175, 215, 255];
    while index < 232 {
        let cube = index - 16;
        table[index] = (levels[cube / 36], levels[(cube / 6) % 6], levels[cube % 6]);
        index += 1;
    }
    while index < 256 {
        let gray = 8 + 10 * (index - 232) as u8;
        table[index] = (gray, gray, gray);
        index += 1;
    }
    table
}
fn nearest_ansi16(red: u8, green: u8, blue: u8) -> Color {
    let target = srgb_to_lab(red, green, blue);
    let mut nearest = Color::Black;
    let mut nearest_distance = f64::MAX;
    for (color, (r, g, b)) in ANSI16_SRGB {
        let lab = srgb_to_lab(r, g, b);
        let distance = squared_distance(target, lab);
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest = color;
        }
    }
    nearest
}

/// Text excludes the bright-black palette entry before choosing the nearest eligible match.
fn nearest_text_ansi16(red: u8, green: u8, blue: u8) -> Color {
    let target = srgb_to_lab(red, green, blue);
    let mut nearest = Color::Black;
    let mut nearest_distance = f64::MAX;
    for (color, (r, g, b)) in ANSI16_SRGB {
        if color == Color::DarkGray {
            continue;
        }
        let distance = squared_distance(target, srgb_to_lab(r, g, b));
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest = color;
        }
    }
    nearest
}
fn nearest_indexed(red: u8, green: u8, blue: u8) -> u8 {
    let target = srgb_to_lab(red, green, blue);
    let mut nearest = 0_u8;
    let mut nearest_distance = f64::MAX;
    for (index, (r, g, b)) in INDEXED_SRGB.iter().enumerate() {
        let lab = srgb_to_lab(*r, *g, *b);
        let distance = squared_distance(target, lab);
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest = index as u8;
        }
    }
    nearest
}
fn squared_distance(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)
}

/// sRGB → CIELAB (D65) for nearest-perceptual-match quantization.
fn srgb_to_lab(red: u8, green: u8, blue: u8) -> (f64, f64, f64) {
    fn linear(component: u8) -> f64 {
        let value = f64::from(component) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    let (r, g, b) = (linear(red), linear(green), linear(blue));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    fn cube_root(t: f64) -> f64 {
        let delta = 6.0 / 29.0;
        if t > delta * delta * delta {
            t.powf(1.0 / 3.0)
        } else {
            t / (3.0 * delta * delta) + 4.0 / 29.0
        }
    }
    let (fx, fy, fz) = (cube_root(x), cube_root(y), cube_root(z));
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

#[cfg(test)]
mod tests;
