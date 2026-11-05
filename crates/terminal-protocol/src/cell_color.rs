//! Shared cell-color codes for OBTF, OBPB and render-detail OBRW. Resolution belongs to each
//! Client; frame metadata defaults and ANSI 16-255 remain Host-resolved RGB.

/// Largest directly encoded RGB value, including explicit black.
pub const COLOR_RGB_MASK: u32 = 0x00ff_ffff;
/// Selects the Client's effective default for this color channel.
pub const COLOR_DEFAULT: u32 = 0x0100_0000;
/// No explicit color; underline inherits its displayed text color.
pub const COLOR_NONE: u32 = 0x0200_0000;
/// Base of the sixteen Client-resolved ANSI palette codes.
pub const COLOR_PALETTE: u32 = 0x0300_0000;

/// Returns an ANSI index only for the sixteen exact palette codes; reserved bits are invalid.
pub fn terminal_palette_index(value: u32) -> Option<u8> {
    (COLOR_PALETTE..=COLOR_PALETTE + 15)
        .contains(&value)
        .then_some((value & 15) as u8)
}

pub(crate) fn valid_cell_color(value: u32) -> bool {
    value <= COLOR_RGB_MASK
        || value == COLOR_DEFAULT
        || value == COLOR_NONE
        || terminal_palette_index(value).is_some()
}
