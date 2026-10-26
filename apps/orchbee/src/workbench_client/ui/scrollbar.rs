//! Projects verified per-pane history and admits scrollbar intent through the
//! existing viewport handshake; it never changes terminal input ownership.

use std::sync::Arc;

use orchbee_workbench_protocol::WorkbenchPanelId;

use super::{KeyedProjection, NativeProjectionStatus, NativeUi, WorkbenchClientView};
use crate::terminal_scrollbar::ScrollbarSnapshot;
use crate::workbench_client::{
    state::PaneRef,
    terminal_viewport::{ViewportNav, ViewportSurface},
};

impl KeyedProjection {
    fn scrollbar_snapshot(&self) -> Option<ScrollbarSnapshot> {
        if let Some(viewport) = &self.viewport
            && (self.tail.is_none() || self.tail_window().is_some())
        {
            let ViewportSurface::Window(window) = viewport.surface() else {
                return None;
            };
            let route = window.route;
            return ScrollbarSnapshot::new(
                route.attachment_id,
                route.host_epoch,
                route.generation,
                viewport.geometry(),
                route.row_count,
                route.total_rows,
                route.start_row,
            );
        }
        let frame = self.value.frame().filter(|frame| {
            self.status == NativeProjectionStatus::FrameAvailable
                && !frame.alternate_screen
                && !frame.rows.is_empty()
        })?;
        ScrollbarSnapshot::new(
            frame.route.attachment_id,
            frame.route.host_epoch,
            frame.route.generation,
            (frame.geometry.columns, frame.geometry.rows),
            frame.geometry.rows,
            frame
                .scrollback_rows
                .checked_add(u64::from(frame.geometry.rows))?,
            frame.scrollback_rows,
        )
    }
}

impl WorkbenchClientView<'_> {
    /// Only the exact installed pane's current frame/window supplies a thumb.
    pub(crate) fn pane_scrollbar(&self, panel: &WorkbenchPanelId) -> Option<ScrollbarSnapshot> {
        let key = self.pane_ref_for_panel(panel)?;
        self.panes
            .iter()
            .find(|projection| projection.key == key)?
            .scrollbar_snapshot()
    }
}

impl NativeUi {
    pub(in crate::workbench_client) fn scrollbar_scroll(
        &mut self,
        key: &PaneRef,
        source: ScrollbarSnapshot,
        offset: u32,
    ) -> bool {
        let probe = self.pointer_probe();
        if probe.input_captured || !probe.selection_ready || probe.focus.as_ref() != Some(key) {
            return false;
        }
        if self.copy_mode.key().is_some() {
            return false;
        }
        if self.viewport_admission_blocked(key) {
            return false;
        }
        let Some(index) = self
            .projections
            .iter()
            .position(|projection| &projection.key == key)
        else {
            return false;
        };
        let projection = &self.projections[index];
        let offset = source.clamp(offset);
        if projection.viewport.is_none() {
            if !projection
                .scrollbar_snapshot()
                .is_some_and(|current| source.same_source(current))
            {
                return false;
            }
            if offset == 0 {
                return true;
            }
            self.enter_history_rows(Some(offset));
            return self
                .projections
                .iter()
                .any(|projection| &projection.key == key && projection.viewport.is_some());
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        let viewport = self.projections[index]
            .viewport
            .as_mut()
            .expect("viewport checked above");
        if !viewport.admits_scrollbar(source) {
            return false;
        }
        // The press supplied a verified bounded range. While its next window is
        // pending, retain intent only; the core still owns all facts/read/ACK gates.
        let effect = viewport.nav(ViewportNav::OffsetFromBottom(offset));
        self.projections[index].tail = None;
        self.settle_viewport_effect(&mailbox, key, effect);
        true
    }
}
