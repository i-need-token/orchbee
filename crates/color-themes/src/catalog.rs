//! The themes this build ships, embedded from `themes/` and admitted once on first use, and the
//! one rule that turns a stored theme id into a shipped theme.

use std::sync::LazyLock;

use crate::{Polarity, Theme};

const FILES: [&str; 9] = [
    include_str!("../themes/orchbee-light.toml"),
    include_str!("../themes/orchbee-dark.toml"),
    include_str!("../themes/gruvbox-dark.toml"),
    include_str!("../themes/rose-pine.toml"),
    include_str!("../themes/catppuccin-mocha.toml"),
    include_str!("../themes/flexoki-dark.toml"),
    include_str!("../themes/gruvbox-light.toml"),
    include_str!("../themes/flexoki-light.toml"),
    include_str!("../themes/rose-pine-dawn.toml"),
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

/// The shipped theme `id` names when it has `polarity`; otherwise the Orchbee theme of `polarity`.
///
/// This is the only fallback for a stored theme id: an id this build does not ship, such as one a
/// newer Client saved or a theme later withdrawn, and an id saved in the other polarity's slot,
/// both resolve to the default of the slot's polarity.
///
/// # Panics
///
/// Under the same condition as [`shipped`].
#[must_use]
pub fn select(polarity: Polarity, id: &str) -> &'static Theme {
    shipped()
        .iter()
        .find(|theme| theme.polarity() == polarity && theme.id() == id)
        .unwrap_or_else(|| orchbee(polarity))
}
