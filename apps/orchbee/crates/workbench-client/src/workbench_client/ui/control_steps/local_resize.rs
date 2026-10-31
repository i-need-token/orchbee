//! One local resize acknowledgement for keys and divider releases; no remote receipts or backlog.
use super::{
    ClientControl, ClientControlLane, ClientLayoutAdjustment, ClientViewObservation, ControlStep,
    Instant, Reason, SharedAdjustment, WorkbenchClientUi,
};
use crate::client_view::{Notice, PanelDirection};
use orchbee_workbench_protocol::{ClientLayoutChange, ClientPanelDirection, WorkbenchRequestId};

pub(super) struct LocalResizePending {
    pub(super) request_id: WorkbenchRequestId,
    pub(super) flow: SharedAdjustment,
    deadline: Instant,
    interrupted: bool,
    awaiting_draw: bool,
    presentation: Option<ReleasedDivider>,
}

struct ReleasedDivider {
    source: crate::tui_layout::DrawnDivider,
    ratio: orchbee_workbench_protocol::ClientSplitRatio,
}

pub(in crate::workbench_client) enum LocalResizeFeedback {
    Clear,
    Notice(Notice),
}

impl ClientControl {
    pub(in crate::workbench_client) fn resize_drawn(
        &mut self,
        native: &WorkbenchClientUi,
        target: &crate::tui_layout::DrawnResize,
        direction: PanelDirection,
        fine: bool,
    ) {
        if !native
            .view()
            .divider_source()
            .is_some_and(|source| source.same_layout(&target.source))
            || self.viewport != Some((target.viewport, target.sidebar))
        {
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(Notice::ResizeFailed));
            return;
        }
        self.resize(native, direction, fine);
    }
    pub(in crate::workbench_client) fn resize(
        &mut self,
        native: &WorkbenchClientUi,
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

    pub(in crate::workbench_client) fn divider_ready(
        &mut self,
        native: &WorkbenchClientUi,
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

    pub(in crate::workbench_client) fn resize_divider(
        &mut self,
        native: &WorkbenchClientUi,
        target: &crate::tui_layout::DrawnDivider,
        ratio: orchbee_workbench_protocol::ClientSplitRatio,
    ) {
        let submitted = self.submit_local(
            native,
            native.prepare_divider_adjustment(target, ratio, self.viewport),
        );
        if submitted && let Some(pending) = &mut self.local_resize {
            pending.presentation = Some(ReleasedDivider {
                source: target.clone(),
                ratio,
            });
        }
    }

    fn submit_local(
        &mut self,
        native: &WorkbenchClientUi,
        prepared: Result<
            (
                orchbee_workbench_protocol::SessionId,
                ClientLayoutAdjustment,
                ClientViewObservation,
            ),
            Reason,
        >,
    ) -> bool {
        if self.stopped {
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(Notice::ResizeFailed));
            return false;
        }
        if self.pending.is_some() || self.local_resize.is_some() {
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(Notice::ResizeBusy));
            return false;
        }
        let result = prepared.and_then(|(session, adjustment, before)| {
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
                SharedAdjustment::prepare(session, request_id.clone(), adjustment, before)?;
            self.lane
                .as_ref()
                .ok_or(Reason::ProjectionUnavailable)?
                .submit_layout(mutation)?;
            self.local_resize = Some(LocalResizePending {
                request_id,
                flow,
                deadline: self.clock.now() + crate::workbench_state::OPERATION_TOTAL,
                interrupted: false,
                awaiting_draw: false,
                presentation: None,
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
        self.local_resize.is_some()
    }

    pub(super) fn interrupt_local_resize(&mut self) {
        if let Some(pending) = &mut self.local_resize {
            pending.interrupted = true;
            pending.presentation = None;
        }
    }

    pub(in crate::workbench_client) fn resize_projection(
        &mut self,
        native: &WorkbenchClientUi,
        viewport: ratatui::layout::Rect,
        sidebar: crate::client_view::SidebarSurface,
        presentation_allowed: bool,
    ) -> crate::tui_layout::ResizeProjection {
        let Some(pending) = &mut self.local_resize else {
            return crate::tui_layout::ResizeProjection::Canonical;
        };
        let Some(released) = pending.presentation.as_ref() else {
            return crate::tui_layout::ResizeProjection::Canonical;
        };
        if !presentation_allowed
            || pending.interrupted
            || self.clock.now() >= pending.deadline
            || !native.divider_projection_current(&released.source, (viewport, sidebar))
        {
            // Canonical publication also retires the draft: its new revision now paints the
            // accepted ratio directly and remains eligible for the existing draw acknowledgement.
            pending.presentation = None;
            return crate::tui_layout::ResizeProjection::Canonical;
        }
        crate::tui_layout::ResizeProjection::Divider {
            source: released.source.clone(),
            ratio: released.ratio,
        }
    }

    pub(in crate::workbench_client) fn local_resize_pending(&self) -> bool {
        self.local_resize.is_some()
    }
    pub(in crate::workbench_client) fn take_local_resize_feedback(
        &mut self,
    ) -> Option<LocalResizeFeedback> {
        self.local_resize_feedback.take()
    }

    pub(super) fn advance_local_resize(
        &mut self,
        native: &mut WorkbenchClientUi,
        too_small: bool,
    ) -> bool {
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

    pub(super) fn local_resize_after_draw(&mut self, native: &WorkbenchClientUi) {
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
