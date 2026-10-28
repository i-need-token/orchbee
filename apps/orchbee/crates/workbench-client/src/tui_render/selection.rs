//! Projects Host absolute selection endpoints without deriving text from the paint buffer.
use orchbee_terminal_protocol::SelectionChangedState;
use ratatui::{Frame, buffer::CellWidth, layout::Rect, style::Style};

pub(super) fn paint(
    frame: &mut Frame<'_>,
    area: Rect,
    origin: u64,
    selection: Option<&SelectionChangedState>,
    style: Style,
) {
    paint_cropped(frame, area, origin, 0, selection, style);
}

/// Project a range into a horizontally cropped live view using physical terminal columns.
pub(super) fn paint_cropped(
    frame: &mut Frame<'_>,
    area: Rect,
    origin: u64,
    first_column: u16,
    selection: Option<&SelectionChangedState>,
    style: Style,
) {
    if area.is_empty() {
        return;
    }
    let Some(SelectionChangedState::Active {
        start,
        end,
        rectangle,
        ..
    }) = selection
    else {
        return;
    };
    let (first, last) = if (start.row, start.column) <= (end.row, end.column) {
        (*start, *end)
    } else {
        (*end, *start)
    };
    for row in 0..area.height {
        let Some(absolute) = origin.checked_add(u64::from(row)) else {
            continue;
        };
        if absolute < first.row || absolute > last.row {
            continue;
        }
        let low = if *rectangle {
            start.column.min(end.column)
        } else if absolute == first.row {
            first.column
        } else {
            0
        };
        let high = if *rectangle {
            start.column.max(end.column)
        } else if absolute == last.row {
            last.column
        } else {
            first_column.saturating_add(area.width).saturating_sub(1)
        };
        for column in low.max(first_column)
            ..=high.min(first_column.saturating_add(area.width).saturating_sub(1))
        {
            let column = column - first_column;
            // A wide glyph is one displayed unit even when the endpoint lies
            // on its trailing cell. Apply the background to its leading cell.
            let x = area.x + column;
            let y = area.y + row;
            if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
                cell.set_style(style);
            }
            if column > 0
                && frame
                    .buffer_mut()
                    .cell((x - 1, y))
                    .is_some_and(|cell| cell.cell_width() == 2)
                && let Some(cell) = frame.buffer_mut().cell_mut((x - 1, y))
            {
                cell.set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests;
