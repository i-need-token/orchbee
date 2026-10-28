//! Bounded plain-link policy and viewport regions over validated physical glyphs.
use crate::terminal_buffer_text::{Segment, TerminalTextPoint, segments};
use orchbee_terminal_protocol::{TerminalProjectionRow, TerminalTextWindowRow};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkRegion {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedLink {
    pub url: Option<String>,
    pub regions: Vec<LinkRegion>,
}
impl ResolvedLink {
    pub(crate) fn none() -> Self {
        Self {
            url: None,
            regions: Vec::new(),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub enum LinkRead {
    Complete(ResolvedLink),
    NeedContext { before: bool, after: bool },
    LimitExceeded,
}

/// Resolve OSC 8 metadata for one rendered cell. Projection rows expose a
/// wide glyph's tail with the head width marker, so the tail inherits the
/// preceding head's hyperlink instead of becoming an unrelated cell.
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
pub fn explicit_url(row: TerminalProjectionRow<'_, '_>, column: u16) -> Result<Option<String>, ()> {
    let cell = row.cells().nth(usize::from(column)).ok_or(())?;
    let cell = if cell.width == 2 && column > 0 {
        row.cells().nth(usize::from(column - 1)).ok_or(())?
    } else {
        cell
    };
    Ok(cell.hyperlink.map(str::to_owned))
}
struct Glyph<'a> {
    text: &'a str,
    point: TerminalTextPoint,
    width: u16,
}
impl Glyph<'_> {
    fn blank(&self) -> bool {
        self.text.chars().next().is_some_and(char::is_whitespace)
    }
}

pub fn resolve(
    rows: &[TerminalTextWindowRow],
    columns: u16,
    cursor: TerminalTextPoint,
    total_rows: u64,
    viewport: Range<u64>,
) -> LinkRead {
    let mut glyphs = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 && !row.wrap_continuation {
            glyphs.push(Glyph {
                text: " ",
                point: TerminalTextPoint {
                    row: row.index as u32,
                    col: 0,
                },
                width: 0,
            });
        }
        segments(row, rows.get(i + 1), columns, |segment| match segment {
            Segment::Text {
                text,
                col,
                width,
                ascii,
            } => {
                if ascii {
                    for offset in 0..text.len() {
                        glyphs.push(Glyph {
                            text: &text[offset..offset + 1],
                            point: TerminalTextPoint {
                                row: row.index as u32,
                                col: col + offset as u16,
                            },
                            width: 1,
                        });
                    }
                } else {
                    glyphs.push(Glyph {
                        text,
                        point: TerminalTextPoint {
                            row: row.index as u32,
                            col,
                        },
                        width,
                    });
                }
            }
            Segment::Blank { col, width } => glyphs.push(Glyph {
                text: " ",
                point: TerminalTextPoint {
                    row: row.index as u32,
                    col,
                },
                width,
            }),
            Segment::Spacer { col } => glyphs.push(Glyph {
                text: "",
                point: TerminalTextPoint {
                    row: row.index as u32,
                    col,
                },
                width: 1,
            }),
        });
    }
    let Some(clicked) = glyphs.iter().position(|glyph| {
        glyph.point.row == cursor.row
            && glyph.point.col <= cursor.col
            && cursor.col < glyph.point.col.saturating_add(glyph.width)
    }) else {
        return LinkRead::Complete(ResolvedLink::none());
    };
    if glyphs[clicked].blank() || glyphs[clicked].text.is_empty() {
        return LinkRead::Complete(ResolvedLink::none());
    }
    let mut start = clicked;
    let mut end = clicked + 1;
    let mut cells = usize::from(glyphs[clicked].width);
    while start > 0 && !glyphs[start - 1].blank() {
        start -= 1;
        cells += usize::from(glyphs[start].width);
        if cells > 8192 {
            return LinkRead::Complete(ResolvedLink::none());
        }
    }
    while end < glyphs.len() && !glyphs[end].blank() {
        cells += usize::from(glyphs[end].width);
        end += 1;
        if cells > 8192 {
            return LinkRead::Complete(ResolvedLink::none());
        }
    }
    // Ghostty's shared8192 budget counts physical cells inspected in both
    // directions, including wide spacers/tails and a boundary probe when one
    // exists. A hard full-row end stops the forward iterator without a probe.
    let first = &glyphs[start];
    let last = &glyphs[end - 1];
    let row = &rows[(u64::from(last.point.row) - rows[0].index) as usize];
    let before_probe = usize::from(first.point.col > 0 || first.point.row > 0);
    let after_probe = usize::from(
        last.point.col + last.width < columns || (row.wrap && row.index + 1 < total_rows),
    );
    if cells + before_probe + after_probe > 8192 {
        return LinkRead::Complete(ResolvedLink::none());
    }
    let before = start == 0
        && rows
            .first()
            .is_some_and(|row| row.index > 0 && row.wrap_continuation);
    // One unencoded last cell could be a wide spacer head. Its meaning requires
    // the next row, so a token ending immediately before it is not yet proven.
    let possible_spacer = rows.last().is_some_and(|row| {
        let used = if row.identity_ascii {
            row.text.len() as u16
        } else {
            row.mappings
                .last()
                .map_or(0, |cell| cell.column + u16::from(cell.width))
        };
        row.wrap
            && used.checked_add(1) == Some(columns)
            && end > start
            && u64::from(glyphs[end - 1].point.row) == row.index
            && glyphs[end - 1].point.col + glyphs[end - 1].width == used
    });
    let after = rows
        .last()
        .is_some_and(|row| row.wrap && row.index + 1 < total_rows)
        && (end == glyphs.len() || possible_spacer);
    if before || after {
        return LinkRead::NeedContext { before, after };
    }
    let length: usize = glyphs[start..end]
        .iter()
        .map(|glyph| glyph.text.len())
        .sum();
    if length > 8 * 1024 * 1024 {
        return LinkRead::LimitExceeded;
    }
    let mut text = String::with_capacity(length);
    let mut clicked_byte = 0;
    for (i, glyph) in glyphs[start..end].iter().enumerate() {
        if start + i == clicked {
            clicked_byte = text.len();
        }
        text.push_str(glyph.text);
    }
    let Some(range) = url_range(&text, clicked_byte) else {
        return LinkRead::Complete(ResolvedLink::none());
    };
    let mut regions: Vec<LinkRegion> = Vec::new();
    let mut byte = 0;
    for glyph in &glyphs[start..end] {
        let next = byte + glyph.text.len();
        if !glyph.text.is_empty()
            && byte < range.end
            && next > range.start
            && viewport.contains(&u64::from(glyph.point.row))
        {
            let row = (u64::from(glyph.point.row) - viewport.start) as u16;
            let end_col = glyph.point.col + glyph.width - 1;
            if let Some(last) = regions.last_mut().filter(|last| {
                last.row == row && last.end_col.checked_add(1) == Some(glyph.point.col)
            }) {
                last.end_col = end_col;
            } else {
                regions.push(LinkRegion {
                    row,
                    start_col: glyph.point.col,
                    end_col,
                });
            }
        }
        byte = next;
    }
    LinkRead::Complete(ResolvedLink {
        url: Some(text[range].to_owned()),
        regions,
    })
}

// HTTP(S), whitespace and edge policy, implemented in linear time
// even for long runs of unmatched closers. Coordinates never derive from widths
// guessed from Unicode: the caller retains authoritative whole-cell mappings.
fn url_range(text: &str, clicked: usize) -> Option<Range<usize>> {
    let mut offset = 0;
    let (start, stop) = loop {
        let start = [
            text[offset..].find("http://"),
            text[offset..].find("https://"),
        ]
        .into_iter()
        .flatten()
        .min()?
            + offset;
        let stop = text[start..]
            .find(char::is_whitespace)
            .map_or(text.len(), |end| start + end);
        if clicked < start {
            return None;
        }
        if clicked < stop {
            break (start, stop);
        }
        offset = stop;
    };
    let chars: Vec<_> = text[start..stop]
        .char_indices()
        .map(|(byte, ch)| (start + byte, ch))
        .collect();
    let mut balances = [0_i32; 3];
    let bracket = |ch| match ch {
        '(' | ')' => Some(0),
        '[' | ']' => Some(1),
        '{' | '}' => Some(2),
        _ => None,
    };
    for (_, ch) in &chars {
        if let Some(i) = bracket(*ch) {
            balances[i] += if matches!(ch, '(' | '[' | '{') { 1 } else { -1 };
        }
    }
    let mut end = stop;
    for &(byte, ch) in chars.iter().rev() {
        let trim = match ch {
            '"' | '\'' | '`' | '.' | ',' | ';' | ':' | '!' | '?' => true,
            ')' | ']' | '}' => balances[bracket(ch)?] < 0,
            _ => false,
        };
        if !trim {
            break;
        }
        if let Some(i) = bracket(ch) {
            balances[i] += 1;
        }
        end = byte;
    }
    (start <= clicked && clicked < end).then_some(start..end)
}

/// Crate-internal `ResolvedLink` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait ResolvedLinkTestAccess {
    fn none() -> Self;
}

#[cfg(any(test, feature = "test-support"))]
impl ResolvedLinkTestAccess for ResolvedLink {
    fn none() -> Self {
        ResolvedLink::none()
    }
}

#[cfg(test)]
mod tests;
