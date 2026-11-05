use orchbee_color_themes::{Polarity, orchbee};
use orchbee_local_runtime::UserTerminalColors;
use orchbee_terminal_protocol::ColorScheme;

use super::effective;

/// A terminal must get the cursor and ANSI colors of the theme whose polarity matches its
/// default colors, the pairing the theme gates measured. The light-tuned blue is 2.2:1 on the dark
/// canvas, and a dark-tuned yellow would be as unreadable on a light one.
#[test]
fn each_scheme_gets_the_cursor_and_ansi_colors_of_its_own_theme() {
    let shared = effective(UserTerminalColors::LIGHT).palette;
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
        let terminal = effective(colors);
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
