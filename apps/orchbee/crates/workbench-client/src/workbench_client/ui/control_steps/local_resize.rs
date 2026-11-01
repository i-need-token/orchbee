//! One local resize acknowledgement for keys and divider releases; no remote receipts or backlog.
use super::{
    ClientControl, ClientControlLane, ClientLayoutAdjustment, ClientViewObservation, ControlStep,
    Instant, Reason, SharedAdjustment, WorkbenchClientUi,
};
use crate::client_view::{Notice, PanelDirection};
use orchbee_workbench_protocol::{ClientLayoutChange, ClientPanelDirection, WorkbenchRequestId};
use std::time::Duration;

/// How long one release keeps being resubmitted after the first refusal that named only an older
/// Workbench revision. Each resubmission waits for the Client to read a newer revision, so this
/// bounds the wait the person sees rather than a count that a slow read would exhaust.
const CONTENTION: Duration = Duration::from_secs(5);

pub(super) struct LocalResizePending {
    pub(super) request_id: WorkbenchRequestId,
    pub(super) flow: SharedAdjustment,
    deadline: Instant,
    interrupted: bool,
    awaiting_draw: bool,
    presentation: Option<ReleasedDivider>,
    contended_since: Option<Instant>,
}

pub(super) struct ReleasedDivider {
    source: crate::tui_layout::DrawnDivider,
    ratio: orchbee_workbench_protocol::ClientSplitRatio,
}

impl ReleasedDivider {
    fn projection(&self) -> crate::tui_layout::ResizeProjection {
        crate::tui_layout::ResizeProjection::Divider {
            source: self.source.clone(),
            ratio: self.ratio,
        }
    }
}

/// What a waiting release paints: over the settling resize's own draft while that shows, then
/// over the layout the release was dragged on.
fn queued_projection(
    native: &WorkbenchClientUi,
    queued: &ReleasedDivider,
    draft: Option<&ReleasedDivider>,
    viewport: (ratatui::layout::Rect, crate::client_view::SidebarSurface),
) -> Option<crate::tui_layout::ResizeProjection> {
    if let Some(draft) = draft.filter(|draft| {
        draft
            .projection()
            .divider()
            .is_some_and(|drawn| drawn.same_split(&queued.source))
    }) {
        return Some(crate::tui_layout::ResizeProjection::Divider {
            source: draft.source.clone(),
            ratio: queued.ratio,
        });
    }
    native
        .divider_projection_current(&queued.source, viewport)
        .then(|| queued.projection())
}

pub(in crate::workbench_client) enum LocalResizeFeedback {
    Clear,
    Notice(Notice),
}

impl LocalResizePending {
    /// Another commit moved the Workbench past the revision this CAS named, so the Server refused
    /// it without effect. The same split move is resubmitted against the newer revision once it is
    /// installed, while the split keeps its previous ratio; the draft presentation stays meanwhile.
    fn resubmit(
        &mut self,
        native: &mut WorkbenchClientUi,
        lane: Option<&ClientControlLane>,
        now: Instant,
    ) -> Result<(), Notice> {
        let contended_since = *self.contended_since.get_or_insert(now);
        if now >= contended_since + CONTENTION {
            return Err(Notice::ResizeContended);
        }
        let Some((flow, mutation)) = self.flow.rebase(native).map_err(|_| Notice::ResizeFailed)?
        else {
            return Ok(());
        };
        let request_id = mutation.request_id.clone();
        lane.ok_or(Reason::ProjectionUnavailable)
            .and_then(|lane| lane.submit_layout(mutation))
            .map_err(|reason| {
                if reason == Reason::Busy {
                    Notice::ResizeContended
                } else {
                    Notice::ResizeFailed
                }
            })?;
        self.request_id = request_id;
        self.flow = flow;
        Ok(())
    }
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
        } else if self.pending.is_some() {
            Err(Notice::ResizeBusy)
        } else if self.local_resize.is_some() {
            // The Client's own earlier resize is still settling. A drag of a split the next frame
            // paints, that resize's released draft included, previews meanwhile; its release
            // waits for that resize and is checked against the layout it leaves. Once a release
            // waits, only a drag on that release's own draft may supersede it.
            if self.viewport.is_some_and(|viewport| {
                self.settling_paints(native, target, viewport)
                    || (self.queued_divider.is_none()
                        && native.divider_projection_current(target, viewport))
            }) {
                Ok(())
            } else {
                Err(Notice::ResizeBusy)
            }
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
        if !self.stopped && self.pending.is_none() && self.local_resize.is_some() {
            // Released while the Client's own earlier resize settles: the newest release waits
            // for it and is submitted once it lands, against the layout it left. A release dragged
            // on the waiting one's draft replaces it and keeps the layout it was dragged on.
            let source = match &self.queued_divider {
                Some(queued)
                    if self
                        .viewport
                        .is_some_and(|viewport| self.queued_paints(native, target, viewport)) =>
                {
                    queued.source.clone()
                }
                _ => target.clone(),
            };
            self.queued_divider =
                (ratio != source.ratio).then_some(ReleasedDivider { source, ratio });
            return;
        }
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
                contended_since: None,
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

    /// Whether the next frame paints `target` from the settling resize: the draft of the release
    /// queued over it while one waits, otherwise its own released draft. A drag may start there
    /// before canonical publication catches up.
    pub(in crate::workbench_client) fn settling_paints(
        &self,
        native: &WorkbenchClientUi,
        target: &crate::tui_layout::DrawnDivider,
        viewport: (ratatui::layout::Rect, crate::client_view::SidebarSurface),
    ) -> bool {
        if self.queued_divider.is_some() {
            self.queued_paints(native, target, viewport)
        } else {
            self.draft_paints(native, target, viewport)
        }
    }

    fn draft_paints(
        &self,
        native: &WorkbenchClientUi,
        target: &crate::tui_layout::DrawnDivider,
        viewport: (ratatui::layout::Rect, crate::client_view::SidebarSurface),
    ) -> bool {
        self.local_resize.as_ref().is_some_and(|pending| {
            !pending.interrupted
                && self.clock.now() < pending.deadline
                && pending.presentation.as_ref().is_some_and(|released| {
                    native.divider_projection_current(&released.source, viewport)
                        && released
                            .projection()
                            .divider()
                            .is_some_and(|draft| draft.same_split(target))
                })
        })
    }

    fn queued_paints(
        &self,
        native: &WorkbenchClientUi,
        target: &crate::tui_layout::DrawnDivider,
        viewport: (ratatui::layout::Rect, crate::client_view::SidebarSurface),
    ) -> bool {
        let (Some(pending), Some(queued)) = (&self.local_resize, &self.queued_divider) else {
            return false;
        };
        if pending.interrupted || self.clock.now() >= pending.deadline {
            return false;
        }
        let draft = pending
            .presentation
            .as_ref()
            .filter(|released| native.divider_projection_current(&released.source, viewport));
        queued_projection(native, queued, draft, viewport)
            .and_then(|projection| projection.divider())
            .is_some_and(|drawn| drawn.same_split(target))
    }

    pub(super) fn interrupt_local_resize(&mut self) {
        self.queued_divider = None;
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
        if !presentation_allowed || pending.interrupted || self.clock.now() >= pending.deadline {
            pending.presentation = None;
            return crate::tui_layout::ResizeProjection::Canonical;
        }
        if pending.presentation.as_ref().is_some_and(|released| {
            !native.divider_projection_current(&released.source, (viewport, sidebar))
        }) {
            // Canonical publication also retires the draft: its new revision now paints the
            // accepted ratio directly and remains eligible for the existing draw acknowledgement.
            pending.presentation = None;
        }
        let draft = pending.presentation.as_ref();
        if let Some(projection) = self
            .queued_divider
            .as_ref()
            .and_then(|queued| queued_projection(native, queued, draft, (viewport, sidebar)))
        {
            return projection;
        }
        draft.map_or(
            crate::tui_layout::ResizeProjection::Canonical,
            ReleasedDivider::projection,
        )
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
        } else if pending.flow.refused_for_revision() {
            let Err(notice) = pending.resubmit(native, self.lane.as_ref(), self.clock.now()) else {
                return false;
            };
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(notice));
            self.local_resize = None;
            self.queued_divider = None;
            return true;
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
                self.queued_divider = None;
                true
            }
            // A local adjustment is always selected; inactive and navigation outputs are inadmissible.
            ControlStep::Completed(_) | ControlStep::ZoomUnchanged => {
                self.local_resize_feedback =
                    Some(LocalResizeFeedback::Notice(Notice::ResizeUnknown));
                self.local_resize = None;
                self.queued_divider = None;
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
            // The settled resize left the layout the queued release was dragged on.
            if let Some(queued) = self.queued_divider.take() {
                self.resize_divider(native, &queued.source, queued.ratio);
            }
        }
    }
}
