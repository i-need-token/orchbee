//! The one line-breaking rule for terminal prose: overlay bodies, notices, Help, and tooltips.
//! Latin words break only at spaces; wide (CJK) graphemes break between each other and next to
//! narrow text; spaces at a break vanish; closing punctuation never starts a line and opening
//! punctuation never ends one. Widths are [`display_width`](super::display_width) per grapheme.

use std::ops::Range;

use ratatui::{
    style::Style,
    text::{Line, Span, Text},
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    /// Never ends a line: `（「『《` and their kin.
    Open,
    /// Never starts a line: `，。、；：！？）」』》` and their kin.
    Close,
    Other,
}

#[derive(Clone, Copy)]
struct Cell {
    width: usize,
    wide: bool,
    class: Class,
}

impl Cell {
    fn of(grapheme: &str) -> Self {
        let width = super::grapheme_width(grapheme);
        let first = grapheme.chars().next().unwrap_or(' ');
        // A no-break space is content, not a break opportunity.
        let class = if grapheme.chars().all(char::is_whitespace) && grapheme != "\u{a0}" {
            Class::Space
        } else if "，。、；：！？）」』》】〕〉〗〙〛”’…‥,.;:!?)]}".contains(first)
        {
            Class::Close
        } else if "（「『《【〔〈〖〘〚“‘([{".contains(first) {
            Class::Open
        } else {
            Class::Other
        };
        Self {
            width,
            wide: width >= 2,
            class,
        }
    }
}

/// Whether a line may end between `before` and `after`. A break after spaces drops them; the
/// conventional space between CJK and digits or Latin text is no stronger than a break between two
/// CJK characters, so a full line never stops early at it.
fn breaks_between(before: Cell, after: Cell) -> bool {
    match (before.class, after.class) {
        (_, Class::Space | Class::Close) | (Class::Open, _) => false,
        (Class::Space, _) => true,
        _ => before.wide || after.wide,
    }
}

/// Greedy lines of at most `width` cells over one source line, as cell ranges with the spaces at
/// each break trimmed. Leading spaces of the source stay as indentation. A run with no break
/// opportunity that is wider than the line breaks between graphemes; a grapheme wider than the
/// line stands alone.
fn break_cells(cells: &[Cell], width: usize) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    if width == 0 {
        return lines;
    }
    if cells.is_empty() {
        // An empty source line still takes a row.
        lines.push(0..0);
    }
    let mut start = 0;
    while start < cells.len() {
        let mut used = 0;
        let mut content = false;
        let mut opportunity = None;
        let mut end = cells.len();
        for index in start..cells.len() {
            let cell = cells[index];
            if content && breaks_between(cells[index - 1], cell) {
                opportunity = Some(index);
            }
            if cell.class != Class::Space && index > start && used + cell.width > width {
                end = opportunity.unwrap_or(index);
                break;
            }
            used += cell.width;
            content |= cell.class != Class::Space;
        }
        let mut line_end = end;
        while line_end > start && cells[line_end - 1].class == Class::Space {
            line_end -= 1;
        }
        lines.push(start..line_end);
        start = end;
        while start < cells.len() && cells[start].class == Class::Space {
            start += 1;
        }
    }
    lines
}

/// Byte ranges of `text` (one source line) broken into lines of at most `width` cells. Empty
/// text yields one empty range; zero width yields none.
pub(crate) fn wrap_str(text: &str, width: usize) -> Vec<Range<usize>> {
    let graphemes: Vec<(usize, &str)> = text.grapheme_indices(true).collect();
    let cells: Vec<Cell> = graphemes.iter().map(|(_, g)| Cell::of(g)).collect();
    let byte = |index: usize| {
        graphemes
            .get(index)
            .map_or(text.len(), |(offset, _)| *offset)
    };
    break_cells(&cells, width)
        .into_iter()
        .map(|range| byte(range.start)..byte(range.end))
        .collect()
}

/// `text` broken into lines of at most `width` cells, keeping span, line, and text styles and
/// alignments. At a nonzero width every source line yields at least one line. Render the result without ratatui's
/// own wrapping, and count its rows with `lines.len()`.
pub(crate) fn wrap_text(text: &Text<'_>, width: u16) -> Text<'static> {
    let mut lines = Vec::new();
    for line in &text.lines {
        let graphemes: Vec<(&str, Style)> = line
            .spans
            .iter()
            .flat_map(|span| span.content.graphemes(true).map(|g| (g, span.style)))
            .collect();
        let cells: Vec<Cell> = graphemes.iter().map(|(g, _)| Cell::of(g)).collect();
        let ranges = break_cells(&cells, usize::from(width));
        for range in ranges {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for (grapheme, style) in &graphemes[range] {
                match spans.last_mut() {
                    Some(span) if span.style == *style => span.content.to_mut().push_str(grapheme),
                    _ => spans.push(Span::styled((*grapheme).to_owned(), *style)),
                }
            }
            lines.push(Line {
                style: line.style,
                alignment: line.alignment,
                spans,
            });
        }
    }
    Text {
        style: text.style,
        alignment: text.alignment,
        lines,
    }
}

#[cfg(test)]
mod tests;
