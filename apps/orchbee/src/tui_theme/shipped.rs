//! Shipped RGB palettes project every existing semantic role; no renderer owns colors.
use super::{Color, ColorLevel, GlyphSet, Modifier, Polarity, RoleStyles, Style, Theme};
use ratatui::widgets::BorderType;

struct Palette {
    #[cfg(test)]
    name: &'static str,
    polarity: Polarity,
    primary: [u8; 3],
    secondary: [u8; 3],
    inverse: [u8; 3],
    focus: [u8; 3],
    success: [u8; 3],
    warning: [u8; 3],
    error: [u8; 3],
    info: [u8; 3],
    divider: [u8; 3],
    background: [u8; 3],
    overlay: [u8; 3],
    selection: [u8; 3],
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
        styles: RoleStyles {
            text_primary: primary,
            text_secondary: secondary,
            text_disabled: secondary.add_modifier(Modifier::DIM),
            text_inverse: Style::new().fg(rgb(palette.inverse)),
            accent_focus: focus,
            accent_selection: primary.bg(rgb(palette.selection)),
            #[cfg(test)]
            accent_link: focus.add_modifier(Modifier::UNDERLINED),
            status_success: Style::new().fg(rgb(palette.success)),
            status_warning: Style::new().fg(rgb(palette.warning)),
            status_error: Style::new().fg(rgb(palette.error)),
            status_info: Style::new().fg(rgb(palette.info)),
            status_pending: secondary.add_modifier(Modifier::DIM),
            frame_focused: focus,
            #[cfg(test)]
            frame_unfocused: Style::new().fg(rgb(palette.divider)),
            frame_divider: Style::new().fg(rgb(palette.divider)),
            surface_chrome: primary.bg(rgb(palette.background)),
            surface_overlay: primary.bg(rgb(palette.overlay)),
            surface_selection: primary.bg(rgb(palette.selection)),
            surface_inactive: primary
                .bg(rgb(palette.background))
                .add_modifier(Modifier::DIM),
            hint_key: primary.add_modifier(Modifier::BOLD),
            hint_label: secondary,
        },
    }
}

impl Theme {
    pub(crate) fn one_light() -> Self {
        named(Palette {
            #[cfg(test)]
            name: "one-light",
            polarity: Polarity::Light,
            primary: [56, 58, 66],
            secondary: [104, 107, 119],
            inverse: [250, 250, 250],
            focus: [48, 100, 220],
            success: [47, 125, 65],
            warning: [152, 104, 1],
            error: [178, 52, 47],
            info: [1, 112, 159],
            divider: [160, 161, 167],
            background: [250, 250, 250],
            overlay: [240, 240, 241],
            selection: [205, 219, 248],
        })
    }
    pub(crate) fn one_dark() -> Self {
        named(Palette {
            #[cfg(test)]
            name: "one-dark",
            polarity: Polarity::Dark,
            primary: [171, 178, 191],
            secondary: [150, 156, 168],
            inverse: [40, 44, 52],
            focus: [97, 175, 239],
            success: [152, 195, 121],
            warning: [229, 192, 123],
            error: [224, 108, 117],
            info: [86, 182, 194],
            divider: [62, 68, 81],
            background: [40, 44, 52],
            overlay: [44, 49, 58],
            selection: [51, 70, 89],
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
