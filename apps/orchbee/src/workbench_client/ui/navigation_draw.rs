//! Keep the last flushed shell visible while navigation attaches its first frame.

use super::{ClientPhaseKind, NativeProjectionStatus, NativeUi};
use ratatui::layout::Rect;

#[derive(Default)]
pub(super) struct NavigationDraw {
    viewport: Option<Rect>,
    presented: Option<Rect>,
}

impl NativeUi {
    /// Preparing a draw never claims that its candidate reached the terminal.
    /// The shell may retain its last frame only without an intervening resize or overlay.
    pub(in crate::workbench_client) fn prepare_draw(
        &mut self,
        viewport: Rect,
        allow_defer: bool,
    ) -> bool {
        self.navigation_draw.viewport = Some(viewport);
        if !allow_defer || !self.navigation_waits_for_frame(viewport) {
            return false;
        }
        // A lifecycle, selection, or resnapshot reply can precede the first pixels.
        // Settle those polls so the worker can continue; no current frame is pending here.
        self.settle_pending_polls(false);
        self.retirement_failure.is_none()
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
                    NativeProjectionStatus::Empty | NativeProjectionStatus::WaitingForFullFrame
                )
        })
    }

    pub(super) fn remember_navigation_draw(&mut self) {
        self.navigation_draw.presented = self.navigation_draw.viewport.and_then(|viewport| {
            self.state.as_ref()?.active()?;
            Some(viewport)
        });
    }
}
