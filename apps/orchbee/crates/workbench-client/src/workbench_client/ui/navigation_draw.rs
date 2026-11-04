//! Keep the last flushed shell visible while navigation attaches its first frame.

use super::{ClientPhaseKind, ClientProjectionStatus, WorkbenchClientUi};
use crate::client_view::SidebarSurface;
use orchbee_workbench_protocol::WorkbenchTabId;
use ratatui::layout::Rect;

/// What a draw keeps of the last flushed frame while navigation waits for the next terminal's
/// first frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationHold {
    /// The whole candidate frame is drawn.
    None,
    /// The whole last frame stays on screen.
    Frame,
    /// The Panel area keeps its last frame while the rest of the shell is drawn, because the
    /// Tab that frame shows is leaving with its accepted close. One such draw shows the strip
    /// without that Tab; later draws of the same wait keep the whole frame.
    Panel,
}

#[derive(Default)]
pub(super) struct NavigationDraw {
    viewport: Option<Rect>,
    sidebar: Option<SidebarSurface>,
    presented: Option<Rect>,
    presented_sidebar: Option<SidebarSurface>,
    /// The selected Tab of the last completed draw, whose terminal the Panel area shows.
    presented_tab: Option<WorkbenchTabId>,
    /// A draw since then already showed the shell around the held Panel.
    chrome_drawn: bool,
}

impl WorkbenchClientUi {
    /// Preparing a draw never claims that its candidate reached the terminal.
    /// The shell may retain its last frame only without an intervening resize or overlay, and
    /// may redraw around the Panel only while the sidebar it was drawn beside keeps its surface,
    /// so that the Panel keeps its geometry.
    pub(in crate::workbench_client) fn prepare_draw(
        &mut self,
        viewport: Rect,
        sidebar: SidebarSurface,
        allow_defer: bool,
    ) -> NavigationHold {
        self.navigation_draw.viewport = Some(viewport);
        self.navigation_draw.sidebar = Some(sidebar);
        if let Some(state) = self.state.as_mut() {
            state.settle_tab_strip_scroll();
        }
        if !allow_defer || !self.navigation_waits_for_frame(viewport) {
            return NavigationHold::None;
        }
        // A lifecycle, selection, or resnapshot reply can precede the first pixels.
        // Settle those polls so the worker can continue; no current frame is pending here.
        self.settle_pending_polls(false);
        if self.retirement_failure.is_some() {
            NavigationHold::None
        } else if !self.navigation_draw.chrome_drawn
            && self.navigation_draw.presented_sidebar == Some(sidebar)
            && self.presented_tab_leaving()
        {
            // Each candidate is a whole render; drawing it once keeps the wait's ticks cheap.
            self.navigation_draw.chrome_drawn = true;
            NavigationHold::Panel
        } else {
            NavigationHold::Frame
        }
    }

    fn navigation_waits_for_frame(&self, viewport: Rect) -> bool {
        let Some(presented_viewport) = self.navigation_draw.presented else {
            return false;
        };
        let Some(state) = &self.state else {
            return false;
        };
        if self.retirement_failure.is_some()
            || presented_viewport != viewport
            || !matches!(
                state.phase_kind(),
                ClientPhaseKind::Starting | ClientPhaseKind::Switching
            )
            || self.pending_draws.iter().any(|pending| {
                pending.frame.is_some() && state.pane_projected(pending.poll_key.pane_ref())
            })
        {
            return false;
        }
        let view = self.view();
        view.focus_panel_id().is_some_and(|panel| {
            state
                .pane_by_panel(panel)
                .is_none_or(|pane| pane.fault().is_none())
                && matches!(
                    view.pane_projection_status(panel),
                    ClientProjectionStatus::Empty | ClientProjectionStatus::WaitingForFullFrame
                )
        })
    }

    /// Whether the Panel still shows a Tab that has left the strip with its accepted close, so
    /// the strip and the rest of the chrome may already show the selection that left it.
    fn presented_tab_leaving(&self) -> bool {
        let (Some(state), Some(tab)) = (&self.state, &self.navigation_draw.presented_tab) else {
            return false;
        };
        let view = state.view();
        view.tab_leaving(tab) && view.active_tab_id() != Some(tab)
    }

    pub(super) fn remember_navigation_draw(&mut self) {
        let state = self.state.as_ref();
        self.navigation_draw.presented = self.navigation_draw.viewport.and_then(|viewport| {
            state?.active()?;
            Some(viewport)
        });
        self.navigation_draw.presented_sidebar = self
            .navigation_draw
            .presented
            .and(self.navigation_draw.sidebar);
        self.navigation_draw.presented_tab =
            state.and_then(|state| state.view().active_tab_id().cloned());
        self.navigation_draw.chrome_drawn = false;
    }
}
