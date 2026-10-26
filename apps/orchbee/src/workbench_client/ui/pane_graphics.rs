use super::{WorkbenchClientUi, WorkbenchClientView};
#[cfg(test)]
mod tests;
use crate::workbench_client::{
    ClientPanel, PaneGraphicsCanvas, PaneGraphicsDraw,
    pane_graphics::{Scope, Source},
};

impl WorkbenchClientView<'_> {
    pub(crate) fn set_graphics_surface(&mut self, surface: Option<crate::tui_graphics::Surface>) {
        self.graphics_surface = surface;
    }
    pub(crate) fn cover_graphics(&self) {
        if let Some(surface) = &self.graphics_surface {
            surface.cover();
        }
    }
    pub(crate) fn physical_graphics(
        &self,
        panel: &ClientPanel,
        body: ratatui::layout::Rect,
    ) -> Option<crate::tui_graphics::Choice> {
        self.graphics_surface.as_ref()?.select(
            self.navigation_session()?,
            self.panel_graphics(panel)?,
            body,
        )
    }
    pub(crate) fn graphics_cell_pixels(
        &self,
    ) -> Option<orchbee_workbench_protocol::PaneGraphicsCellPixels> {
        self.graphics?.cell_pixels()
    }
    pub(crate) fn panel_graphics(&self, panel: &ClientPanel) -> Option<&PaneGraphicsCanvas> {
        self.graphics?
            .view(panel, self.selected_presentation(panel))
    }

    pub(crate) fn graphics_draw(&self, panel: &ClientPanel) -> Option<PaneGraphicsDraw> {
        self.graphics?;
        Some(PaneGraphicsDraw::new(
            Source::capture(panel, self.selected_presentation(panel)),
            self.panel_graphics(panel).map(PaneGraphicsCanvas::revision),
        ))
    }
}

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn observe_host_graphics(
        &mut self,
        facts: crate::terminal_graphics::Facts,
    ) {
        self.graphics.observe_host(facts);
    }
    pub(in crate::workbench_client) fn graphics_observation(
        &self,
        view: &orchbee_workbench_protocol::ClientViewObservation,
    ) -> Option<orchbee_workbench_protocol::PaneGraphicsClientObservation> {
        self.state.as_ref().filter(|state| {
            self.retirement.is_none() && state.cache().revision() == view.workbench_revision
        })?;
        self.graphics.observation(view)
    }
    pub(in crate::workbench_client) fn after_graphics_draw(
        &mut self,
        draws: Vec<PaneGraphicsDraw>,
    ) {
        let Some(state) = self.state.as_ref().filter(|_| self.retirement.is_none()) else {
            return;
        };
        self.graphics.observe(
            Scope {
                session: state.cache().session_id().clone(),
                revision: state.cache().revision(),
                sources: draws.iter().map(|draw| draw.source.clone()).collect(),
            },
            draws,
        );
    }
}
