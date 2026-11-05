use super::*;
use crate::host_appearance::HostColors;
use crate::tui_theme::ColorPreferences;
use orchbee_workbench_protocol::{
    ApplicationAppearance as App, TerminalDisplayAppearance as Terminal, TuiColorSource,
};

fn preferences(
    application_theme: App,
    tui_color_source: TuiColorSource,
    terminal_display_theme: Terminal,
) -> ColorPreferences<'static> {
    ColorPreferences {
        application_theme,
        light_theme: "orchbee-light",
        dark_theme: "orchbee-dark",
        tui_color_source,
        terminal_display_theme,
        custom_foreground: "#123456",
        custom_background: "#654321",
    }
}

fn rgb(value: u32) -> Color {
    let [_, r, g, b] = value.to_be_bytes();
    Color::Rgb(r, g, b)
}

fn canvas(polarity: orchbee_color_themes::Polarity) -> (Color, Color) {
    let canvas = orchbee_color_themes::orchbee(polarity).canvas();
    (
        rgb(u32::from(canvas.foreground)),
        rgb(u32::from(canvas.background)),
    )
}

/// Theme and Host follow the painted slot when Appearance is fixed; System and Terminal colors
/// defer them to the host. Custom always wins, and no terminal choice changes the chrome.
#[test]
fn terminal_defaults_follow_the_painted_slot_defer_to_the_host_or_take_the_custom_pair() {
    use orchbee_color_themes::Polarity::{Dark, Light};
    let captured = (rgb(0xabcdef), rgb(0x102030));
    let custom = (rgb(0x123456), rgb(0x654321));
    for (application, source, slot) in [
        (App::Light, TuiColorSource::Theme, Some(canvas(Light))),
        (App::Dark, TuiColorSource::Theme, Some(canvas(Dark))),
        (App::System, TuiColorSource::Theme, None),
        (App::Dark, TuiColorSource::Terminal, None),
    ] {
        for terminal in [Terminal::Theme, Terminal::Host, Terminal::Custom] {
            let chrome = Theme::resolve(
                preferences(application, source, Terminal::Theme),
                None,
                ColorLevel::Truecolor,
            );
            let theme = Theme::resolve(
                preferences(application, source, terminal),
                None,
                ColorLevel::Truecolor,
            )
            .with_host_terminal_colors(0xabcdef, 0x102030);
            let expected = match terminal {
                Terminal::Custom => custom,
                Terminal::Theme | Terminal::Host => slot.unwrap_or(captured),
            };
            assert_eq!(
                theme.terminal_defaults(),
                expected,
                "{application:?} {source:?} {terminal:?}"
            );
            assert_eq!(
                theme.style(Role::SurfaceChrome),
                chrome.style(Role::SurfaceChrome)
            );
        }
    }
}

#[test]
fn a_reported_host_polarity_never_replaces_an_explicit_terminal_display_choice() {
    use orchbee_color_themes::Polarity::{Dark, Light};
    let custom = Theme::resolve(
        preferences(App::System, TuiColorSource::Theme, Terminal::Custom),
        Some(Polarity::Light),
        ColorLevel::Truecolor,
    )
    .with_host_terminal_colors(0xabcdef, 0x102030);
    assert_eq!(custom.name, "orchbee-light");
    assert_eq!(custom.terminal_defaults(), (rgb(0x123456), rgb(0x654321)));

    let fixed = Theme::resolve(
        preferences(App::Light, TuiColorSource::Theme, Terminal::Theme),
        Some(Polarity::Dark),
        ColorLevel::Truecolor,
    );
    assert_eq!(fixed.name, "orchbee-light");
    assert_eq!(fixed.terminal_defaults(), canvas(Light));

    let system = Theme::resolve(
        preferences(App::System, TuiColorSource::Theme, Terminal::Theme),
        Some(Polarity::Dark),
        ColorLevel::Truecolor,
    );
    assert_eq!(system.name, "orchbee-dark");
    assert_eq!(system.terminal_defaults(), canvas(Dark));
}

#[test]
fn a_preview_takes_captured_host_defaults_without_mutating_the_confirmed_theme() {
    let confirmed = Theme::resolve(
        preferences(App::Dark, TuiColorSource::Theme, Terminal::Host),
        None,
        ColorLevel::Truecolor,
    );
    let preview = Theme::resolve(
        preferences(App::System, TuiColorSource::Theme, Terminal::Host),
        None,
        ColorLevel::Truecolor,
    )
    .with_host_terminal_colors(0x112233, 0xeeddbb);
    assert_eq!(
        confirmed.terminal_defaults(),
        canvas(orchbee_color_themes::Polarity::Dark)
    );
    assert_eq!(preview.terminal_defaults(), (rgb(0x112233), rgb(0xeeddbb)));
}

/// A deferred default takes the outer terminal's observed colors, never a retained child's
/// launch colors, and keeps emitting the host's own defaults.
#[test]
fn deferred_defaults_take_outer_host_colors_and_never_a_child_baseline() {
    let outer = HostColors {
        foreground: Some(0xcccccc),
        background: Some(0x0c0c0c),
        ..Default::default()
    };
    for (source, terminal, expected) in [
        (
            TuiColorSource::Terminal,
            Terminal::Theme,
            (0xcccccc, 0x0c0c0c),
        ),
        (
            TuiColorSource::Terminal,
            Terminal::Host,
            (0xcccccc, 0x0c0c0c),
        ),
        (
            TuiColorSource::Terminal,
            Terminal::Custom,
            (0x123456, 0x654321),
        ),
        (TuiColorSource::Theme, Terminal::Host, (0xcccccc, 0x0c0c0c)),
    ] {
        let theme = Theme::resolve(
            preferences(App::System, source, terminal),
            Some(Polarity::Dark),
            ColorLevel::Truecolor,
        )
        .with_outer_host_colors(outer)
        .with_host_terminal_colors(0x141414, 0xfcfcfc);
        assert_eq!(theme.terminal_rgb_defaults(), Some(expected));
        if terminal == Terminal::Custom {
            assert_eq!(
                theme.terminal_defaults(),
                (rgb(expected.0), rgb(expected.1))
            );
        } else {
            assert_eq!(theme.terminal_defaults(), (Color::Reset, Color::Reset));
        }
    }
    let unobserved = Theme::resolve(
        preferences(App::System, TuiColorSource::Terminal, Terminal::Host),
        None,
        ColorLevel::Truecolor,
    )
    .with_outer_host_colors(HostColors::default())
    .with_host_terminal_colors(0x141414, 0xfcfcfc);
    assert_eq!(unobserved.terminal_rgb_defaults(), None);
    assert_eq!(unobserved.terminal_defaults(), (Color::Reset, Color::Reset));
}

#[test]
fn terminal_native_working_breathes_through_intensity_on_the_host_foreground() {
    use orchbee_workbench_protocol::MetadataState::Working;
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
