//! Pure buffer-coordinate copy policies over validated physical row mappings.
//! This domain owns no viewer, input, selection, clipboard or process state.
use crate::terminal_buffer_text::Segment;
pub(crate) use crate::terminal_buffer_text::{TerminalTextPoint, TerminalTextRange};
use orchbee_terminal_protocol::TerminalTextWindowRow;
mod motion;
mod read;
mod search;
pub(crate) use motion::motion;
pub(crate) use read::read_motion_with;
pub(crate) use search::search;

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

// Copy policy treats Kitty image placeholder cells as whitespace; shared glyph
// traversal preserves raw text for other consumers such as link resolution.
fn segments<'a>(
    row: &'a TerminalTextWindowRow,
    next: Option<&TerminalTextWindowRow>,
    columns: u16,
    mut consume: impl FnMut(Segment<'a>),
) {
    crate::terminal_buffer_text::segments(row, next, columns, |segment| {
        let segment = match segment {
            Segment::Text {
                text,
                col,
                width,
                ascii: false,
            } if text.starts_with('\u{10eeee}') => Segment::Text {
                text: " ",
                col,
                width,
                ascii: false,
            },
            segment => segment,
        };
        consume(segment);
    });
}

#[cfg(test)]
mod tests;
