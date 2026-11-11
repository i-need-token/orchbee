use super::*;

#[test]
fn theme_and_host_take_the_active_slot_only_under_explicit_appearance_and_custom_always_wins() {
    use ApplicationAppearance as App;
    use TerminalDisplayAppearance as Terminal;
    let themes = UserColorThemes::default();
    for application in [App::System, App::Light, App::Dark] {
        for terminal in [Terminal::Theme, Terminal::Host, Terminal::Custom] {
            let expected = match (terminal, application) {
                (Terminal::Custom, _) => Some(((0x123456, 0xabcdef), false)),
                (_, App::System) => None,
                (_, App::Light) => Some(((0x141414, 0xfcfcfc), false)),
                (_, App::Dark) => Some(((0xd4d4d4, 0x1e1e1e), true)),
            };
            let resolved =
                UserTerminalColors::resolve(application, terminal, &themes, "#123456", "#abcdef")
                    .unwrap();
            assert_eq!(
                resolved.map(|colors| (colors.rgb(), colors.dark())),
                expected,
                "{application:?} {terminal:?}"
            );
        }
    }
}

#[test]
fn an_unshipped_or_wrong_polarity_theme_id_falls_back_to_the_orchbee_theme_of_the_slot() {
    for (light, dark) in [
        ("a-theme-from-a-newer-client", "withdrawn"),
        ("orchbee-dark", "orchbee-light"),
    ] {
        let themes = UserColorThemes::new(light.into(), dark.into());
        for (application, polarity) in [
            (ApplicationAppearance::Light, Polarity::Light),
            (ApplicationAppearance::Dark, Polarity::Dark),
        ] {
            let orchbee = orchbee_color_themes::orchbee(polarity);
            assert_eq!(themes.theme(polarity).id(), orchbee.id());
            let resolved = UserTerminalColors::resolve(
                application,
                TerminalDisplayAppearance::Theme,
                &themes,
                "#000000",
                "#ffffff",
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                resolved.rgb(),
                (
                    u32::from(orchbee.canvas().foreground),
                    u32::from(orchbee.canvas().background)
                )
            );
            assert_eq!(resolved.polarity(), polarity);
        }
    }
}

#[test]
fn invalid_custom_rgb_is_refused_only_when_custom_is_effective() {
    let themes = UserColorThemes::default();
    for invalid in [
        "",
        "#12345",
        "#1234567",
        "123456",
        "#12x456",
        "#+12345",
        "#-12345",
        "#\u{1f4a1}00",
    ] {
        for (foreground, background) in [(invalid, "#ffffff"), ("#000000", invalid)] {
            assert_eq!(
                UserTerminalColors::resolve(
                    ApplicationAppearance::Dark,
                    TerminalDisplayAppearance::Custom,
                    &themes,
                    foreground,
                    background,
                ),
                Err(PreferencesFailure::InvalidSource)
            );
            assert!(
                UserTerminalColors::resolve(
                    ApplicationAppearance::Dark,
                    TerminalDisplayAppearance::Theme,
                    &themes,
                    foreground,
                    background,
                )
                .unwrap()
                .is_some()
            );
        }
    }
}
