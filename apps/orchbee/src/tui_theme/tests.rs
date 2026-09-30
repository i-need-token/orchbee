use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier},
    text::{Line, Span},
    widgets::{BorderType, Paragraph},
};

use super::{ColorLevel, GlyphSet, Polarity, Role, Theme, quantize_color};

fn style_colors(theme: &Theme) -> Vec<(Role, Option<Color>, Option<Color>)> {
    Role::ALL
        .iter()
        .map(|role| {
            let style = theme.style(*role);
            (*role, style.fg, style.bg)
        })
        .collect()
}

#[cfg(windows)]
#[test]
fn explicit_sidebar_styles_preserve_absence_and_clear_false_modifiers() {
    use orchbee_local_site_runtime::WindowsSidebarTokenStyle;
    for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
        assert_eq!(
            theme.sidebar_token_style(Role::TextSecondary, WindowsSidebarTokenStyle::default()),
            theme.style(Role::TextSecondary)
        );
        let explicit = theme.sidebar_token_style(
            Role::HintKey,
            WindowsSidebarTokenStyle {
                fg: Some([16, 32, 48]),
                bold: Some(false),
                dim: Some(true),
            },
        );
        assert_eq!(explicit.fg, Some(Color::Rgb(16, 32, 48)));
        assert!(explicit.sub_modifier.contains(Modifier::BOLD));
        assert!(explicit.add_modifier.contains(Modifier::DIM));
        let cleared = theme.sidebar_token_style(
            Role::TextSecondary,
            WindowsSidebarTokenStyle {
                dim: Some(false),
                ..Default::default()
            },
        );
        assert!(cleared.sub_modifier.contains(Modifier::DIM));
        assert!(!cleared.add_modifier.contains(Modifier::DIM));
    }
}

#[test]
fn every_role_maps_to_a_style_in_both_themes() {
    for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
        for role in Role::ALL {
            let style = theme.style(role);
            assert!(
                style.fg.is_some() || style.bg.is_some() || !style.add_modifier.is_empty(),
                "{theme:?} leaves {role:?} unset"
            );
        }
    }
}

#[test]
fn terminal_native_uses_only_named_colors() {
    let theme = Theme::terminal_native();
    assert_eq!(theme.name, "terminal-native");
    assert_eq!(theme.polarity, None);
    for (role, fg, bg) in style_colors(&theme) {
        for color in [fg, bg].into_iter().flatten() {
            assert!(
                !matches!(color, Color::Rgb(..) | Color::Indexed(_)),
                "terminal-native {role:?} carries non-ANSI color {color:?}"
            );
        }
    }
}

#[test]
fn terminal_native_text_and_surface_safety() {
    let theme = Theme::terminal_native();
    assert_eq!(theme.style(Role::TextPrimary).fg, Some(Color::Reset));
    assert_eq!(theme.style(Role::TextSecondary).fg, Some(Color::Reset));
    assert!(
        theme
            .style(Role::TextSecondary)
            .add_modifier
            .contains(Modifier::DIM)
    );
    assert_eq!(theme.style(Role::SurfaceChrome).bg, Some(Color::Reset));
    assert_eq!(theme.style(Role::SurfaceOverlay).bg, Some(Color::Reset));
    for (role, fg, bg) in style_colors(&theme) {
        if matches!(bg, None | Some(Color::Reset)) {
            assert!(
                !matches!(fg, Some(Color::Black | Color::White)),
                "{role:?} paints Black/White text on an unpainted surface"
            );
            assert!(
                !matches!(fg, Some(Color::Yellow | Color::Blue)),
                "{role:?} paints Yellow/Blue text on Reset"
            );
        }
        if !matches!(
            role,
            Role::FrameFocused | Role::FrameUnfocused | Role::FrameDivider
        ) {
            assert!(
                !matches!(fg, Some(Color::DarkGray)),
                "{role:?} uses DarkGray as text"
            );
        }
    }
}

#[test]
fn terminal_native_quantize_is_identity() {
    for level in [
        ColorLevel::Truecolor,
        ColorLevel::Ansi256,
        ColorLevel::Ansi16,
    ] {
        let quantized = Theme::terminal_native().quantize(level);
        for role in Role::ALL {
            let before = Theme::terminal_native().style(role);
            let after = quantized.style(role);
            assert_eq!(
                (before.fg, before.bg),
                (after.fg, after.bg),
                "{role:?} changed at {level:?}"
            );
        }
    }
}

#[test]
fn ansi16_quantization_leaves_no_rgb_or_indexed() {
    let theme = Theme::rgb_test_dark().quantize(ColorLevel::Ansi16);
    for (role, fg, bg) in style_colors(&theme) {
        for color in [fg, bg].into_iter().flatten() {
            assert!(
                !matches!(color, Color::Rgb(..) | Color::Indexed(_)),
                "{role:?} still carries {color:?} after Ansi16 quantization"
            );
        }
    }
}

#[test]
fn ansi256_quantization_maps_rgb_to_indexed() {
    let theme = Theme::rgb_test_dark().quantize(ColorLevel::Ansi256);
    assert!(matches!(
        theme.style(Role::TextPrimary).fg,
        Some(Color::Indexed(_))
    ));
    assert_eq!(
        Theme::terminal_native()
            .quantize(ColorLevel::Ansi256)
            .style(Role::AccentFocus)
            .fg,
        Some(Color::Cyan)
    );
}

#[test]
fn quantize_color_pinned_mappings() {
    assert_eq!(
        quantize_color(Color::Reset, ColorLevel::Ansi16),
        Color::Reset
    );
    assert_eq!(quantize_color(Color::Cyan, ColorLevel::Ansi16), Color::Cyan);
    assert_eq!(
        quantize_color(Color::Rgb(0, 0, 0), ColorLevel::Ansi16),
        Color::Black
    );
    assert_eq!(
        quantize_color(Color::Rgb(255, 255, 255), ColorLevel::Ansi16),
        Color::White
    );
    assert_eq!(
        quantize_color(Color::Rgb(0, 255, 0), ColorLevel::Ansi16),
        Color::LightGreen
    );
    assert_eq!(
        quantize_color(Color::Rgb(11, 14, 20), ColorLevel::Ansi16),
        Color::Black
    );
    assert!(matches!(
        quantize_color(Color::Rgb(211, 218, 232), ColorLevel::Ansi256),
        Color::Indexed(_)
    ));
}

#[test]
fn themes_carry_uniform_corners_and_resolved_glyphs() {
    assert_eq!(Theme::terminal_native().corners, BorderType::Plain);
    let rgb = Theme::rgb_test_dark();
    assert_eq!(rgb.corners, BorderType::Rounded);
    assert_eq!(rgb.polarity, Some(Polarity::Dark));
    assert_ne!(Polarity::Dark, Polarity::Light);

    let native = Theme::terminal_native();
    assert_eq!(native.color(Role::TextPrimary), Color::Reset);
    assert_eq!(native.color(Role::SurfaceChrome), Color::Reset);
    assert_eq!(native.color(Role::AccentFocus), Color::Cyan);

    assert!(!GlyphSet::UTF8.ellipsis.is_ascii());
    assert!(!GlyphSet::UTF8.middle_dot.is_ascii());
    assert!(!GlyphSet::UTF8.status_bullet.is_ascii());
    for value in [
        GlyphSet::ASCII.ellipsis,
        GlyphSet::ASCII.middle_dot,
        GlyphSet::ASCII.status_bullet,
        GlyphSet::ASCII.active_marker,
    ] {
        assert!(value.is_ascii());
    }
    assert_eq!(GlyphSet::resolve(false), GlyphSet::ASCII);
    assert_eq!(GlyphSet::resolve(true), GlyphSet::UTF8);
    assert_eq!(
        Theme::terminal_native().with_glyphs(GlyphSet::ASCII).glyphs,
        GlyphSet::ASCII
    );
}

#[test]
fn quantized_style_flows_into_a_rendered_buffer() {
    let theme = Theme::rgb_test_dark().quantize(ColorLevel::Ansi16);
    let mut terminal = Terminal::new(TestBackend::new(4, 1)).expect("test terminal");
    terminal
        .draw(|frame| {
            let style = theme.style(Role::TextPrimary);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled("hi", style))),
                Rect::new(0, 0, 4, 1),
            );
        })
        .expect("draw");
    let cell = terminal.backend().buffer().cell((0, 0)).expect("cell");
    assert_eq!(cell.fg, theme.style(Role::TextPrimary).fg.unwrap());
    assert!(!matches!(cell.fg, Color::Rgb(..) | Color::Indexed(_)));
}
