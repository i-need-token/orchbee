//! Attachment-local selection commands and complete Host-owned text admission.
mod export;
mod record;
use crate::terminal_input::{PointerSelectionCell, PointerSelectionIntent};
use orchbee_client_server_protocol::{
    TerminalViewerAction, ViewerId, ViewerSelectionExportCancelReason, ViewerSelectionExportFormat,
    ViewerSelectionGestureAction, ViewerSelectionGestureGeometry, ViewerSelectionGesturePosition,
    ViewerSelectionViewportPoint,
};
use orchbee_terminal_protocol::{
    AttachmentId, SelectionChanged, SelectionChangedState, SelectionExportChunk,
    SelectionExportFinished, SelectionExportStarted,
};
pub(super) use record::{Event, event as decode_event, reply as decode_reply};
use std::{collections::VecDeque, num::NonZeroU64};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Invalid,
    Rejected,
    IdentityExhausted,
}

impl From<export::Failure> for Failure {
    fn from(value: export::Failure) -> Self {
        match value {
            export::Failure::Invalid => Self::Invalid,
            export::Failure::Memory => Self::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Gesture {
        generation: NonZeroU64,
        gesture: ViewerSelectionGestureAction,
    },
    Export {
        sequence: NonZeroU64,
    },
    Ack {
        sequence: NonZeroU64,
        chunk: u32,
    },
    CancelExport {
        sequence: NonZeroU64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Command {
    serial: NonZeroU64,
    action: Action,
}

impl Command {
    pub(super) fn viewer_action(self, attachment_id: ViewerId) -> TerminalViewerAction {
        match self.action {
            Action::Gesture {
                generation,
                gesture,
            } => TerminalViewerAction::SelectionGesture {
                attachment_id,
                generation,
                gesture,
            },
            Action::Export { sequence } => TerminalViewerAction::SelectionExport {
                attachment_id,
                export_sequence: sequence,
                format: ViewerSelectionExportFormat::Plain,
            },
            Action::Ack { sequence, chunk } => TerminalViewerAction::SelectionExportChunkApplied {
                attachment_id,
                export_sequence: sequence,
                chunk_sequence: chunk,
            },
            Action::CancelExport { sequence } => TerminalViewerAction::SelectionExportCancel {
                attachment_id,
                export_sequence: sequence,
                reason: ViewerSelectionExportCancelReason::Replaced,
            },
        }
    }
}

pub(super) enum Reply {
    Changed(SelectionChanged),
    Started(SelectionExportStarted),
    Ack,
    Finished(SelectionExportFinished),
    Failed { stale_generation: bool },
}

#[derive(Clone, Copy)]
struct Gesture {
    generation: NonZeroU64,
    columns: u16,
    rows: u16,
    closing: bool,
    cancelled: bool,
}

pub(super) struct Driver {
    attachment: AttachmentId,
    generation: Option<NonZeroU64>,
    gesture: Option<Gesture>,
    queue: VecDeque<Command>,
    flight: Option<Command>,
    serial: u64,
    last_export: u64,
    export: Option<(NonZeroU64, export::Export)>,
    selection: SelectionChangedState,
    copy_ready: Option<(NonZeroU64, String)>,
}

impl Driver {
    pub(super) fn new(attachment: AttachmentId) -> Self {
        Self {
            attachment,
            generation: None,
            gesture: None,
            queue: VecDeque::new(),
            flight: None,
            serial: 0,
            last_export: 0,
            export: None,
            selection: SelectionChangedState::None,
            copy_ready: None,
        }
    }

    pub(super) fn selection(&self) -> &SelectionChangedState {
        &self.selection
    }

    pub(super) fn observe_generation(&mut self, generation: NonZeroU64) -> Result<(), Failure> {
        if self.generation.is_some_and(|old| old != generation) {
            self.queue.clear();
            self.flight = None;
            self.gesture = None;
            self.copy_ready = None;
            self.cancel_export()?;
        }
        self.generation = Some(generation);
        Ok(())
    }

    pub(super) fn pointer(
        &mut self,
        generation: NonZeroU64,
        columns: u16,
        rows: u16,
        intent: PointerSelectionIntent,
    ) -> Result<bool, Failure> {
        self.observe_generation(generation)?;
        match intent {
            PointerSelectionIntent::Start { click_count, cell } => {
                if self.gesture.is_some()
                    || !(1..=2).contains(&click_count)
                    || cell.column >= columns
                    || cell.row >= rows
                {
                    return Ok(false);
                }
                self.copy_ready = None;
                self.cancel_export()?;
                self.gesture = Some(Gesture {
                    generation,
                    columns,
                    rows,
                    closing: false,
                    cancelled: false,
                });
                self.schedule(Action::Gesture {
                    generation,
                    gesture: ViewerSelectionGestureAction::Start {
                        click_count,
                        point: point(cell),
                        position: position(cell),
                        select_output: false,
                    },
                })?;
            }
            PointerSelectionIntent::Update { cell } | PointerSelectionIntent::End { cell } => {
                let Some(mut gesture) = self.gesture else {
                    return Ok(false);
                };
                if gesture.closing
                    || gesture.cancelled
                    || cell.column >= gesture.columns
                    || cell.row >= gesture.rows
                {
                    return Ok(false);
                }
                self.queue.retain(|command| {
                    !matches!(
                        command.action,
                        Action::Gesture {
                            gesture: ViewerSelectionGestureAction::Update { .. },
                            ..
                        }
                    )
                });
                self.schedule(Action::Gesture {
                    generation: gesture.generation,
                    gesture: ViewerSelectionGestureAction::Update {
                        geometry: ViewerSelectionGestureGeometry {
                            cell_width: 1,
                            columns: gesture.columns,
                            padding_left: 0,
                            screen_height: gesture.rows,
                        },
                        point: point(cell),
                        position: position(cell),
                        rectangle: false,
                    },
                })?;
                if matches!(intent, PointerSelectionIntent::End { .. }) {
                    gesture.closing = true;
                    self.gesture = Some(gesture);
                    self.schedule(Action::Gesture {
                        generation: gesture.generation,
                        gesture: ViewerSelectionGestureAction::End { point: point(cell) },
                    })?;
                }
            }
            PointerSelectionIntent::Cancel => self.cancel()?,
        }
        Ok(true)
    }

    pub(super) fn cancel(&mut self) -> Result<(), Failure> {
        self.copy_ready = None;
        self.queue.clear();
        if let Some(mut gesture) = self.gesture {
            gesture.cancelled = true;
            self.gesture = Some(gesture);
            self.schedule(Action::Gesture {
                generation: gesture.generation,
                gesture: ViewerSelectionGestureAction::Cancel,
            })?;
        }
        self.cancel_export()
    }

    fn cancel_export(&mut self) -> Result<(), Failure> {
        if let Some((sequence, _)) = self.export.take() {
            self.schedule(Action::CancelExport { sequence })?;
        }
        Ok(())
    }

    fn schedule(&mut self, action: Action) -> Result<(), Failure> {
        if self.queue.len() >= 5 {
            return Err(Failure::Invalid);
        }
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or(Failure::IdentityExhausted)?;
        self.queue.push_back(Command {
            serial: NonZeroU64::new(self.serial).ok_or(Failure::IdentityExhausted)?,
            action,
        });
        Ok(())
    }

    pub(super) fn pending(&self) -> Option<Command> {
        if self.flight.is_some() {
            None
        } else {
            self.queue.front().copied()
        }
    }

    pub(super) fn admitted(&mut self, command: Command) -> Result<(), Failure> {
        if self.pending() != Some(command) {
            return Err(Failure::Invalid);
        }
        self.queue.pop_front();
        self.flight = Some(command);
        Ok(())
    }

    pub(super) fn reply(&mut self, command: Command, reply: Reply) -> Result<bool, Failure> {
        if self.flight != Some(command) {
            return Ok(false);
        }
        self.flight = None;
        if let Reply::Failed { stale_generation } = reply {
            self.queue.clear();
            self.gesture = None;
            self.copy_ready = None;
            self.export = None;
            return if stale_generation && matches!(command.action, Action::Gesture { .. }) {
                Ok(true)
            } else {
                Err(Failure::Rejected)
            };
        }
        match (command.action, reply) {
            (Action::Gesture { gesture, .. }, Reply::Changed(value)) => {
                self.changed(value)?;
                let cancelled = self.gesture.is_none_or(|gesture| gesture.cancelled);
                if matches!(gesture, ViewerSelectionGestureAction::End { .. }) {
                    self.gesture = None;
                    if !cancelled && matches!(self.selection, SelectionChangedState::Active { .. })
                    {
                        self.last_export = self
                            .last_export
                            .checked_add(1)
                            .ok_or(Failure::IdentityExhausted)?;
                        let sequence =
                            NonZeroU64::new(self.last_export).ok_or(Failure::IdentityExhausted)?;
                        self.export =
                            Some((sequence, export::Export::new(self.attachment, sequence)));
                        self.schedule(Action::Export { sequence })?;
                    }
                } else if matches!(gesture, ViewerSelectionGestureAction::Cancel) {
                    self.gesture = None;
                }
            }
            (Action::Export { sequence }, Reply::Started(value)) => {
                if let Some((current, export)) = self.export.as_mut() {
                    if *current != sequence {
                        return Err(Failure::Invalid);
                    }
                    if let Some(chunk) = export.started(value)? {
                        self.schedule(Action::Ack { sequence, chunk })?;
                    }
                }
            }
            (Action::Ack { sequence, chunk }, Reply::Ack) => {
                if let Some((current, export)) = self.export.as_mut() {
                    if *current != sequence {
                        return Err(Failure::Invalid);
                    }
                    export.acknowledged(chunk)?;
                    self.complete_export()?;
                }
            }
            (Action::CancelExport { sequence }, Reply::Finished(value))
                if value.attachment_id == self.attachment
                    && value.export_sequence == sequence
                    && value.status
                        == orchbee_terminal_protocol::SelectionExportStatus::Cancelled => {}
            _ => return Err(Failure::Invalid),
        }
        Ok(true)
    }

    pub(super) fn changed(&mut self, value: SelectionChanged) -> Result<bool, Failure> {
        if value.attachment_id != self.attachment {
            return Err(Failure::Invalid);
        }
        let evicted = matches!(value.state, SelectionChangedState::Evicted);
        self.selection = value.state;
        if evicted {
            self.cancel()?;
        }
        Ok(evicted)
    }

    pub(super) fn chunk(&mut self, value: SelectionExportChunk) -> Result<(), Failure> {
        if value.attachment_id != self.attachment {
            return Err(Failure::Invalid);
        }
        let sequence = value.export_sequence;
        let Some((current, export)) = self.export.as_mut() else {
            return if sequence.get() <= self.last_export {
                Ok(())
            } else {
                Err(Failure::Invalid)
            };
        };
        if *current != sequence {
            return if sequence.get() < current.get() {
                Ok(())
            } else {
                Err(Failure::Invalid)
            };
        }
        if let Some(chunk) = export.chunk(value)? {
            self.schedule(Action::Ack { sequence, chunk })?;
        }
        Ok(())
    }

    pub(super) fn finished(&mut self, value: SelectionExportFinished) -> Result<(), Failure> {
        if value.attachment_id != self.attachment {
            return Err(Failure::Invalid);
        }
        let Some((current, export)) = self.export.as_mut() else {
            return if value.export_sequence.get() <= self.last_export {
                Ok(())
            } else {
                Err(Failure::Invalid)
            };
        };
        if *current != value.export_sequence {
            return if value.export_sequence.get() < current.get() {
                Ok(())
            } else {
                Err(Failure::Invalid)
            };
        }
        export.finished(value)?;
        self.complete_export()
    }

    fn complete_export(&mut self) -> Result<(), Failure> {
        if let Some((sequence, export)) = self.export.as_mut()
            && let Some(text) = export.take_text()?
        {
            self.copy_ready = Some((*sequence, text));
            self.export = None;
        }
        Ok(())
    }

    pub(super) fn take_copy(&mut self) -> Option<(NonZeroU64, String)> {
        self.copy_ready.take()
    }
}

fn point(cell: PointerSelectionCell) -> ViewerSelectionViewportPoint {
    ViewerSelectionViewportPoint {
        column: cell.column,
        row: cell.row,
    }
}
fn position(cell: PointerSelectionCell) -> ViewerSelectionGesturePosition {
    ViewerSelectionGesturePosition {
        x: i32::from(cell.column),
        y: i32::from(cell.row),
    }
}

#[cfg(test)]
mod tests;
