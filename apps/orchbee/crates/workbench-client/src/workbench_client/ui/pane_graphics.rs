//! Connects current graphics sources and surface observations to UI views and exact post-draw
//! receipts. It does not decode image bytes, own Host resources, or acknowledge graphics that
//! were merely prepared for drawing. Images a program printed in a Panel share the renderers but
//! never enter the Session receipt scope.

use super::{WorkbenchClientUi, WorkbenchClientView};
#[cfg(any(test, feature = "test-support"))]
pub(super) mod test_support;
#[cfg(test)]
mod tests;
use crate::pane_canvas::{Lane, terminal};
use crate::workbench_client::pane_graphics::{Scope, Source};
use crate::workbench_client::{PaneGraphicsCanvas, PaneGraphicsDraw, ViewportSurface};
use crate::workbench_projection::ClientPanel;

impl WorkbenchClientView<'_> {
    pub fn set_graphics_surface(&mut self, surface: Option<crate::tui_graphics::Surface>) {
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

    /// The Kitty images the program in this Panel printed, one canvas per Host placement, for
    /// the surface the Panel paints: the live screen's current set, or a history window's own
    /// set. A loading or recovering history surface, a reading stand-in and a revealed tail show
    /// none, so a live placement never lands on rows it does not belong to.
    pub(crate) fn terminal_graphics(&self, panel: &ClientPanel) -> Vec<PaneGraphicsCanvas> {
        let panel_id = panel.panel_id();
        if self.pane_history_stand_in(panel_id).is_some()
            || self.pane_tail_window(panel_id).is_some()
        {
            return Vec::new();
        }
        let source = self.terminal_graphics_source(panel);
        match self.pane_viewport(panel_id) {
            Some(ViewportSurface::Window { window }) => {
                return window.graphics.as_ref().map_or_else(Vec::new, |graphics| {
                    terminal::canvases(&source, &graphics.resources, graphics.cell_pixels)
                });
            }
            Some(ViewportSurface::Entering) | None => {}
            Some(ViewportSurface::AwaitingWindow | ViewportSurface::Recovering) => {
                return Vec::new();
            }
        }
        let Some(projection) = self.pane_projection(panel_id) else {
            return Vec::new();
        };
        let (Some(frame), Some(graphics)) = (projection.frame(), projection.graphics()) else {
            return Vec::new();
        };
        terminal::canvases(
            &source,
            graphics,
            (
                frame.geometry.cell_pixel_width,
                frame.geometry.cell_pixel_height,
            ),
        )
    }

    /// Keeps this Panel's staged physical terminal images through the frame; never a receipt.
    pub(crate) fn terminal_graphics_draw(&self, panel: &ClientPanel) -> PaneGraphicsDraw {
        PaneGraphicsDraw::new(self.terminal_graphics_source(panel), None)
    }

    /// Physical output for one terminal image canvas when the host admits it.
    pub(crate) fn physical_terminal_graphics(
        &self,
        canvas: &PaneGraphicsCanvas,
        body: ratatui::layout::Rect,
    ) -> Option<crate::tui_graphics::Choice> {
        self.graphics_surface
            .as_ref()?
            .select(self.navigation_session()?, canvas, body)
    }

    fn terminal_graphics_source(&self, panel: &ClientPanel) -> Source {
        Source {
            lane: Lane::Terminal,
            ..Source::capture(panel, self.selected_presentation(panel))
        }
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
        mut draws: Vec<PaneGraphicsDraw>,
    ) {
        let Some(state) = self.state.as_ref().filter(|_| self.retirement.is_none()) else {
            return;
        };
        // Program-printed images are not Session layers and have no Workbench receipt.
        draws.retain(|draw| draw.source.lane == Lane::Session);
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
