//! Pure buffer-coordinate copy policies over validated physical row mappings.
//! This domain owns no viewer, input, selection, clipboard or process state.
use orchbee_terminal_protocol::TerminalTextWindowRow;
mod motion;
mod search;
pub(crate) use motion::motion;
pub(crate) use search::search;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TerminalTextPoint {
    pub(crate) row: u32,
    pub(crate) col: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalTextRange {
    pub(crate) start: TerminalTextPoint,
    pub(crate) end: TerminalTextPoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopyMotion {
    LineEnd,
    FirstNonBlank,
    NextWordStart,
    PreviousWordStart,
    NextWordEnd,
    NextBigWordStart,
    PreviousBigWordStart,
    NextBigWordEnd,
    PreviousParagraph,
    NextParagraph,
}

impl CopyMotion {
    pub(crate) const ALL: [Self; 10] = [
        Self::LineEnd,
        Self::FirstNonBlank,
        Self::NextWordStart,
        Self::PreviousWordStart,
        Self::NextWordEnd,
        Self::NextBigWordStart,
        Self::PreviousBigWordStart,
        Self::NextBigWordEnd,
        Self::PreviousParagraph,
        Self::NextParagraph,
    ];
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::LineEnd => "line-end",
            Self::FirstNonBlank => "first-non-blank",
            Self::NextWordStart => "next-word-start",
            Self::PreviousWordStart => "previous-word-start",
            Self::NextWordEnd => "next-word-end",
            Self::NextBigWordStart => "next-big-word-start",
            Self::PreviousBigWordStart => "previous-big-word-start",
            Self::NextBigWordEnd => "next-big-word-end",
            Self::PreviousParagraph => "previous-paragraph",
            Self::NextParagraph => "next-paragraph",
        }
    }
    pub(crate) const fn backward(self) -> bool {
        matches!(
            self,
            Self::PreviousWordStart | Self::PreviousBigWordStart | Self::PreviousParagraph
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopySearchDirection {
    Forward,
    Backward,
}
impl CopySearchDirection {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CopySearchResult {
    pub(crate) matches: Vec<TerminalTextRange>,
    pub(crate) total: u64,
    pub(crate) current: Option<usize>,
    pub(crate) current_global: Option<u64>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopyError {
    LimitExceeded,
    PatternTooComplex,
}
const MAXIMUM_LOGICAL_BYTES: usize = 8 * 1024 * 1024;
const WORD_SEPARATORS: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^\u{60}{|}~";

#[derive(Clone, Copy, Eq, PartialEq)]
enum Class {
    Blank,
    Separator,
    Word,
}
fn class(text: &str) -> Class {
    match text.chars().next() {
        None => Class::Blank,
        Some(ch) if ch.is_whitespace() => Class::Blank,
        Some(ch) if ch.is_ascii() && WORD_SEPARATORS.contains(ch) => Class::Separator,
        Some(_) => Class::Word,
    }
}

enum Segment<'a> {
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
fn segments<'a>(
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
            let text = if text.starts_with('\u{10eeee}') {
                " "
            } else {
                text
            };
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

#[cfg(test)]
mod tests;
