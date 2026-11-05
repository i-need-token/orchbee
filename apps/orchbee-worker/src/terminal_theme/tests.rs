use orchbee_color_themes::{Polarity, orchbee};
use orchbee_local_runtime::{UserColorThemes, UserTerminalColors};
use orchbee_terminal_protocol::ColorScheme;

use super::effective;

#[test]
fn client_query_palette_replaces_os_fallback_and_preserves_unobserved_slots() {
    use orchbee_capability_node_protocol::HostDefaultColors;
    let fallback = effective(UserTerminalColors::LIGHT, &UserColorThemes::default());
    let dark = effective(
        UserTerminalColors::DARK,
        &UserColorThemes::new("orchbee-light".into(), "gruvbox-dark".into()),
    );
    let known = std::array::from_fn(|index| Some(dark.palette[index]));
    let report = HostDefaultColors::new(
        dark.default_foreground_rgb,
        dark.default_background_rgb,
        dark.default_cursor_rgb,
        known,
    )
    .unwrap();
    let current = super::reported(report, &fallback);
    assert_eq!(current, dark);
    let mut partial = [None; 16];
    partial[0] = Some(0);
    partial[15] = Some(0xffffff);
    let outer = HostDefaultColors::new(0xcccccc, 0x101010, None, partial).unwrap();
    let next = super::reported(outer, &current);
    assert_eq!(
        (next.default_foreground_rgb, next.default_background_rgb),
        (0xcccccc, 0x101010)
    );
    assert_eq!(next.palette[0], 0);
    assert_eq!(next.palette[15], 0xffffff);
    assert_eq!(next.palette[1..15], current.palette[1..15]);
    assert_eq!(next.palette[16..], current.palette[16..]);
    assert_eq!(next.default_cursor_rgb, current.default_cursor_rgb);
}

/// A terminal must get the cursor and ANSI colors of the theme whose polarity matches its
/// default colors, the pairing the theme gates measured. The light-tuned blue is 2.2:1 on the dark
/// canvas, and a dark-tuned yellow would be as unreadable on a light one.
#[test]
fn each_scheme_gets_the_cursor_and_ansi_colors_of_its_own_theme() {
    // An unshipped id and an id of the other polarity both take that slot's Orchbee theme.
    let themes = UserColorThemes::new("orchbee-dark".into(), "not-shipped".into());
    let shared = effective(UserTerminalColors::LIGHT, &themes).palette;
    for (colors, polarity) in [
        (UserTerminalColors::LIGHT, Polarity::Light),
        (UserTerminalColors::DARK, Polarity::Dark),
        (
            UserTerminalColors::from_rgb(0xcc_cc_cc, 0x0c_0c_0c),
            Polarity::Dark,
        ),
        (
            UserTerminalColors::from_rgb(0x20_20_20, 0xf0_f0_e0),
            Polarity::Light,
        ),
    ] {
        let terminal = effective(colors, &themes);
        let theme = orchbee(polarity);
        let scheme = match polarity {
            Polarity::Light => ColorScheme::Light,
            Polarity::Dark => ColorScheme::Dark,
        };
        assert_eq!(terminal.color_scheme, scheme);
        assert_eq!(
            (
                terminal.default_foreground_rgb,
                terminal.default_background_rgb
            ),
            colors.rgb()
        );
        assert_eq!(
            terminal.default_cursor_rgb,
            Some(u32::from(theme.canvas().cursor.value())),
            "{polarity:?} cursor"
        );
        assert_eq!(
            terminal.palette[..16],
            theme.ansi().map(|color| u32::from(color.value())),
            "{polarity:?} ANSI 0-15"
        );
        // Themes own only ANSI 0-15; the xterm cube and gray ramp are the same for every theme.
        assert_eq!(terminal.palette[16..], shared[16..]);
    }
    // The shipped default colors are the canvas the gates measured each palette on.
    for (colors, polarity) in [
        (UserTerminalColors::LIGHT, Polarity::Light),
        (UserTerminalColors::DARK, Polarity::Dark),
    ] {
        let canvas = orchbee(polarity).canvas();
        assert_eq!(
            colors.rgb(),
            (u32::from(canvas.foreground), u32::from(canvas.background))
        );
    }
}
