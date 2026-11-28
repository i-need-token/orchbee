//! How VS Code previews one line's matches at ef5ca45 (`TextSearchMatch` in `search.ts`): with a
//! one-line preview, the line up to its first line break, from a fifth of `charsPerLine` before
//! each match to `charsPerLine` after its start, a long gap between matches elided with a
//! `⟪ N characters skipped ⟫` marker; otherwise the whole text. Columns are UTF-16 code units.

use super::js;

const ELIDED_PREFIX: &str = "⟪ ";
const ELIDED_SUFFIX: &str = " characters skipped ⟫";
/// `SEARCH_ELIDED_MIN_LEN`: both markers' lengths and 5, twice.
const ELIDED_MIN_LENGTH: i64 = (2 + 21 + 5) * 2;

/// The preview a search asked for.
#[derive(Clone, Copy, Debug)]
pub(super) struct PreviewOptions {
    pub(super) match_lines: u32,
    pub(super) chars_per_line: u32,
}

/// A range of lines and columns, end exclusive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Range {
    pub(super) start_line: i64,
    pub(super) start_column: i64,
    pub(super) end_line: i64,
    pub(super) end_column: i64,
}

/// A match's range in the file and in its preview.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Located {
    pub(super) source: Range,
    pub(super) preview: Range,
}

/// The preview of `text` with `ranges`, which are not empty, and where each range lies in it.
pub(super) fn preview(
    text: &str,
    ranges: &[Range],
    options: PreviewOptions,
) -> (Vec<Located>, String) {
    let line = ranges.first().map_or(0, |first| first.start_line);
    let one_line = ranges
        .iter()
        .all(|range| range.start_line == line && range.end_line == line);
    if options.match_lines != 1 || !one_line {
        let located = ranges
            .iter()
            .map(|range| Located {
                source: *range,
                preview: Range {
                    start_line: range.start_line - line,
                    end_line: range.end_line - line,
                    ..*range
                },
            })
            .collect();
        return (located, text.to_owned());
    }
    let text = js::units(first_line(text));
    let chars_per_line = i64::from(options.chars_per_line);
    let leading = chars_per_line / 5;
    let mut result: Vec<u16> = Vec::new();
    let (mut shift, mut last_end) = (0_i64, 0_i64);
    let mut located = Vec::with_capacity(ranges.len());
    for range in ranges {
        let start = (range.start_column - leading).max(0);
        let end = range.start_column + chars_per_line;
        if start > last_end + leading + ELIDED_MIN_LENGTH {
            let elision = format!("{ELIDED_PREFIX}{}{ELIDED_SUFFIX}", start - last_end);
            let elision = js::units(&elision);
            result.extend_from_slice(&elision);
            result.extend_from_slice(slice(&text, start, end));
            shift += start - (last_end + i64::try_from(elision.len()).unwrap_or(i64::MAX));
        } else {
            result.extend_from_slice(slice(&text, last_end, end));
        }
        last_end = end;
        located.push(Located {
            source: *range,
            preview: Range {
                start_line: 0,
                start_column: range.start_column - shift,
                end_line: 0,
                end_column: range.end_column - shift,
            },
        });
    }
    (located, js::text(&result))
}

/// `getNLines(text, 1)`: the text before its first line feed, without a carriage return before
/// it.
fn first_line(text: &str) -> &str {
    match text.find('\n') {
        Some(end) => text[..end].strip_suffix('\r').unwrap_or(&text[..end]),
        None => text,
    }
}

/// JavaScript's `slice` with non-negative bounds: empty when `start` is not before `end`.
fn slice(units: &[u16], start: i64, end: i64) -> &[u16] {
    let clamp = |at: i64| usize::try_from(at).unwrap_or(0).min(units.len());
    let (start, end) = (clamp(start), clamp(end));
    units.get(start..end).unwrap_or_default()
}
