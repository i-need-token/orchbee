//! 24-bit sRGB values, spelled `#rrggbb` in theme files and packed as `0xRRGGBB` by the terminal
//! protocol.

use std::fmt;

/// One opaque 8-bit-per-channel sRGB color. Every value is valid.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rgb {
    /// The gamma-encoded red channel.
    pub red: u8,
    /// The gamma-encoded green channel.
    pub green: u8,
    /// The gamma-encoded blue channel.
    pub blue: u8,
}

impl Rgb {
    /// The color with these channel values.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// Reads exactly `#` followed by six hexadecimal digits, in either case.
    pub(crate) fn parse_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |offset: usize| {
            digits
                .get(offset..offset + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        };
        Some(Self::new(channel(0)?, channel(2)?, channel(4)?))
    }

    /// The linear-light channels, red, green, blue, each in `0.0..=1.0`.
    pub(crate) fn linear(self) -> [f64; 3] {
        [self.red, self.green, self.blue].map(|channel| {
            let encoded = f64::from(channel) / 255.0;
            if encoded <= 0.040_45 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }
        })
    }

    /// The nearest 8-bit color to linear-light channels, clipping each to `0.0..=1.0`.
    pub(crate) fn from_linear(linear: [f64; 3]) -> Self {
        let [red, green, blue] = linear.map(|channel| {
            let channel = channel.clamp(0.0, 1.0);
            let encoded = if channel <= 0.003_130_8 {
                channel * 12.92
            } else {
                1.055 * channel.powf(1.0 / 2.4) - 0.055
            };
            // The clamped product lies within 0.0..=255.0, so the cast neither truncates nor wraps.
            (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
        });
        Self::new(red, green, blue)
    }
}

impl From<Rgb> for u32 {
    /// Packs the color as `0xRRGGBB`, the terminal protocol's RGB encoding.
    fn from(color: Rgb) -> Self {
        (Self::from(color.red) << 16) | (Self::from(color.green) << 8) | Self::from(color.blue)
    }
}

impl fmt::Display for Rgb {
    /// Writes the theme-file spelling, `#rrggbb` in lowercase.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "#{:02x}{:02x}{:02x}",
            self.red, self.green, self.blue
        )
    }
}
