//! CLI adapter for the same retained-history state machine used by the TUI.
use super::display::Display;
use super::{
    Action, Failure, FailureKind, Gate, Instant, LocalClient, Outcome, TerminalProjection,
    ViewerId, classify, exchange, json,
};
use crate::terminal_session::terminal_viewport::{
    TerminalViewport, ViewportEffect, ViewportNav, ViewportReadRejection, ViewportStep,
    ViewportSurface, ViewportWindowFailure, decode_history_changed, decode_viewport_window,
};
use orchbee_client_server_protocol::{HistoryDemand, ProjectionDemand};
use orchbee_terminal_protocol::TerminalTextFrame;
#[cfg(test)]
use serde_json::Value;

#[derive(Default)]
pub(super) struct History {
    core: Option<TerminalViewport>,
    sequence: u64,
    event_sequence: u64,
}
impl History {
    pub(super) fn active(&self) -> bool {
        self.core.is_some()
    }
    fn apply(&mut self, effect: ViewportEffect) -> Result<(), Failure> {
        if effect.protocol_error {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        if effect.live {
            self.core = None;
        }
        Ok(())
    }
    pub(super) fn scroll(
        &mut self,
        lines: i64,
        frame: &TerminalTextFrame,
        client: &mut LocalClient,
        viewer: &ViewerId,
        gate: &Gate<'_>,
    ) -> Result<(), Failure> {
        if let Some(core) = self.core.as_mut() {
            let effect = core.nav(ViewportNav::Lines(lines));
            return self.apply(effect);
        }
        if lines <= 0 || frame.alternate_screen || frame.scrollback_rows == 0 {
            return Ok(());
        }
        feedback(
            client,
            Action::SetHistoryDemand {
                attachment_id: viewer.clone(),
                demand: HistoryDemand::ViewingHistory,
            },
            gate,
        )?;
        feedback(
            client,
            Action::SetProjectionDemand {
                attachment_id: viewer.clone(),
                demand: ProjectionDemand::Paused,
            },
            gate,
        )?;
        let mut core = TerminalViewport::entering(
            frame.route.attachment_id,
            frame.route.host_epoch,
            frame.geometry.columns,
            frame.geometry.rows,
            u32::try_from(lines).unwrap_or(u32::MAX),
            self.sequence
                .checked_add(1)
                .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?,
            Instant::now(),
        );
        core.enter_outcome(true);
        self.core = Some(core);
        Ok(())
    }
    pub(super) fn resume(
        &mut self,
        client: &mut LocalClient,
        viewer: &ViewerId,
        gate: &Gate<'_>,
    ) -> Result<(), Failure> {
        if let Some(core) = self.core.as_mut() {
            core.typed();
        }
        self.advance(client, viewer, gate, None)?;
        Ok(())
    }
    pub(super) fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Failure> {
        if let Some(core) = self.core.as_mut() {
            let effect = core.resize(cols, rows, Instant::now());
            self.apply(effect)?;
        }
        Ok(())
    }
    pub(super) fn event(
        &mut self,
        record: &[u8],
        projection: &TerminalProjection,
    ) -> Result<(), Failure> {
        let changed = decode_history_changed(record, projection.attachment(), projection.epoch())
            .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?;
        if let Some(changed) = changed {
            if changed.sequence.get() <= self.event_sequence {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
            self.event_sequence = changed.sequence.get();
            if let Some(core) = self.core.as_mut() {
                let effect = core.facts(changed);
                self.apply(effect)?;
            }
        }
        Ok(())
    }
    pub(super) fn frame(&mut self) -> Result<(), Failure> {
        if let Some(core) = self.core.as_mut() {
            let effect = core.frame_applied();
            self.apply(effect)?;
        }
        Ok(())
    }
    pub(super) fn advance(
        &mut self,
        client: &mut LocalClient,
        viewer: &ViewerId,
        gate: &Gate<'_>,
        mut relay: Option<&mut dyn Display>,
    ) -> Result<(), Failure> {
        let Some(core) = self.core.as_mut() else {
            return Ok(());
        };
        let effect = core.tick(Instant::now());
        self.apply(effect)?;
        let Some(core) = self.core.as_mut() else {
            return Ok(());
        };
        let Some(step) = core.peek_command() else {
            return Ok(());
        };
        // Rows are acknowledged only after their complete active surface was flushed.
        if matches!(step, ViewportStep::Ack { .. }) && relay.is_none() {
            return Ok(());
        }
        core.command_admitted();
        let effect = match step {
            ViewportStep::Enter => return Err(Failure::new(FailureKind::ProtocolInvalid)),
            ViewportStep::Read {
                origin,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
            } => {
                self.sequence = request_sequence.get();
                match exchange(
                    client,
                    Action::ReadViewportRows {
                        detail: orchbee_client_server_protocol::ViewerRowDetail::Render,
                        attachment_id: viewer.clone(),
                        origin,
                        row_count,
                        revision,
                        generation,
                        request_sequence,
                        maximum_bytes,
                    },
                    gate,
                )? {
                    Outcome::Record(record) => match decode_viewport_window(&record, maximum_bytes) {
                        Ok(window) => core.window(window, Instant::now()),
                        Err(ViewportWindowFailure::Invalid) => return Err(Failure::new(FailureKind::ProtocolInvalid)),
                        Err(ViewportWindowFailure::MemoryLimit) => core.read_outcome(request_sequence,
                            ViewportReadRejection::Typed(orchbee_client_server_protocol::ViewerRowsRejectedReason::ResourceExhausted), Instant::now()),
                    },
                    Outcome::RowsRejected { reason } => core.read_outcome(
                        request_sequence,
                        ViewportReadRejection::Typed(reason),
                        Instant::now(),
                    ),
                    Outcome::Rejected { .. } => core.read_outcome(
                        request_sequence,
                        ViewportReadRejection::Host,
                        Instant::now(),
                    ),
                    other => return Err(classify(other)),
                }
            }
            ViewportStep::Ack {
                request_sequence,
                generation,
                revision,
            } => {
                present(core, *relay.as_mut().expect("ACK requires output"), gate)?;
                feedback(
                    client,
                    Action::RowsApplied {
                        attachment_id: viewer.clone(),
                        request_sequence,
                        generation,
                        revision,
                    },
                    gate,
                )?;
                core.ack_outcome(request_sequence, true)
            }
            ViewportStep::Recover => {
                feedback(
                    client,
                    Action::SetHistoryDemand {
                        attachment_id: viewer.clone(),
                        demand: HistoryDemand::FollowingBottom,
                    },
                    gate,
                )?;
                feedback(
                    client,
                    Action::SetProjectionDemand {
                        attachment_id: viewer.clone(),
                        demand: ProjectionDemand::Active,
                    },
                    gate,
                )?;
                core.recovered();
                ViewportEffect::default()
            }
        };
        self.apply(effect)
    }
    pub(super) fn present(&self, output: &mut dyn Display, gate: &Gate<'_>) -> Result<(), Failure> {
        if let Some(core) = self.core.as_ref() {
            present(core, output, gate)
        } else {
            output.event(json!({"kind":"reset","reason":"history-recovering"}), gate)
        }
    }
    #[cfg(test)]
    pub(super) fn record(&self) -> Result<Value, Failure> {
        self.core.as_ref().map_or_else(
            || Ok(json!({"kind":"reset","reason":"history-recovering"})),
            surface,
        )
    }
}
fn feedback(client: &mut LocalClient, action: Action, gate: &Gate<'_>) -> Result<(), Failure> {
    match exchange(client, action, gate)? {
        Outcome::FeedbackAccepted {} => Ok(()),
        other => Err(classify(other)),
    }
}
fn present(
    core: &TerminalViewport,
    output: &mut dyn Display,
    gate: &Gate<'_>,
) -> Result<(), Failure> {
    let ViewportSurface::Window(window) = core.surface() else {
        return output.event(json!({"kind":"reset","reason":"history-pending"}), gate);
    };
    let (columns, rows) = core.geometry();
    output.history(window, columns, rows, gate)
}
#[cfg(test)]
fn surface(core: &TerminalViewport) -> Result<Value, Failure> {
    let ViewportSurface::Window(window) = core.surface() else {
        return Ok(json!({"kind":"reset","reason":"history-pending"}));
    };
    let (columns, rows) = core.geometry();
    super::frame::history(window, columns, rows)
}

#[cfg(test)]
mod tests;
