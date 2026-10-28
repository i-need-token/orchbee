use super::*;
use orchbee_workbench_protocol::{
    ApplicationAppearance as App, TerminalDisplayAppearance as Terminal,
};

#[test]
fn host_defaults_follow_resolved_application_appearance_without_replacing_explicit_choices() {
    for (application, host_pair) in [
        (App::Light, (0x141414, 0xfcfcfc)),
        (App::Dark, (0xd4d4d4, 0x1e1e1e)),
        (App::System, (0xabcdef, 0x102030)),
    ] {
        for (terminal, explicit) in [
            (Terminal::Host, None),
            (Terminal::Light, Some((0x141414, 0xfcfcfc))),
            (Terminal::Dark, Some((0xd4d4d4, 0x1e1e1e))),
            (Terminal::Custom, Some((0x123456, 0x654321))),
        ] {
            let chrome = Theme::application_with_host(
                application,
                crate::terminal_host_appearance::HostAppearance::Unknown,
                ColorLevel::Truecolor,
            );
            let theme = chrome
                .with_terminal_appearance_for_application(
                    application,
                    terminal,
                    "#123456",
                    "#654321",
                )
                .with_host_terminal_colors(0xabcdef, 0x102030);
            let color = |rgb: u32| Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
            let (foreground, background) = explicit.unwrap_or(host_pair);
            assert_eq!(
                theme.terminal_defaults(),
                (color(foreground), color(background))
            );
            assert_eq!(
                theme.style(Role::SurfaceChrome),
                chrome.style(Role::SurfaceChrome)
            );
        }
    }
}

#[test]
fn a_system_preview_uses_captured_host_defaults_without_mutating_the_confirmed_theme() {
    let confirmed = Theme::one_dark().with_terminal_appearance_for_application(
        App::Dark,
        Terminal::Host,
        "#141414",
        "#fcfcfc",
    );
    let preview = Theme::terminal_native()
        .with_terminal_appearance_for_application(App::System, Terminal::Host, "#141414", "#fcfcfc")
        .with_host_terminal_colors(0x112233, 0xeeddbb);
    assert_eq!(
        confirmed.terminal_defaults(),
        (Color::Rgb(212, 212, 212), Color::Rgb(30, 30, 30))
    );
    assert_eq!(
        preview.terminal_defaults(),
        (Color::Rgb(17, 34, 51), Color::Rgb(238, 221, 187))
    );
}

#[test]
fn detected_system_polarity_changes_chrome_but_host_terminal_defaults_stay_captured() {
    use crate::terminal_host_appearance::HostAppearance;

    let host_foreground = Color::Rgb(0x12, 0x34, 0x56);
    let host_background = Color::Rgb(0x20, 0x30, 0x40);
    let theme =
        Theme::application_with_host(App::System, HostAppearance::Dark, ColorLevel::Truecolor)
            .with_terminal_appearance_for_application(
                App::System,
                Terminal::Host,
                "#141414",
                "#fcfcfc",
            )
            .with_host_terminal_colors(0x123456, 0x203040);

    assert_eq!(theme.name, "one-dark");
    assert_eq!(
        theme.terminal_defaults(),
        (host_foreground, host_background)
    );
}

#[test]
fn detected_host_polarity_does_not_replace_explicit_terminal_display_choices() {
    use crate::terminal_host_appearance::HostAppearance;

    let custom =
        Theme::application_with_host(App::System, HostAppearance::Light, ColorLevel::Truecolor)
            .with_terminal_appearance_for_application(
                App::System,
                Terminal::Custom,
                "#123456",
                "#654321",
            )
            .with_host_terminal_colors(0xabcdef, 0x102030);
    assert_eq!(custom.name, "one-light");
    assert_eq!(
        custom.terminal_defaults(),
        (Color::Rgb(0x12, 0x34, 0x56), Color::Rgb(0x65, 0x43, 0x21))
    );

    let explicit =
        Theme::application_with_host(App::Light, HostAppearance::Dark, ColorLevel::Truecolor)
            .with_terminal_appearance_for_application(
                App::Light,
                Terminal::Dark,
                "#123456",
                "#654321",
            );
    assert_eq!(explicit.name, "one-light");
    assert_eq!(
        explicit.terminal_defaults(),
        (Color::Rgb(0xd4, 0xd4, 0xd4), Color::Rgb(0x1e, 0x1e, 0x1e))
    );
}

#[test]
fn outer_host_defaults_replace_child_baselines_without_replacing_explicit_preferences() {
    use crate::terminal_host_appearance::{HostAppearance, HostColors};
    for (terminal, expected) in [
        (Terminal::Host, (0xcccccc, 0x0c0c0c)),
        (Terminal::Light, (0x141414, 0xfcfcfc)),
        (Terminal::Dark, (0xd4d4d4, 0x1e1e1e)),
        (Terminal::Custom, (0x123456, 0x654321)),
    ] {
        let theme =
            Theme::application_with_host(App::System, HostAppearance::Dark, ColorLevel::Truecolor)
                .with_terminal_appearance_for_application(
                    App::System,
                    terminal,
                    "#123456",
                    "#654321",
                )
                .with_outer_host_colors(
                    App::System,
                    HostColors {
                        foreground: Some(0xcccccc),
                        background: Some(0x0c0c0c),
                    },
                )
                .with_host_terminal_colors(0x141414, 0xfcfcfc);
        assert_eq!(theme.terminal_rgb_defaults(), Some(expected));
        if terminal == Terminal::Host {
            assert_eq!(theme.terminal_defaults(), (Color::Reset, Color::Reset));
        } else {
            assert_eq!(
                theme.terminal_defaults(),
                (
                    theme.terminal_color(expected.0),
                    theme.terminal_color(expected.1)
                )
            );
        }
        for role in [
            Role::SurfaceCanvas,
            Role::SurfaceChrome,
            Role::SurfaceInactive,
        ] {
            assert_eq!(theme.color(role), Color::Reset);
        }
    }
}

#[test]
fn system_defaults_remain_host_relative_without_a_probe_or_exact_color_support() {
    use crate::terminal_host_appearance::{HostAppearance, HostColors};
    for (appearance, colors) in [
        (HostAppearance::Unknown, HostColors::default()),
        (HostAppearance::Light, HostColors::default()),
        (HostAppearance::Dark, HostColors::default()),
        (
            HostAppearance::Light,
            HostColors {
                foreground: Some(0x141414),
                background: None,
            },
        ),
        (
            HostAppearance::Dark,
            HostColors {
                foreground: None,
                background: Some(0x0c0c0c),
            },
        ),
        (
            HostAppearance::Dark,
            HostColors {
                foreground: Some(0xcccccc),
                background: Some(0x0c0c0c),
            },
        ),
        (
            HostAppearance::Light,
            HostColors {
                foreground: Some(0x141414),
                background: Some(0xfcfcfc),
            },
        ),
    ] {
        for level in [
            ColorLevel::Truecolor,
            ColorLevel::Ansi256,
            ColorLevel::Ansi16,
            ColorLevel::Monochrome,
        ] {
            let theme = Theme::application_with_host(App::System, appearance, level)
                .with_terminal_appearance_for_application(
                    App::System,
                    Terminal::Host,
                    "#141414",
                    "#fcfcfc",
                )
                .with_outer_host_colors(App::System, colors)
                .with_host_terminal_colors(0x141414, 0xfcfcfc);
            assert_eq!(theme.terminal_defaults(), (Color::Reset, Color::Reset));
            for role in [
                Role::SurfaceCanvas,
                Role::SurfaceChrome,
                Role::SurfaceInactive,
            ] {
                assert_eq!(
                    theme.color(role),
                    Color::Reset,
                    "{appearance:?} {colors:?} {level:?}"
                );
            }
            if colors.background.is_none() {
                assert_eq!(theme.name, "terminal-native");
                assert_eq!(theme.color(Role::TextPrimary), Color::Reset);
                assert_eq!(theme.terminal_rgb_defaults(), None);
            }
        }
    }
}

#[test]
fn host_default_backgrounds_keep_working_breath_smooth_and_unknown_static() {
    use crate::terminal_host_appearance::{HostAppearance, HostColors};
    use orchbee_workbench_protocol::MetadataState::{Unknown, Working};

    for (appearance, foreground, background) in [
        (HostAppearance::Dark, 0xcccccc, 0x0c0c0c),
        (HostAppearance::Light, 0x141414, 0xfcfcfc),
    ] {
        let theme = Theme::application_with_host(App::System, appearance, ColorLevel::Truecolor)
            .with_terminal_appearance_for_application(
                App::System,
                Terminal::Host,
                "#141414",
                "#fcfcfc",
            )
            .with_outer_host_colors(
                App::System,
                HostColors {
                    foreground: Some(foreground),
                    background: Some(background),
                },
            );
        assert_eq!(theme.color(Role::SurfaceCanvas), Color::Reset);
        let colors: Vec<_> = [0, 3, 7, 10]
            .map(|frame| theme.agent_style(Working, frame))
            .into_iter()
            .map(|style| {
                assert!(style.bg.is_none());
                assert!(!style.add_modifier.contains(Modifier::DIM));
                let Some(Color::Rgb(r, _, _)) = style.fg else {
                    panic!("truecolor Working must retain interpolated RGB");
                };
                r
            })
            .collect();
        assert!(
            colors
                .windows(2)
                .all(|pair| if appearance == HostAppearance::Dark {
                    pair[0] > pair[1]
                } else {
                    pair[0] < pair[1]
                })
        );
        assert_eq!(
            theme.agent_style(Working, 0),
            theme.agent_style(Working, 20)
        );
        assert_eq!(theme.agent_style(Unknown, 0), theme.agent_style(Unknown, 7));
    }

    // Terminal-native Working keeps the host foreground and breathes through intensity.
    let native = Theme::terminal_native();
    assert_eq!(native.agent_style(Working, 3).fg, Some(Color::Reset));
    assert_eq!(native.agent_style(Working, 7).fg, Some(Color::Reset));
    assert_ne!(
        native.agent_style(Working, 3),
        native.agent_style(Working, 7)
    );
    for level in [
        ColorLevel::Ansi16,
        ColorLevel::Ansi256,
        ColorLevel::Monochrome,
    ] {
        let theme = native.quantize(level);
        assert!(
            (0..20)
                .all(|frame| !matches!(theme.agent_style(Working, frame).fg, Some(Color::Rgb(..))))
        );
    }
}

#[test]
fn system_derives_strip_and_interaction_fills_from_observed_host_colors() {
    use crate::terminal_host_appearance::{HostAppearance, HostColors};
    use orchbee_workbench_protocol::TabGroupColor;
    let system = |appearance, level, foreground, background| {
        Theme::application_with_host(App::System, appearance, level).with_outer_host_colors(
            App::System,
            HostColors {
                foreground,
                background: Some(background),
            },
        )
    };
    // Strip, related, hover, current and pressed mix 14/10/17/21/28% (dark) or
    // 5/7/9/12/16% (light) of the host foreground into its background.
    for (appearance, foreground, background, fills, palette) in [
        (
            HostAppearance::Dark,
            0xe5e5e5,
            0x000000,
            [32, 23, 39, 48, 64],
            Theme::one_dark(),
        ),
        (
            HostAppearance::Light,
            0x000000,
            0xffffff,
            [242, 237, 232, 224, 214],
            Theme::one_light(),
        ),
    ] {
        for level in [
            ColorLevel::Truecolor,
            ColorLevel::Ansi256,
            ColorLevel::Ansi16,
        ] {
            let theme = system(appearance, level, Some(foreground), background);
            for (role, gray) in [
                Role::SurfaceStrip,
                Role::SurfaceRelated,
                Role::SurfaceHover,
                Role::SurfaceCurrent,
                Role::SurfacePressed,
            ]
            .into_iter()
            .zip(fills)
            {
                let derived = quantize_color(Color::Rgb(gray, gray, gray), level);
                let collapsed = matches!(role, Role::SurfaceHover | Role::SurfacePressed)
                    && theme.style(role) == crate::tui_theme::interaction_surface_fallback();
                assert!(
                    theme.color(role) == derived || collapsed,
                    "{appearance:?} {level:?} {role:?}"
                );
            }
            assert_eq!(
                theme.style(Role::SurfaceStrip).fg,
                palette.quantize(level).style(Role::TextPrimary).fg
            );
            assert_eq!(
                theme.style(Role::SurfaceSelection),
                palette.quantize(level).style(Role::SurfaceSelection)
            );
        }
        // A resting group header tints the derived strip.
        let theme = system(
            appearance,
            ColorLevel::Truecolor,
            Some(foreground),
            background,
        );
        let strip = fills[0];
        let Color::Rgb(r, g, b) = theme.tab_groups.accent(TabGroupColor::Blue) else {
            panic!("named accents stay RGB at truecolor");
        };
        let tint = |accent: u8| (f64::from(accent) * 0.2 + f64::from(strip) * 0.8).round() as u8;
        assert_eq!(
            theme.tab_groups.tint(TabGroupColor::Blue),
            Some(Color::Rgb(tint(r), tint(g), tint(b)))
        );
    }

    // Until the host foreground arrives, the palette's primary text stands in for it.
    let Color::Rgb(r, g, b) = Theme::one_dark().color(Role::TextPrimary) else {
        panic!("One Dark text is RGB");
    };
    let mix = |channel: u8| (f64::from(channel) * 0.14).round() as u8;
    assert_eq!(
        system(HostAppearance::Dark, ColorLevel::Truecolor, None, 0x000000)
            .color(Role::SurfaceStrip),
        Color::Rgb(mix(r), mix(g), mix(b))
    );

    // Monochrome keeps the resolver's modifier fallbacks.
    let mono = system(
        HostAppearance::Dark,
        ColorLevel::Monochrome,
        Some(0xe5e5e5),
        0,
    );
    assert_eq!(
        mono.style(Role::SurfaceHover),
        crate::tui_theme::interaction_surface_fallback()
    );
    assert_eq!(mono.color(Role::SurfaceStrip), Color::Reset);
}
