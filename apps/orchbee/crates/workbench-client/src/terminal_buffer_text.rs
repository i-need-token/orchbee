//! Pure physical glyph traversal over fully validated retained-buffer rows.
use orchbee_terminal_protocol::TerminalTextWindowRow;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TerminalTextPoint {
    pub row: u32,
    pub col: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTextRange {
    pub start: TerminalTextPoint,
    pub end: TerminalTextPoint,
}

pub(crate) enum Segment<'a> {
    Text {
        text: &'a str,
        col: u16,
        width: u16,
        ascii: bool,
    },
    Blank {
        col: u16,
        width: u16,
    },
    Spacer {
        col: u16,
    },
}

// Unencoded trailing blank cells and the dropped wide spacer head are distinct.
pub(crate) fn segments<'a>(
    row: &'a TerminalTextWindowRow,
    next: Option<&TerminalTextWindowRow>,
    columns: u16,
    mut consume: impl FnMut(Segment<'a>),
) {
    let mut used = 0;
    if row.identity_ascii {
        used = row.text.len() as u16;
        if used > 0 {
            consume(Segment::Text {
                text: &row.text,
                col: 0,
                width: used,
                ascii: true,
            });
        }
    } else {
        for (i, cell) in row.mappings.iter().enumerate() {
            if cell.column > used {
                consume(Segment::Blank {
                    col: used,
                    width: cell.column - used,
                });
            }
            let end = row
                .mappings
                .get(i + 1)
                .map_or(row.text.len(), |cell| cell.byte_offset as usize);
            let text = &row.text[cell.byte_offset as usize..end];
            consume(Segment::Text {
                text,
                col: cell.column,
                width: u16::from(cell.width),
                ascii: false,
            });
            used = cell.column + u16::from(cell.width);
        }
    }
    if used < columns {
        let spacer = used + 1 == columns
            && row.wrap
            && next.is_some_and(|next| {
                next.wrap_continuation
                    && !next.identity_ascii
                    && next
                        .mappings
                        .first()
                        .is_some_and(|cell| cell.column == 0 && cell.width == 2)
            });
        if spacer {
            consume(Segment::Spacer { col: used });
        } else {
            consume(Segment::Blank {
                col: used,
                width: columns - used,
            });
        }
    }
}
