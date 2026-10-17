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
            let chrome = Theme::application(application, ColorLevel::Truecolor);
            let theme = chrome
                .with_terminal_appearance(terminal, "#123456", "#654321")
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
    let confirmed =
        Theme::one_dark().with_terminal_appearance(Terminal::Host, "#141414", "#fcfcfc");
    let preview = Theme::terminal_native()
        .with_terminal_appearance(Terminal::Host, "#141414", "#fcfcfc")
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
