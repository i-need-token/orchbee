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
