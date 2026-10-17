use super::*;

#[test]
fn host_follows_only_explicit_application_appearance_and_terminal_choices_win() {
    use ApplicationAppearance as App;
    use TerminalDisplayAppearance as Terminal;
    for application in [App::System, App::Light, App::Dark] {
        for (terminal, explicit) in [
            (Terminal::Host, None),
            (Terminal::Light, Some(((0x141414, 0xfcfcfc), Some(false)))),
            (Terminal::Dark, Some(((0xd4d4d4, 0x1e1e1e), Some(true)))),
            (Terminal::Custom, Some(((0x123456, 0xabcdef), None))),
        ] {
            let expected = explicit.or(match application {
                App::System => None,
                App::Light => Some(((0x141414, 0xfcfcfc), Some(false))),
                App::Dark => Some(((0xd4d4d4, 0x1e1e1e), Some(true))),
            });
            let resolved =
                UserTerminalColors::resolve(application, terminal, "#123456", "#abcdef").unwrap();
            assert_eq!(
                resolved.map(|colors| (colors.rgb(), colors.dark())),
                expected,
                "{application:?} {terminal:?}"
            );
        }
    }
}

#[test]
fn invalid_custom_rgb_is_refused_only_when_custom_is_effective() {
    for invalid in [
        "", "#12345", "#1234567", "123456", "#12x456", "#+12345", "#-12345", "#💡00",
    ] {
        for (foreground, background) in [(invalid, "#ffffff"), ("#000000", invalid)] {
            assert_eq!(
                UserTerminalColors::resolve(
                    ApplicationAppearance::Dark,
                    TerminalDisplayAppearance::Custom,
                    foreground,
                    background,
                ),
                Err(PreferencesFailure::InvalidSource)
            );
            assert!(
                UserTerminalColors::resolve(
                    ApplicationAppearance::Dark,
                    TerminalDisplayAppearance::Host,
                    foreground,
                    background,
                )
                .unwrap()
                .is_some()
            );
        }
    }
}
