//! One local resize acknowledgement for keys and divider releases; no remote receipts or backlog.
use super::*;
use crate::client_view::{Notice, PanelDirection};
use orchbee_workbench_protocol::{ClientLayoutChange, ClientPanelDirection, WorkbenchRequestId};

pub(super) struct LocalResizePending {
    pub(super) request_id: WorkbenchRequestId,
    pub(super) flow: SharedAdjustment,
    deadline: Instant,
    interrupted: bool,
    awaiting_draw: bool,
}

pub(in crate::terminal_session) enum LocalResizeFeedback {
    Clear,
    Notice(Notice),
}

impl ClientControl {
    pub(in crate::terminal_session) fn resize_drawn(
        &mut self,
        native: &NativeUi,
        target: &crate::tui_layout::DrawnResize,
        direction: PanelDirection,
        fine: bool,
    ) {
        if native.view().divider_source().as_ref() != Some(&target.source)
            || self.viewport != Some((target.viewport, target.sidebar))
        {
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(Notice::ResizeFailed));
            return;
        }
        self.resize(native, direction, fine);
    }
    pub(in crate::terminal_session) fn resize(
        &mut self,
        native: &NativeUi,
        direction: PanelDirection,
        fine: bool,
    ) {
        let direction = match direction {
            PanelDirection::Left => ClientPanelDirection::Left,
            PanelDirection::Right => ClientPanelDirection::Right,
            PanelDirection::Up => ClientPanelDirection::Up,
            PanelDirection::Down => ClientPanelDirection::Down,
        };
        self.submit_local(
            native,
            native.prepare_keyboard_adjustment(direction, fine, self.viewport),
        );
    }

    pub(in crate::terminal_session) fn divider_ready(
        &mut self,
        native: &NativeUi,
        target: &crate::tui_layout::DrawnDivider,
    ) -> Result<(), Notice> {
        let result = if self.stopped {
            Err(Notice::ResizeFailed)
        } else if self.pending.is_some() || self.local_resize.is_some() {
            Err(Notice::ResizeBusy)
        } else {
            self.lane
                .as_ref()
                .ok_or(Reason::ProjectionUnavailable)
                .and_then(ClientControlLane::layout_admission)
                .and_then(|()| {
                    native
                        .prepare_divider_adjustment(target, target.ratio, self.viewport)
                        .map(|_| ())
                })
                .map_err(|reason| {
                    if reason == Reason::Busy {
                        Notice::ResizeBusy
                    } else {
                        Notice::ResizeFailed
                    }
                })
        };
        if let Err(notice) = result {
            // A newer pointer refusal supersedes submission's earlier Clear in this input batch.
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(notice));
        }
        result
    }

    pub(in crate::terminal_session) fn resize_divider(
        &mut self,
        native: &NativeUi,
        target: &crate::tui_layout::DrawnDivider,
        ratio: orchbee_workbench_protocol::ClientSplitRatio,
    ) {
        self.submit_local(
            native,
            native.prepare_divider_adjustment(target, ratio, self.viewport),
        );
    }

    fn submit_local(
        &mut self,
        native: &NativeUi,
        prepared: Result<
            (
                orchbee_workbench_protocol::SiteId,
                ClientLayoutAdjustment,
                ClientViewObservation,
            ),
            Reason,
        >,
    ) {
        if self.stopped {
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(Notice::ResizeFailed));
            return;
        }
        if self.pending.is_some() || self.local_resize.is_some() {
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(Notice::ResizeBusy));
            return;
        }
        let result = prepared.and_then(|(site, adjustment, before)| {
            if !adjustment.change.changed() {
                let notice = match adjustment.change {
                    ClientLayoutChange::NoSplit {} => Notice::ResizeNoSplit,
                    ClientLayoutChange::Split { .. } => Notice::ResizeLimit,
                };
                self.local_resize_feedback = Some(LocalResizeFeedback::Notice(notice));
                return Ok(());
            }
            let request_id = native.local_adjustment_request_id()?;
            let (flow, mutation) =
                SharedAdjustment::prepare(site, request_id.clone(), adjustment, before)?;
            self.lane
                .as_ref()
                .ok_or(Reason::ProjectionUnavailable)?
                .submit_layout(mutation)?;
            self.local_resize = Some(LocalResizePending {
                request_id,
                flow,
                deadline: self.clock.now() + super::super::state::OPERATION_TOTAL,
                interrupted: false,
                awaiting_draw: false,
            });
            self.local_resize_feedback = Some(LocalResizeFeedback::Clear);
            Ok(())
        });
        if let Err(reason) = result {
            self.local_resize_feedback =
                Some(LocalResizeFeedback::Notice(if reason == Reason::Busy {
                    Notice::ResizeBusy
                } else {
                    Notice::ResizeFailed
                }));
        }
    }

    pub(super) fn interrupt_local_resize(&mut self) {
        if let Some(pending) = &mut self.local_resize {
            pending.interrupted = true;
        }
    }

    pub(in crate::terminal_session) fn local_resize_pending(&self) -> bool {
        self.local_resize.is_some()
    }
    pub(in crate::terminal_session) fn take_local_resize_feedback(
        &mut self,
    ) -> Option<LocalResizeFeedback> {
        self.local_resize_feedback.take()
    }

    pub(super) fn advance_local_resize(&mut self, native: &mut NativeUi, too_small: bool) -> bool {
        let Some(pending) = &mut self.local_resize else {
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
                self.local_resize_feedback =
                    Some(LocalResizeFeedback::Notice(if pending.flow.had_effect() {
                        Notice::ResizeUnknown
                    } else {
                        Notice::ResizeFailed
                    }));
                self.local_resize = None;
                true
            }
            // A local adjustment is always selected; inactive and navigation outputs are inadmissible.
            ControlStep::Completed(_) | ControlStep::ZoomUnchanged => {
                self.local_resize_feedback =
                    Some(LocalResizeFeedback::Notice(Notice::ResizeUnknown));
                self.local_resize = None;
                true
            }
        }
    }

    pub(super) fn local_resize_after_draw(&mut self, native: &NativeUi) {
        if self.local_resize.as_ref().is_some_and(|pending| {
            pending.awaiting_draw
                && !pending.interrupted
                && self.clock.now() < pending.deadline
                && pending.flow.outcome(native).is_some()
        }) {
            self.local_resize = None;
            self.local_resize_feedback = Some(LocalResizeFeedback::Clear);
        }
    }
}
