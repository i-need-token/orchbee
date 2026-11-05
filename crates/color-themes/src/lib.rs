//! The single authority for Orchbee color themes: validated theme data embedded from `themes/`, and
//! the resolver that derives every semantic color role from a theme's palette. It does not choose a
//! theme for a device, quantize colors, compose styles, localize theme names, or render anything.

mod catalog;
mod contrast;
mod oklab;
mod rgb;
mod roles;
mod theme;

pub use catalog::{orchbee, select, shipped};
pub use rgb::Rgb;
pub use roles::Roles;
pub use theme::{Canvas, PaletteColor, Polarity, Source, Theme, ThemeFileError};
