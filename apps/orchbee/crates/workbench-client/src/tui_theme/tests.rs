use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BorderType, Paragraph},
};

use super::{ColorLevel, GlyphMode, GlyphSet, Polarity, Role, Theme, quantize_color};

mod contrast_pair;
mod cursor;
mod source;
mod terminal_defaults;

#[test]
fn sidebar_pointer_fallbacks_keep_one_background_and_readable_navigation() {
    for base in [
        Theme::terminal_native(),
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
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
                if band.fg.is_some() {
                    // The reverse band is primary text on chrome: the host's own pair in
                    // terminal-native and monochrome, as on the Tab strip.
                    assert_eq!(band.fg, theme.style(Role::TextPrimary).fg);
                    assert_eq!(band.bg, theme.style(Role::SurfaceChrome).bg);
                    assert!(band.add_modifier.contains(Modifier::REVERSED));
                }
                for role in [
                    Role::TextPrimary,
                    Role::TextSecondary,
                    Role::TextMuted,
                    Role::TextDisabled,
                    Role::HintLabel,
                    Role::AccentAction,
                    Role::AccentFocus,
                    Role::AccentSelection,
                    Role::StatusWorking,
                ] {
                    let style = theme.style(role);
                    let content = theme.content_on_surface(
                        Role::SurfaceChrome,
                        Some(surface),
                        Some(role),
                        style,
                    );
                    let case = format!("{} {level:?} {surface:?} {role:?}", base.name);
                    if role == Role::StatusWorking
                        && band.fg.is_some()
                        && level != ColorLevel::Monochrome
                        && !matches!(style.fg, None | Some(Color::Reset))
                    {
                        // A Working hue keeps its color on the contrast pair, like other hues.
                        assert_eq!(content.fg, style.fg, "{case}");
                        assert!(
                            matches!(content.bg, Some(Color::Black | Color::White)),
                            "{case}"
                        );
                        assert!(!content.add_modifier.contains(Modifier::REVERSED), "{case}");
                        continue;
                    }
                    assert_eq!(content.bg, band.bg, "{case}");
                    assert_eq!(
                        content.add_modifier.contains(Modifier::REVERSED),
                        band.add_modifier.contains(Modifier::REVERSED),
                        "{case}"
                    );
                    if let Some(foreground) = band.fg {
                        assert_eq!(content.fg, Some(foreground), "{case}");
                        // A host may apply DIM before reverse video swaps the pair, which would
                        // give the token a darker background of its own.
                        assert!(!content.add_modifier.contains(Modifier::DIM), "{case}");
                    }
                }
            }
        }
    }
}

#[test]
fn paired_pointer_projection_preserves_other_roles_and_explicit_colors_outside_the_band() {
    for original in [
        Theme::terminal_native(),
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
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
            let band = theme.style(surface);
            let explicit = theme.style(Role::TextPrimary).fg(Color::Rgb(16, 32, 48));
            for role in [None, Some(Role::TextPrimary)] {
                let composed =
                    theme.content_on_surface(Role::SurfaceOverlay, Some(surface), role, explicit);
                if band.add_modifier.contains(Modifier::REVERSED) {
                    // An explicit color joins the reverse band instead of reversing into a block
                    // of its own color.
                    assert_eq!(composed.fg, band.fg);
                    assert!(composed.add_modifier.contains(Modifier::REVERSED));
                } else {
                    assert_eq!(composed.fg, explicit.fg);
                }
            }
        }
    }
}

/// The colors both Clients' Light and Dark themes share. The CSS registry is checked against the
/// same file, and neither Client reads the other's source.
const SHARED_THEME_COLORS: &str =
    include_str!("../../../../../../schemas/workbench/v1/shared-theme-colors-v1.json");

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SharedThemeColors {
    contract_version: u8,
    every_theme: std::collections::BTreeMap<String, SharedColor>,
    light: std::collections::BTreeMap<String, SharedColor>,
    dark: std::collections::BTreeMap<String, SharedColor>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedColor {
    rgb: String,
    /// A translucent color lies over the surface beneath it.
    opacity: Option<u16>,
}

fn shared_color_role(name: &str) -> Role {
    match name {
        "action" => Role::AccentAction,
        "hover" => Role::SurfaceHover,
        "press" => Role::SurfacePressed,
        "agentAttention" => Role::AgentAttention,
        "agentDone" => Role::AgentDone,
        "agentError" => Role::AgentError,
        "agentIdle" => Role::StatusIdle,
        "agentWorking" => Role::StatusWorking,
        _ => panic!("shared color {name} has no native role"),
    }
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
fn named_roles_conform_to_the_shared_theme_colors() {
    let shared: SharedThemeColors = serde_json::from_str(SHARED_THEME_COLORS).unwrap();
    assert_eq!(shared.contract_version, 1);
    for (theme, colors) in [
        (Theme::orchbee_light(), &shared.light),
        (Theme::orchbee_dark(), &shared.dark),
    ] {
        // The cell grid has no translucency, so a translucent color is composited over chrome.
        let chrome = rgb_channels(theme.color(Role::SurfaceChrome));
        for (name, color) in shared.every_theme.iter().chain(colors) {
            let rgb = parse_hex_rgb(&color.rgb);
            let expected = color
                .opacity
                .map_or(rgb, |opacity| composite_rgb(rgb, chrome, opacity));
            assert_eq!(
                rgb_channels(theme.color(shared_color_role(name))),
                expected,
                "{} {name}",
                theme.name
            );
        }
    }
}

#[test]
fn named_hint_labels_and_pending_status_are_secondary_text_below_primary() {
    for theme in orchbee_color_themes::shipped().iter().map(Theme::named) {
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

/// Muted text is the quietest step from secondary toward chrome that still reads there; on any
/// fill it is secondary text.
#[test]
fn named_muted_text_is_the_quietest_readable_step_and_rises_to_secondary_on_fills() {
    for theme in [Theme::orchbee_light(), Theme::orchbee_dark()] {
        let chrome = theme.color(Role::SurfaceChrome);
        let resting = theme.color(Role::TextMuted);
        let contrast = contrast_ratio(resting, chrome);
        assert!((4.5..4.6).contains(&contrast), "{} {contrast}", theme.name);
        assert!(contrast < contrast_ratio(theme.color(Role::TextSecondary), chrome));
        for fill in [
            Role::SurfaceHover,
            Role::SurfacePressed,
            Role::SurfaceCurrent,
            Role::SurfaceRelated,
        ] {
            let content = theme.content_on_surface(
                Role::SurfaceChrome,
                Some(fill),
                Some(Role::TextMuted),
                theme.style(Role::TextMuted),
            );
            assert_eq!(
                content.fg,
                theme.style(Role::TextSecondary).fg,
                "{} {fill:?}",
                theme.name
            );
        }
    }
}

/// Every shipped theme maps to a complete role set of its declared polarity. Text clears 4.5:1
/// on the surfaces it rests on, and accent and status colors clear the resolver's floors on chrome.
#[test]
fn shipped_palettes_have_declared_polarity_complete_roles_and_visible_text() {
    for named in orchbee_color_themes::shipped() {
        let theme = Theme::named(named);
        let name = named.id();
        let polarity = match named.polarity() {
            orchbee_color_themes::Polarity::Light => Polarity::Light,
            orchbee_color_themes::Polarity::Dark => Polarity::Dark,
        };
        assert_eq!(theme.name, name);
        assert_eq!(theme.polarity, Some(polarity));
        let background = relative_luminance(theme.color(Role::SurfaceCanvas));
        assert_eq!(background > 0.5, polarity == Polarity::Light, "{name}");
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
            Role::SurfaceInactive,
        ] {
            for role in [Role::TextPrimary, Role::TextSecondary, Role::HintLabel] {
                let contrast = contrast_ratio(theme.color(role), theme.color(surface));
                assert!(contrast >= 4.5, "{name} {role:?}/{surface:?}: {contrast}");
            }
        }
        let chrome = theme.color(Role::SurfaceChrome);
        let action_contrast = contrast_ratio(theme.color(Role::AccentAction), chrome);
        assert!(
            action_contrast >= 3.0,
            "{name} accent-action: {action_contrast}"
        );
        for role in [
            Role::StatusSuccess,
            Role::StatusWarning,
            Role::StatusError,
            Role::StatusInfo,
            Role::StatusPending,
            Role::StatusUnread,
        ] {
            let contrast = contrast_ratio(theme.color(role), chrome);
            assert!(contrast >= 3.0, "{name} {role:?}: {contrast}");
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
    }
}

#[test]
fn shipped_disabled_text_stays_quieter_than_primary_without_faint_inversion() {
    for theme in [Theme::orchbee_light(), Theme::orchbee_dark()] {
        for level in [ColorLevel::Truecolor, ColorLevel::Ansi256] {
            let theme = theme.quantize(level);
            let disabled = theme.style(Role::TextDisabled);
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
    for theme in [Theme::orchbee_light(), Theme::orchbee_dark()] {
        for level in [ColorLevel::Ansi256, ColorLevel::Ansi16] {
            assert_interaction_fills_or_fallback(theme, level);
        }
    }
}

#[test]
fn shipped_low_color_and_monochrome_preserve_readable_hierarchy() {
    for theme in [
        Theme::terminal_native(),
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
    ] {
        // Shipped fills keep the current row distinct, so no underline joins them.
        assert_eq!(
            theme
                .underline_collapsed_current()
                .style(Role::SurfaceCurrent),
            theme.style(Role::SurfaceCurrent)
        );
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
        // Monochrome has no fill to mark the current row: underline marks it, never bold.
        let current = mono
            .underline_collapsed_current()
            .style(Role::SurfaceCurrent);
        assert!(current.add_modifier.contains(Modifier::UNDERLINED));
        assert!(!current.add_modifier.contains(Modifier::BOLD));
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
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
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
    for theme in [Theme::orchbee_light(), Theme::orchbee_dark()] {
        for (base, interaction) in [
            (Role::SurfaceCanvas, None),
            (Role::SurfaceChrome, None),
            (Role::SurfaceOverlay, None),
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
        Theme::orchbee_light().quantize(ColorLevel::Ansi16),
        Theme::orchbee_dark().quantize(ColorLevel::Ansi16),
        Theme::orchbee_dark().quantize(ColorLevel::Monochrome),
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
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
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
fn every_agent_state_glyph_keeps_one_cell_in_every_glyph_set_and_color_level() {
    use orchbee_workbench_protocol::MetadataState::{Blocked, Done, Error, Idle, Unknown, Working};
    // State glyphs sit in a fixed one-cell column beside names; a wider glyph would shift them.
    for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
        for theme in [
            Theme::orchbee_dark(),
            Theme::terminal_native().quantize(ColorLevel::Monochrome),
            Theme::terminal_native().with_no_color(true),
        ] {
            let theme = theme.with_glyphs(glyphs);
            for state in [Working, Blocked, Done, Error, Idle, Unknown] {
                let glyph = theme.agent_glyph(state);
                assert_eq!(crate::terminal_text::display_width(glyph), 1, "{glyph}");
            }
            for frame in glyphs.working_frames {
                assert_eq!(crate::terminal_text::display_width(frame), 1, "{frame}");
            }
        }
    }
    // Filled and hollow hexagons keep the filled and hollow meanings where color shows.
    let theme = Theme::orchbee_dark();
    assert_eq!(theme.agent_glyph(Working), "⬢");
    assert_eq!(theme.agent_glyph(Done), "⬢");
    assert_eq!(theme.agent_glyph(Idle), "⬡");
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
        GlyphSet::ASCII.active_marker,
        GlyphSet::ASCII.checked_marker,
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

#[test]
fn form_fields_band_only_where_the_field_fill_stays_distinct_from_its_dialog() {
    for theme in [
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
        Theme::rgb_test_dark(),
    ] {
        assert!(theme.field_band(), "{}", theme.name);
        assert_ne!(
            theme.color(Role::SurfaceField),
            theme.color(Role::SurfaceOverlay),
            "{}",
            theme.name
        );
        assert!(!theme.quantize(ColorLevel::Monochrome).field_band());
    }
    assert!(!Theme::terminal_native().field_band());
    for level in [ColorLevel::Ansi16, ColorLevel::Ansi256] {
        for theme in [Theme::orchbee_light(), Theme::orchbee_dark()] {
            // A band survives quantization only while its fill stays distinct from the dialog.
            let distinct = super::quantize_color(theme.color(Role::SurfaceField), level)
                != super::quantize_color(theme.color(Role::SurfaceOverlay), level);
            let quantized = theme.quantize(level);
            assert_eq!(quantized.field_band(), distinct, "{} {level:?}", theme.name);
            if !distinct {
                assert_eq!(quantized.color(Role::SurfaceField), Color::Reset);
            }
        }
    }
}

#[test]
fn selected_named_text_keeps_contrast_and_primary_hierarchy() {
    for palette in orchbee_color_themes::shipped() {
        let theme = Theme::named(palette);
        let mut ratios = Vec::new();
        for role in [Role::TextPrimary, Role::TextSecondary] {
            let style = theme.content_on_surface(
                Role::SurfaceChrome,
                Some(Role::SurfaceSelection),
                Some(role),
                theme.style(role),
            );
            let ratio = contrast_ratio(style.fg.unwrap(), style.bg.unwrap());
            assert!(ratio >= 4.5, "{} {role:?}: {ratio}", palette.id());
            ratios.push(ratio);
        }
        assert!(
            ratios[0] >= ratios[1],
            "{} selected hierarchy",
            palette.id()
        );
    }
}
