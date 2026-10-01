//! Pure revision-independent extraction; callers admit the row window and content fence.
use crate::{
    ProtocolError, ProtocolErrorCode, SelectionPoint, TerminalTextRowWindow, TerminalTextWindowRow,
};
fn invalid() -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, "invalid text selection")
}
fn ordered(a: SelectionPoint, b: SelectionPoint) -> (SelectionPoint, SelectionPoint) {
    if (a.row, a.column) <= (b.row, b.column) {
        (a, b)
    } else {
        (b, a)
    }
}
pub fn extract_terminal_text_selection(
    window: &TerminalTextRowWindow,
    a: SelectionPoint,
    b: SelectionPoint,
    maximum_bytes: usize,
) -> Result<String, ProtocolError> {
    let (start, end) = ordered(a, b);
    if window.columns == 0
        || start.column >= window.columns
        || end.column >= window.columns
        || window.start_row != start.row
        || end.row >= window.total_rows
        || end.row.abs_diff(start.row) >= 4096
        || window.rows.len() as u64 != end.row - start.row + 1
    {
        return Err(invalid());
    }
    let mut text = String::new();
    for (index, row) in window.rows.iter().enumerate() {
        if row.index != start.row + index as u64 {
            return Err(invalid());
        }
        let first = if row.index == start.row {
            start.column
        } else {
            0
        };
        let last = if row.index == end.row {
            end.column
        } else {
            window.columns - 1
        };
        let part = slice(row, first, last)?;
        let joined = row.wrap
            && window
                .rows
                .get(index + 1)
                .is_some_and(|next| next.wrap_continuation);
        text.push_str(if joined {
            part
        } else {
            part.trim_end_matches(' ')
        });
        if text.len() > maximum_bytes {
            return Err(invalid());
        }
        if index + 1 < window.rows.len() && !joined {
            text.push('\n');
        }
    }
    if text.len() > maximum_bytes {
        return Err(invalid());
    }
    Ok(text)
}
fn slice(row: &TerminalTextWindowRow, first: u16, last: u16) -> Result<&str, ProtocolError> {
    if row.identity_ascii {
        return row
            .text
            .get(
                usize::from(first).min(row.text.len())..(usize::from(last) + 1).min(row.text.len()),
            )
            .ok_or_else(invalid);
    }
    let intersects = |index: usize| {
        let cell = row.mappings[index];
        cell.column <= last && u32::from(cell.column) + u32::from(cell.width) > u32::from(first)
    };
    let Some(start) = (0..row.mappings.len()).find(|index| intersects(*index)) else {
        return Ok("");
    };
    let end = (start..row.mappings.len())
        .take_while(|index| intersects(*index))
        .last()
        .unwrap_or(start);
    let from = row.mappings[start].byte_offset as usize;
    let to = row
        .mappings
        .get(end + 1)
        .map_or(row.text.len(), |cell| cell.byte_offset as usize);
    row.text.get(from..to).ok_or_else(invalid)
}
