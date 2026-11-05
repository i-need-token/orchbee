//! Named themes take every role from the shipped theme crate's resolver; no renderer owns colors.
use super::{
    Color, ColorLevel, DestructiveRest, GlyphSet, Modifier, Polarity, RoleStyles, Style,
    TabGroupPalette, TerminalColorSource, Theme,
};
use orchbee_color_themes::{Rgb, Roles};
use ratatui::widgets::BorderType;

const fn rgb(color: Rgb) -> Color {
    Color::Rgb(color.red, color.green, color.blue)
}

const fn channels(color: Rgb) -> [u8; 3] {
    [color.red, color.green, color.blue]
}

/// A translucent shared color over an opaque surface, rounded once per channel.
fn composite(color: Rgb, surface: Rgb, opacity: u16) -> Rgb {
    let channel = |color: u8, surface: u8| {
        let mixed = (u16::from(color) * opacity + u16::from(surface) * (100 - opacity) + 50) / 100;
        u8::try_from(mixed).expect("a weighted mean of two channels stays a channel")
    };
    Rgb::new(
        channel(color.red, surface.red),
        channel(color.green, surface.green),
        channel(color.blue, surface.blue),
    )
}

/// The GUI keeps a reviewed hand-authored mapping for the Orchbee pair, and the
/// [shared theme colors](../../../../../../schemas/workbench/v1/shared-theme-colors-v1.json) are
/// the colors both Clients must agree on there: the primary action, hover and press, and the dark
/// Agent idle and working glyphs. Every other Orchbee role, and every role of the other themes,
/// is the crate resolver's.
fn shared_orchbee_colors(polarity: orchbee_color_themes::Polarity, roles: &mut Roles) {
    match polarity {
        orchbee_color_themes::Polarity::Light => {
            let ink = Rgb::new(0x14, 0x14, 0x14);
            roles.accent_action = Rgb::new(0x27, 0x78, 0xc1);
            roles.surface_hover = composite(ink, roles.surface_chrome, 8);
            roles.surface_pressed = composite(ink, roles.surface_chrome, 12);
        }
        orchbee_color_themes::Polarity::Dark => {
            roles.accent_action = Rgb::new(0xf5, 0xf5, 0xf5);
            roles.surface_hover = Rgb::new(0x26, 0x26, 0x26);
            roles.surface_pressed = Rgb::new(0x40, 0x40, 0x40);
            roles.status_idle = Rgb::new(0x58, 0x58, 0x58);
            roles.status_working = Rgb::new(0xe5, 0xe5, 0xe5);
        }
    }
}

impl Theme {
    /// One shipped theme at truecolor; callers quantize it once to the host's color level.
    pub(crate) fn named(theme: &'static orchbee_color_themes::Theme) -> Self {
        let mut roles = Roles::resolve(theme);
        if theme.family() == "orchbee" {
            shared_orchbee_colors(theme.polarity(), &mut roles);
        }
        let fg = |color: Rgb| Style::new().fg(rgb(color));
        let bg = |color: Rgb| Style::new().bg(rgb(color));
        let primary = fg(roles.text_primary);
        let secondary = fg(roles.text_secondary);
        let groups = roles.tab_groups;
        Self {
            #[cfg(any(test, feature = "test-support"))]
            name: theme.id(),
            polarity: Some(match theme.polarity() {
                orchbee_color_themes::Polarity::Light => Polarity::Light,
                orchbee_color_themes::Polarity::Dark => Polarity::Dark,
            }),
            palette: Some(theme),
            corners: BorderType::Plain,
            glyphs: GlyphSet::UTF8,
            color_level: ColorLevel::Truecolor,
            terminal_colors: None,
            terminal_color_source: TerminalColorSource::Palette,
            terminal_defaults_known: true,
            destructive_rest: DestructiveRest::Paired,
            no_color: false,
            styles: RoleStyles {
                text_primary: primary,
                text_secondary: secondary,
                text_muted: fg(roles.text_muted),
                text_disabled: fg(roles.text_disabled),
                text_inverse: fg(roles.text_inverse),
                accent_action: fg(roles.accent_action),
                accent_destructive: fg(roles.accent_destructive),
                accent_focus: fg(roles.accent_focus),
                accent_selection: fg(roles.text_selection).bg(rgb(roles.surface_selection)),
                #[cfg(any(test, feature = "test-support"))]
                accent_link: fg(roles.accent_link).add_modifier(Modifier::UNDERLINED),
                status_success: fg(roles.status_success),
                status_warning: fg(roles.status_warning),
                status_error: fg(roles.status_error),
                status_info: fg(roles.status_info),
                status_pending: fg(roles.status_pending).add_modifier(Modifier::DIM),
                status_working: fg(roles.status_working),
                status_idle: fg(roles.status_idle),
                status_unread: fg(roles.status_unread),
                agent_attention: fg(roles.agent_attention),
                agent_done: fg(roles.agent_done),
                agent_error: fg(roles.agent_error),
                frame_focused: fg(roles.frame_focused),
                frame_unfocused: fg(roles.frame_unfocused),
                frame_divider: fg(roles.frame_divider),
                frame_guide: fg(roles.frame_guide),
                surface_canvas: primary.bg(rgb(roles.surface_canvas)),
                surface_chrome: primary.bg(rgb(roles.surface_chrome)),
                surface_strip: primary.bg(rgb(roles.surface_strip)),
                surface_overlay: primary.bg(rgb(roles.surface_overlay)),
                surface_field: bg(roles.surface_field),
                surface_hover: bg(roles.surface_hover),
                surface_pressed: bg(roles.surface_pressed),
                surface_selection: fg(roles.text_selection).bg(rgb(roles.surface_selection)),
                surface_current: bg(roles.surface_current),
                surface_related: bg(roles.surface_related),
                surface_inactive: primary.bg(rgb(roles.surface_inactive)),
                hint_key: primary.add_modifier(Modifier::BOLD),
                hint_label: secondary,
            },
            tab_groups: TabGroupPalette::named(
                [
                    groups.blue,
                    groups.cyan,
                    groups.green,
                    groups.grey,
                    groups.orange,
                    groups.pink,
                    groups.purple,
                    groups.red,
                    groups.yellow,
                ]
                .map(channels),
                channels(roles.surface_strip),
            ),
        }
    }

    /// The Orchbee Light fixture that tests render beside `terminal-native`.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn orchbee_light() -> Self {
        Self::named(orchbee_color_themes::orchbee(
            orchbee_color_themes::Polarity::Light,
        ))
    }

    /// The Orchbee Dark fixture that tests render beside `terminal-native`.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn orchbee_dark() -> Self {
        Self::named(orchbee_color_themes::orchbee(
            orchbee_color_themes::Polarity::Dark,
        ))
    }

    pub(crate) fn selected(name: super::ThemeName, level: ColorLevel) -> Self {
        use super::ThemeName;
        match name {
            ThemeName::TerminalNative => Self::terminal_native(),
            #[cfg(any(test, feature = "test-support"))]
            ThemeName::OrchbeeLight => Self::orchbee_light(),
            #[cfg(any(test, feature = "test-support"))]
            ThemeName::OrchbeeDark => Self::orchbee_dark(),
        }
        .quantize(level)
    }

    /// The picker swatch of one shipped theme: its canvas behind a foreground sample, then ANSI
    /// 1-5. Only a truecolor theme shows it; lower levels cannot hold the canvas tint.
    pub(crate) fn swatch(&self, theme: &orchbee_color_themes::Theme) -> Option<[Style; 6]> {
        if self.color_level != ColorLevel::Truecolor || self.no_color {
            return None;
        }
        let canvas = theme.canvas();
        let ansi = theme.ansi();
        let fill = |index: usize| Style::new().bg(rgb(ansi[index].value()));
        Some([
            Style::new()
                .fg(rgb(canvas.foreground))
                .bg(rgb(canvas.background)),
            fill(1),
            fill(2),
            fill(3),
            fill(4),
            fill(5),
        ])
    }
}
