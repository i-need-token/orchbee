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

/// One semantic color role. Rendering code references roles only;
/// `Color::Rgb`, `Color::Indexed`, and named ANSI variants appear only here
/// and in shipped theme values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    TextPrimary,
    TextSecondary,
    #[cfg(any(windows, test))]
    TextDisabled,
    TextInverse,
    AccentFocus,
    #[cfg(test)]
    AccentSelection,
    #[cfg(test)]
    AccentLink,
    StatusSuccess,
    StatusWarning,
    StatusError,
    #[cfg(any(windows, test))]
    StatusInfo,
    #[cfg(any(windows, test))]
    StatusPending,
    FrameFocused,
    #[cfg(test)]
    FrameUnfocused,
    FrameDivider,
    SurfaceChrome,
    SurfaceOverlay,
    #[cfg(any(windows, test))]
    SurfaceSelection,
    SurfaceInactive,
    HintKey,
    HintLabel,
}

impl Role {
    /// Test/validator helper for totality checks; production consumes roles
    /// individually.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 21] = [
        Self::TextPrimary,
        Self::TextSecondary,
        Self::TextDisabled,
        Self::TextInverse,
        Self::AccentFocus,
        Self::AccentSelection,
        Self::AccentLink,
        Self::StatusSuccess,
        Self::StatusWarning,
        Self::StatusError,
        Self::StatusInfo,
        Self::StatusPending,
        Self::FrameFocused,
        Self::FrameUnfocused,
        Self::FrameDivider,
        Self::SurfaceChrome,
        Self::SurfaceOverlay,
        Self::SurfaceSelection,
        Self::SurfaceInactive,
        Self::HintKey,
        Self::HintLabel,
    ];
}

/// The detected host color level, supplied by the environment owner.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColorLevel {
    Truecolor,
    Ansi256,
    Ansi16,
}

/// Background polarity an RGB theme is designed for. The terminal-native theme
/// carries none: it must work on an unknown host appearance.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Polarity {
    Dark,
    Light,
}

/// Non-ASCII chrome glyphs with a declared ASCII fallback, resolved once for a
/// UTF-8 or simplified-glyph host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GlyphSet {
    /// Truncation marker. `…` is East Asian ambiguous width; `..` is the
    /// ASCII form.
    pub ellipsis: &'static str,
    /// Separator inside frame titles and list rows.
    pub middle_dot: &'static str,
    /// Status glyph used in the sidebar site section.
    pub status_bullet: &'static str,
    pub git_ahead: &'static str,
    pub git_behind: &'static str,
    /// Active marker in compact tab and context lists.
    pub active_marker: &'static str,
}

impl GlyphSet {
    pub(crate) const UTF8: Self = Self {
        ellipsis: "…",
        middle_dot: "·",
        status_bullet: "●",
        git_ahead: "↑",
        git_behind: "↓",
        active_marker: "*",
    };

    #[cfg(test)]
    pub(crate) const ASCII: Self = Self {
        ellipsis: "..",
        middle_dot: "-",
        status_bullet: "*",
        git_ahead: "+",
        git_behind: "-",
        active_marker: "*",
    };

    #[cfg(test)]
    #[must_use]
    pub(crate) fn resolve(utf8: bool) -> Self {
        if utf8 { Self::UTF8 } else { Self::ASCII }
    }
}

/// One concrete value per semantic role. A field per role makes a theme with a
/// missing role invalid at compile time.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RoleStyles {
    pub text_primary: Style,
    pub text_secondary: Style,
    #[cfg(any(windows, test))]
    pub text_disabled: Style,
    pub text_inverse: Style,
    pub accent_focus: Style,
    #[cfg(test)]
    pub accent_selection: Style,
    #[cfg(test)]
    pub accent_link: Style,
    pub status_success: Style,
    pub status_warning: Style,
    pub status_error: Style,
    #[cfg(any(windows, test))]
    pub status_info: Style,
    #[cfg(any(windows, test))]
    pub status_pending: Style,
    pub frame_focused: Style,
    #[cfg(test)]
    pub frame_unfocused: Style,
    pub frame_divider: Style,
    pub surface_chrome: Style,
    pub surface_overlay: Style,
    #[cfg(any(windows, test))]
    pub surface_selection: Style,
    pub surface_inactive: Style,
    pub hint_key: Style,
    pub hint_label: Style,
}

impl RoleStyles {
    fn style(&self, role: Role) -> Style {
        match role {
            Role::TextPrimary => self.text_primary,
            Role::TextSecondary => self.text_secondary,
            #[cfg(any(windows, test))]
            Role::TextDisabled => self.text_disabled,
            Role::TextInverse => self.text_inverse,
            Role::AccentFocus => self.accent_focus,
            #[cfg(test)]
            Role::AccentSelection => self.accent_selection,
            #[cfg(test)]
            Role::AccentLink => self.accent_link,
            Role::StatusSuccess => self.status_success,
            Role::StatusWarning => self.status_warning,
            Role::StatusError => self.status_error,
            #[cfg(any(windows, test))]
            Role::StatusInfo => self.status_info,
            #[cfg(any(windows, test))]
            Role::StatusPending => self.status_pending,
            Role::FrameFocused => self.frame_focused,
            #[cfg(test)]
            Role::FrameUnfocused => self.frame_unfocused,
            Role::FrameDivider => self.frame_divider,
            Role::SurfaceChrome => self.surface_chrome,
            Role::SurfaceOverlay => self.surface_overlay,
            #[cfg(any(windows, test))]
            Role::SurfaceSelection => self.surface_selection,
            Role::SurfaceInactive => self.surface_inactive,
            Role::HintKey => self.hint_key,
            Role::HintLabel => self.hint_label,
        }
    }

    #[cfg(test)]
    fn map(&self, apply: impl Fn(Style) -> Style) -> Self {
        Self {
            text_primary: apply(self.text_primary),
            text_secondary: apply(self.text_secondary),
            text_disabled: apply(self.text_disabled),
            text_inverse: apply(self.text_inverse),
            accent_focus: apply(self.accent_focus),
            accent_selection: apply(self.accent_selection),
            accent_link: apply(self.accent_link),
            status_success: apply(self.status_success),
            status_warning: apply(self.status_warning),
            status_error: apply(self.status_error),
            status_info: apply(self.status_info),
            status_pending: apply(self.status_pending),
            frame_focused: apply(self.frame_focused),
            frame_unfocused: apply(self.frame_unfocused),
            frame_divider: apply(self.frame_divider),
            surface_chrome: apply(self.surface_chrome),
            surface_overlay: apply(self.surface_overlay),
            surface_selection: apply(self.surface_selection),
            surface_inactive: apply(self.surface_inactive),
            hint_key: apply(self.hint_key),
            hint_label: apply(self.hint_label),
        }
    }
}

/// A complete mapping of roles to concrete styles plus the uniform frame
/// corner option and the resolved glyph set.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Theme {
    #[cfg(test)]
    pub name: &'static str,
    #[cfg(test)]
    pub polarity: Option<Polarity>,
    pub corners: BorderType,
    pub glyphs: GlyphSet,
    styles: RoleStyles,
}

impl Theme {
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
            #[cfg(test)]
            polarity: None,
            corners: BorderType::Plain,
            glyphs: GlyphSet::UTF8,
            styles: RoleStyles {
                text_primary: reset,
                text_secondary: dim,
                #[cfg(any(windows, test))]
                text_disabled: dim,
                text_inverse: reset,
                accent_focus: Style::new().fg(Color::Cyan),
                #[cfg(test)]
                accent_selection: Style::new().fg(Color::White).bg(Color::Blue),
                #[cfg(test)]
                accent_link: Style::new()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
                status_success: Style::new().fg(Color::Green),
                status_warning: Style::new().fg(Color::Magenta),
                status_error: Style::new().fg(Color::Red),
                #[cfg(any(windows, test))]
                status_info: Style::new().fg(Color::Cyan),
                #[cfg(any(windows, test))]
                status_pending: Style::new().fg(Color::Cyan).add_modifier(Modifier::DIM),
                frame_focused: Style::new().fg(Color::Cyan),
                #[cfg(test)]
                frame_unfocused: Style::new().fg(Color::DarkGray),
                frame_divider: Style::new().fg(Color::DarkGray),
                surface_chrome: Style::new().bg(Color::Reset),
                surface_overlay: Style::new().bg(Color::Reset),
                #[cfg(any(windows, test))]
                surface_selection: Style::new().bg(Color::Blue),
                surface_inactive: Style::new().bg(Color::Reset).add_modifier(Modifier::DIM),
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
            styles: RoleStyles {
                text_primary: Style::new().fg(Color::Rgb(211, 218, 232)),
                text_secondary: Style::new().fg(Color::Rgb(133, 146, 171)),
                text_disabled: Style::new().fg(Color::Rgb(91, 101, 119)),
                text_inverse: Style::new().fg(Color::Rgb(11, 14, 20)),
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
                status_pending: Style::new()
                    .fg(Color::Rgb(133, 146, 171))
                    .add_modifier(Modifier::DIM),
                frame_focused: Style::new().fg(Color::Rgb(122, 162, 247)),
                frame_unfocused: Style::new().fg(Color::Rgb(52, 61, 78)),
                frame_divider: Style::new().fg(Color::Rgb(52, 61, 78)),
                surface_chrome: Style::new().bg(Color::Rgb(11, 14, 20)),
                surface_overlay: Style::new().bg(Color::Rgb(18, 23, 32)),
                surface_selection: Style::new().bg(Color::Rgb(45, 63, 99)),
                surface_inactive: Style::new()
                    .bg(Color::Rgb(11, 14, 20))
                    .add_modifier(Modifier::DIM),
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

    /// Explicit user token styles remain data owned by the theme boundary.
    #[cfg(windows)]
    pub(crate) fn sidebar_token_style(
        &self,
        role: Role,
        token: orchbee_local_site_runtime::WindowsSidebarTokenStyle,
    ) -> Style {
        let mut style = self.style(role);
        if let Some([r, g, b]) = token.fg {
            style = style.fg(Color::Rgb(r, g, b));
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

    /// The role's color: the background for `surface-*` roles, the foreground
    /// for every other family.
    #[must_use]
    pub(crate) fn color(&self, role: Role) -> Color {
        let style = self.style(role);
        match role {
            Role::SurfaceChrome | Role::SurfaceOverlay | Role::SurfaceInactive => style.bg,
            #[cfg(any(windows, test))]
            Role::SurfaceSelection => style.bg,
            _ => style.fg,
        }
        .unwrap_or(Color::Reset)
    }

    /// Swap the resolved glyph set (UTF-8 host or declared ASCII fallbacks).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_glyphs(mut self, glyphs: GlyphSet) -> Self {
        self.glyphs = glyphs;
        self
    }

    /// Quantize every role value once at startup to the detected color level.
    /// `Reset` and named colors pass through at every level; at `Ansi16` no
    /// `Rgb` or `Indexed` value remains.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn quantize(self, level: ColorLevel) -> Self {
        Self {
            styles: self.styles.map(|style| quantize_style(style, level)),
            ..self
        }
    }
}

#[cfg(test)]
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

#[cfg(test)]
fn quantize_color(color: Color, level: ColorLevel) -> Color {
    match level {
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
#[cfg(test)]
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

#[cfg(test)]
const INDEXED_SRGB: [(u8, u8, u8); 256] = build_indexed_srgb();

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn squared_distance(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)
}

/// sRGB → CIELAB (D65) for nearest-perceptual-match quantization.
#[cfg(test)]
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
