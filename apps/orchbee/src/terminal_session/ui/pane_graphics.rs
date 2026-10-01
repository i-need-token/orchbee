use super::{NativeSessionView, NativeUi};
#[cfg(test)]
mod tests;
use crate::terminal_session::{
    NativePanel, PaneGraphicsCanvas, PaneGraphicsDraw,
    pane_graphics::{Scope, Source},
};

impl NativeSessionView<'_> {
    pub(crate) fn panel_graphics(&self, panel: &NativePanel) -> Option<&PaneGraphicsCanvas> {
        self.graphics?
            .view(panel, self.selected_presentation(panel))
    }

    pub(crate) fn graphics_draw(&self, panel: &NativePanel) -> Option<PaneGraphicsDraw> {
        self.graphics?;
        Some(PaneGraphicsDraw::new(
            Source::capture(panel, self.selected_presentation(panel)),
            self.panel_graphics(panel).map(PaneGraphicsCanvas::revision),
        ))
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn graphics_observation(
        &self,
        view: &orchbee_workbench_protocol::ClientViewObservation,
    ) -> Option<orchbee_workbench_protocol::PaneGraphicsClientObservation> {
        self.state.as_ref().filter(|state| {
            self.retirement.is_none() && state.cache().revision() == view.workbench_revision
        })?;
        self.graphics.observation(view)
    }
    pub(in crate::terminal_session) fn after_graphics_draw(
        &mut self,
        draws: Vec<PaneGraphicsDraw>,
    ) {
        let Some(state) = self.state.as_ref().filter(|_| self.retirement.is_none()) else {
            return;
        };
        self.graphics.observe(
            Scope {
                site: state.cache().site_id().clone(),
                revision: state.cache().revision(),
                sources: draws.iter().map(|draw| draw.source.clone()).collect(),
            },
            draws,
        );
    }
}
