//! Adapts Client popup controls, capture context, and surface geometry to the exact active
//! popup and its correlated completion. It does not own the popup's physical launch or
//! authorize input to another presentation.

use super::WorkbenchClientUi;
use orchbee_workbench_protocol::{
    ClientViewCompletionOutcome, ClientViewControl, ClientViewControlReason as Reason,
    ClientViewIntent, ClientViewPhase, Snapshot, WorkbenchRequestId,
};
use ratatui::layout::Rect;

impl super::WorkbenchClientView<'_> {
    pub(crate) fn popup(&self) -> Option<crate::independent_popup::PopupView<'_>> {
        self.popup?.view()
    }
    pub(crate) fn popup_capture_claim(&self) -> bool {
        self.popup.is_some_and(|popup| popup.captured())
    }
    pub(crate) fn popup_input_context(
        &self,
    ) -> Option<crate::independent_popup::PopupInputContext> {
        self.popup?.input_context()
    }
}
impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn popup_capture_claim(&self) -> bool {
        self.view().popup_capture_claim()
    }
    pub(in crate::workbench_client) fn popup_captured(&self) -> bool {
        self.popup_capture_claim()
    }
    pub(in crate::workbench_client) fn popup_surface(&mut self, area: Rect, pixels: (u16, u16)) {
        self.popup.observe_surface(area, pixels);
    }
    pub(in crate::workbench_client) fn popup_input(
        &mut self,
        id: &WorkbenchRequestId,
        item: &crate::terminal_events::DecodedItem,
    ) {
        self.popup.input(id, item);
    }
    pub(in crate::workbench_client) fn popup_mouse(
        &mut self,
        id: &WorkbenchRequestId,
        content: Rect,
        event: crate::terminal_events::MouseEvent,
    ) {
        self.popup.mouse(id, content, event);
    }
    pub(in crate::workbench_client) fn activate_drawn_popup_close(
        &mut self,
        target: &crate::independent_popup::DrawnPopupClose,
    ) -> bool {
        self.popup.activate_drawn_close(target)
    }
    pub(in crate::workbench_client) fn begin_popup_control(
        &mut self,
        command: &ClientViewControl,
        snapshot: Option<&Snapshot>,
    ) -> Result<(), Reason> {
        match &command.intent {
            ClientViewIntent::PopupClose {} => {
                self.popup.begin_close();
                Ok(())
            }
            ClientViewIntent::CommandPopupOpen { .. }
            | ClientViewIntent::PopupOpen { .. }
            | ClientViewIntent::EditorOpen { .. } => {
                let source = command.intent.popup_source().ok_or(Reason::Unsupported)?;
                if self.popup_modal_open() {
                    return Err(Reason::ModalOpen);
                }
                let (session, view) = self
                    .control_observation(false)
                    .ok_or(Reason::ProjectionUnavailable)?;
                if view.phase != ClientViewPhase::Ready
                    || view.workbench_revision < command.minimum_workbench_revision
                {
                    return Err(Reason::Busy);
                }
                let selected = view.selection.ok_or(Reason::ProjectionUnavailable)?;
                if selected.terminal_context_id != source.terminal_context_id
                    || selected.tab_id.as_ref() != Some(&source.tab_id)
                    || selected.panel_id.as_ref() != Some(&source.panel_id)
                {
                    return Err(Reason::StaleView);
                }
                let read = ClientViewControl {
                    intent: ClientViewIntent::Presentation {
                        panel_id: source.panel_id.clone(),
                    },
                    ..command.clone()
                };
                let member = self.read_client_presentation(
                    &read,
                    snapshot.ok_or(Reason::ProjectionUnavailable)?,
                )?;
                let matches = member
                    .presentation
                    .as_ref()
                    .is_some_and(|current| current.same_member(&source.member));
                if !matches {
                    return Err(Reason::StaleView);
                }
                match &command.intent {
                    ClientViewIntent::CommandPopupOpen { spec } => self.popup.begin_configured(
                        session,
                        command.operation_id.clone(),
                        spec.as_ref().clone(),
                    ),
                    ClientViewIntent::PopupOpen { spec } => self.popup.begin_open(
                        session,
                        command.operation_id.clone(),
                        spec.as_ref().clone(),
                    ),
                    ClientViewIntent::EditorOpen { spec } => self.popup.begin_editor(
                        session,
                        command.operation_id.clone(),
                        spec.as_ref().clone(),
                    ),
                    // This handler implements its own ClientView intent subset; other intents belong to their feature handlers.
                    _ => Err(Reason::Unsupported),
                }
            }
            // This handler implements its own ClientView intent subset; other intents belong to their feature handlers.
            _ => Err(Reason::Unsupported),
        }
    }
    /// Whether another popup, a dialog, a history view, or a native surface that captures input,
    /// such as the action menu or copy mode, owns the input a new popup would take; Client
    /// overlays and modes are reported by the caller that observes them.
    pub(in crate::workbench_client) fn popup_modal_open(&self) -> bool {
        self.popup.busy()
            || self.control_modal()
            || crate::tui_composition::project_capture(
                crate::tui_composition::ClientCaptureClaims::default(),
                self.capture_claims(),
            )
            .is_some()
    }
    pub(in crate::workbench_client) fn popup_control_outcome(
        &self,
        command: &ClientViewControl,
    ) -> Option<ClientViewCompletionOutcome> {
        self.popup.completion(
            &command.operation_id,
            command.intent.popup_source().is_some(),
        )
    }
}

#[cfg(test)]
mod tests;
