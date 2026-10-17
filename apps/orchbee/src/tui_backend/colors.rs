//! Encode resolved frame colors without a second process-global color policy.
//! Theme admission already applies chrome capabilities; explicit program colors remain intact.

use std::io::{self, Write};

use ratatui::style::Color;

pub(super) fn write_foreground(writer: &mut impl Write, color: Color) -> io::Result<()> {
    write_color(writer, color, 38)
}

pub(super) fn write_background(writer: &mut impl Write, color: Color) -> io::Result<()> {
    write_color(writer, color, 48)
}

fn write_color(writer: &mut impl Write, color: Color, plane: u8) -> io::Result<()> {
    let index = match color {
        Color::Reset => return write!(writer, "\x1b[{}m", plane + 1),
        Color::Rgb(red, green, blue) => {
            return write!(writer, "\x1b[{plane};2;{red};{green};{blue}m");
        }
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
    };
    write!(writer, "\x1b[{plane};5;{index}m")
}
