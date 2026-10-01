use super::NativeUi;
use orchbee_workbench_protocol::{
    ClientViewCompletionOutcome, ClientViewControl, ClientViewControlReason as Reason,
    ClientViewIntent, ClientViewPhase, Snapshot, WorkbenchRequestId,
};
use ratatui::layout::Rect;

impl super::NativeSessionView<'_> {
    pub(crate) fn popup(&self) -> Option<&crate::terminal_session::PopupView<'_>> {
        self.popup.as_ref()
    }
}
impl NativeUi {
    pub(in crate::terminal_session) fn popup_identity(&self) -> Option<&WorkbenchRequestId> {
        self.popup.identity()
    }
    pub(in crate::terminal_session) fn popup_captured(&self) -> bool {
        self.popup.captured()
    }
    pub(in crate::terminal_session) fn popup_surface(&mut self, area: Rect, pixels: (u16, u16)) {
        self.popup.observe_surface(area, pixels);
    }
    pub(in crate::terminal_session) fn popup_input(
        &mut self,
        item: &crate::terminal_input::DecodedItem,
    ) {
        self.popup.input(item);
    }
    pub(in crate::terminal_session) fn popup_mouse(
        &mut self,
        id: &WorkbenchRequestId,
        content: Rect,
        event: crate::terminal_input::MouseEvent,
        close: bool,
    ) {
        self.popup.mouse(id, content, event, close);
    }
    pub(in crate::terminal_session) fn begin_popup_control(
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
                if self.popup.busy() || self.control_modal() {
                    return Err(Reason::Busy);
                }
                let (site, view) = self
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
                        site,
                        command.operation_id.clone(),
                        spec.as_ref().clone(),
                    ),
                    ClientViewIntent::PopupOpen { spec } => self.popup.begin_open(
                        site,
                        command.operation_id.clone(),
                        spec.as_ref().clone(),
                    ),
                    ClientViewIntent::EditorOpen { spec } => self.popup.begin_editor(
                        site,
                        command.operation_id.clone(),
                        spec.as_ref().clone(),
                    ),
                    _ => Err(Reason::Unsupported),
                }
            }
            _ => Err(Reason::Unsupported),
        }
    }
    pub(in crate::terminal_session) fn popup_control_outcome(
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
