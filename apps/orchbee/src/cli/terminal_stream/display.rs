//! Two real consumers of validated surfaces: NDJSON and the inherited console.
use super::*;
use orchbee_terminal_protocol::TerminalTextFrame;

pub(super) trait Display {
    fn event(&mut self, record: Value, gate: &Gate<'_>) -> Result<(), Failure>;

    fn live(
        &mut self,
        frame: &TerminalTextFrame,
        columns: u16,
        rows: u16,
        gate: &Gate<'_>,
    ) -> Result<(), Failure> {
        self.event(frame::record(frame, columns, rows)?, gate)
    }
    fn history(
        &mut self,
        window: &crate::terminal_session::terminal_viewport::ViewportWindow,
        columns: u16,
        rows: u16,
        gate: &Gate<'_>,
    ) -> Result<(), Failure> {
        self.event(frame::history(window, columns, rows)?, gate)
    }
}

impl Display for relay::Relay {
    fn event(&mut self, record: Value, gate: &Gate<'_>) -> Result<(), Failure> {
        emit(self, record, gate.step()?, gate.cancelled)
    }
}
