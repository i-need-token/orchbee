//! Tab group accents and their resting tint over the Tab strip.
use super::{Color, ColorLevel, nearest_text_ansi16, quantize_color};
use orchbee_workbench_protocol::TabGroupColor;

/// One accent per shared group color. Named palettes also carry a faint resting fill of each
/// accent over their Tab strip; host-relative themes have no concrete strip color to tint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TabGroupPalette {
    accents: [Color; 9],
    tints: [Option<Color>; 9],
}

const fn slot(color: TabGroupColor) -> usize {
    match color {
        TabGroupColor::Blue => 0,
        TabGroupColor::Cyan => 1,
        TabGroupColor::Green => 2,
        TabGroupColor::Grey => 3,
        TabGroupColor::Orange => 4,
        TabGroupColor::Pink => 5,
        TabGroupColor::Purple => 6,
        TabGroupColor::Red => 7,
        TabGroupColor::Yellow => 8,
    }
}

/// Share of the accent in a resting group fill; the rest is the Tab strip surface.
const TINT_WEIGHT: f64 = 0.2;

impl TabGroupPalette {
    /// Named ANSI colors resolved by the host palette. Orange has no ANSI name and quantizes.
    pub(super) const TERMINAL: Self = Self {
        accents: [
            Color::Blue,
            Color::Cyan,
            Color::Green,
            Color::DarkGray,
            Color::Rgb(255, 165, 0),
            Color::LightMagenta,
            Color::Magenta,
            Color::Red,
            Color::Yellow,
        ],
        tints: [None; 9],
    };

    pub(super) fn named(accents: [[u8; 3]; 9], strip: [u8; 3]) -> Self {
        // The weighted channel stays within 0..=255, so rounding cannot truncate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mix = |accent: u8, base: u8| {
            (f64::from(accent) * TINT_WEIGHT + f64::from(base) * (1.0 - TINT_WEIGHT)).round() as u8
        };
        Self {
            accents: accents.map(|[r, g, b]| Color::Rgb(r, g, b)),
            tints: accents.map(|[r, g, b]| {
                Some(Color::Rgb(
                    mix(r, strip[0]),
                    mix(g, strip[1]),
                    mix(b, strip[2]),
                ))
            }),
        }
    }

    /// Accents stay readable text at every color level. Sixteen colors cannot express a faint
    /// fill, so headers rest on the strip there.
    pub(super) fn quantize(self, level: ColorLevel) -> Self {
        Self {
            accents: self.accents.map(|color| match (level, color) {
                (ColorLevel::Ansi16, Color::Rgb(r, g, b)) => nearest_text_ansi16(r, g, b),
                _ => quantize_color(color, level),
            }),
            tints: self.tints.map(|tint| {
                tint.filter(|_| matches!(level, ColorLevel::Truecolor | ColorLevel::Ansi256))
                    .map(|tint| quantize_color(tint, level))
            }),
        }
    }

    pub(crate) const fn accent(&self, color: TabGroupColor) -> Color {
        self.accents[slot(color)]
    }

    pub(crate) const fn tint(&self, color: TabGroupColor) -> Option<Color> {
        self.tints[slot(color)]
    }
}
