//! The themes this build ships, embedded from `themes/` and admitted once on first use.

use std::sync::LazyLock;

use crate::{Polarity, Theme};

const FILES: [&str; 2] = [
    include_str!("../themes/orchbee-light.toml"),
    include_str!("../themes/orchbee-dark.toml"),
];

// Indexes into `FILES` for the default pair.
const ORCHBEE_LIGHT: usize = 0;
const ORCHBEE_DARK: usize = 1;

static SHIPPED: LazyLock<[Theme; FILES.len()]> = LazyLock::new(|| {
    FILES.map(|file| {
        Theme::parse(file).unwrap_or_else(|error| panic!("embedded theme file: {error}"))
    })
});

/// Every shipped theme, in a stable order.
///
/// # Panics
///
/// Only if an embedded theme file is malformed. The crate's gate test admits every embedded
/// file, so a build that passes verification cannot reach this.
#[must_use]
pub fn shipped() -> &'static [Theme] {
    SHIPPED.as_slice()
}

/// The Orchbee theme of `polarity`, the default pair.
///
/// # Panics
///
/// Under the same condition as [`shipped`].
#[must_use]
pub fn orchbee(polarity: Polarity) -> &'static Theme {
    match polarity {
        Polarity::Light => &SHIPPED[ORCHBEE_LIGHT],
        Polarity::Dark => &SHIPPED[ORCHBEE_DARK],
    }
}
