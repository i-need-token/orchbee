//! Shipped RGB palettes project every existing semantic role; no renderer owns colors.
use super::{Color, ColorLevel, GlyphSet, Modifier, Polarity, RoleStyles, Style, Theme};
use crate::terminal_host_appearance::HostAppearance;
use ratatui::widgets::BorderType;

struct Palette {
    #[cfg(test)]
    name: &'static str,
    polarity: Polarity,
    primary: [u8; 3],
    secondary: [u8; 3],
    inverse: [u8; 3],
    action_accent: [u8; 3],
    destructive_accent: [u8; 3],
    focus: [u8; 3],
    success: [u8; 3],
    warning: [u8; 3],
    error: [u8; 3],
    info: [u8; 3],
    working: [u8; 3],
    unread: [u8; 3],
    frame: [u8; 3],
    divider: [u8; 3],
    background: [u8; 3],
    chrome: [u8; 3],
    overlay: [u8; 3],
    hover_surface: [u8; 3],
    pressed_surface: [u8; 3],
    selection: [u8; 3],
    current: [u8; 3],
}
fn rgb([r, g, b]: [u8; 3]) -> Color {
    Color::Rgb(r, g, b)
}
fn named(palette: Palette) -> Theme {
    let primary = Style::new().fg(rgb(palette.primary));
    let secondary = Style::new().fg(rgb(palette.secondary));
    let focus = Style::new().fg(rgb(palette.focus));
    Theme {
        #[cfg(test)]
        name: palette.name,
        polarity: Some(palette.polarity),
        corners: BorderType::Plain,
        glyphs: GlyphSet::UTF8,
        color_level: ColorLevel::Truecolor,
        terminal_colors: None,
        terminal_uses_host_colors: false,
        styles: RoleStyles {
            text_primary: primary,
            text_secondary: secondary,
            text_disabled: secondary,
            text_inverse: Style::new().fg(rgb(palette.inverse)),
            accent_action: Style::new().fg(rgb(palette.action_accent)),
            accent_destructive: Style::new().fg(rgb(palette.destructive_accent)),
            accent_focus: focus,
            accent_selection: primary.bg(rgb(palette.selection)),
            #[cfg(test)]
            accent_link: focus.add_modifier(Modifier::UNDERLINED),
            status_success: Style::new().fg(rgb(palette.success)),
            status_warning: Style::new().fg(rgb(palette.warning)),
            status_error: Style::new().fg(rgb(palette.error)),
            status_info: Style::new().fg(rgb(palette.info)),
            status_working: Style::new().fg(rgb(palette.working)),
            status_unread: Style::new().fg(rgb(palette.unread)),
            status_pending: secondary.add_modifier(Modifier::DIM),
            frame_focused: focus,
            frame_unfocused: Style::new().fg(rgb(palette.frame)),
            frame_divider: Style::new().fg(rgb(palette.divider)),
            surface_canvas: primary.bg(rgb(palette.background)),
            surface_chrome: primary.bg(rgb(palette.chrome)),
            surface_overlay: primary.bg(rgb(palette.overlay)),
            surface_hover: Style::new().bg(rgb(palette.hover_surface)),
            surface_pressed: Style::new().bg(rgb(palette.pressed_surface)),
            surface_selection: primary.bg(rgb(palette.selection)),
            surface_current: Style::new().bg(rgb(palette.current)),
            surface_inactive: primary.bg(rgb(palette.background)),
            hint_key: primary.add_modifier(Modifier::BOLD),
            hint_label: secondary,
        },
    }
}

impl Theme {
    /// Resolve System appearance from outer-host polarity; Unknown falls back to One Dark.
    pub(crate) fn application_with_host(
        appearance: orchbee_workbench_protocol::ApplicationAppearance,
        host: HostAppearance,
        level: ColorLevel,
    ) -> Self {
        use orchbee_workbench_protocol::ApplicationAppearance;
        match appearance {
            ApplicationAppearance::System => match host {
                HostAppearance::Dark => Self::one_dark(),
                HostAppearance::Light => Self::one_light(),
                HostAppearance::Unknown => Self::one_dark(),
            },
            ApplicationAppearance::Light => Self::one_light(),
            ApplicationAppearance::Dark => Self::one_dark(),
        }
        .quantize(level)
    }
    pub(crate) fn one_light() -> Self {
        named(Palette {
            #[cfg(test)]
            name: "one-light",
            polarity: Polarity::Light,
            primary: [56, 58, 66],
            secondary: [90, 93, 105],
            inverse: [250, 250, 250],
            action_accent: [39, 120, 193],
            destructive_accent: [178, 52, 47],
            focus: [48, 100, 220],
            success: [47, 125, 65],
            warning: [152, 104, 1],
            error: [178, 52, 47],
            info: [1, 112, 159],
            working: [128, 55, 170],
            unread: [35, 90, 190],
            frame: [160, 161, 167],
            divider: [160, 161, 167],
            background: [250, 250, 250],
            chrome: [240, 242, 245],
            overlay: [255, 255, 255],
            hover_surface: [222, 224, 227],
            pressed_surface: [214, 215, 218],
            selection: [205, 219, 248],
            current: [224, 228, 235],
        })
    }
    pub(crate) fn one_dark() -> Self {
        named(Palette {
            #[cfg(test)]
            name: "one-dark",
            polarity: Polarity::Dark,
            primary: [204, 211, 224],
            secondary: [170, 177, 189],
            inverse: [40, 44, 52],
            action_accent: [245, 245, 245],
            destructive_accent: [224, 108, 117],
            focus: [97, 175, 239],
            success: [152, 195, 121],
            warning: [229, 192, 123],
            error: [224, 108, 117],
            info: [86, 182, 194],
            working: [198, 120, 221],
            unread: [97, 175, 239],
            frame: [76, 84, 98],
            divider: [62, 68, 81],
            background: [40, 44, 52],
            chrome: [31, 35, 42],
            overlay: [50, 56, 66],
            hover_surface: [38, 38, 38],
            pressed_surface: [64, 64, 64],
            selection: [51, 70, 89],
            current: [42, 47, 56],
        })
    }
    #[cfg(windows)]
    pub(crate) fn selected(
        name: orchbee_local_site_runtime::WindowsThemeName,
        level: ColorLevel,
    ) -> Self {
        use orchbee_local_site_runtime::WindowsThemeName;
        match name {
            WindowsThemeName::TerminalNative => Self::terminal_native(),
            WindowsThemeName::OneLight => Self::one_light(),
            WindowsThemeName::OneDark => Self::one_dark(),
        }
        .quantize(level)
    }
}
