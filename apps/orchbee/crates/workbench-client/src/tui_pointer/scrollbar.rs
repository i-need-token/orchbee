//! Press-owned scrollbar dragging. A changed history range cancels capture;
//! every effect rechecks the exact pane and source in WorkbenchClientUi.

use super::{
    FrameHitMap, Gesture, HitResolution, HitTarget, PointerOutcome, PointerState, contains,
};
use crate::{
    client_view::{ClientAction, ClientView, Modifiers},
    terminal_input::{MouseAction, MouseButton, MouseEvent, PointerProbe, TerminalInputPort},
    terminal_scrollbar::ScrollbarSnapshot,
    workbench_client::state::PaneRef,
};
use ratatui::layout::Rect;

pub(super) struct ScrollbarDrag {
    pane: PaneRef,
    content: Rect,
    source: ScrollbarSnapshot,
    grab: u16,
    last_row: u16,
    last_offset: Option<u32>,
}

impl ScrollbarDrag {
    fn track(&self) -> Rect {
        Rect::new(self.content.right(), self.content.y, 1, self.content.height)
    }

    pub(super) fn alive(&self, probe: &PointerProbe) -> bool {
        probe.focus.as_ref() == Some(&self.pane)
            && probe.desired.as_ref() == Some(self.pane.pane())
            && probe.selection_ready
            && probe
                .panes
                .iter()
                .any(|pane| pane.record == self.pane && pane.attached)
    }

    pub(super) fn painted_in(&self, map: &FrameHitMap) -> bool {
        // A pending window has no thumb. The same exposed content keeps the
        // gesture, but no newly pressed gesture can start from unknown metrics.
        matches!(map.hit(self.content.x, self.content.y),
            HitResolution::Region(HitTarget::Pane { pane, content })
                if *pane == self.pane && *content == self.content)
            && map.regions.iter().all(|region| match &region.target {
                HitTarget::Scrollbar { pane, source, .. } if *pane == self.pane => {
                    self.source.same_source(*source)
                }
                _ => true,
            })
    }

    fn scroll(&mut self, port: &mut (impl TerminalInputPort + ?Sized), row: u16) -> bool {
        self.last_row = row;
        let Some(offset) = self.source.offset_at(self.track(), row, self.grab) else {
            return false;
        };
        if self.last_offset == Some(offset) {
            return true;
        }
        if port.pane_input(crate::terminal_input::PaneInput::Scrollbar {
            pane: &self.pane,
            source: self.source,
            offset,
        }) != crate::terminal_input::PaneAdmission::Admitted
        {
            return false;
        }
        self.last_offset = Some(offset);
        true
    }
}

impl PointerState {
    /// Keyboard input or host focus loss ends this Client-owned gesture.
    pub(crate) fn cancel_scrollbar(&mut self) {
        if matches!(self.gesture, Gesture::Scrollbar(_)) {
            self.gesture = Gesture::Idle;
        }
    }

    #[expect(clippy::too_many_arguments)]
    pub(super) fn begin_scrollbar(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        probe: &PointerProbe,
        event: MouseEvent,
        pane: PaneRef,
        content: Rect,
        source: ScrollbarSnapshot,
    ) -> PointerOutcome {
        let modal = view.overlay().is_some();
        let modal = modal || view.resize_mode();
        if event.action != MouseAction::Press
            || event.button != MouseButton::Left
            || event.mods != Modifiers::NONE
            || modal
        {
            return PointerOutcome::Ignored;
        }
        if probe.desired.as_ref().map(|desired| desired.anchor()) != Some(pane.anchor())
            || !probe
                .panes
                .iter()
                .any(|current| current.record == pane && current.attached)
        {
            return PointerOutcome::Handled;
        }
        if probe.focus.as_ref() != Some(&pane)
            && let Some(disposition) = port
                .apply_client_action((ClientAction::FocusPanel(pane.pane().panel().clone())).into())
        {
            view.commit_pointer(disposition);
        }
        let after = port.pointer_probe();
        let track = Rect::new(content.right(), content.y, 1, content.height);
        let Some(thumb) = source.thumb(track) else {
            return PointerOutcome::Handled;
        };
        let on_thumb = contains(thumb, event.column, event.row);
        let mut drag = ScrollbarDrag {
            pane,
            content,
            source,
            grab: if on_thumb {
                event.row - thumb.y
            } else {
                thumb.height / 2
            },
            last_row: event.row,
            last_offset: None,
        };
        if drag.alive(&after) && (on_thumb || drag.scroll(port, event.row)) {
            self.gesture = Gesture::Scrollbar(drag);
        }
        PointerOutcome::Handled
    }

    pub(super) fn scrollbar_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        probe: &PointerProbe,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Gesture::Scrollbar(drag) = &mut self.gesture else {
            return PointerOutcome::Ignored;
        };
        let release = event.action == MouseAction::Release
            && matches!(event.button, MouseButton::Left | MouseButton::None);
        let movement = event.action == MouseAction::Motion && event.button == MouseButton::Left;
        if !drag.alive(probe) || event.mods != Modifiers::NONE || (!release && !movement) {
            self.gesture = Gesture::Idle;
            return PointerOutcome::Handled;
        }
        if drag.last_row != event.row && !drag.scroll(port, event.row) {
            self.gesture = Gesture::Idle;
            return PointerOutcome::Handled;
        }
        if release {
            self.gesture = Gesture::Idle;
        }
        PointerOutcome::Handled
    }
}
