//! Client-local cropping of a live physical grid; never changes terminal size, demand or control.
use orchbee_terminal_protocol::TerminalTextFrame;
use ratatui::layout::Rect;

/// The visible source rectangle, following the current cursor or the bottom when it is hidden.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LiveView {
    pub(crate) column: u16,
    pub(crate) row: u16,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) clipped: bool,
}

impl LiveView {
    /// Keep a clipped live grid's cursor visible and reserve a local indicator above its content.
    /// The indicator is a viewing adaptation: the physical grid and its controller are unchanged.
    pub(crate) fn new(frame: &TerminalTextFrame, columns: u16, rows: u16) -> Self {
        let clipped = frame.geometry.columns > columns || frame.geometry.rows > rows;
        let columns = columns.min(frame.geometry.columns);
        // The existing shell status overlays the pane's last row (including
        // read-only/control feedback). Keep the live cursor above that row.
        let reserved = if clipped {
            rows.saturating_sub(1).min(2)
        } else {
            0
        };
        let rows = rows.saturating_sub(reserved).min(frame.geometry.rows);
        let column = if frame.cursor.visible {
            frame
                .cursor
                .column
                .saturating_sub(columns.saturating_sub(1))
        } else {
            0
        }
        .min(frame.geometry.columns.saturating_sub(columns));
        let row = if frame.cursor.visible {
            frame.cursor.row.saturating_sub(rows.saturating_sub(1))
        } else {
            frame.geometry.rows.saturating_sub(rows)
        }
        .min(frame.geometry.rows.saturating_sub(rows));
        Self {
            column,
            row,
            columns,
            rows,
            clipped,
        }
    }

    /// Geometry of the actual cells below the indicator, in the outer pane's coordinates.
    pub(crate) fn content(self, area: Rect) -> Rect {
        Rect::new(
            area.x,
            area.y
                .saturating_add(u16::from(self.clipped && area.height > 1)),
            self.columns.min(area.width),
            self.rows.min(area.height),
        )
    }
}

#[cfg(test)]
mod tests;
