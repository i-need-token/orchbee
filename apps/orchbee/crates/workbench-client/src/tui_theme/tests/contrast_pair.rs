use super::*;

// Reference xterm palette values are an independent contrast oracle. An actual host may customize
// these slots, so real-host acceptance remains required in addition to this nominal-palette proof.
fn nominal_rgb(color: Color) -> Color {
    let indexed = match color {
        Color::Rgb(..) => return color,
        Color::Indexed(index) => index,
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Reset => panic!("destructive label must have a resolved contrast pair"),
    };
    let [red, green, blue] = match indexed {
        0..=15 => [
            [0, 0, 0],
            [205, 0, 0],
            [0, 205, 0],
            [205, 205, 0],
            [0, 0, 238],
            [205, 0, 205],
            [0, 205, 205],
            [229, 229, 229],
            [127, 127, 127],
            [255, 0, 0],
            [0, 255, 0],
            [255, 255, 0],
            [92, 92, 255],
            [255, 0, 255],
            [0, 255, 255],
            [255, 255, 255],
        ][usize::from(indexed)],
        16..=231 => {
            let offset = indexed - 16;
            let channel = |value| [0, 95, 135, 175, 215, 255][usize::from(value)];
            [
                channel(offset / 36),
                channel(offset / 6 % 6),
                channel(offset % 6),
            ]
        }
        232..=255 => [8 + 10 * (indexed - 232); 3],
    };
    Color::Rgb(red, green, blue)
}

#[test]
fn destructive_intent_keeps_its_foreground_and_contrast_across_all_interaction_surfaces() {
    for theme in [
        Theme::one_light(),
        Theme::one_dark(),
        Theme::terminal_native(),
    ] {
        for level in [
            ColorLevel::Truecolor,
            ColorLevel::Ansi256,
            ColorLevel::Ansi16,
        ] {
            let theme = theme.quantize(level);
            for interaction in [
                None,
                Some(Role::SurfaceHover),
                Some(Role::SurfacePressed),
                Some(Role::SurfaceSelection),
            ] {
                let content = theme.content_on_surface(
                    Role::SurfaceOverlay,
                    interaction,
                    Some(Role::AccentDestructive),
                    theme.style(Role::AccentDestructive),
                );
                assert_eq!(content.fg, theme.style(Role::AccentDestructive).fg);
                assert!(
                    !content.add_modifier.intersects(
                        Modifier::REVERSED
                            | Modifier::BOLD
                            | Modifier::UNDERLINED
                            | Modifier::SLOW_BLINK
                            | Modifier::RAPID_BLINK
                    ),
                    "{level:?} {interaction:?}"
                );
                let contrast = contrast_ratio(
                    nominal_rgb(content.fg.unwrap()),
                    nominal_rgb(content.bg.unwrap()),
                );
                assert!(contrast >= 3.0, "{level:?} {interaction:?}: {contrast}");
            }
        }
    }
}

#[test]
fn destructive_role_is_independent_of_error_status_and_monochrome_warning_modifiers() {
    let mut theme = Theme::one_dark();
    let original = theme.style(Role::AccentDestructive);
    theme.styles.status_error = Style::new().fg(Color::Green);
    assert_eq!(theme.style(Role::AccentDestructive), original);
    assert_ne!(theme.style(Role::StatusError), original);
    let monochrome = theme.quantize(ColorLevel::Monochrome);
    let content = monochrome.content_on_surface(
        Role::SurfaceOverlay,
        None,
        Some(Role::AccentDestructive),
        monochrome.style(Role::AccentDestructive),
    );
    assert!(!content.add_modifier.intersects(
        Modifier::BOLD | Modifier::UNDERLINED | Modifier::SLOW_BLINK | Modifier::RAPID_BLINK
    ));
}

#[test]
fn state_hues_leave_reverse_video_bands_for_a_contrast_pair() {
    let mut reversed_hues = 0;
    for theme in [
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
            let theme = theme.quantize(level);
            for interaction in [Role::SurfaceHover, Role::SurfacePressed] {
                let band = theme.style(interaction);
                for role in Role::ALL.into_iter().filter(|role| role.is_state_hue()) {
                    let hue = theme.style(role);
                    let content = theme.content_on_surface(
                        Role::SurfaceStrip,
                        Some(interaction),
                        Some(role),
                        hue,
                    );
                    let case = format!("{level:?} {interaction:?} {role:?}");
                    let known = !matches!(hue.fg, None | Some(Color::Reset));
                    if !band.add_modifier.contains(Modifier::REVERSED) {
                        assert_eq!(content.bg, band.bg, "{case}");
                    } else if known && level != ColorLevel::Monochrome {
                        // The hue keeps its foreground, never a block of its own color.
                        reversed_hues += 1;
                        assert_eq!(content.fg, hue.fg, "{case}");
                        assert!(!content.add_modifier.contains(Modifier::REVERSED), "{case}");
                        let contrast = contrast_ratio(
                            nominal_rgb(content.fg.unwrap()),
                            nominal_rgb(content.bg.unwrap()),
                        );
                        assert!(contrast >= 4.5, "{case}: {contrast}");
                    } else {
                        // Host-relative and monochrome content reads like the band's own text.
                        assert!(content.add_modifier.contains(Modifier::REVERSED), "{case}");
                    }
                }
            }
        }
    }
    assert!(reversed_hues > 0);
}
