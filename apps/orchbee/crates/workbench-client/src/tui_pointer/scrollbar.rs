//! Press-owned scrollbar dragging. A pane's changed history range cancels capture, and every
//! effect rechecks the exact pane and source in WorkbenchClientUi. The sidebar list's scrollbar
//! shares the same geometry and gesture shape over its own window.

use super::{
    FrameHitMap, Gesture, HitResolution, HitTarget, PointerOutcome, PointerState, contains,
};
use crate::client_view::{ClientAction, ClientView, Modifiers};
use crate::machine_picker::SidebarWindow;
use crate::terminal_events::{MouseAction, MouseButton, MouseEvent};
use crate::terminal_scrollbar::{ScrollbarRange, ScrollbarSnapshot};
use crate::workbench_client::input::{PaneAdmission, PaneInput, PointerProbe, TerminalInputPort};
use crate::workbench_state::PaneRef;
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
        if port.pane_input(PaneInput::Scrollbar {
            pane: &self.pane,
            source: self.source,
            offset,
        }) != PaneAdmission::Admitted
        {
            return false;
        }
        self.last_offset = Some(offset);
        true
    }
}

/// A press-owned drag of the sidebar list's scrollbar. It keeps the press's track and range
/// extent, which scrolling never changes; a frame that paints another extent, or no scrollbar,
/// ends it. A press on the track centers the thumb there, as on a pane's.
pub(super) struct SidebarScrollbarDrag {
    track: Rect,
    range: ScrollbarRange,
    window: SidebarWindow,
    grab: u16,
    last_row: u16,
}

impl SidebarScrollbarDrag {
    /// Whether `map` still paints this scrollbar; its window, moved by this drag, is taken over.
    pub(super) fn repainted(&mut self, map: &FrameHitMap) -> bool {
        let painted = map.regions.iter().find_map(|region| match &region.target {
            HitTarget::SidebarScrollbar {
                track,
                range,
                window,
            } if *track == self.track && range.same_extent(self.range) => Some(*window),
            _ => None,
        });
        if let Some(window) = painted {
            self.window = window;
        }
        painted.is_some()
    }

    /// Puts the grabbed thumb at `row`. Returns whether the list moved.
    fn scroll(&mut self, view: &mut ClientView, row: u16) -> bool {
        self.last_row = row;
        let Some(top) = self
            .range
            .start_at(self.track, row, self.grab)
            .and_then(|top| usize::try_from(top).ok())
        else {
            return false;
        };
        if !view.machines.scroll_sidebar_to(self.window, top) {
            return false;
        }
        self.window.top = top.min(self.window.maximum);
        true
    }
}

impl PointerState {
    /// Keyboard input or host focus loss ends this Client-owned gesture.
    pub(crate) fn cancel_scrollbar(&mut self) {
        if matches!(
            self.gesture,
            Gesture::Scrollbar(_) | Gesture::SidebarScrollbar(_)
        ) {
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

    /// A press on the sidebar list's scrollbar. The wheel scrolls the list as it does over its
    /// rows; a plain left press grabs the thumb or centers it on the pressed track row, without
    /// moving keyboard focus.
    pub(super) fn begin_sidebar_scrollbar(
        &mut self,
        view: &mut ClientView,
        event: MouseEvent,
        (track, range, window): (Rect, ScrollbarRange, SidebarWindow),
    ) -> PointerOutcome {
        if event.action != MouseAction::Press || event.mods != Modifiers::NONE {
            return PointerOutcome::Ignored;
        }
        match event.button {
            MouseButton::WheelUp | MouseButton::WheelDown => {
                view.machines
                    .queue_space_scroll(window, event.button == MouseButton::WheelUp);
                return PointerOutcome::Handled;
            }
            MouseButton::Left => {}
            _ => return PointerOutcome::Ignored,
        }
        // Like the wheel over the list, only the sidebar's own overlay leaves it scrollable.
        if view
            .overlay()
            .is_some_and(|overlay| overlay != crate::client_view::Overlay::BusinessSidebar)
            || view.machines.captures()
        {
            return PointerOutcome::Handled;
        }
        let Some(thumb) = range.thumb(track) else {
            return PointerOutcome::Handled;
        };
        let on_thumb = contains(thumb, event.column, event.row);
        let mut drag = SidebarScrollbarDrag {
            track,
            range,
            window,
            grab: if on_thumb {
                event.row - thumb.y
            } else {
                thumb.height / 2
            },
            last_row: event.row,
        };
        let moved = !on_thumb && drag.scroll(view, event.row);
        self.click = None;
        self.wheel.clear();
        self.gesture = Gesture::SidebarScrollbar(drag);
        if moved {
            PointerOutcome::RegionsChanged
        } else {
            PointerOutcome::Handled
        }
    }

    pub(super) fn sidebar_scrollbar_event(
        &mut self,
        view: &mut ClientView,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Gesture::SidebarScrollbar(drag) = &mut self.gesture else {
            return PointerOutcome::Ignored;
        };
        let release = event.action == MouseAction::Release
            && matches!(event.button, MouseButton::Left | MouseButton::None);
        let movement = event.action == MouseAction::Motion && event.button == MouseButton::Left;
        if event.mods != Modifiers::NONE || (!release && !movement) {
            self.gesture = Gesture::Idle;
            return PointerOutcome::Handled;
        }
        let moved = drag.last_row != event.row && drag.scroll(view, event.row);
        if release {
            self.gesture = Gesture::Idle;
        }
        if moved {
            PointerOutcome::RegionsChanged
        } else {
            PointerOutcome::Handled
        }
    }
}
