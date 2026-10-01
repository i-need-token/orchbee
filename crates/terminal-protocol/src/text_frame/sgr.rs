//! Canonical SGR for already-validated cell styles, shared by CLI output and event probes.
use crate::TerminalTextStyle;
use std::fmt;

pub struct TerminalSgrStyle(TerminalTextStyle);

#[must_use]
pub fn terminal_sgr_style(style: TerminalTextStyle) -> TerminalSgrStyle {
    TerminalSgrStyle(style)
}
impl fmt::Display for TerminalSgrStyle {
    fn fmt(&self, text: &mut fmt::Formatter<'_>) -> fmt::Result {
        let style = self.0;
        text.write_str("\x1b[0m")?;
        for (bit, sgr) in [
            (1, 1),
            (2, 3),
            (4, 2),
            (8, 5),
            (16, 7),
            (32, 8),
            (64, 9),
            (128, 53),
        ] {
            if style.flags & bit != 0 {
                write!(text, "\x1b[{sgr}m")?;
            }
        }
        if style.underline != 0 {
            write!(text, "\x1b[4:{}m", style.underline)?;
        }
        for (color, sgr) in [
            (style.foreground, 38),
            (style.background, 48),
            (style.underline_color, 58),
        ] {
            if color <= 0x00ff_ffff {
                write!(
                    text,
                    "\x1b[{sgr};2;{};{};{}m",
                    (color >> 16) & 255,
                    (color >> 8) & 255,
                    color & 255
                )?;
            }
        }
        Ok(())
    }
}
