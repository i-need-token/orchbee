use super::*;
use crate::tui_theme::{ColorPreferences, Resolution, TerminalReason, named_themes_available};
use orchbee_workbench_protocol::{
    ApplicationAppearance as App, MetadataState, TerminalDisplayAppearance, TuiColorSource,
};

fn preferences(
    application_theme: App,
    tui_color_source: TuiColorSource,
) -> ColorPreferences<'static> {
    ColorPreferences {
        application_theme,
        light_theme: "dayfox",
        dark_theme: "gruvbox-dark",
        tui_color_source,
        terminal_display_theme: TerminalDisplayAppearance::Theme,
        custom_foreground: "#141414",
        custom_background: "#fcfcfc",
    }
}

const LEVELS: [ColorLevel; 4] = [
    ColorLevel::Truecolor,
    ColorLevel::Ansi256,
    ColorLevel::Ansi16,
    ColorLevel::Monochrome,
];

/// Appearance x host x color level: a named theme needs truecolor and a known polarity, which
/// a fixed Appearance supplies itself and System takes from the host's reported background.
#[test]
fn the_resolution_matrix_paints_the_active_slot_only_on_a_truecolor_host_of_known_appearance() {
    for application in [App::System, App::Light, App::Dark] {
        for host in [None, Some(Polarity::Light), Some(Polarity::Dark)] {
            for level in LEVELS {
                for source in [TuiColorSource::Theme, TuiColorSource::Terminal] {
                    let resolution = Resolution::of(preferences(application, source), host, level);
                    let polarity = match application {
                        App::Light => Some(Polarity::Light),
                        App::Dark => Some(Polarity::Dark),
                        App::System => host,
                    };
                    let expected = if level != ColorLevel::Truecolor {
                        Resolution::Terminal(TerminalReason::ReducedColor)
                    } else if source == TuiColorSource::Terminal {
                        Resolution::Terminal(TerminalReason::Chosen)
                    } else {
                        match polarity {
                            None => Resolution::Terminal(TerminalReason::UnknownAppearance),
                            Some(Polarity::Light) => {
                                Resolution::Theme(orchbee_color_themes::select(
                                    orchbee_color_themes::Polarity::Light,
                                    "dayfox",
                                ))
                            }
                            Some(Polarity::Dark) => {
                                Resolution::Theme(orchbee_color_themes::select(
                                    orchbee_color_themes::Polarity::Dark,
                                    "gruvbox-dark",
                                ))
                            }
                        }
                    };
                    assert_eq!(
                        resolution, expected,
                        "{application:?} {host:?} {level:?} {source:?}"
                    );
                    let theme = Theme::resolve(preferences(application, source), host, level);
                    match expected {
                        Resolution::Theme(named) => {
                            assert_eq!(theme.name, named.id());
                            assert_eq!(theme.color_level, ColorLevel::Truecolor);
                        }
                        Resolution::Terminal(_) => assert_eq!(theme.name, "terminal-native"),
                    }
                }
            }
        }
    }
}

#[test]
fn the_truecolor_gate_admits_named_themes_only_at_24_bit_color() {
    for level in LEVELS {
        assert_eq!(
            named_themes_available(level),
            level == ColorLevel::Truecolor
        );
    }
}

#[test]
fn a_slot_id_this_build_does_not_ship_or_of_the_other_polarity_paints_the_orchbee_theme() {
    for (light, dark) in [("missing", "missing"), ("gruvbox-dark", "dayfox")] {
        let mut stored = preferences(App::Light, TuiColorSource::Theme);
        stored.light_theme = light;
        stored.dark_theme = dark;
        let light = Theme::resolve(stored, None, ColorLevel::Truecolor);
        assert_eq!(light.name, "orchbee-light");
        stored.application_theme = App::Dark;
        let dark = Theme::resolve(stored, None, ColorLevel::Truecolor);
        assert_eq!(dark.name, "orchbee-dark");
    }
}

/// Terminal colors keep the host's own defaults: chrome emits Reset and ANSI indexes, and only the
/// fixed Agent traffic light carries exact colors.
#[test]
fn terminal_colors_emit_host_defaults_and_ansi_indexes_beside_the_fixed_agent_colors() {
    let theme = Theme::resolve(
        preferences(App::Dark, TuiColorSource::Terminal),
        Some(Polarity::Dark),
        ColorLevel::Truecolor,
    );
    for role in Role::ALL {
        if matches!(
            role,
            Role::AgentAttention | Role::AgentDone | Role::AgentError
        ) {
            continue;
        }
        let style = theme.style(role);
        for color in [style.fg, style.bg].into_iter().flatten() {
            assert!(!matches!(color, Color::Rgb(..)), "{role:?} {color:?}");
        }
    }
    assert_eq!(theme.color(Role::SurfaceCanvas), Color::Reset);
}

/// Agent state colors are a fixed traffic light: no theme, polarity or color source moves them.
#[test]
fn agent_status_colors_are_identical_under_every_shipped_theme_and_terminal_colors() {
    let reference = Theme::terminal_native();
    for named in orchbee_color_themes::shipped() {
        let theme = Theme::named(named);
        for role in [Role::AgentAttention, Role::AgentDone, Role::AgentError] {
            assert_eq!(
                theme.color(role),
                reference.color(role),
                "{} {role:?}",
                named.id()
            );
        }
        for level in LEVELS {
            let (theme, reference) = (theme.quantize(level), reference.quantize(level));
            for state in [
                MetadataState::Blocked,
                MetadataState::Done,
                MetadataState::Error,
            ] {
                assert_eq!(
                    theme.agent_style(state, 0).fg,
                    reference.agent_style(state, 0).fg,
                    "{} {state:?} {level:?}",
                    named.id()
                );
            }
        }
    }
}
