//! Conservative live-tail presentation: only a proved empty suffix may reveal retained rows.

// module-layer: presentation

use crate::terminal_viewport::ViewportWindow;
use orchbee_terminal_protocol::{
    MouseTracking, TerminalFrameRoute, TerminalRowWindowRoute, TerminalTextFrame, TerminalTextRow,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Target {
    pub(super) frame: TerminalFrameRoute,
    pub(super) rows_before: u32,
}

impl Target {
    pub(super) fn capture(frame: &TerminalTextFrame) -> Option<Self> {
        let rows_before = blank_rows(frame);
        (rows_before > 0).then_some(Self {
            frame: frame.route,
            rows_before,
        })
    }

    pub(super) fn admits(self, live: &TerminalTextFrame, window: &ViewportWindow) -> bool {
        self.frame == live.route
            && !window.graphics_present
            && window.route.attachment_id == live.route.attachment_id
            && window.route.host_epoch == live.route.host_epoch
            && window.route.generation == live.route.generation
            && window.route.revision == live.route.revision
            && window.route.columns == live.geometry.columns
            && window.route.row_count == live.geometry.rows
            && window.route.total_rows
                == live
                    .scrollback_rows
                    .saturating_add(u64::from(live.geometry.rows))
            && window.route.start_row
                == live
                    .scrollback_rows
                    .saturating_sub(u64::from(self.rows_before))
    }
}

/// The retained rows above the seam of the last acknowledged tail window. A later live frame is
/// painted below them at once while they still describe the same history; the exact read that
/// frame starts replaces them when its window is acknowledged.
#[derive(Debug)]
pub(super) struct RetainedTail {
    route: TerminalRowWindowRoute,
    foreground: u32,
    background: u32,
    rows: Vec<TerminalTextRow>,
}

impl RetainedTail {
    /// Copy the rows above the seam; `None` for a window that is not a graphics-free tail.
    pub(super) fn capture(window: &ViewportWindow) -> Option<Self> {
        let rows_before = window
            .route
            .total_rows
            .checked_sub(u64::from(window.route.row_count))?
            .checked_sub(window.route.start_row)?;
        let rows_before = usize::try_from(rows_before).ok()?;
        if window.graphics_present
            || rows_before == 0
            || rows_before >= usize::from(window.route.row_count)
        {
            return None;
        }
        Some(Self {
            route: window.route,
            foreground: window.foreground,
            background: window.background,
            rows: window
                .rows
                .iter()
                .filter(|row| usize::from(row.index) < rows_before)
                .cloned()
                .collect(),
        })
    }

    pub(super) const fn route(&self) -> &TerminalRowWindowRoute {
        &self.route
    }

    /// Admit `live` below these rows: same attachment, generation, geometry, default colors,
    /// scrollback count and blank suffix, so only rows the live grid no longer shows stay hidden.
    pub(super) fn presents(&self, live: &TerminalTextFrame) -> Option<TailPresentation<'_>> {
        let rows_before = u32::try_from(
            self.route
                .total_rows
                .checked_sub(u64::from(self.route.row_count))?
                .checked_sub(self.route.start_row)?,
        )
        .ok()?;
        (self.route.attachment_id == live.route.attachment_id
            && self.route.host_epoch == live.route.host_epoch
            && self.route.generation == live.route.generation
            && self.route.columns == live.geometry.columns
            && self.route.row_count == live.geometry.rows
            && self.foreground == live.foreground
            && self.background == live.background
            && self.route.total_rows
                == live
                    .scrollback_rows
                    .saturating_add(u64::from(live.geometry.rows))
            && blank_rows(live) == rows_before)
            .then_some(TailPresentation {
                retained: &self.rows,
                foreground: self.foreground,
                background: self.background,
                rows_before,
                start_row: self.route.start_row,
            })
    }
}

/// What an automatic tail paints: retained rows above the seam, live rows below it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TailPresentation<'a> {
    pub(crate) retained: &'a [TerminalTextRow],
    pub(crate) foreground: u32,
    pub(crate) background: u32,
    pub(crate) rows_before: u32,
    /// The absolute row painted first, for selection and find coordinates.
    pub(crate) start_row: u64,
}

fn blank_rows(frame: &TerminalTextFrame) -> u32 {
    if frame.alternate_screen
        || frame.interaction.mouse_tracking() != MouseTracking::None
        || frame.geometry.rows < 2
        || frame.geometry.columns == 0
        || frame.rows.len() != usize::from(frame.geometry.rows)
        || frame.scrollback_rows == 0
        || frame.graphics_present == Some(true)
    {
        return 0;
    }
    let blank = frame
        .rows
        .iter()
        .rev()
        .take(frame.rows.len() - 1)
        .take_while(|row| {
            row.flags == 0
                && row.prompt_class == 0
                && row.cells.len() == usize::from(frame.geometry.columns)
                && !(frame.cursor.visible && frame.cursor.row == row.index)
                && row.cells.iter().all(|cell| {
                    cell.text.trim().is_empty()
                        && cell.style.flags == 0
                        && cell.style.background == 0x1000000
                        && cell.style.underline == 0
                        && cell.style.underline_color == 0x2000000
                })
        })
        .count();
    u32::try_from(blank.min(usize::try_from(frame.scrollback_rows).unwrap_or(usize::MAX)))
        .unwrap_or(0)
}
