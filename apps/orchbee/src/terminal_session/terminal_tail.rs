//! Conservative live-tail presentation: only a proved empty suffix may reveal retained rows.
use super::terminal_viewport::ViewportWindow;
use orchbee_terminal_protocol::{MouseTracking, TerminalFrameRoute, TerminalTextFrame};

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
