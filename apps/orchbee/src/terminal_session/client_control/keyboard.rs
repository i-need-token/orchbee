//! Local resize acknowledgement owns no remote Client-view receipt and never queues key repeats.
use super::*;
use crate::client_view::{Notice, PanelDirection};
use orchbee_workbench_protocol::{ClientLayoutChange, ClientPanelDirection, WorkbenchRequestId};

pub(super) struct KeyboardPending {
    pub(super) request_id: WorkbenchRequestId,
    pub(super) flow: SharedAdjustment,
    deadline: Instant,
    interrupted: bool,
    awaiting_draw: bool,
}

pub(in crate::terminal_session) enum KeyboardFeedback {
    Clear,
    Notice(Notice),
}

impl ClientControl {
    pub(in crate::terminal_session) fn resize(
        &mut self,
        native: &NativeUi,
        direction: PanelDirection,
        fine: bool,
    ) {
        if self.stopped || self.pending.is_some() || self.keyboard.is_some() {
            self.keyboard_feedback = Some(KeyboardFeedback::Notice(Notice::ResizeBusy));
            return;
        }
        let direction = match direction {
            PanelDirection::Left => ClientPanelDirection::Left,
            PanelDirection::Right => ClientPanelDirection::Right,
            PanelDirection::Up => ClientPanelDirection::Up,
            PanelDirection::Down => ClientPanelDirection::Down,
        };
        let result = native
            .prepare_keyboard_adjustment(direction, fine, self.viewport)
            .and_then(|(site, adjustment, before)| {
                if !adjustment.change.changed() {
                    let notice = match adjustment.change {
                        ClientLayoutChange::NoSplit {} => Notice::ResizeNoSplit,
                        ClientLayoutChange::Split { .. } => Notice::ResizeLimit,
                    };
                    self.keyboard_feedback = Some(KeyboardFeedback::Notice(notice));
                    return Ok(());
                }
                let request_id = native.keyboard_adjustment_request_id()?;
                let (flow, mutation) =
                    SharedAdjustment::prepare(site, request_id.clone(), adjustment, before)?;
                if !self
                    .lane
                    .as_ref()
                    .is_some_and(|lane| lane.submit_layout(mutation))
                {
                    return Err(Reason::Busy);
                }
                self.keyboard = Some(KeyboardPending {
                    request_id,
                    flow,
                    deadline: self.clock.now() + super::super::state::OPERATION_TOTAL,
                    interrupted: false,
                    awaiting_draw: false,
                });
                self.keyboard_feedback = Some(KeyboardFeedback::Clear);
                Ok(())
            });
        if let Err(reason) = result {
            self.keyboard_feedback = Some(KeyboardFeedback::Notice(if reason == Reason::Busy {
                Notice::ResizeBusy
            } else {
                Notice::ResizeFailed
            }));
        }
    }

    pub(super) fn interrupt_keyboard(&mut self) {
        if let Some(pending) = &mut self.keyboard {
            pending.interrupted = true;
        }
    }

    pub(in crate::terminal_session) fn keyboard_pending(&self) -> bool {
        self.keyboard.is_some()
    }
    pub(in crate::terminal_session) fn take_keyboard_feedback(
        &mut self,
    ) -> Option<KeyboardFeedback> {
        self.keyboard_feedback.take()
    }

    pub(super) fn advance_keyboard(&mut self, native: &mut NativeUi, too_small: bool) -> bool {
        let Some(pending) = &mut self.keyboard else {
            return false;
        };
        let step = if pending.interrupted || too_small || native.control_modal() {
            ControlStep::Failed(Reason::Interrupted)
        } else if self.clock.now() >= pending.deadline {
            ControlStep::Failed(Reason::Expired)
        } else {
            pending.flow.step(native)
        };
        match step {
            ControlStep::Waiting => false,
            ControlStep::Changed => true,
            ControlStep::Observed => {
                pending.awaiting_draw = true;
                true
            }
            ControlStep::Failed(_) => {
                self.keyboard_feedback =
                    Some(KeyboardFeedback::Notice(if pending.flow.had_effect() {
                        Notice::ResizeUnknown
                    } else {
                        Notice::ResizeFailed
                    }));
                self.keyboard = None;
                true
            }
            // A local keyboard adjustment is always selected; inactive and navigation outputs are inadmissible.
            ControlStep::Completed(_) | ControlStep::ZoomUnchanged => {
                self.keyboard_feedback = Some(KeyboardFeedback::Notice(Notice::ResizeUnknown));
                self.keyboard = None;
                true
            }
        }
    }

    pub(super) fn keyboard_after_draw(&mut self, native: &NativeUi) {
        if self.keyboard.as_ref().is_some_and(|pending| {
            pending.awaiting_draw
                && !pending.interrupted
                && self.clock.now() < pending.deadline
                && pending.flow.outcome(native).is_some()
        }) {
            self.keyboard = None;
            self.keyboard_feedback = Some(KeyboardFeedback::Clear);
        }
    }
}
