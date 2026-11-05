//! Semantic theme roles and shipped themes for Orchbee-drawn chrome.
//!
//! Pure data: no environment reads, no clock, no I/O. Host detection lives with the
//! terminal-environment owner; this module chooses between a shipped theme and Terminal colors,
//! maps the semantic roles of `docs/tui/visual-foundations.md` to concrete values, and quantizes
//! them once to the detected color level.

use crate::terminal_events::keys::KeyLabelGlyphs;
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
mod host_colors;
mod name;
mod shipped;
mod source;
mod tab_groups;
pub use host_colors::resolve_color_level;
pub use name::ThemeName;
pub(crate) use source::{ColorPreferences, named_themes_available};
#[cfg(test)]
pub(crate) use source::{Resolution, TerminalReason};
pub(crate) use tab_groups::TabGroupPalette;

/// One semantic color role. Rendering code references roles only;
/// `Color::Rgb`, `Color::Indexed`, and named ANSI variants appear only here
/// and in shipped theme values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    TextPrimary,
    TextSecondary,
    /// Quieter than secondary, for labels that only group what follows them.
    TextMuted,
    TextDisabled,
    TextInverse,
    AccentAction,
    AccentDestructive,
    AccentFocus,
    AccentSelection,
    #[cfg(any(test, feature = "test-support"))]
    AccentLink,
    StatusSuccess,
    StatusWarning,
    StatusError,
    StatusInfo,
    StatusPending,
    StatusWorking,
    StatusIdle,
    StatusUnread,
    AgentAttention,
    AgentDone,
    AgentError,
    FrameFocused,
    FrameUnfocused,
    FrameDivider,
    FrameGuide,
    SurfaceCanvas,
    SurfaceChrome,
    SurfaceStrip,
    SurfaceOverlay,
    SurfaceField,
    SurfaceHover,
    SurfacePressed,
    SurfaceSelection,
    SurfaceCurrent,
    SurfaceRelated,
    SurfaceInactive,
    HintKey,
    HintLabel,
}

impl Role {
    /// Test/validator helper for totality checks; production consumes roles
    /// individually.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 38] = [
        Self::TextPrimary,
        Self::TextSecondary,
        Self::TextMuted,
        Self::TextDisabled,
        Self::TextInverse,
        Self::AccentAction,
        Self::AccentDestructive,
        Self::AccentFocus,
        Self::AccentSelection,
        Self::AccentLink,
        Self::StatusSuccess,
        Self::StatusWarning,
        Self::StatusError,
        Self::StatusInfo,
        Self::StatusPending,
        Self::StatusWorking,
        Self::StatusIdle,
        Self::StatusUnread,
        Self::AgentAttention,
        Self::AgentDone,
        Self::AgentError,
        Self::FrameFocused,
        Self::FrameUnfocused,
        Self::FrameDivider,
        Self::FrameGuide,
        Self::SurfaceCanvas,
        Self::SurfaceChrome,
        Self::SurfaceStrip,
        Self::SurfaceOverlay,
        Self::SurfaceField,
        Self::SurfaceHover,
        Self::SurfacePressed,
        Self::SurfaceSelection,
        Self::SurfaceCurrent,
        Self::SurfaceRelated,
        Self::SurfaceInactive,
        Self::HintKey,
        Self::HintLabel,
    ];

    /// Agent state and status roles, whose hue carries meaning even on a one-cell glyph.
    const fn is_state_hue(self) -> bool {
        matches!(
            self,
            Self::StatusSuccess
                | Self::StatusWarning
                | Self::StatusError
                | Self::StatusInfo
                | Self::StatusPending
                | Self::StatusWorking
                | Self::StatusIdle
                | Self::StatusUnread
                | Self::AgentAttention
                | Self::AgentDone
                | Self::AgentError
        )
    }
}

/// The detected host color level, supplied by the environment owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorLevel {
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

impl Polarity {
    /// The polarity a named theme slot follows under System; unknown appearance has none.
    pub(crate) const fn of_host(host: crate::host_appearance::HostAppearance) -> Option<Self> {
        match host {
            crate::host_appearance::HostAppearance::Dark => Some(Self::Dark),
            crate::host_appearance::HostAppearance::Light => Some(Self::Light),
            crate::host_appearance::HostAppearance::Unknown => None,
        }
    }
}

/// The host-safe vocabulary selected for authored chrome glyphs and key labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GlyphMode {
    Unicode,
    #[cfg(any(test, feature = "test-support"))]
    Simplified,
}

/// Non-ASCII chrome glyphs with a declared ASCII fallback, resolved once for a
/// UTF-8 or simplified-glyph host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlyphSet {
    mode: GlyphMode,
    /// Truncation marker. `…` is East Asian ambiguous width; `..` is the
    /// ASCII form.
    pub(crate) ellipsis: &'static str,
    /// Separator inside frame titles and list rows.
    pub(crate) middle_dot: &'static str,
    /// Status glyph used in the sidebar session section.
    pub(crate) status_bullet: &'static str,
    pub(crate) git_ahead: &'static str,
    pub(crate) git_behind: &'static str,
    /// Active marker in navigator and context lists.
    pub(crate) active_marker: &'static str,
    /// The checked option in a menu's checked track, such as a group's applied color.
    pub(crate) checked_marker: &'static str,
    /// Reversible keyboard-navigation anchor within a collection.
    pub(crate) navigation_marker: &'static str,
    /// One-cell rule between strip regions, such as pinned and ordinary Tabs.
    pub(crate) vertical_rule: &'static str,
    /// One-cell scrollbar chrome, including host-safe ASCII alternatives.
    pub(crate) scrollbar_track: &'static str,
    pub(crate) scrollbar_thumb: &'static str,
    /// Project disclosure, with one-cell ASCII fallbacks.
    pub(crate) disclosure_open: &'static str,
    pub(crate) disclosure_closed: &'static str,
    /// One-cell vertical guide connecting sibling Agents to their Workspace.
    pub(crate) tree_guide: &'static str,
    /// The sidebar toggle's glyph while it hides the visible sidebar and while it shows the hidden
    /// one. The ASCII forms appear only inside that bracketed toggle, never in a marker track.
    pub(crate) sidebar_hide: &'static str,
    pub(crate) sidebar_show: &'static str,
    /// One-cell Working frames; every frame has the same display width.
    pub(crate) working_frames: &'static [&'static str],
}

impl GlyphSet {
    pub(crate) const UTF8: Self = Self {
        mode: GlyphMode::Unicode,
        ellipsis: "…",
        middle_dot: "·",
        status_bullet: "•",
        git_ahead: "↑",
        git_behind: "↓",
        active_marker: "*",
        checked_marker: "✓",
        navigation_marker: "›",
        vertical_rule: "│",
        scrollbar_track: "│",
        scrollbar_thumb: "┃",
        disclosure_open: "▾",
        disclosure_closed: "▸",
        tree_guide: "│",
        sidebar_hide: "«",
        sidebar_show: "»",
        working_frames: &["⬢"],
    };

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) const ASCII: Self = Self {
        mode: GlyphMode::Simplified,
        ellipsis: "..",
        middle_dot: "-",
        status_bullet: "*",
        git_ahead: "+",
        git_behind: "-",
        active_marker: "*",
        checked_marker: "*",
        navigation_marker: ">",
        vertical_rule: "|",
        scrollbar_track: "|",
        scrollbar_thumb: "#",
        disclosure_open: "v",
        disclosure_closed: ">",
        tree_guide: "|",
        sidebar_hide: "<",
        sidebar_show: ">",
        working_frames: &["-", "\\", "|", "/"],
    };

    pub(crate) const fn mode(self) -> GlyphMode {
        self.mode
    }

    /// The key-label vocabulary that matches this glyph set.
    pub(crate) const fn key_labels(self) -> KeyLabelGlyphs {
        match self.mode {
            GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
            #[cfg(any(test, feature = "test-support"))]
            GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
        }
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
    pub(crate) text_primary: Style,
    pub(crate) text_secondary: Style,
    pub(crate) text_muted: Style,
    pub(crate) text_disabled: Style,
    pub(crate) text_inverse: Style,
    pub(crate) accent_action: Style,
    pub(crate) accent_destructive: Style,
    pub(crate) accent_focus: Style,
    pub(crate) accent_selection: Style,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) accent_link: Style,
    pub(crate) status_success: Style,
    pub(crate) status_warning: Style,
    pub(crate) status_error: Style,
    pub(crate) status_info: Style,
    pub(crate) status_pending: Style,
    pub(crate) status_working: Style,
    pub(crate) status_idle: Style,
    pub(crate) status_unread: Style,
    pub(crate) agent_attention: Style,
    pub(crate) agent_done: Style,
    pub(crate) agent_error: Style,
    pub(crate) frame_focused: Style,
    pub(crate) frame_unfocused: Style,
    pub(crate) frame_divider: Style,
    pub(crate) frame_guide: Style,
    pub(crate) surface_canvas: Style,
    pub(crate) surface_chrome: Style,
    pub(crate) surface_strip: Style,
    pub(crate) surface_overlay: Style,
    pub(crate) surface_field: Style,
    pub(crate) surface_hover: Style,
    pub(crate) surface_pressed: Style,
    pub(crate) surface_selection: Style,
    pub(crate) surface_current: Style,
    pub(crate) surface_related: Style,
    pub(crate) surface_inactive: Style,
    pub(crate) hint_key: Style,
    pub(crate) hint_label: Style,
}

impl RoleStyles {
    fn style(&self, role: Role) -> Style {
        match role {
            Role::TextPrimary => self.text_primary,
            Role::TextSecondary => self.text_secondary,
            Role::TextMuted => self.text_muted,
            Role::TextDisabled => self.text_disabled,
            Role::TextInverse => self.text_inverse,
            Role::AccentAction => self.accent_action,
            Role::AccentDestructive => self.accent_destructive,
            Role::AccentFocus => self.accent_focus,
            Role::AccentSelection => self.accent_selection,
            #[cfg(any(test, feature = "test-support"))]
            Role::AccentLink => self.accent_link,
            Role::StatusSuccess => self.status_success,
            Role::StatusWarning => self.status_warning,
            Role::StatusError => self.status_error,
            Role::StatusInfo => self.status_info,
            Role::StatusPending => self.status_pending,
            Role::StatusWorking => self.status_working,
            Role::StatusIdle => self.status_idle,
            Role::StatusUnread => self.status_unread,
            Role::AgentAttention => self.agent_attention,
            Role::AgentDone => self.agent_done,
            Role::AgentError => self.agent_error,
            Role::FrameFocused => self.frame_focused,
            Role::FrameUnfocused => self.frame_unfocused,
            Role::FrameDivider => self.frame_divider,
            Role::FrameGuide => self.frame_guide,
            Role::SurfaceCanvas => self.surface_canvas,
            Role::SurfaceChrome => self.surface_chrome,
            Role::SurfaceStrip => self.surface_strip,
            Role::SurfaceOverlay => self.surface_overlay,
            Role::SurfaceField => self.surface_field,
            Role::SurfaceHover => self.surface_hover,
            Role::SurfacePressed => self.surface_pressed,
            Role::SurfaceSelection => self.surface_selection,
            Role::SurfaceCurrent => self.surface_current,
            Role::SurfaceRelated => self.surface_related,
            Role::SurfaceInactive => self.surface_inactive,
            Role::HintKey => self.hint_key,
            Role::HintLabel => self.hint_label,
        }
    }
    fn map(&self, apply: impl Fn(Style) -> Style) -> Self {
        Self {
            text_primary: apply(self.text_primary),
            text_secondary: apply(self.text_secondary),
            text_muted: apply(self.text_muted),
            text_disabled: apply(self.text_disabled),
            text_inverse: apply(self.text_inverse),
            accent_action: apply(self.accent_action),
            accent_destructive: apply(self.accent_destructive),
            accent_focus: apply(self.accent_focus),
            accent_selection: apply(self.accent_selection),
            #[cfg(any(test, feature = "test-support"))]
            accent_link: apply(self.accent_link),
            status_success: apply(self.status_success),
            status_warning: apply(self.status_warning),
            status_error: apply(self.status_error),
            status_info: apply(self.status_info),
            status_pending: apply(self.status_pending),
            status_working: apply(self.status_working),
            status_idle: apply(self.status_idle),
            status_unread: apply(self.status_unread),
            agent_attention: apply(self.agent_attention),
            agent_done: apply(self.agent_done),
            agent_error: apply(self.agent_error),
            frame_focused: apply(self.frame_focused),
            frame_unfocused: apply(self.frame_unfocused),
            frame_divider: apply(self.frame_divider),
            frame_guide: apply(self.frame_guide),
            surface_canvas: apply(self.surface_canvas),
            surface_chrome: apply(self.surface_chrome),
            surface_strip: apply(self.surface_strip),
            surface_overlay: apply(self.surface_overlay),
            surface_field: apply(self.surface_field),
            surface_hover: apply(self.surface_hover),
            surface_pressed: apply(self.surface_pressed),
            surface_selection: apply(self.surface_selection),
            surface_current: apply(self.surface_current),
            surface_related: apply(self.surface_related),
            surface_inactive: apply(self.surface_inactive),
            hint_key: apply(self.hint_key),
            hint_label: apply(self.hint_label),
        }
    }
}

/// Agent state glyphs follow a traffic light with one value in every theme and host: Needs
/// attention, Completed and Failed. Quantization maps them to Yellow, Green and Red at 16 colors.
const AGENT_ATTENTION: Color = Color::Rgb(254, 188, 46);
const AGENT_DONE: Color = Color::Rgb(40, 200, 64);
const AGENT_ERROR: Color = Color::Rgb(255, 95, 87);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalColorSource {
    Palette,
    RetainedFrame,
    OuterHost,
}

/// How resting destructive wording meets the unpainted host default background.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DestructiveRest {
    /// The terminal-native contract: the host palette's own red, key hint included, rests on
    /// the host default without a backing fill.
    HostRed,
    /// Every other theme keeps the resolver's contrast pair wherever the background is unknown.
    Paired,
}

/// A complete mapping of roles to concrete styles plus the uniform frame
/// corner option and the resolved glyph set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) name: &'static str,
    pub(crate) polarity: Option<Polarity>,
    /// The shipped theme a named theme was resolved from; `terminal-native` has none.
    palette: Option<&'static orchbee_color_themes::Theme>,
    pub(crate) corners: BorderType,
    pub(crate) glyphs: GlyphSet,
    color_level: ColorLevel,
    no_color: bool,
    terminal_colors: Option<(Color, Color)>,
    terminal_rgb_colors: Option<(u32, u32)>,
    terminal_color_source: TerminalColorSource,
    terminal_defaults_known: bool,
    destructive_rest: DestructiveRest,
    styles: RoleStyles,
    pub(crate) tab_groups: TabGroupPalette,
}

impl Theme {
    /// The filled `⬢` U+2B22 and hollow `⬡` U+2B21 hexagons echo the honeycomb mark. Both are East
    /// Asian Neutral, so they keep one cell even on hosts that widen ambiguous-width text.
    pub(crate) fn agent_glyph(
        &self,
        state: orchbee_workbench_protocol::MetadataState,
    ) -> &'static str {
        use orchbee_workbench_protocol::MetadataState::*;
        let ascii = self.glyphs.ellipsis == "..";
        let distinguish_outcomes =
            ascii || self.no_color || self.color_level == ColorLevel::Monochrome;
        match state {
            Working => self.glyphs.working_frames.first().copied().unwrap_or("~"),
            Blocked => {
                if distinguish_outcomes {
                    "#"
                } else {
                    "⬢"
                }
            }
            Done => {
                if distinguish_outcomes {
                    "+"
                } else {
                    "⬢"
                }
            }
            Error => {
                if distinguish_outcomes {
                    "x"
                } else {
                    "⬢"
                }
            }
            Idle => {
                if ascii {
                    "o"
                } else {
                    "⬡"
                }
            }
            Unknown => {
                if ascii {
                    "."
                } else {
                    self.glyphs.status_bullet
                }
            }
        }
    }
    pub(crate) fn agent_role(state: orchbee_workbench_protocol::MetadataState) -> Role {
        use orchbee_workbench_protocol::MetadataState::*;
        match state {
            Working => Role::StatusWorking,
            Blocked => Role::AgentAttention,
            Done => Role::AgentDone,
            Error => Role::AgentError,
            Idle => Role::StatusIdle,
            Unknown => Role::TextSecondary,
        }
    }

    /// A stationary Working hexagon breathes over two seconds; attention keeps its 500 ms pulse.
    pub(crate) fn agent_style(
        &self,
        state: orchbee_workbench_protocol::MetadataState,
        frame: usize,
    ) -> Style {
        let style = self.style(Self::agent_role(state));
        use orchbee_workbench_protocol::MetadataState::{Blocked, Working};
        if self.no_color || self.color_level == ColorLevel::Monochrome {
            return style;
        }
        let intensity = match state {
            Working => {
                let phase = (frame % 20) as f64 / 20.0 * std::f64::consts::TAU;
                0.5 + 0.5 * (1.0 + phase.cos()) / 2.0
            }
            Blocked if !(frame / 5).is_multiple_of(2) => 0.5,
            Blocked
            | orchbee_workbench_protocol::MetadataState::Idle
            | orchbee_workbench_protocol::MetadataState::Done
            | orchbee_workbench_protocol::MetadataState::Error
            | orchbee_workbench_protocol::MetadataState::Unknown => return style,
        };
        let colors = match (style.fg, self.styles.surface_canvas.bg) {
            (Some(Color::Rgb(r, g, b)), Some(Color::Rgb(br, bg, bb))) => {
                Some(((r, g, b), (br, bg, bb)))
            }
            (Some(foreground), _)
                if state == Working && self.color_level == ColorLevel::Truecolor =>
            {
                // System surfaces keep Reset even when the outer host's RGB is known. Child
                // terminal preferences must not replace the background used by authored chrome.
                let background = (self.terminal_color_source == TerminalColorSource::OuterHost)
                    .then_some(self.terminal_colors)
                    .flatten()
                    .and_then(|(_, background)| sampled_color(background))
                    .unwrap_or(if self.polarity == Some(Polarity::Light) {
                        (255, 255, 255)
                    } else {
                        (0, 0, 0)
                    });
                sampled_color(foreground).map(|foreground| (foreground, background))
            }
            _ => None,
        };
        match colors {
            Some(((r, g, b), (br, bg, bb))) => {
                let blend = |a: u8, b: u8| {
                    (f64::from(b) + (f64::from(a) - f64::from(b)) * intensity).round() as u8
                };
                style.fg(Color::Rgb(blend(r, br), blend(g, bg), blend(b, bb)))
            }
            _ if intensity < 0.75 => style.add_modifier(Modifier::DIM),
            _ => style,
        }
    }

    /// Group header content rests on a faint fill of the group's accent over the Tab strip. A
    /// current, selected or pointer surface replaces only that fill.
    pub(crate) fn tab_group_header_style(
        &self,
        color: orchbee_workbench_protocol::TabGroupColor,
        surface: Option<Role>,
        content_role: Option<Role>,
        content: Style,
    ) -> Style {
        let style = self.content_on_surface(Role::SurfaceStrip, surface, content_role, content);
        match (surface, self.tab_groups.tint(color)) {
            (None, Some(tint)) => style.bg(tint),
            _ => style,
        }
    }

    /// The default theme for an unknown host appearance or color level.
    /// Text and surface roles sit on `Color::Reset`; accent, status, and frame
    /// roles use named ANSI-16 colors; `text-secondary` uses `DIM`, not a gray.
    #[must_use]
    pub fn terminal_native() -> Self {
        let reset = Style::new().fg(Color::Reset);
        let dim = Style::new().fg(Color::Reset).add_modifier(Modifier::DIM);
        Self {
            #[cfg(any(test, feature = "test-support"))]
            name: "terminal-native",
            polarity: None,
            palette: None,
            corners: BorderType::Plain,
            glyphs: GlyphSet::UTF8,
            color_level: ColorLevel::Truecolor,
            terminal_colors: None,
            terminal_rgb_colors: None,
            terminal_color_source: TerminalColorSource::Palette,
            terminal_defaults_known: true,
            destructive_rest: DestructiveRest::HostRed,
            no_color: false,
            styles: RoleStyles {
                text_primary: reset,
                text_secondary: dim,
                // DIM is the quietest host-relative text that stays readable on any host palette.
                text_muted: dim,
                text_disabled: dim,
                text_inverse: reset,
                accent_action: Style::new().fg(Color::Cyan),
                accent_destructive: Style::new().fg(Color::Red),
                accent_focus: Style::new().fg(Color::Cyan),
                accent_selection: Style::new().fg(Color::White).bg(Color::Blue),
                #[cfg(any(test, feature = "test-support"))]
                accent_link: Style::new()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
                status_success: Style::new().fg(Color::Green),
                status_warning: Style::new().fg(Color::Magenta),
                status_error: Style::new().fg(Color::Red),
                status_info: Style::new().fg(Color::Cyan),
                status_pending: Style::new().fg(Color::Cyan).add_modifier(Modifier::DIM),
                // Working follows the host foreground so it reads on either polarity.
                status_working: reset,
                status_idle: dim,
                status_unread: Style::new().fg(Color::LightBlue),
                agent_attention: Style::new().fg(AGENT_ATTENTION),
                agent_done: Style::new().fg(AGENT_DONE),
                agent_error: Style::new().fg(AGENT_ERROR),
                frame_focused: Style::new().fg(Color::Cyan),
                frame_unfocused: Style::new().fg(Color::DarkGray),
                frame_divider: Style::new().fg(Color::DarkGray),
                frame_guide: Style::new().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                surface_canvas: Style::new().bg(Color::Reset),
                surface_chrome: Style::new().bg(Color::Reset),
                surface_strip: Style::new().bg(Color::Reset),
                surface_overlay: Style::new().bg(Color::Reset),
                // No field fill of its own: form fields draw their bracketed boundary.
                surface_field: Style::new().bg(Color::Reset),
                surface_hover: interaction_surface_fallback(),
                surface_pressed: interaction_surface_fallback(),
                surface_selection: Style::new().bg(Color::Blue),
                // ANSI 7 can sit within a few steps of a light host's default background, so
                // current identity also keeps an underline across the row, as in Monochrome.
                surface_current: Style::new()
                    .fg(Color::Black)
                    .bg(Color::Gray)
                    .add_modifier(Modifier::UNDERLINED),
                // Unknown host colors cannot promise a softer fill; ancestry keeps its marker.
                surface_related: Style::new().bg(Color::Reset),
                surface_inactive: Style::new().bg(Color::Reset),
                hint_key: Style::new().fg(Color::Reset).add_modifier(Modifier::BOLD),
                hint_label: dim,
            },
            tab_groups: TabGroupPalette::TERMINAL,
        }
    }

    /// The RGB theme the render evidence requires alongside `terminal-native`.
    /// Declared dark polarity; a production host-facing RGB theme arrives with
    /// the environment seam — this constructor is test evidence only.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub(crate) fn rgb_test_dark() -> Self {
        Self {
            name: "rgb-test-dark",
            polarity: Some(Polarity::Dark),
            palette: None,
            corners: BorderType::Rounded,
            glyphs: GlyphSet::UTF8,
            color_level: ColorLevel::Truecolor,
            terminal_colors: None,
            terminal_rgb_colors: None,
            terminal_color_source: TerminalColorSource::Palette,
            terminal_defaults_known: true,
            destructive_rest: DestructiveRest::Paired,
            no_color: false,
            styles: RoleStyles {
                text_primary: Style::new().fg(Color::Rgb(211, 218, 232)),
                text_secondary: Style::new().fg(Color::Rgb(133, 146, 171)),
                text_muted: Style::new().fg(Color::Rgb(111, 123, 144)),
                text_disabled: Style::new().fg(Color::Rgb(91, 101, 119)),
                text_inverse: Style::new().fg(Color::Rgb(11, 14, 20)),
                accent_action: Style::new().fg(Color::Rgb(245, 245, 245)),
                accent_destructive: Style::new().fg(Color::Rgb(255, 116, 128)),
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
                status_working: Style::new().fg(Color::Rgb(229, 229, 229)),
                status_idle: Style::new().fg(Color::Rgb(88, 88, 88)),
                status_unread: Style::new().fg(Color::Rgb(96, 165, 250)),
                agent_attention: Style::new().fg(AGENT_ATTENTION),
                agent_done: Style::new().fg(AGENT_DONE),
                agent_error: Style::new().fg(AGENT_ERROR),
                status_pending: Style::new()
                    .fg(Color::Rgb(133, 146, 171))
                    .add_modifier(Modifier::DIM),
                frame_focused: Style::new().fg(Color::Rgb(122, 162, 247)),
                frame_unfocused: Style::new().fg(Color::Rgb(52, 61, 78)),
                frame_divider: Style::new().fg(Color::Rgb(52, 61, 78)),
                frame_guide: Style::new().fg(Color::Rgb(36, 43, 55)),
                surface_canvas: Style::new().bg(Color::Rgb(11, 14, 20)),
                surface_chrome: Style::new().bg(Color::Rgb(11, 14, 20)),
                surface_strip: Style::new().bg(Color::Rgb(24, 29, 38)),
                surface_overlay: Style::new().bg(Color::Rgb(18, 23, 32)),
                surface_field: Style::new().bg(Color::Rgb(32, 37, 46)),
                surface_hover: Style::new().bg(Color::Rgb(38, 38, 38)),
                surface_pressed: Style::new().bg(Color::Rgb(64, 64, 64)),
                surface_selection: Style::new().bg(Color::Rgb(45, 63, 99)),
                surface_current: Style::new().bg(Color::Rgb(33, 39, 49)),
                surface_related: Style::new().bg(Color::Rgb(22, 27, 35)),
                surface_inactive: Style::new().bg(Color::Rgb(11, 14, 20)),
                hint_key: Style::new()
                    .fg(Color::Rgb(211, 218, 232))
                    .add_modifier(Modifier::BOLD),
                hint_label: Style::new().fg(Color::Rgb(133, 146, 171)),
            },
            tab_groups: TabGroupPalette::TERMINAL,
        }
    }

    #[must_use]
    pub(crate) fn style(&self, role: Role) -> Style {
        self.styles.style(role)
    }

    /// Whether form fields paint their own `surface-field` band. Without one, as in
    /// `terminal-native` and Monochrome, a field draws its bracketed boundary instead.
    #[must_use]
    pub(crate) fn field_band(&self) -> bool {
        !matches!(self.styles.surface_field.bg, None | Some(Color::Reset))
            && self.styles.surface_field.bg != self.styles.surface_overlay.bg
    }

    /// The Shell supplies the observed reason; monochrome alone does not imply NO_COLOR.
    pub const fn with_no_color(mut self, no_color: bool) -> Self {
        self.no_color = no_color;
        self
    }

    pub(crate) const fn no_color(&self) -> bool {
        self.no_color
    }

    /// Mixed-role controls keep one displayed background despite different content foregrounds.
    /// A reverse-video fallback would turn every token foreground into its own background patch.
    /// Pair that fallback with primary text on chrome, reversed, so ordinary text forms one band:
    /// host-relative in terminal-native and monochrome, as on the Tab strip.
    pub(crate) fn paired_pointer_surfaces(mut self) -> Self {
        let band = Style::new()
            .fg(self.styles.text_primary.fg.unwrap_or(Color::Reset))
            .bg(self.styles.surface_chrome.bg.unwrap_or(Color::Reset))
            .add_modifier(Modifier::REVERSED);
        for surface in [
            &mut self.styles.surface_hover,
            &mut self.styles.surface_pressed,
        ] {
            if surface.add_modifier.contains(Modifier::REVERSED) {
                *surface = band;
            }
        }
        self
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
                | Role::SurfaceStrip
                | Role::SurfaceOverlay
                | Role::SurfaceField
                | Role::SurfaceInactive
        ));
        debug_assert!(interaction.is_none_or(|role| matches!(
            role,
            Role::SurfaceHover
                | Role::SurfacePressed
                | Role::SurfaceSelection
                | Role::SurfaceCurrent
                | Role::SurfaceRelated
        )));
        // Muted text keeps its contrast only on a resting surface; on any fill it reads as
        // secondary text.
        let content_role = if content_role == Some(Role::TextMuted)
            && interaction.is_some()
            && content.fg == self.styles.text_muted.fg
        {
            content = content
                .remove_modifier(self.styles.text_muted.add_modifier)
                .patch(self.styles.text_secondary);
            Some(Role::TextSecondary)
        } else {
            content_role
        };
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

        // Preserve a destructive label's semantic foreground in reverse-video
        // bands and on low-contrast fills; terminal-native red rests unpaired on
        // the host default background. The contrast pair belongs
        // to the theme, not a renderer or a status role; surrounding pointer and
        // focus tracks retain their independent state. Monochrome keeps the
        // ordinary control resolver and explicit localized action wording.
        if content_role == Some(Role::AccentDestructive)
            && self.color_level != ColorLevel::Monochrome
        {
            content = destructive_content_pair(content, self.destructive_rest);
        }
        // Reverse video would turn an Agent or status hue into a block of its own color behind a
        // glyph drawn in the band's text color. The hue keeps its foreground on the same pair.
        if content_role.is_some_and(Role::is_state_hue)
            && self.color_level != ColorLevel::Monochrome
            && content.add_modifier.contains(Modifier::REVERSED)
        {
            content = contrast_pair(content);
        }

        let paired = match interaction {
            Some(Role::SurfaceCurrent | Role::SurfaceHover | Role::SurfacePressed) => {
                interaction_style.and_then(|style| style.fg)
            }
            Some(Role::SurfaceSelection) => self.style(Role::AccentSelection).fg,
            _ => None,
        };
        // `paired_pointer_surfaces` made hover and press one reverse band of primary text on
        // chrome. Whatever still reverses into it takes the band's foreground so the row shows one
        // background: ordinary and secondary text, resting accents, explicit colors and blanks
        // alike. Hues and destructive labels left the band for their contrast pair above. A host
        // may apply DIM before reverse video swaps the pair, so the band drops DIM rather than
        // give a token a darker background of its own.
        if matches!(interaction, Some(Role::SurfaceHover | Role::SurfacePressed))
            && let Some(foreground) = paired
        {
            if content.add_modifier.contains(Modifier::REVERSED) {
                content = content.fg(foreground).remove_modifier(Modifier::DIM);
            }
            return content;
        }
        // Monochrome's provisional selection is a reverse band as well; the same host order would
        // give secondary text a darker background of its own, so the band drops DIM.
        if interaction == Some(Role::SurfaceSelection)
            && interaction_style
                .is_some_and(|style| style.add_modifier.contains(Modifier::REVERSED))
            && content.add_modifier.contains(Modifier::DIM)
        {
            content = content.remove_modifier(Modifier::DIM);
        }
        // Host-relative Working and Idle glyphs follow the host foreground like body text; other
        // status and Agent roles keep their own hue.
        let ordinary = matches!(
            content_role,
            Some(
                Role::TextPrimary
                    | Role::TextSecondary
                    | Role::TextMuted
                    | Role::TextDisabled
                    | Role::HintKey
                    | Role::HintLabel
                    | Role::StatusWorking
                    | Role::StatusIdle
            )
        );
        if self.color_level != ColorLevel::Monochrome
            && (self.polarity.is_none()
                || (interaction == Some(Role::SurfaceSelection)
                    && content_role != Some(Role::TextDisabled)))
            && ordinary
            && let Some(role) = content_role
            && content_foreground == self.style(role).fg
            && let Some(foreground) = paired
            && foreground != Color::Reset
        {
            content = content.fg(foreground);
            if matches!(
                role,
                Role::TextSecondary | Role::TextMuted | Role::HintLabel | Role::StatusIdle
            ) {
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

    /// The key hint inside an action. Terminal-native destructive wording is red across the
    /// whole button, so its hint keeps that hue; every other theme and Monochrome keep the
    /// hint as supporting text.
    pub(crate) fn action_hint_role(&self, action: Role) -> Role {
        if action == Role::AccentDestructive
            && self.destructive_rest == DestructiveRest::HostRed
            && !self.no_color
            && self.color_level != ColorLevel::Monochrome
        {
            Role::AccentDestructive
        } else {
            Role::TextSecondary
        }
    }

    /// A surface whose current row is marked by its fill alone underlines that row where
    /// quantization collapses the fill onto chrome, as Monochrome does.
    pub(crate) fn underline_collapsed_current(mut self) -> Self {
        if self.styles.surface_current.bg == self.styles.surface_chrome.bg {
            self.styles.surface_current = self
                .styles
                .surface_current
                .add_modifier(Modifier::UNDERLINED);
        }
        self
    }

    /// Explicit user token styles remain data owned by the theme boundary.
    pub(crate) fn sidebar_token_style(
        &self,
        role: Role,
        token: orchbee_local_runtime::SidebarTokenStyle,
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
                    | Role::TextMuted
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
    pub(crate) fn workspace_token_style(
        &self,
        role: Role,
        token: orchbee_local_runtime::SidebarTokenStyle,
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
            | Role::SurfaceStrip
            | Role::SurfaceOverlay
            | Role::SurfaceField
            | Role::SurfaceHover
            | Role::SurfacePressed
            | Role::SurfaceSelection
            | Role::SurfaceCurrent
            | Role::SurfaceRelated
            | Role::SurfaceInactive => style.bg,
            _ => style.fg,
        }
        .unwrap_or(Color::Reset)
    }

    /// Default-marked cells follow the theme, including host-relative Reset.
    /// Frame defaults describe the retained program, not the Client's host.
    /// Explicit program colors bypass this mapping.
    pub(crate) fn terminal_defaults(&self) -> (Color, Color) {
        if self.terminal_color_source == TerminalColorSource::OuterHost {
            return (Color::Reset, Color::Reset);
        }
        self.terminal_colors.unwrap_or((
            self.color(Role::TextPrimary),
            self.color(Role::SurfaceCanvas),
        ))
    }

    /// Palette-coded cells use this Client's ANSI colors; Terminal colors preserves the outer
    /// terminal's index. Font weight, faint and inverse remain independent cell attributes.
    pub(crate) fn terminal_cell_color(&self, value: u32, default: Color) -> Color {
        if let Some(index) = orchbee_terminal_protocol::terminal_palette_index(value) {
            let color = self.palette.map_or(Color::Indexed(index), |theme| {
                let rgb = theme.ansi()[usize::from(index)].value();
                Color::Rgb(rgb.red, rgb.green, rgb.blue)
            });
            return quantize_color(color, self.color_level);
        }
        if value <= 0xffffff {
            Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
        } else {
            default
        }
    }

    /// ANSI colors follow the same named palette as tagged cells, or the observed outer terminal.
    pub(crate) fn terminal_query_palette(
        &self,
        outer: crate::host_appearance::HostColors,
    ) -> (Option<u32>, [Option<u32>; 16]) {
        self.palette.map_or((outer.cursor, outer.palette), |theme| {
            (
                Some(u32::from(theme.canvas().cursor.value())),
                theme.ansi().map(|color| Some(u32::from(color.value()))),
            )
        })
    }

    pub(crate) fn terminal_rgb_defaults(&self) -> Option<(u32, u32)> {
        if !self.terminal_defaults_known {
            return None;
        }
        if let Some(colors) = self.terminal_rgb_colors {
            return Some(colors);
        }
        let rgb = |color| match color {
            Color::Rgb(red, green, blue) => {
                Some((u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue))
            }
            _ => None,
        };
        let (foreground, background) =
            if self.terminal_color_source == TerminalColorSource::OuterHost {
                self.terminal_colors?
            } else {
                self.terminal_defaults()
            };
        Some((rgb(foreground)?, rgb(background)?))
    }

    /// Theme takes the painted theme's canvas. Under Terminal colors, Theme and Host defer to the
    /// outer host's own defaults; with a named theme, Host does so only under System. Custom takes
    /// the custom pair. Unbound frame consumers can still resolve a deferred choice from their
    /// frame until the Client applies its captured outer-host colors.
    pub(crate) fn with_terminal_appearance(mut self, preferences: ColorPreferences<'_>) -> Self {
        use orchbee_workbench_protocol::{ApplicationAppearance, TerminalDisplayAppearance};
        let colors = match (preferences.terminal_display_theme, self.palette) {
            (TerminalDisplayAppearance::Theme, Some(theme)) => Some((
                u32::from(theme.canvas().foreground),
                u32::from(theme.canvas().background),
            )),
            (appearance, palette) => orchbee_local_runtime::UserTerminalColors::resolve(
                if palette.is_some() {
                    preferences.application_theme
                } else {
                    ApplicationAppearance::System
                },
                appearance,
                &orchbee_local_runtime::UserColorThemes::new(
                    preferences.light_theme.to_owned(),
                    preferences.dark_theme.to_owned(),
                ),
                preferences.custom_foreground,
                preferences.custom_background,
            )
            .expect("validated appearance preferences")
            .map(orchbee_local_runtime::UserTerminalColors::rgb),
        };
        self.terminal_color_source = if colors.is_none() {
            TerminalColorSource::RetainedFrame
        } else {
            TerminalColorSource::Palette
        };
        self.terminal_defaults_known = colors.is_some();
        self.terminal_colors = None;
        self.terminal_rgb_colors = None;
        if let Some((foreground, background)) = colors {
            self.set_terminal_colors(foreground, background);
        }
        self
    }

    pub(crate) fn with_host_terminal_colors(mut self, foreground: u32, background: u32) -> Self {
        if self.terminal_color_source == TerminalColorSource::RetainedFrame {
            self.set_terminal_colors(foreground, background);
            self.terminal_defaults_known = true;
        }
        self
    }

    /// A deferred terminal default takes the outer terminal's observed colors, never a retained
    /// child's launch colors. Named themes keep their painted surfaces; `terminal-native` already
    /// rests on the host defaults.
    pub(crate) fn with_outer_host_colors(
        mut self,
        colors: crate::host_appearance::HostColors,
    ) -> Self {
        if self.terminal_color_source == TerminalColorSource::RetainedFrame {
            self.terminal_color_source = TerminalColorSource::OuterHost;
            self.terminal_defaults_known =
                colors.foreground.is_some() && colors.background.is_some();
            self.terminal_rgb_colors = colors.foreground.zip(colors.background);
            self.terminal_colors =
                colors
                    .foreground
                    .zip(colors.background)
                    .map(|(foreground, background)| {
                        (
                            self.terminal_color(foreground),
                            self.terminal_color(background),
                        )
                    });
        }
        self
    }

    fn terminal_color(&self, value: u32) -> Color {
        quantize_color(
            Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8),
            self.color_level,
        )
    }

    fn set_terminal_colors(&mut self, foreground: u32, background: u32) {
        self.terminal_rgb_colors = Some((foreground, background));
        self.terminal_colors = Some((
            self.terminal_color(foreground),
            self.terminal_color(background),
        ));
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
                (&mut styles.text_muted, self.styles.text_muted),
                (&mut styles.text_disabled, self.styles.text_disabled),
                (&mut styles.text_inverse, self.styles.text_inverse),
                (&mut styles.accent_action, self.styles.accent_action),
                (
                    &mut styles.accent_destructive,
                    self.styles.accent_destructive,
                ),
                (&mut styles.accent_focus, self.styles.accent_focus),
                (&mut styles.accent_selection, self.styles.accent_selection),
                #[cfg(any(test, feature = "test-support"))]
                (&mut styles.accent_link, self.styles.accent_link),
                (&mut styles.status_success, self.styles.status_success),
                (&mut styles.status_warning, self.styles.status_warning),
                (&mut styles.status_error, self.styles.status_error),
                (&mut styles.status_info, self.styles.status_info),
                (&mut styles.status_pending, self.styles.status_pending),
                (&mut styles.status_working, self.styles.status_working),
                (&mut styles.status_idle, self.styles.status_idle),
                (&mut styles.status_unread, self.styles.status_unread),
                (&mut styles.agent_attention, self.styles.agent_attention),
                (&mut styles.agent_done, self.styles.agent_done),
                (&mut styles.agent_error, self.styles.agent_error),
                (&mut styles.surface_canvas, self.styles.surface_canvas),
                (&mut styles.surface_chrome, self.styles.surface_chrome),
                (&mut styles.surface_strip, self.styles.surface_strip),
                (&mut styles.surface_overlay, self.styles.surface_overlay),
                (&mut styles.surface_hover, self.styles.surface_hover),
                (&mut styles.surface_pressed, self.styles.surface_pressed),
                (&mut styles.surface_selection, self.styles.surface_selection),
                (&mut styles.surface_current, self.styles.surface_current),
                (&mut styles.surface_related, self.styles.surface_related),
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
            // Hierarchy guides live on chrome, which can quantize differently from the canvas.
            if styles.frame_guide.fg == styles.surface_chrome.bg
                && let Some(Color::Rgb(r, g, b)) = self.styles.frame_guide.fg
            {
                styles.frame_guide = styles.frame_guide.fg(nearest_visible_ansi16(
                    r,
                    g,
                    b,
                    styles.surface_chrome.bg,
                ));
            }
        }
        if level == ColorLevel::Monochrome
            || (matches!(level, ColorLevel::Ansi16 | ColorLevel::Ansi256)
                && interaction_surfaces_collapse(&styles))
        {
            styles.surface_hover = interaction_surface_fallback();
            styles.surface_pressed = interaction_surface_fallback();
        }
        // A field band that quantizes onto its dialog, or any Monochrome fill, falls back to brackets.
        if level == ColorLevel::Monochrome || styles.surface_field.bg == styles.surface_overlay.bg {
            styles.surface_field = Style::new().bg(Color::Reset);
        }
        if matches!(level, ColorLevel::Ansi16 | ColorLevel::Monochrome) {
            let primary = styles.text_primary.fg;
            for secondary in [
                &mut styles.text_secondary,
                &mut styles.text_muted,
                &mut styles.text_disabled,
                &mut styles.status_idle,
                &mut styles.hint_label,
            ] {
                if secondary.fg == primary {
                    *secondary = secondary.add_modifier(Modifier::DIM);
                }
            }
        }
        if level == ColorLevel::Monochrome {
            styles.frame_focused = styles
                .frame_focused
                .add_modifier(Modifier::BOLD | Modifier::REVERSED);
            styles.accent_focus = styles
                .accent_focus
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            styles.surface_selection = styles
                .surface_selection
                .fg(Color::Reset)
                .bg(Color::Reset)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED);
            // Current identity owners that document bold add it themselves.
            styles.surface_current = styles.surface_current.add_modifier(Modifier::UNDERLINED);
            styles.accent_selection = styles
                .accent_selection
                .add_modifier(Modifier::BOLD | Modifier::REVERSED);
        }
        Self {
            styles,
            tab_groups: self.tab_groups.quantize(level),
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

/// A semantic foreground cannot be a readable foreground after reverse video
/// swaps its channels. Use one resolver-owned contrast pair on that content
/// band when a known fill cannot support the declared hue, or when the ambient
/// background is unknown. Terminal-native red is the host palette's own hue,
/// whose reference contrast on the host default admits it there unpaired.
fn destructive_content_pair(content: Style, rest: DestructiveRest) -> Style {
    let Some(foreground) = content.fg.and_then(sampled_color) else {
        return content;
    };
    if content.add_modifier.contains(Modifier::REVERSED) {
        return contrast_pair(content);
    }
    match content.bg.and_then(sampled_color) {
        Some(background) if color_contrast(foreground, background) >= 3.0 => content,
        None if rest == DestructiveRest::HostRed => content,
        _ => contrast_pair(content),
    }
}

/// Backs a known foreground with whichever of ANSI black or white contrasts more with it, in place
/// of reverse video. A host-relative foreground has no known hue and keeps its style.
fn contrast_pair(content: Style) -> Style {
    let Some(foreground) = content.fg.and_then(sampled_color) else {
        return content;
    };
    let background =
        if color_contrast(foreground, (0, 0, 0)) >= color_contrast(foreground, (255, 255, 255)) {
            Color::Black
        } else {
            Color::White
        };
    content.bg(background).remove_modifier(Modifier::REVERSED)
}

fn sampled_color(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Reset => None,
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Indexed(index) => Some(INDEXED_SRGB[usize::from(index)]),
        color => ANSI16_SRGB
            .iter()
            .find(|(name, _)| *name == color)
            .map(|(_, rgb)| *rgb),
    }
}

/// Resolve a software block cursor against the cell's displayed background, independently of
/// the host's cursor palette. Unknown default colors invert the displayed host-default pair.
pub fn cursor_style(foreground: Color, background: Color, modifiers: Modifier) -> Style {
    let reversed = modifiers.contains(Modifier::REVERSED);
    let background = if reversed { foreground } else { background };
    let Some(rgb) = sampled_color(background) else {
        let defaults = Style::new().fg(Color::Reset).bg(Color::Reset);
        // A reversed default foreground is already the displayed background. In that case
        // the normal host background, rather than another reversal, supplies the cursor fill.
        return if reversed {
            defaults
        } else {
            defaults.add_modifier(Modifier::REVERSED)
        };
    };
    let dark = color_contrast(rgb, (0, 0, 0)) >= color_contrast(rgb, (255, 255, 255));
    let fill = match background {
        Color::Rgb(..) => {
            let channel = if dark { 0 } else { 255 };
            Color::Rgb(channel, channel, channel)
        }
        Color::Indexed(_) => Color::Indexed(if dark { 16 } else { 231 }),
        _ => {
            if dark {
                Color::Black
            } else {
                Color::White
            }
        }
    };
    Style::new().fg(background).bg(fill)
}

fn color_contrast(left: (u8, u8, u8), right: (u8, u8, u8)) -> f64 {
    let luminance = |(r, g, b): (u8, u8, u8)| {
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    };
    let left = luminance(left);
    let right = luminance(right);
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
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

/// Crate-internal `Theme` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait ThemeTestAccess {
    fn glyphs(&self) -> &GlyphSet;
    fn color(&self, role: Role) -> Color;
    fn with_terminal_preferences(
        self,
        values: &orchbee_workbench_protocol::UserPreferencesValues,
    ) -> Self;
    /// The resolution of confirmed preferences on a host whose settled appearance is this one,
    /// before the Client applies its captured outer-host colors.
    fn for_preferences(
        values: &orchbee_workbench_protocol::UserPreferencesValues,
        host: crate::host_appearance::HostAppearance,
        level: ColorLevel,
    ) -> Self;
    fn orchbee_light() -> Self;
    fn orchbee_dark() -> Self;
}

#[cfg(any(test, feature = "test-support"))]
impl ThemeTestAccess for Theme {
    fn glyphs(&self) -> &GlyphSet {
        &self.glyphs
    }
    fn color(&self, role: Role) -> Color {
        Theme::color(self, role)
    }
    fn with_terminal_preferences(
        self,
        values: &orchbee_workbench_protocol::UserPreferencesValues,
    ) -> Self {
        self.with_terminal_appearance(ColorPreferences::from(values))
    }
    fn for_preferences(
        values: &orchbee_workbench_protocol::UserPreferencesValues,
        host: crate::host_appearance::HostAppearance,
        level: ColorLevel,
    ) -> Self {
        Theme::resolve(
            ColorPreferences::from(values),
            Polarity::of_host(host),
            level,
        )
    }
    fn orchbee_light() -> Self {
        Theme::orchbee_light()
    }
    fn orchbee_dark() -> Self {
        Theme::orchbee_dark()
    }
}

/// Crate-internal `GlyphSet` glyphs the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait GlyphSetTestAccess {
    fn ellipsis(&self) -> &'static str;
    fn middle_dot(&self) -> &'static str;
    fn status_bullet(&self) -> &'static str;
    fn active_marker(&self) -> &'static str;
    fn navigation_marker(&self) -> &'static str;
}

#[cfg(any(test, feature = "test-support"))]
impl GlyphSetTestAccess for GlyphSet {
    fn ellipsis(&self) -> &'static str {
        self.ellipsis
    }
    fn middle_dot(&self) -> &'static str {
        self.middle_dot
    }
    fn status_bullet(&self) -> &'static str {
        self.status_bullet
    }
    fn active_marker(&self) -> &'static str {
        self.active_marker
    }
    fn navigation_marker(&self) -> &'static str {
        self.navigation_marker
    }
}

#[cfg(test)]
mod tests;
