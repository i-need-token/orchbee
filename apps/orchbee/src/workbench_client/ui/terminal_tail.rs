//! Automatic tail shares the existing bounded read/ack/recovery owner with manual history.
use super::{
    AdmissionError, Arc, CarrierFailure, CommandMailbox, KeyedProjection, NativeProjectionStatus,
    NativeUi, PaneRef, QueuedCommand, Tagged, TerminalViewport, ViewportNav, ViewportStep,
    ViewportWindow, WorkbenchClientView, WorkbenchPanelId,
};
use crate::workbench_client::terminal_tail::Target;

impl KeyedProjection {
    pub(super) fn tail_window(&self) -> Option<(&ViewportWindow, u32)> {
        if self.status != NativeProjectionStatus::FrameAvailable {
            return None;
        }
        let target = self.tail?;
        let window = self.viewport.as_ref()?.committed_window()?;
        target
            .admits(self.value.frame()?, window)
            .then_some((window, target.rows_before))
    }
}

impl WorkbenchClientView<'_> {
    /// Only a committed window for the exact currently painted live frame may replace its rows.
    pub(crate) fn pane_tail_window(
        &self,
        panel: &WorkbenchPanelId,
    ) -> Option<(&ViewportWindow, u32)> {
        let key = self.pane_ref_for_panel(panel)?;
        let projection = self.panes.iter().find(|projection| projection.key == key)?;
        projection.tail_window()
    }

    fn tail_target(&self, key: &PaneRef) -> Option<Target> {
        let panel_id = key.pane().panel();
        if self.pane_ref_for_panel(panel_id).as_ref() != Some(key)
            || (self.zoom_active() && self.focus_panel_id() != Some(panel_id))
            || self.pane_projection_status(panel_id) != NativeProjectionStatus::FrameAvailable
        {
            return None;
        }
        let tab = self.tab(self.active_position()?.checked_sub(1)?)?;
        let panel = (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .find(|panel| panel.panel_id() == panel_id)?;
        let projection = self.pane_projection(panel_id)?;
        let frame = projection.frame()?;
        let geometry = self
            .state
            .as_ref()?
            .pane_by_panel(panel_id)?
            .observed_geometry();
        if crate::terminal_live_view::LiveView::new(frame, geometry.columns(), geometry.rows())
            .clipped
        {
            return None;
        }
        if !self.attention?.agent_in_terminal(
            self.selected_presentation(panel).presentation_id().as_str(),
            &projection.epoch().to_text(),
            projection.host_terminal_id()?,
        ) {
            return None;
        }
        Target::capture(frame)
    }
}

impl NativeUi {
    pub(super) fn drive_terminal_tails(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        let mut changed = false;
        for index in 0..self.projections.len() {
            let key = self.projections[index].key.clone();
            if self.viewport_admission_blocked(&key) {
                continue;
            }
            if self.projections[index]
                .selection
                .as_ref()
                .is_some_and(|selection| {
                    selection.gesture_intent_active()
                        || !selection.gesture_settled()
                        || selection.host_selection_active()
                        || selection.export_pending()
                })
            {
                continue;
            }
            if self.projections[index].viewport.is_some() && self.projections[index].tail.is_none()
            {
                continue;
            }
            let target = self
                .preferences
                .trim_trailing_blank_rows_for_agents
                .then(|| self.view().tail_target(&key))
                .flatten();
            if let Some(viewport) = self.projections[index].viewport.as_mut() {
                if target.is_none() {
                    let effect = viewport.nav(ViewportNav::Exit);
                    self.settle_viewport_effect(mailbox, &key, effect);
                    changed |= self.projections[index].tail_attempted.is_some();
                    self.projections[index].tail_attempted = None;
                } else if let Some(target) = target
                    && Some(target) != self.projections[index].tail
                {
                    self.projections[index].tail = Some(target);
                    self.projections[index].tail_attempted = Some(target);
                    let effect = self.projections[index]
                        .viewport
                        .as_mut()
                        .expect("existing core")
                        .nav(ViewportNav::OffsetFromBottom(target.rows_before));
                    self.settle_viewport_effect(mailbox, &key, effect);
                    changed = true;
                }
                continue;
            }
            let Some(target) = target else {
                self.projections[index].tail_attempted = None;
                continue;
            };
            // A failed read remains a live presentation until new evidence or an explicit preference change.
            if self.projections[index].tail_attempted == Some(target) {
                continue;
            }
            let Some(frame) = self.projections[index].value.frame() else {
                continue;
            };
            let (columns, rows) = (frame.geometry.columns, frame.geometry.rows);
            match mailbox.try_enqueue(QueuedCommand::Viewport(Tagged::new(
                key.clone(),
                ViewportStep::EnterTail,
            ))) {
                Ok(_) => {
                    let projection = &mut self.projections[index];
                    projection.viewport = Some(TerminalViewport::entering(
                        *projection.value.attachment(),
                        *projection.value.epoch(),
                        columns,
                        rows,
                        target.rows_before,
                        projection.read_watermark.saturating_add(1),
                        self.clock.now(),
                    ));
                    projection.tail = Some(target);
                    projection.tail_attempted = Some(target);
                    changed = true;
                }
                Err(
                    AdmissionError::Full | AdmissionError::Busy | AdmissionError::StaleAttachment,
                ) => {}
                Err(AdmissionError::Closed(closed)) => {
                    self.retire(closed.failure());
                    break;
                }
                Err(_) => {
                    self.retire(CarrierFailure::Protocol);
                    break;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests;
