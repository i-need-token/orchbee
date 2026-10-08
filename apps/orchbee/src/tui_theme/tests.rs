use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier},
    text::{Line, Span},
    widgets::{BorderType, Paragraph},
};

use super::{ColorLevel, GlyphSet, Polarity, Role, Theme, quantize_color};

#[test]
fn host_color_precedence_unknown_and_disabled_levels_are_explicit() {
    use super::resolve_color_level as resolve;
    for (inputs, expected) in [
        ([None, None, None, None], ColorLevel::Ansi16),
        (
            [
                Some("1"),
                Some("3"),
                Some("truecolor"),
                Some("xterm-256color"),
            ],
            ColorLevel::Monochrome,
        ),
        (
            [Some(""), Some("0"), Some("truecolor"), None],
            ColorLevel::Monochrome,
        ),
        (
            [None, Some("1"), Some("truecolor"), Some("xterm-256color")],
            ColorLevel::Ansi16,
        ),
        (
            [None, Some("2"), Some("truecolor"), None],
            ColorLevel::Ansi256,
        ),
        ([None, Some("3"), None, Some("dumb")], ColorLevel::Truecolor),
        (
            [
                None,
                Some("invalid"),
                Some("truecolor"),
                Some("xterm-256color"),
            ],
            ColorLevel::Ansi16,
        ),
        ([None, None, Some("TRUECOLOR"), None], ColorLevel::Truecolor),
        (
            [None, None, Some("24bit"), Some("dumb")],
            ColorLevel::Truecolor,
        ),
        (
            [None, None, Some("unknown"), Some("XTERM-256COLOR")],
            ColorLevel::Ansi256,
        ),
        ([None, None, None, Some("dumb")], ColorLevel::Monochrome),
        ([None, None, None, Some("unknown")], ColorLevel::Ansi16),
    ] {
        assert_eq!(
            resolve(inputs[0], inputs[1], inputs[2], inputs[3], None, false),
            expected
        );
    }
    let oversized = format!("{}256color", "x".repeat(129));
    assert_eq!(
        resolve(None, None, None, Some(&oversized), None, false),
        ColorLevel::Ansi16
    );
}

#[test]
fn windows_terminal_color_capability_respects_explicit_descriptors_and_multiplexers() {
    use super::resolve_color_level as resolve;
    let session = "5720ee6d-6474-47b0-88db-fa7e10e60d37";
    for indicator in [session, "5720EE6D-6474-47B0-88DB-FA7E10E60D37"] {
        assert_eq!(
            resolve(None, None, None, None, Some(indicator), false),
            ColorLevel::Truecolor
        );
        assert_eq!(
            resolve(None, None, Some(""), Some(""), Some(indicator), false),
            ColorLevel::Truecolor
        );
    }
    for (inputs, expected) in [
        ([Some("1"), None, None, None], ColorLevel::Monochrome),
        ([None, Some("0"), None, None], ColorLevel::Monochrome),
        ([None, Some("1"), None, None], ColorLevel::Ansi16),
        ([None, Some("2"), None, None], ColorLevel::Ansi256),
        ([None, Some("3"), None, None], ColorLevel::Truecolor),
        ([None, Some("invalid"), None, None], ColorLevel::Ansi16),
        ([None, None, Some("truecolor"), None], ColorLevel::Truecolor),
        (
            [None, None, Some("24bit"), Some("dumb")],
            ColorLevel::Truecolor,
        ),
        ([None, None, Some("unknown"), None], ColorLevel::Ansi16),
        ([None, None, None, Some("dumb")], ColorLevel::Monochrome),
        (
            [None, None, None, Some("xterm-256color")],
            ColorLevel::Ansi256,
        ),
        ([None, None, None, Some("unknown")], ColorLevel::Ansi16),
    ] {
        assert_eq!(
            resolve(
                inputs[0],
                inputs[1],
                inputs[2],
                inputs[3],
                Some(session),
                false
            ),
            expected
        );
    }
    assert_eq!(
        resolve(None, None, None, None, Some(session), true),
        ColorLevel::Ansi16
    );
    for indicator in [
        None,
        Some(""),
        Some("Windows Terminal"),
        Some("{5720ee6d-6474-47b0-88db-fa7e10e60d37}"),
        Some("5720ee6d-6474-47b0-88db-fa7e10e60d3"),
        Some("5720ee6d-6474-47b0-88db-fa7e10e60d3z"),
        Some("5720ee6d_6474-47b0-88db-fa7e10e60d37"),
        Some("5720ee6d-6474-47b0-88db-fa7e10e60d37\n"),
    ] {
        assert_eq!(
            resolve(None, None, None, None, indicator, false),
            ColorLevel::Ansi16
        );
    }
    let oversized = session.repeat(1024);
    assert_eq!(
        resolve(None, None, None, None, Some(&oversized), false),
        ColorLevel::Ansi16
    );
}

#[test]
fn shipped_palettes_have_declared_polarity_complete_roles_and_visible_text() {
    let luminance = |color| {
        let Color::Rgb(r, g, b) = color else {
            panic!("shipped RGB palette")
        };
        let linear = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    };
    for (theme, name, polarity) in [
        (Theme::one_light(), "one-light", Polarity::Light),
        (Theme::one_dark(), "one-dark", Polarity::Dark),
    ] {
        assert_eq!(theme.name, name);
        assert_eq!(theme.polarity, Some(polarity));
        let background = luminance(theme.color(Role::SurfaceCanvas));
        assert_eq!(background > 0.5, polarity == Polarity::Light);
        for role in Role::ALL {
            let style = theme.style(role);
            assert!(style.fg.is_some() || style.bg.is_some(), "{name} {role:?}");
            assert!(
                !style
                    .add_modifier
                    .intersects(Modifier::REVERSED | Modifier::SLOW_BLINK | Modifier::RAPID_BLINK)
            );
        }
        for surface in [
            Role::SurfaceCanvas,
            Role::SurfaceChrome,
            Role::SurfaceOverlay,
        ] {
            let background = luminance(theme.color(surface));
            for role in [Role::TextPrimary, Role::TextSecondary, Role::HintLabel] {
                let text = luminance(theme.color(role));
                let contrast = (background.max(text) + 0.05) / (background.min(text) + 0.05);
                assert!(contrast >= 4.5, "{name} {role:?}/{surface:?}: {contrast}");
            }
        }
        for (foreground, background) in [
            (Role::TextPrimary, Role::SurfaceOverlay),
            (Role::TextPrimary, Role::SurfaceSelection),
            (Role::TextSecondary, Role::SurfaceOverlay),
            (Role::TextInverse, Role::AccentFocus),
        ] {
            let (text, surface) = (
                luminance(theme.color(foreground)),
                luminance(theme.color(background)),
            );
            let contrast = (surface.max(text) + 0.05) / (surface.min(text) + 0.05);
            assert!(
                contrast >= 4.5,
                "{name} {foreground:?}/{background:?}: {contrast}"
            );
        }
    }
}

#[test]
fn shipped_disabled_text_keeps_its_muted_color_without_faint_inversion() {
    for theme in [Theme::one_light(), Theme::one_dark()] {
        for level in [ColorLevel::Truecolor, ColorLevel::Ansi256] {
            let theme = theme.quantize(level);
            let disabled = theme.style(Role::TextDisabled);
            assert_eq!(disabled.fg, theme.style(Role::TextSecondary).fg);
            assert_ne!(disabled.fg, theme.style(Role::TextPrimary).fg);
            assert!(
                !disabled.add_modifier.contains(Modifier::DIM),
                "{} {level:?}: faint can darken muted text back to primary on a light surface",
                theme.name
            );
        }
    }
}

#[test]
fn shipped_low_color_and_monochrome_preserve_readable_hierarchy() {
    for theme in [
        Theme::terminal_native(),
        Theme::one_light(),
        Theme::one_dark(),
    ] {
        let low = theme.quantize(ColorLevel::Ansi16);
        if theme.polarity.is_some() {
            for role in [
                Role::TextPrimary,
                Role::TextSecondary,
                Role::FrameFocused,
                Role::FrameUnfocused,
                Role::FrameDivider,
            ] {
                assert_ne!(
                    low.style(role).fg,
                    low.style(Role::SurfaceChrome).bg,
                    "{} {role:?} hidden on chrome",
                    theme.name
                );
            }
            assert_ne!(
                low.style(Role::TextInverse).fg,
                low.style(Role::AccentFocus).fg
            );
        }
        for role in Role::ALL {
            let style = low.style(role);
            for color in [style.fg, style.bg].into_iter().flatten() {
                assert!(!matches!(color, Color::Rgb(..) | Color::Indexed(_)));
            }
            if !matches!(
                role,
                Role::FrameFocused | Role::FrameUnfocused | Role::FrameDivider
            ) {
                assert_ne!(style.fg, Some(Color::DarkGray), "{} {role:?}", theme.name);
            }
        }
        let mono = theme.quantize(ColorLevel::Monochrome);
        for role in Role::ALL {
            for color in [mono.style(role).fg, mono.style(role).bg]
                .into_iter()
                .flatten()
            {
                assert_eq!(color, Color::Reset, "{} {role:?}", theme.name);
            }
        }
        assert!(
            mono.style(Role::FrameFocused)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert!(
            mono.style(Role::TextSecondary)
                .add_modifier
                .contains(Modifier::DIM)
        );
        assert!(
            mono.style(Role::TextDisabled)
                .add_modifier
                .contains(Modifier::DIM)
        );
        assert!(
            mono.style(Role::HintKey)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert!(
            mono.style(Role::SurfaceSelection)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert!(
            mono.style(Role::AccentSelection)
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }
}

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
