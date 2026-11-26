//! The text a search shows around its matches: matched bytes become a UTF-16 preview with the
//! matches as code unit spans, and a line too long to show whole becomes the parts around its
//! matches.
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_SEARCH_PREVIEW_UNITS, WorkspaceSearchLine, WorkspaceSearchRange,
};

/// UTF-16 code units a cut preview keeps before its first match.
const LEADING_UNITS: u32 = 40;

/// Text decoded for display, with the UTF-16 offset of each byte offset asked about.
struct Rendered {
    text: String,
    /// Each asked offset's code unit offset, in the order asked.
    units: Vec<u32>,
}

/// Decodes `bytes` as UTF-8, each invalid sequence reading as U+FFFD and a carriage return before
/// a line feed dropped, and finds where each of `offsets`, ascending, lands in the result.
fn render(bytes: &[u8], offsets: &[usize]) -> Rendered {
    let mut text = String::with_capacity(bytes.len());
    let mut units = Vec::with_capacity(offsets.len());
    let mut wanted = offsets.iter().copied().peekable();
    let mut unit = 0u32;
    let mut position = 0usize;
    let mut note = |position: usize, unit: u32, units: &mut Vec<u32>| {
        while wanted.next_if(|offset| *offset <= position).is_some() {
            units.push(unit);
        }
    };
    for chunk in bytes.utf8_chunks() {
        for (index, character) in chunk.valid().char_indices() {
            note(position + index, unit, &mut units);
            let next = position + index + character.len_utf8();
            if character == '\r' && bytes.get(next) == Some(&b'\n') {
                continue;
            }
            text.push(character);
            unit = unit.saturating_add(u32::try_from(character.len_utf16()).unwrap_or(2));
        }
        position += chunk.valid().len();
        if !chunk.invalid().is_empty() {
            note(position, unit, &mut units);
            text.push(char::REPLACEMENT_CHARACTER);
            unit = unit.saturating_add(1);
            position += chunk.invalid().len();
        }
    }
    note(usize::MAX, unit, &mut units);
    Rendered { text, units }
}

/// The byte index in `text` at the last character boundary at or before code unit `target`.
fn byte_at_unit(text: &str, target: u32) -> usize {
    let mut unit = 0u32;
    for (index, character) in text.char_indices() {
        let next = unit.saturating_add(u32::try_from(character.len_utf16()).unwrap_or(2));
        if next > target {
            return index;
        }
        unit = next;
    }
    text.len()
}

/// The code units of `text`.
fn units_of(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// Previews for matches that start on one line. `bytes` holds that line, and for a match that
/// runs on, the lines it spans, without the last line terminator; `spans` are the matches' byte
/// offsets in it, ascending and apart. A preview that would pass the size limit shows the part
/// from shortly before a match, so a long line can give several.
pub(super) fn previews(
    line: u64,
    bytes: &[u8],
    spans: &[(usize, usize)],
) -> Vec<WorkspaceSearchLine> {
    let offsets: Vec<usize> = spans
        .iter()
        .flat_map(|(start, end)| [*start, *end])
        .collect();
    let rendered = render(bytes, &offsets);
    let ranges: Vec<WorkspaceSearchRange> = rendered
        .units
        .chunks_exact(2)
        .filter(|pair| pair[0] < pair[1])
        .map(|pair| WorkspaceSearchRange {
            start: pair[0],
            end: pair[1],
        })
        .collect();
    let limit = u32::try_from(MAXIMUM_WORKSPACE_SEARCH_PREVIEW_UNITS).unwrap_or(u32::MAX);
    let total = units_of(&rendered.text);
    if ranges.is_empty() {
        return Vec::new();
    }
    if total <= limit {
        return vec![WorkspaceSearchLine {
            line,
            preview_start: 0,
            preview: rendered.text,
            matches: ranges,
        }];
    }
    let mut previews = Vec::new();
    let mut remaining = ranges.as_slice();
    while let Some(first) = remaining.first() {
        let from = byte_at_unit(&rendered.text, first.start.saturating_sub(LEADING_UNITS));
        let start = units_of(&rendered.text[..from]);
        let to = from + byte_at_unit(&rendered.text[from..], limit);
        let preview = &rendered.text[from..to];
        let end = start.saturating_add(units_of(preview));
        let taken = remaining
            .iter()
            .take_while(|range| range.start < end)
            .count()
            .max(1);
        let matches: Vec<WorkspaceSearchRange> = remaining[..taken]
            .iter()
            .map(|range| WorkspaceSearchRange {
                start: range.start - start,
                end: range.end.min(end) - start,
            })
            .filter(|range| range.start < range.end)
            .collect();
        if !matches.is_empty() {
            previews.push(WorkspaceSearchLine {
                line,
                preview_start: start,
                preview: preview.to_owned(),
                matches,
            });
        }
        remaining = &remaining[taken..];
    }
    previews
}
