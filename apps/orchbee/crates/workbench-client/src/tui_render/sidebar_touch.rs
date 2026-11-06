//! Touch spacing for the mobile sidebar list: the fills that sit centered on their rows.
//!
//! A spaced list leaves a blank line beside every row and fixed action. A fill covers only its
//! text lines; each blank line beside it shows half of it as an upper or lower half block, so the
//! fill reads as centered on the row and its blank line. A blank line between two fills of one
//! color takes that color whole. Hosts whose fills are not concrete colors draw no halves, so
//! their fills keep to the text lines.

use ratatui::{Frame, layout::Rect, style::Color};

use crate::tui_theme::{Role, Theme};

/// The fills and text lines one spaced list painted, for the halves its blank lines show.
pub(super) struct Halos {
    area: Rect,
    bands: Vec<(Rect, Color)>,
    text: Vec<bool>,
}

impl Halos {
    pub(super) fn new(area: Rect) -> Self {
        Self {
            area,
            bands: Vec::new(),
            text: vec![false; usize::from(area.height)],
        }
    }

    /// Marks every line of `rect` as text, which never shows a half.
    pub(super) fn text(&mut self, rect: Rect) {
        for y in rect.y..rect.bottom() {
            if let Some(line) = y
                .checked_sub(self.area.y)
                .and_then(|line| self.text.get_mut(usize::from(line)))
            {
                *line = true;
            }
        }
    }

    /// Records the fill `role` painted across `rect`, whose lines are text.
    pub(super) fn fill(&mut self, theme: &Theme, rect: Rect, role: Option<Role>) {
        self.text(rect);
        if let Some(color) = role.and_then(|role| theme.half_block_fill(role)) {
            self.bands.push((rect, color));
        }
    }

    fn color_at(&self, x: u16, y: Option<u16>) -> Option<Color> {
        let y = y?;
        self.bands
            .iter()
            .find(|(rect, _)| rect.contains((x, y).into()))
            .map(|(_, color)| *color)
    }

    /// Paints the halves into blank cells of the blank lines next to each recorded fill.
    pub(super) fn paint(&self, frame: &mut Frame<'_>, theme: &Theme) {
        let Some((upper, lower)) = theme.glyphs.half_blocks else {
            return;
        };
        if self.bands.is_empty() {
            return;
        }
        let area = self.area.intersection(frame.area());
        for y in area.y..area.bottom() {
            if self.text[usize::from(y - self.area.y)] {
                continue;
            }
            let above = y.checked_sub(1).filter(|above| *above >= area.y);
            let below = y.checked_add(1).filter(|below| *below < area.bottom());
            for x in area.x..area.right() {
                let up = self.color_at(x, above);
                let down = self.color_at(x, below);
                if up.is_none() && down.is_none() {
                    continue;
                }
                let cell = &mut frame.buffer_mut()[(x, y)];
                if !cell.symbol().trim().is_empty() {
                    continue;
                }
                match (up, down) {
                    (Some(up), Some(down)) if up == down => {
                        cell.set_symbol(" ").set_bg(up);
                    }
                    (Some(up), down) => {
                        cell.set_symbol(upper).set_fg(up);
                        if let Some(down) = down {
                            cell.set_bg(down);
                        }
                    }
                    (None, Some(down)) => {
                        cell.set_symbol(lower).set_fg(down);
                    }
                    (None, None) => {}
                }
            }
        }
    }
}
