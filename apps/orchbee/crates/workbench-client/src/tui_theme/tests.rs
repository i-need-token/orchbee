use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BorderType, Paragraph},
};

use super::{ColorLevel, GlyphMode, GlyphSet, Polarity, Role, Theme, quantize_color};

mod cursor;
mod destructive;
mod terminal_defaults;

#[test]
fn sidebar_pointer_fallbacks_keep_one_background_and_readable_navigation() {
    for base in [
        Theme::terminal_native(),
        Theme::one_light(),
        Theme::one_dark(),
    ] {
        for level in [
            ColorLevel::Truecolor,
            ColorLevel::Ansi256,
            ColorLevel::Ansi16,
            ColorLevel::Monochrome,
        ] {
            let theme = base.quantize(level).paired_pointer_surfaces();
            assert_eq!(theme.paired_pointer_surfaces(), theme);
            for surface in [Role::SurfaceHover, Role::SurfacePressed] {
                let band = theme.style(surface);
                for role in [
                    Role::TextPrimary,
                    Role::TextSecondary,
                    Role::HintLabel,
                    Role::AccentSelection,
                    Role::StatusWorking,
                ] {
                    let content = theme.content_on_surface(
                        Role::SurfaceChrome,
                        Some(surface),
                        Some(role),
                        theme.style(role),
                    );
                    assert_eq!(content.bg, band.bg);
                    assert_eq!(
                        content.add_modifier.contains(Modifier::REVERSED),
                        band.add_modifier.contains(Modifier::REVERSED)
                    );
                    if let Some(foreground) = band.fg {
                        assert_eq!(content.fg, Some(foreground));
                    }
                }
            }
        }
    }
}

#[test]
fn paired_pointer_projection_preserves_other_roles_and_explicit_content_colors() {
    for original in [
        Theme::terminal_native(),
        Theme::one_light(),
        Theme::one_dark(),
    ] {
        let theme = original.paired_pointer_surfaces();
        for role in Role::ALL {
            if !matches!(role, Role::SurfaceHover | Role::SurfacePressed)
                || !original
                    .style(role)
                    .add_modifier
                    .contains(Modifier::REVERSED)
            {
                assert_eq!(theme.style(role), original.style(role));
            }
        }
        for surface in [Role::SurfaceHover, Role::SurfacePressed] {
            let explicit = theme.style(Role::TextPrimary).fg(Color::Rgb(16, 32, 48));
            for role in [None, Some(Role::TextPrimary)] {
                let composed =
                    theme.content_on_surface(Role::SurfaceOverlay, Some(surface), role, explicit);
                assert_eq!(composed.fg, explicit.fg);
            }
        }
    }
}

const DESIGN_TOKENS_CSS: &str =
    include_str!("../../../../../../packages/design-tokens/src/tokens.css");

fn css_block<'a>(source: &'a str, selector: &str) -> &'a str {
    let marker = format!("{selector} {{");
    let (_, body) = source
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing CSS selector {selector}"));
    body.split_once('}')
        .unwrap_or_else(|| panic!("unterminated CSS selector {selector}"))
        .0
}

fn css_value(block: &str, property: &str) -> String {
    let marker = format!("{property}:");
    let (_, value) = block
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing CSS property {property}"));
    value
        .split_once(';')
        .unwrap_or_else(|| panic!("unterminated CSS property {property}"))
        .0
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_hex_rgb(value: &str) -> [u8; 3] {
    let value = value
        .strip_prefix('#')
        .filter(|value| value.len() == 6)
        .unwrap_or_else(|| panic!("expected six-digit hex color, got {value}"));
    [
        u8::from_str_radix(&value[0..2], 16).expect("red hex channel"),
        u8::from_str_radix(&value[2..4], 16).expect("green hex channel"),
        u8::from_str_radix(&value[4..6], 16).expect("blue hex channel"),
    ]
}

fn parse_neutral_oklch(value: &str) -> u8 {
    let mut parts = value
        .strip_prefix("oklch(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or_else(|| panic!("expected oklch color, got {value}"))
        .split_ascii_whitespace();
    let lightness = parts
        .next()
        .and_then(|value| value.strip_suffix('%'))
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing OKLCH lightness in {value}"));
    assert_eq!(parts.next(), Some("0"), "expected an achromatic token");
    assert_eq!(parts.next(), Some("none"), "expected an achromatic hue");
    assert_eq!(parts.next(), None, "unexpected OKLCH component");
    let linear = (lightness / 100.0).powi(3);
    let srgb = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

fn rgb_channels(color: Color) -> [u8; 3] {
    let Color::Rgb(red, green, blue) = color else {
        panic!("expected RGB color, got {color:?}")
    };
    [red, green, blue]
}

fn relative_luminance(color: Color) -> f64 {
    let [red, green, blue] = rgb_channels(color);
    let linear = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

fn contrast_ratio(foreground: Color, background: Color) -> f64 {
    let foreground = relative_luminance(foreground);
    let background = relative_luminance(background);
    (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
}

fn composite_rgb(foreground: [u8; 3], background: [u8; 3], opacity: u16) -> [u8; 3] {
    std::array::from_fn(|index| {
        let foreground = u16::from(foreground[index]);
        let background = u16::from(background[index]);
        ((foreground * opacity + background * (100 - opacity) + 50) / 100) as u8
    })
}

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
fn named_roles_conform_to_the_public_design_token_contract() {
    let references = css_block(DESIGN_TOKENS_CSS, ":root");
    let light = css_block(DESIGN_TOKENS_CSS, "[data-theme='light']");
    let dark = css_block(DESIGN_TOKENS_CSS, "[data-theme='dark']");

    let accent = css_value(references, "--ob-ref-color-orchbee-accent");
    let light_text = css_value(references, "--ob-ref-color-orchbee-light-text");
    let neutral_100 = css_value(references, "--ob-ref-color-neutral-100");
    let neutral_700 = css_value(references, "--ob-ref-color-neutral-700");
    let neutral_800 = css_value(references, "--ob-ref-color-neutral-800");
    assert_eq!(accent, "#2778c1");
    assert_eq!(light_text, "#141414");
    assert_eq!(neutral_100, "oklch(97% 0 none)");
    assert_eq!(neutral_700, "oklch(37.1% 0 none)");
    assert_eq!(neutral_800, "oklch(26.9% 0 none)");

    assert_eq!(
        css_value(light, "--ob-sys-color-action-primary"),
        "var(--ob-ref-color-orchbee-accent)"
    );
    assert_eq!(
        css_value(light, "--ob-sys-color-surface-hovered"),
        "color-mix( in srgb, var(--ob-ref-color-orchbee-light-text) 8%, transparent )"
    );
    assert_eq!(
        css_value(light, "--ob-sys-color-surface-pressed"),
        "color-mix( in srgb, var(--ob-ref-color-orchbee-light-text) 12%, transparent )"
    );
    assert_eq!(
        css_value(dark, "--ob-sys-color-action-primary"),
        "var(--ob-ref-color-neutral-100)"
    );
    assert_eq!(
        css_value(dark, "--ob-sys-color-surface-hovered"),
        "var(--ob-ref-color-neutral-800)"
    );
    assert_eq!(
        css_value(dark, "--ob-sys-color-surface-pressed"),
        "var(--ob-ref-color-neutral-700)"
    );

    let theme = Theme::one_light();
    assert_eq!(
        rgb_channels(theme.color(Role::AccentAction)),
        parse_hex_rgb(&accent)
    );
    let chrome = rgb_channels(theme.color(Role::SurfaceChrome));
    let interaction_foreground = parse_hex_rgb(&light_text);
    assert_eq!(
        rgb_channels(theme.color(Role::SurfaceHover)),
        composite_rgb(interaction_foreground, chrome, 8)
    );
    assert_eq!(
        rgb_channels(theme.color(Role::SurfacePressed)),
        composite_rgb(interaction_foreground, chrome, 12)
    );

    let theme = Theme::one_dark();
    for (role, system, reference) in [
        (Role::StatusWarning, "status-attention", "yellow"),
        (Role::StatusSuccess, "status-agent-done", "green"),
        (Role::StatusError, "status-agent-error", "red"),
        (Role::StatusIdle, "status-agent-idle", "gray"),
    ] {
        let reference = format!("--ob-ref-color-groknight-{reference}");
        assert_eq!(
            css_value(dark, &format!("--ob-sys-color-{system}")),
            format!("var({reference})")
        );
        assert_eq!(
            rgb_channels(theme.color(role)),
            parse_hex_rgb(&css_value(references, &reference))
        );
    }
    assert_eq!(
        css_value(dark, "--ob-sys-color-status-working"),
        "var(--ob-ref-color-agent-working-dark)"
    );
    assert_eq!(
        rgb_channels(theme.color(Role::StatusWorking)),
        parse_hex_rgb(&css_value(references, "--ob-ref-color-agent-working-dark"))
    );
    let neutral = |value| [parse_neutral_oklch(value); 3];
    assert_eq!(
        rgb_channels(theme.color(Role::AccentAction)),
        neutral(&neutral_100)
    );
    assert_eq!(
        rgb_channels(theme.color(Role::SurfaceHover)),
        neutral(&neutral_800)
    );
    assert_eq!(
        rgb_channels(theme.color(Role::SurfacePressed)),
        neutral(&neutral_700)
    );
}

#[test]
fn named_secondary_adjustments_are_the_minimum_readable_steps() {
    let light = Theme::one_light();
    let light_secondary = light.color(Role::TextSecondary);
    assert_eq!(rgb_channels(light_secondary), [90, 93, 105]);
    assert!(contrast_ratio(light_secondary, light.color(Role::SurfacePressed)) >= 4.5);
    assert!(contrast_ratio(Color::Rgb(91, 94, 106), light.color(Role::SurfacePressed)) < 4.5);

    let dark = Theme::one_dark();
    let dark_secondary = dark.color(Role::TextSecondary);
    assert_eq!(rgb_channels(dark_secondary), [170, 177, 189]);
    assert!(contrast_ratio(dark_secondary, dark.color(Role::SurfaceSelection)) >= 4.5);
    for nearer in [
        Color::Rgb(169, 177, 189),
        Color::Rgb(170, 176, 189),
        Color::Rgb(170, 177, 188),
    ] {
        assert!(
            contrast_ratio(nearer, dark.color(Role::SurfaceSelection)) < 4.5,
            "{nearer:?} is not the componentwise boundary"
        );
    }

    for theme in [light, dark] {
        assert_eq!(
            theme.style(Role::HintLabel).fg,
            theme.style(Role::TextSecondary).fg
        );
        assert_eq!(
            theme.style(Role::StatusPending).fg,
            theme.style(Role::TextSecondary).fg
        );
        assert!(
            theme
                .style(Role::StatusPending)
                .add_modifier
                .contains(Modifier::DIM)
        );
        for surface in [
            Role::SurfaceCanvas,
            Role::SurfaceChrome,
            Role::SurfaceOverlay,
            Role::SurfaceHover,
            Role::SurfacePressed,
            Role::SurfaceSelection,
            Role::SurfaceCurrent,
            Role::SurfaceRelated,
            Role::SurfaceInactive,
        ] {
            assert!(
                contrast_ratio(theme.color(Role::TextPrimary), theme.color(surface))
                    > contrast_ratio(theme.color(Role::TextSecondary), theme.color(surface)),
                "{} hierarchy on {surface:?}",
                theme.name
            );
        }
    }
}

#[test]
fn dark_primary_separation_exceeds_the_readable_near_equal_pair_after_composition() {
    let theme = Theme::one_dark();
    // Both old foregrounds clear the surface readability floor. That alone did not protect
    // priority: they were almost equally bright. Keep this control separate from RGB snapshots;
    // a larger nominal separation is not a universal perceptual threshold or a host result.
    let near_equal = [Color::Rgb(171, 178, 191), Color::Rgb(170, 177, 189)];
    let near_equal_separation = contrast_ratio(near_equal[0], near_equal[1]);
    for (base, interaction) in [
        (Role::SurfaceCanvas, None),
        (Role::SurfaceChrome, None),
        (Role::SurfaceOverlay, None),
        (Role::SurfaceOverlay, Some(Role::SurfaceHover)),
        (Role::SurfaceOverlay, Some(Role::SurfacePressed)),
        (Role::SurfaceOverlay, Some(Role::SurfaceSelection)),
        (Role::SurfaceChrome, Some(Role::SurfaceCurrent)),
        (Role::SurfaceInactive, None),
    ] {
        let primary = theme.content_on_surface(
            base,
            interaction,
            Some(Role::TextPrimary),
            theme.style(Role::TextPrimary),
        );
        let secondary = theme.content_on_surface(
            base,
            interaction,
            Some(Role::TextSecondary),
            theme.style(Role::TextSecondary),
        );
        assert_eq!(primary.bg, secondary.bg);
        let background = primary.bg.expect("named content has a painted surface");
        for foreground in near_equal {
            assert!(contrast_ratio(foreground, background) >= 4.5);
        }
        let primary = primary.fg.expect("primary content has a foreground");
        let secondary = secondary.fg.expect("secondary content has a foreground");
        assert!(contrast_ratio(primary, background) >= 4.5);
        assert!(contrast_ratio(secondary, background) >= 4.5);
        assert!(
            relative_luminance(primary) > relative_luminance(secondary),
            "{base:?}/{interaction:?} reverses information priority"
        );
        assert!(
            contrast_ratio(primary, secondary) > near_equal_separation,
            "{base:?}/{interaction:?} retains the readable near-equal hierarchy"
        );
    }
}

#[test]
fn shipped_palettes_have_declared_polarity_complete_roles_and_visible_text() {
    for (theme, name, polarity) in [
        (Theme::one_light(), "one-light", Polarity::Light),
        (Theme::one_dark(), "one-dark", Polarity::Dark),
    ] {
        assert_eq!(theme.name, name);
        assert_eq!(theme.polarity, Some(polarity));
        let background = relative_luminance(theme.color(Role::SurfaceCanvas));
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
        let surfaces = [
            Role::SurfaceCanvas,
            Role::SurfaceChrome,
            Role::SurfaceOverlay,
            Role::SurfaceHover,
            Role::SurfacePressed,
            Role::SurfaceSelection,
            Role::SurfaceCurrent,
            Role::SurfaceRelated,
            Role::SurfaceInactive,
        ];
        for surface in surfaces {
            for role in [Role::TextPrimary, Role::TextSecondary, Role::HintLabel] {
                let contrast = contrast_ratio(theme.color(role), theme.color(surface));
                assert!(contrast >= 4.5, "{name} {role:?}/{surface:?}: {contrast}");
            }
            let action_contrast =
                contrast_ratio(theme.color(Role::AccentAction), theme.color(surface));
            assert!(
                action_contrast >= 3.0,
                "{name} accent-action/{surface:?}: {action_contrast}"
            );
            for role in [
                Role::StatusSuccess,
                Role::StatusWarning,
                Role::StatusError,
                Role::StatusInfo,
                Role::StatusPending,
                Role::StatusWorking,
                Role::StatusUnread,
            ] {
                let contrast = contrast_ratio(theme.color(role), theme.color(surface));
                assert!(contrast >= 3.0, "{name} {role:?}/{surface:?}: {contrast}");
            }
        }
        assert!(
            !theme
                .style(Role::AccentAction)
                .add_modifier
                .intersects(Modifier::BOLD | Modifier::REVERSED),
            "{name} accent-action must remain an at-rest accent"
        );
        let interaction_surfaces = [
            theme.color(Role::SurfaceHover),
            theme.color(Role::SurfacePressed),
        ];
        assert_ne!(interaction_surfaces[0], interaction_surfaces[1], "{name}");
        for interaction in interaction_surfaces {
            for stable in [
                Role::SurfaceCanvas,
                Role::SurfaceChrome,
                Role::SurfaceOverlay,
                Role::SurfaceSelection,
                Role::SurfaceCurrent,
                Role::SurfaceInactive,
            ] {
                assert_ne!(interaction, theme.color(stable), "{name} {stable:?}");
            }
        }
        let inverse_contrast = contrast_ratio(
            theme.color(Role::TextInverse),
            theme.color(Role::AccentFocus),
        );
        assert!(inverse_contrast >= 4.5, "{name} text-inverse/accent-focus");
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

fn assert_interaction_fills_or_fallback(theme: Theme, level: ColorLevel) {
    let theme = theme.quantize(level);
    let hover = theme.style(Role::SurfaceHover);
    let pressed = theme.style(Role::SurfacePressed);
    if hover.add_modifier.contains(Modifier::REVERSED)
        || pressed.add_modifier.contains(Modifier::REVERSED)
    {
        assert!(hover.add_modifier.contains(Modifier::REVERSED));
        assert!(pressed.add_modifier.contains(Modifier::REVERSED));
        assert_eq!(hover.bg, None, "{} {level:?} hover", theme.name);
        assert_eq!(pressed.bg, None, "{} {level:?} pressed", theme.name);
    } else {
        assert_ne!(hover.bg, None, "{} {level:?} hover", theme.name);
        assert_ne!(pressed.bg, None, "{} {level:?} pressed", theme.name);
        assert_ne!(
            hover.bg, pressed.bg,
            "{} {level:?} interaction states",
            theme.name
        );
        for stable in [
            Role::SurfaceCanvas,
            Role::SurfaceChrome,
            Role::SurfaceOverlay,
            Role::SurfaceSelection,
            Role::SurfaceCurrent,
            Role::SurfaceInactive,
        ] {
            assert_ne!(
                hover.bg,
                theme.style(stable).bg,
                "{} {level:?} hover",
                theme.name
            );
            assert_ne!(
                pressed.bg,
                theme.style(stable).bg,
                "{} {level:?} pressed",
                theme.name
            );
        }
    }
}

#[test]
fn reduced_color_interaction_roles_keep_distinct_fills_or_the_declared_fallback() {
    for theme in [Theme::one_light(), Theme::one_dark()] {
        for level in [ColorLevel::Ansi256, ColorLevel::Ansi16] {
            assert_interaction_fills_or_fallback(theme, level);
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
        assert_eq!(theme.current_context_marker(), " ");
        let low = theme.quantize(ColorLevel::Ansi16);
        if theme.polarity.is_some() {
            for role in [
                Role::TextPrimary,
                Role::TextSecondary,
                Role::FrameFocused,
                Role::FrameUnfocused,
                Role::FrameDivider,
                Role::FrameGuide,
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
                Role::FrameFocused | Role::FrameUnfocused | Role::FrameDivider | Role::FrameGuide
            ) {
                assert_ne!(style.fg, Some(Color::DarkGray), "{} {role:?}", theme.name);
            }
        }
        assert!(
            !low.style(Role::AccentAction)
                .add_modifier
                .intersects(Modifier::BOLD | Modifier::REVERSED),
            "{} accent-action must not look focused or pressed at rest",
            theme.name
        );
        let mono = theme.quantize(ColorLevel::Monochrome);
        assert_eq!(mono.current_context_marker(), theme.glyphs.active_marker);
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
        for role in [Role::SurfaceHover, Role::SurfacePressed] {
            assert!(
                mono.style(role).add_modifier.contains(Modifier::REVERSED),
                "{} {role:?}",
                theme.name
            );
        }
        assert!(
            !mono
                .style(Role::AccentAction)
                .add_modifier
                .intersects(Modifier::BOLD | Modifier::REVERSED),
            "{} accent-action must keep at-rest modifier semantics",
            theme.name
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

#[test]
fn explicit_sidebar_styles_preserve_absence_and_clear_false_modifiers() {
    use orchbee_local_runtime::SidebarTokenStyle;
    for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
        assert_eq!(
            theme.sidebar_token_style(
                Role::TextSecondary,
                SidebarTokenStyle::default(),
                false,
                false
            ),
            theme.style(Role::TextSecondary)
        );
        let explicit = theme.sidebar_token_style(
            Role::HintKey,
            SidebarTokenStyle {
                fg: Some([16, 32, 48]),
                bold: Some(false),
                dim: Some(true),
            },
            false,
            false,
        );
        assert_eq!(explicit.fg, Some(Color::Rgb(16, 32, 48)));
        assert!(explicit.sub_modifier.contains(Modifier::BOLD));
        assert!(explicit.add_modifier.contains(Modifier::DIM));
        let cleared = theme.sidebar_token_style(
            Role::TextSecondary,
            SidebarTokenStyle {
                dim: Some(false),
                ..Default::default()
            },
            false,
            false,
        );
        assert!(cleared.sub_modifier.contains(Modifier::DIM));
        assert!(!cleared.add_modifier.contains(Modifier::DIM));
    }
    let explicit = Theme::terminal_native().sidebar_token_style(
        Role::TextSecondary,
        SidebarTokenStyle {
            fg: Some([16, 32, 48]),
            bold: Some(false),
            dim: Some(true),
        },
        true,
        false,
    );
    assert_eq!(explicit.fg, Some(Color::Rgb(16, 32, 48)));
    assert!(explicit.add_modifier.contains(Modifier::DIM));
    assert!(explicit.sub_modifier.contains(Modifier::BOLD));
}

#[test]
fn sidebar_navigation_pairs_text_without_recoloring_status_or_explicit_tokens() {
    use orchbee_local_runtime::SidebarTokenStyle;
    for theme in [
        Theme::terminal_native(),
        Theme::rgb_test_dark(),
        Theme::one_light(),
        Theme::one_dark(),
        Theme::terminal_native().quantize(ColorLevel::Monochrome),
    ] {
        let selected =
            theme.sidebar_token_style(Role::TextSecondary, Default::default(), false, true);
        assert_eq!(selected.fg, theme.style(Role::AccentSelection).fg);
        assert_eq!(selected.bg, theme.style(Role::SurfaceSelection).bg);
        assert!(!selected.add_modifier.contains(Modifier::DIM));
        let status =
            theme.sidebar_token_style(Role::StatusWarning, Default::default(), false, true);
        assert_eq!(status.fg, theme.style(Role::StatusWarning).fg);
        assert_eq!(status.bg, theme.style(Role::SurfaceSelection).bg);
        let token = SidebarTokenStyle {
            fg: Some([16, 32, 48]),
            bold: Some(false),
            dim: Some(true),
        };
        let explicit = theme.sidebar_token_style(Role::TextSecondary, token, false, true);
        assert_eq!(
            explicit.fg,
            theme
                .sidebar_token_style(Role::TextSecondary, token, false, false)
                .fg
        );
        assert!(explicit.add_modifier.contains(Modifier::DIM));
        assert!(explicit.sub_modifier.contains(Modifier::BOLD));
        assert_eq!(
            theme.sidebar_token_style(Role::TextPrimary, Default::default(), true, true),
            theme.current_context_style(theme.style(Role::TextPrimary))
        );
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

const AGENT_ROLES: [(Role, Color, Color); 3] = [
    (
        Role::AgentAttention,
        Color::Rgb(254, 188, 46),
        Color::Yellow,
    ),
    (Role::AgentDone, Color::Rgb(40, 200, 64), Color::Green),
    (Role::AgentError, Color::Rgb(255, 95, 87), Color::Red),
];

#[test]
fn terminal_native_uses_only_named_colors_outside_fixed_agent_glyphs() {
    let theme = Theme::terminal_native();
    assert_eq!(theme.name, "terminal-native");
    assert_eq!(theme.polarity, None);
    for (role, fg, bg) in style_colors(&theme) {
        if AGENT_ROLES.iter().any(|(agent, ..)| *agent == role) {
            continue;
        }
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
    assert_eq!(theme.style(Role::AccentAction).fg, Some(Color::Cyan));
    assert!(
        !theme
            .style(Role::AccentAction)
            .add_modifier
            .intersects(Modifier::BOLD | Modifier::REVERSED)
    );
    for role in [Role::SurfaceHover, Role::SurfacePressed] {
        let interaction = theme.style(role);
        assert_eq!((interaction.fg, interaction.bg), (None, None));
        assert!(interaction.add_modifier.contains(Modifier::REVERSED));
        assert!(!interaction.add_modifier.contains(Modifier::BOLD));
    }
    for role in [Role::TextPrimary, Role::TextSecondary, Role::HintLabel] {
        let current = theme.current_context_style(theme.style(role));
        assert_eq!(current.fg, theme.style(Role::SurfaceCurrent).fg);
        assert_eq!(current.bg, theme.style(Role::SurfaceCurrent).bg);
        assert!(!current.add_modifier.contains(Modifier::DIM));
    }
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
            Role::FrameFocused | Role::FrameUnfocused | Role::FrameDivider | Role::FrameGuide
        ) {
            assert!(
                !matches!(fg, Some(Color::DarkGray)),
                "{role:?} uses DarkGray as text"
            );
        }
    }
}

#[test]
fn content_on_surface_preserves_named_theme_hierarchy_and_explicit_status() {
    for theme in [Theme::one_light(), Theme::one_dark()] {
        for (base, interaction) in [
            (Role::SurfaceCanvas, None),
            (Role::SurfaceChrome, None),
            (Role::SurfaceOverlay, None),
            (Role::SurfaceOverlay, Some(Role::SurfaceSelection)),
            (Role::SurfaceOverlay, Some(Role::SurfaceCurrent)),
        ] {
            let secondary = theme.content_on_surface(
                base,
                interaction,
                Some(Role::TextSecondary),
                theme.style(Role::TextSecondary),
            );
            assert_eq!(secondary.fg, theme.style(Role::TextSecondary).fg);
            assert_eq!(
                secondary.bg,
                interaction
                    .and_then(|role| theme.style(role).bg)
                    .or(theme.style(base).bg)
            );

            let warning = theme.content_on_surface(
                base,
                interaction,
                Some(Role::StatusWarning),
                theme.style(Role::StatusWarning),
            );
            assert_eq!(warning.fg, theme.style(Role::StatusWarning).fg);
            assert_eq!(warning.bg, secondary.bg);

            let explicit = Style::new()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD | Modifier::DIM);
            let explicit = theme.content_on_surface(base, interaction, None, explicit);
            assert_eq!(explicit.fg, Some(Color::LightCyan));
            assert!(
                explicit
                    .add_modifier
                    .contains(Modifier::BOLD | Modifier::DIM)
            );
            assert_eq!(explicit.bg, secondary.bg);
        }
    }
}

#[test]
fn terminal_native_surface_pairs_only_ordinary_semantic_text() {
    let theme = Theme::terminal_native();
    let current = theme.content_on_surface(
        Role::SurfaceOverlay,
        Some(Role::SurfaceCurrent),
        Some(Role::TextSecondary),
        theme.style(Role::TextSecondary),
    );
    assert_eq!(current.fg, theme.style(Role::SurfaceCurrent).fg);
    assert_eq!(current.bg, theme.style(Role::SurfaceCurrent).bg);
    assert!(!current.add_modifier.contains(Modifier::DIM));

    let disabled = theme.content_on_surface(
        Role::SurfaceOverlay,
        Some(Role::SurfaceCurrent),
        Some(Role::TextDisabled),
        theme.style(Role::TextDisabled),
    );
    assert_eq!(disabled.fg, theme.style(Role::SurfaceCurrent).fg);
    assert!(disabled.add_modifier.contains(Modifier::DIM));

    let selected = theme.content_on_surface(
        Role::SurfaceOverlay,
        Some(Role::SurfaceSelection),
        Some(Role::TextSecondary),
        theme.style(Role::TextSecondary),
    );
    assert_eq!(selected.fg, theme.style(Role::AccentSelection).fg);
    assert_eq!(selected.bg, theme.style(Role::SurfaceSelection).bg);
    assert!(!selected.add_modifier.contains(Modifier::DIM));

    for role in [Role::StatusWarning, Role::StatusPending] {
        let status = theme.content_on_surface(
            Role::SurfaceOverlay,
            Some(Role::SurfaceCurrent),
            Some(role),
            theme.style(role),
        );
        assert_eq!(status.fg, theme.style(role).fg);
        assert_eq!(status.bg, theme.style(Role::SurfaceCurrent).bg);
        assert_eq!(
            status.add_modifier.contains(Modifier::DIM),
            theme.style(role).add_modifier.contains(Modifier::DIM)
        );
    }

    let explicit = Style::new()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD | Modifier::DIM);
    let explicit = theme.content_on_surface(
        Role::SurfaceOverlay,
        Some(Role::SurfaceSelection),
        None,
        explicit,
    );
    assert_eq!(explicit.fg, Some(Color::Green));
    assert!(
        explicit
            .add_modifier
            .contains(Modifier::BOLD | Modifier::DIM)
    );

    let key = theme.content_on_surface(
        Role::SurfaceOverlay,
        Some(Role::SurfaceCurrent),
        Some(Role::HintKey),
        theme.style(Role::HintKey),
    );
    assert!(key.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn checked_pair_and_low_color_interaction_layer_over_the_base_surface() {
    let native = Theme::terminal_native();
    for interaction in [
        None,
        Some(Role::SurfaceCurrent),
        Some(Role::SurfaceSelection),
        Some(Role::SurfaceHover),
        Some(Role::SurfacePressed),
    ] {
        let checked = native.content_on_surface(
            Role::SurfaceOverlay,
            interaction,
            Some(Role::AccentSelection),
            native.style(Role::AccentSelection),
        );
        assert_eq!(checked.fg, native.style(Role::AccentSelection).fg);
        let expected_bg = interaction
            .and_then(|role| native.style(role).bg)
            .or(native.style(Role::AccentSelection).bg)
            .or(native.style(Role::SurfaceOverlay).bg);
        assert_eq!(checked.bg, expected_bg, "interaction={interaction:?}");
        assert_eq!(
            checked.add_modifier.contains(Modifier::REVERSED),
            interaction.is_some_and(|role| {
                native.style(role).add_modifier.contains(Modifier::REVERSED)
            }),
            "interaction={interaction:?}"
        );
    }

    for theme in [
        Theme::one_light().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::one_dark().quantize(ColorLevel::Monochrome),
    ] {
        for interaction in [Role::SurfaceHover, Role::SurfacePressed] {
            let composed = theme.content_on_surface(
                Role::SurfaceOverlay,
                Some(interaction),
                Some(Role::TextSecondary),
                theme.style(Role::TextSecondary),
            );
            assert_eq!(
                composed.bg,
                theme
                    .style(interaction)
                    .bg
                    .or(theme.style(Role::SurfaceOverlay).bg),
                "theme={theme:?}, interaction={interaction:?}"
            );
            assert_eq!(composed.fg, theme.style(Role::TextSecondary).fg);
            assert!(
                composed
                    .add_modifier
                    .contains(theme.style(interaction).add_modifier)
            );
        }
    }

    let monochrome = Theme::terminal_native().quantize(ColorLevel::Monochrome);
    let pending = monochrome.content_on_surface(
        Role::SurfaceOverlay,
        Some(Role::SurfaceCurrent),
        Some(Role::StatusPending),
        monochrome.style(Role::StatusPending),
    );
    assert!(pending.add_modifier.contains(Modifier::DIM));
}

#[test]
fn terminal_native_quantize_is_identity_outside_fixed_agent_glyphs() {
    for level in [
        ColorLevel::Truecolor,
        ColorLevel::Ansi256,
        ColorLevel::Ansi16,
    ] {
        let quantized = Theme::terminal_native().quantize(level);
        for role in Role::ALL {
            if AGENT_ROLES.iter().any(|(agent, ..)| *agent == role) {
                continue;
            }
            let before = Theme::terminal_native().style(role);
            let after = quantized.style(role);
            assert_eq!(before, after, "{role:?} changed at {level:?}");
        }
    }
}

#[test]
fn agent_state_glyphs_use_one_traffic_light_in_every_theme() {
    use orchbee_workbench_protocol::MetadataState::{Blocked, Done, Error};
    for theme in [
        Theme::terminal_native(),
        Theme::one_light(),
        Theme::one_dark(),
        Theme::rgb_test_dark(),
    ] {
        for (role, rgb, ansi16) in AGENT_ROLES {
            assert_eq!(theme.style(role), Style::new().fg(rgb), "{}", theme.name);
            assert_eq!(theme.quantize(ColorLevel::Ansi16).color(role), ansi16);
            assert_eq!(
                theme.quantize(ColorLevel::Monochrome).color(role),
                Color::Reset
            );
        }
    }
    assert_eq!(Theme::agent_role(Blocked), Role::AgentAttention);
    assert_eq!(Theme::agent_role(Done), Role::AgentDone);
    assert_eq!(Theme::agent_role(Error), Role::AgentError);
    // Status words keep their own text roles; terminal-native warning text stays Magenta.
    assert_eq!(
        Theme::terminal_native().color(Role::StatusWarning),
        Color::Magenta
    );
}

#[test]
fn terminal_native_working_follows_the_host_foreground_and_pairs_on_current() {
    use orchbee_workbench_protocol::MetadataState::{Idle, Working};
    let theme = Theme::terminal_native();
    assert_eq!(
        theme.style(Role::StatusWorking),
        Style::new().fg(Color::Reset)
    );
    for frame in 0..20 {
        let working = theme.content_on_surface(
            Role::SurfaceStrip,
            Some(Role::SurfaceCurrent),
            Some(Theme::agent_role(Working)),
            theme.agent_style(Working, frame),
        );
        assert_eq!(working.fg, theme.style(Role::SurfaceCurrent).fg);
        assert_eq!(working.bg, theme.style(Role::SurfaceCurrent).bg);
    }
    let idle = theme.content_on_surface(
        Role::SurfaceStrip,
        Some(Role::SurfaceCurrent),
        Some(Theme::agent_role(Idle)),
        theme.agent_style(Idle, 0),
    );
    assert_eq!(idle.fg, theme.style(Role::SurfaceCurrent).fg);
    assert!(!idle.add_modifier.contains(Modifier::DIM));
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
    assert_eq!(GlyphSet::UTF8.mode(), GlyphMode::Unicode);
    assert_eq!(GlyphSet::ASCII.mode(), GlyphMode::Simplified);
    assert_eq!(GlyphSet::UTF8.navigation_marker, "›");
    assert_eq!(GlyphSet::ASCII.navigation_marker, ">");
    assert_eq!(
        (
            GlyphSet::ASCII.disclosure_open,
            GlyphSet::ASCII.disclosure_closed
        ),
        ("v", ">")
    );
    for value in [
        GlyphSet::ASCII.ellipsis,
        GlyphSet::ASCII.middle_dot,
        GlyphSet::ASCII.status_bullet,
        GlyphSet::ASCII.workspace,
        GlyphSet::ASCII.active_marker,
        GlyphSet::ASCII.navigation_marker,
        GlyphSet::ASCII.disclosure_open,
        GlyphSet::ASCII.disclosure_closed,
        GlyphSet::ASCII.tree_guide,
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
