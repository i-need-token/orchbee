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

/// Keyboard-copy semantic gestures use the same Host authority as pointer
/// selection without inheriting application mouse-tracking admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SemanticIntent {
    Start(PointerSelectionCell),
    Update(PointerSelectionCell),
    End(PointerSelectionCell),
    Cancel,
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
    copy_on_select: bool,
    attachment: AttachmentId,
    generation: Option<NonZeroU64>,
    gesture: Option<Gesture>,
    queue: VecDeque<Command>,
    flight: Option<Command>,
    /// The last cleanup command that must enter the real mailbox before a
    /// later viewport command for this exact projection may be admitted.
    viewport_fence: Option<NonZeroU64>,
    serial: u64,
    last_export: u64,
    export: Option<(NonZeroU64, export::Export)>,
    selection: SelectionChangedState,
    copy_ready: Option<(NonZeroU64, String)>,
    copy_selection_owned: bool,
}

impl Driver {
    pub(super) fn new(attachment: AttachmentId) -> Self {
        Self {
            copy_on_select: true,
            attachment,
            generation: None,
            gesture: None,
            queue: VecDeque::new(),
            flight: None,
            viewport_fence: None,
            serial: 0,
            last_export: 0,
            export: None,
            selection: SelectionChangedState::None,
            copy_ready: None,
            copy_selection_owned: false,
        }
    }

    pub(super) fn set_copy_on_select(&mut self, enabled: bool) {
        self.copy_on_select = enabled;
    }

    pub(super) fn selection(&self) -> &SelectionChangedState {
        &self.selection
    }

    pub(super) fn observe_generation(&mut self, generation: NonZeroU64) -> Result<(), Failure> {
        if self.generation.is_some_and(|old| old != generation) {
            let retained = self.queued_cancel_export()?;
            let new_commands = if self.export.is_some() { 1 } else { 0 };
            self.reserve_cleanup(retained.iter().count(), new_commands)?;
            self.replace_queue(retained);
            self.flight = None;
            self.viewport_fence = None;
            self.gesture = None;
            self.copy_ready = None;
            self.copy_selection_owned = false;
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
        let (semantic, click_count) = match intent {
            PointerSelectionIntent::Start { click_count, cell } => {
                (SemanticIntent::Start(cell), click_count)
            }
            PointerSelectionIntent::Update { cell } => (SemanticIntent::Update(cell), 1),
            PointerSelectionIntent::End { cell } => (SemanticIntent::End(cell), 1),
            PointerSelectionIntent::Cancel => (SemanticIntent::Cancel, 1),
        };
        self.gesture(generation, columns, rows, semantic, click_count)
    }

    pub(super) fn semantic(
        &mut self,
        generation: NonZeroU64,
        columns: u16,
        rows: u16,
        intent: SemanticIntent,
    ) -> Result<bool, Failure> {
        let started = matches!(intent, SemanticIntent::Start(_));
        let admitted = self.gesture(generation, columns, rows, intent, 1)?;
        if started && admitted {
            self.copy_selection_owned = true;
        }
        Ok(admitted)
    }

    fn gesture(
        &mut self,
        generation: NonZeroU64,
        columns: u16,
        rows: u16,
        intent: SemanticIntent,
        click_count: u8,
    ) -> Result<bool, Failure> {
        self.observe_generation(generation)?;
        match intent {
            SemanticIntent::Start(cell) => {
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
            SemanticIntent::Update(cell) | SemanticIntent::End(cell) => {
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
                if matches!(intent, SemanticIntent::End(_)) {
                    gesture.closing = true;
                    self.gesture = Some(gesture);
                    self.schedule(Action::Gesture {
                        generation: gesture.generation,
                        gesture: ViewerSelectionGestureAction::End { point: point(cell) },
                    })?;
                }
            }
            SemanticIntent::Cancel => self.cancel()?,
        }
        Ok(true)
    }

    pub(super) fn cancel(&mut self) -> Result<(), Failure> {
        if self.viewport_fence.is_some() {
            return Ok(());
        }
        let retained = self.queued_cancel_export()?;
        let new_commands = (if self.export.is_some() { 1 } else { 0 })
            + (if self.gesture.is_some() { 1 } else { 0 });
        self.reserve_cleanup(retained.iter().count(), new_commands)?;
        self.viewport_fence = None;
        self.copy_ready = None;
        self.replace_queue(retained);
        if let Some(mut gesture) = self.gesture {
            gesture.cancelled = true;
            self.gesture = Some(gesture);
        }
        self.cancel_export()?;
        if let Some(gesture) = self.gesture {
            self.schedule(Action::Gesture {
                generation: gesture.generation,
                gesture: ViewerSelectionGestureAction::Cancel,
            })?;
        }
        Ok(())
    }

    /// Atomically queue cleanup for a selection established by this copy-mode
    /// episode. A valid single-cell Start replaces the retained Host selection;
    /// the final Cancel then releases the cleanup gesture. Existing mailbox
    /// flight is preserved and every new cleanup command follows it.
    pub(super) fn cancel_copy_selection(
        &mut self,
        generation: NonZeroU64,
        columns: u16,
        rows: u16,
        cell: PointerSelectionCell,
    ) -> Result<bool, Failure> {
        if !self.copy_selection_owned {
            self.cancel()?;
            return Ok(false);
        }
        if self.generation != Some(generation)
            || columns == 0
            || rows == 0
            || cell.column >= columns
            || cell.row >= rows
        {
            return Err(Failure::Rejected);
        }
        let retained = self.queued_cancel_export()?;
        let new_commands = (if self.export.is_some() { 1 } else { 0 }) + 2;
        self.reserve_cleanup(retained.iter().count(), new_commands)?;
        self.viewport_fence = None;
        self.copy_ready = None;
        self.replace_queue(retained);
        if let Some(mut gesture) = self.gesture {
            gesture.cancelled = true;
            self.gesture = Some(gesture);
        }
        self.cancel_export()?;
        self.gesture = Some(Gesture {
            generation,
            columns,
            rows,
            closing: false,
            cancelled: true,
        });
        self.schedule(Action::Gesture {
            generation,
            gesture: ViewerSelectionGestureAction::Start {
                click_count: 1,
                point: point(cell),
                position: position(cell),
                select_output: false,
            },
        })?;
        self.schedule(Action::Gesture {
            generation,
            gesture: ViewerSelectionGestureAction::Cancel,
        })?;
        self.copy_selection_owned = false;
        Ok(true)
    }

    /// Cancel an unsettled pointer/keyboard gesture before a copy-mode entry
    /// may place any later viewport command in the shared mailbox. A settled
    /// Host selection remains available for explicit export.
    pub(super) fn cancel_before_viewport(&mut self) -> Result<(), Failure> {
        if self.viewport_fence.is_some() {
            return Ok(());
        }
        self.cancel()?;
        self.arm_viewport_fence();
        Ok(())
    }

    /// Clear copy mode's selection before a viewport recovery may follow it.
    pub(super) fn cancel_copy_selection_before_viewport(
        &mut self,
        generation: NonZeroU64,
        columns: u16,
        rows: u16,
        cell: PointerSelectionCell,
    ) -> Result<(), Failure> {
        self.cancel_copy_selection(generation, columns, rows, cell)?;
        self.arm_viewport_fence();
        Ok(())
    }

    /// A stale or unavailable surface cannot safely replace the Host
    /// selection. End the local ownership claim, cancel only work whose
    /// identity remains known, and fence a later viewport command behind the
    /// final command that could actually be issued.
    pub(super) fn abandon_copy_selection_before_viewport(&mut self) -> Result<(), Failure> {
        let retained = self.queued_cancel_export()?;
        let new_commands = (if self.export.is_some() { 1 } else { 0 })
            + (if self.gesture.is_some() { 1 } else { 0 });
        self.reserve_cleanup(retained.iter().count(), new_commands)?;
        self.viewport_fence = None;
        self.copy_ready = None;
        self.replace_queue(retained);
        if let Some(mut gesture) = self.gesture {
            gesture.cancelled = true;
            self.gesture = Some(gesture);
        }
        self.cancel_export()?;
        if let Some(gesture) = self.gesture {
            self.schedule(Action::Gesture {
                generation: gesture.generation,
                gesture: ViewerSelectionGestureAction::Cancel,
            })?;
        }
        self.copy_selection_owned = false;
        self.arm_viewport_fence();
        Ok(())
    }

    fn arm_viewport_fence(&mut self) {
        self.viewport_fence = self.queue.back().map(|command| command.serial);
    }

    /// `true` only while the exact final cleanup command has not yet entered
    /// the real mailbox. This is an ordering fence, not a claim that Host
    /// cleanup succeeded.
    pub(super) const fn viewport_blocked(&self) -> bool {
        self.viewport_fence.is_some()
    }

    pub(super) const fn copy_selection_owned(&self) -> bool {
        self.copy_selection_owned
    }

    fn reserve_cleanup(&self, retained: usize, new_commands: usize) -> Result<(), Failure> {
        if retained.saturating_add(new_commands) > 5 {
            return Err(Failure::Invalid);
        }
        self.serial
            .checked_add(u64::try_from(new_commands).map_err(|_| Failure::IdentityExhausted)?)
            .ok_or(Failure::IdentityExhausted)
            .map(|_| ())
    }

    fn queued_cancel_export(&self) -> Result<Option<Command>, Failure> {
        let mut retained = self
            .queue
            .iter()
            .copied()
            .filter(|command| matches!(command.action, Action::CancelExport { .. }));
        let first = retained.next();
        if retained.next().is_some() {
            return Err(Failure::Invalid);
        }
        Ok(first)
    }

    fn replace_queue(&mut self, retained: Option<Command>) {
        self.queue.clear();
        self.queue.extend(retained);
    }

    fn cancel_export(&mut self) -> Result<(), Failure> {
        if let Some((sequence, _)) = self.export.take() {
            self.schedule(Action::CancelExport { sequence })?;
        }
        Ok(())
    }

    /// Explicit retry after a complete failed/unknown clipboard attempt. The
    /// Host selection remains authoritative and a new export identity is
    /// minted; no earlier bytes or sequence are replayed.
    pub(super) fn export_active(&mut self) -> Result<bool, Failure> {
        if self.gesture.is_some()
            || self.export.is_some()
            || self.copy_ready.is_some()
            || !self.copy_selection_owned
            || !matches!(self.selection, SelectionChangedState::Active { .. })
        {
            return Ok(false);
        }
        self.start_export()?;
        Ok(true)
    }

    fn start_export(&mut self) -> Result<(), Failure> {
        self.last_export = self
            .last_export
            .checked_add(1)
            .ok_or(Failure::IdentityExhausted)?;
        let sequence = NonZeroU64::new(self.last_export).ok_or(Failure::IdentityExhausted)?;
        self.export = Some((sequence, export::Export::new(self.attachment, sequence)));
        self.schedule(Action::Export { sequence })
    }

    pub(super) fn gesture_settled(&self) -> bool {
        !self
            .queue
            .iter()
            .any(|command| matches!(command.action, Action::Gesture { .. }))
            && self
                .flight
                .is_none_or(|command| !matches!(command.action, Action::Gesture { .. }))
    }

    /// Whether the locally-owned Start/Update intent remains open. Host may
    /// legitimately report `None` until a drag crosses its cell threshold.
    pub(super) fn gesture_intent_active(&self) -> bool {
        self.gesture
            .is_some_and(|gesture| !gesture.closing && !gesture.cancelled)
    }

    pub(super) fn host_selection_active(&self) -> bool {
        matches!(self.selection, SelectionChangedState::Active { .. })
    }

    pub(super) fn export_pending(&self) -> bool {
        self.export.is_some()
            || self.copy_ready.is_some()
            || self
                .queue
                .iter()
                .any(|command| matches!(command.action, Action::Export { .. }))
            || self
                .flight
                .is_some_and(|command| matches!(command.action, Action::Export { .. }))
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
        if self.viewport_fence == Some(command.serial) {
            self.viewport_fence = None;
        }
        Ok(())
    }

    pub(super) fn reply(&mut self, command: Command, reply: Reply) -> Result<bool, Failure> {
        if self.flight != Some(command) {
            return Ok(false);
        }
        self.flight = None;
        if let Reply::Failed { stale_generation } = reply {
            self.queue.clear();
            self.viewport_fence = None;
            self.gesture = None;
            self.copy_ready = None;
            self.export = None;
            if matches!(command.action, Action::Gesture { .. }) {
                self.copy_selection_owned = false;
            }
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
                    if !cancelled
                        && (self.copy_on_select || self.copy_selection_owned)
                        && matches!(self.selection, SelectionChangedState::Active { .. })
                    {
                        self.start_export()?;
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
            self.copy_selection_owned = false;
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
