//! A saved machine's color: a preset or a color of the person's own, their record and command
//! spellings, the default a new machine takes, and the preset an earlier release reads in its
//! place. The catalog stores it; the Clients write the machine's name in it.
// module-layer: values

use std::fmt;

use orchbee_color_themes::{MACHINE_COLORS, Rgb};

/// The color of a saved machine's name wherever it is written: one of the theme crate's
/// [`MACHINE_COLORS`], which every theme makes readable, or a color the person chose, written as
/// chosen. This computer has none.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MachineColor {
    /// The preset at this index of [`MACHINE_COLORS`].
    Preset(Preset),
    Custom(Rgb),
}

/// One of [`MACHINE_COLORS`], by its index there.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Preset(u8);

impl Preset {
    /// Every preset, in the order the Clients list them; the length ties it to the palette.
    pub const ALL: [Self; MACHINE_COLORS.len()] = [
        Self(0),
        Self(1),
        Self(2),
        Self(3),
        Self(4),
        Self(5),
        Self(6),
        Self(7),
        Self(8),
        Self(9),
        Self(10),
        Self(11),
    ];
    /// Every preset's record and command spelling, in [`Self::ALL`]'s order.
    pub const NAMES: [&'static str; MACHINE_COLORS.len()] = {
        let mut names = [""; MACHINE_COLORS.len()];
        let mut index = 0;
        while index < names.len() {
            names[index] = MACHINE_COLORS[index].0;
            index += 1;
        }
        names
    };
    pub const CYAN: Self = Self(0);
    pub const PURPLE: Self = Self(6);
    pub const PINK: Self = Self(8);
    pub const GREY: Self = Self(11);

    /// Its index in [`MACHINE_COLORS`] and in each theme's resolved machine colors.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The record's and every command's spelling, the same in every locale.
    pub const fn name(self) -> &'static str {
        MACHINE_COLORS[self.index()].0
    }

    /// The color it starts from, before a theme raises it to read on its surfaces.
    pub const fn source(self) -> Rgb {
        MACHINE_COLORS[self.index()].1
    }

    /// The preset of the four an earlier release knows, cyan, purple, pink and grey, that this
    /// one is written as for it: the nearest of them in hue, or grey for a near-neutral.
    const fn legacy(self) -> Self {
        match self.0 {
            // Cyan, teal, sky and blue.
            0..=3 => Self::CYAN,
            // Indigo, violet, purple and magenta.
            4..=7 => Self::PURPLE,
            8 => Self::PINK,
            // Brown, slate and grey.
            _ => Self::GREY,
        }
    }
}

impl MachineColor {
    pub const DEFAULT: Self = Self::Preset(Preset::CYAN);

    /// Reads a preset's name, exactly, or `#` and six hexadecimal digits in either case.
    pub fn parse(value: &str) -> Option<Self> {
        if let Some(preset) = Preset::ALL
            .into_iter()
            .find(|preset| preset.name() == value)
        {
            return Some(Self::Preset(preset));
        }
        let digits = value.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
        Some(Self::Custom(Rgb::new(
            channel(0)?,
            channel(2)?,
            channel(4)?,
        )))
    }

    /// The color as written before a theme acts on it: a preset's source, or the custom color.
    pub const fn source(self) -> Rgb {
        match self {
            Self::Preset(preset) => preset.source(),
            Self::Custom(color) => color,
        }
    }

    /// The preset among the four an earlier release knows that the record also names, so that
    /// release shows the nearest color and keeps the record's own: a custom color takes the legacy
    /// preset of the preset nearest to it.
    pub fn legacy(self) -> Self {
        let preset = match self {
            Self::Preset(preset) => preset,
            Self::Custom(color) => nearest_preset(color),
        };
        Self::Preset(preset.legacy())
    }

    /// The preset the fewest of `used` take; among equals, the earliest. A custom color counts
    /// against no preset.
    pub fn least_used(used: impl IntoIterator<Item = Self>) -> Self {
        let mut counts = [0_usize; MACHINE_COLORS.len()];
        for color in used {
            if let Self::Preset(preset) = color {
                counts[preset.index()] += 1;
            }
        }
        Preset::ALL
            .into_iter()
            .min_by_key(|preset| counts[preset.index()])
            .map_or(Self::DEFAULT, Self::Preset)
    }
}

/// The record's and every command's spelling: a preset's name, or `#rrggbb` in lowercase.
impl fmt::Display for MachineColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preset(preset) => formatter.write_str(preset.name()),
            Self::Custom(color) => write!(formatter, "{color}"),
        }
    }
}

/// The preset whose source is nearest `color` in sRGB.
fn nearest_preset(color: Rgb) -> Preset {
    let distance = |preset: &Preset| {
        let source = preset.source();
        [
            (source.red, color.red),
            (source.green, color.green),
            (source.blue, color.blue),
        ]
        .into_iter()
        .map(|(left, right)| u32::from(left.abs_diff(right)).pow(2))
        .sum::<u32>()
    };
    Preset::ALL
        .into_iter()
        .min_by_key(distance)
        .unwrap_or(Preset::CYAN)
}
