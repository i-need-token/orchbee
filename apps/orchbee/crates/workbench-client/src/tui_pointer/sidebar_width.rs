//! Local width gestures admit only a successfully painted sidebar edge.
use super::{
    ClientView, Gesture, MouseAction, MouseButton, MouseEvent, PointerOutcome, PointerState,
};
use crate::{
    client_view::{Modifiers, Overlay},
    tui_layout::{DrawnSidebarWidth, SidebarWidthDrag},
};

impl PointerState {
    pub(crate) fn sidebar_width_preview(&self) -> Option<&SidebarWidthDrag> {
        match &self.gesture {
            Gesture::SidebarWidth(drag) => Some(drag),
            _ => None,
        }
    }

    pub(crate) fn cancel_sidebar_width(&mut self) -> bool {
        if matches!(self.gesture, Gesture::SidebarWidth(_)) {
            self.gesture = Gesture::Idle;
            self.cancelled_chrome_release
                .get_or_insert(super::CancelledChromeRelease::SidebarWidth);
            // The captured gesture owns this frame's input until a clean
            // canonical frame is published after its preview ends.
            self.map = None;
            true
        } else {
            false
        }
    }

    pub(super) fn sidebar_width_source_alive(
        &self,
        view: &ClientView,
        target: &DrawnSidebarWidth,
    ) -> bool {
        let painted = self
            .sidebar_width_preview()
            .filter(|drag| &drag.target == target)
            .map_or(target, |drag| &drag.painted);
        view.machines.available
            && !view.machines.captures()
            && matches!(view.overlay(), None | Some(Overlay::BusinessSidebar))
            && DrawnSidebarWidth::new(target.viewport, view.sidebar_surface()) == Some(*target)
            && self
                .map
                .as_ref()
                .is_some_and(|map| map.exposes_sidebar_width(painted))
    }

    pub(super) fn begin_sidebar_width(
        &mut self,
        view: &mut ClientView,
        target: DrawnSidebarWidth,
        event: MouseEvent,
    ) -> PointerOutcome {
        if event.action != MouseAction::Press
            || event.button != MouseButton::Left
            || event.mods != Modifiers::NONE
            || !self.can_open_action_menu()
            || !self.sidebar_width_source_alive(view, &target)
        {
            return PointerOutcome::Ignored;
        }
        if let Some(drag) = SidebarWidthDrag::begin(target, event.column, event.row) {
            self.click = None;
            self.wheel.clear();
            view.machines.cancel_sidebar_scroll();
            self.gesture = Gesture::SidebarWidth(drag);
        }
        PointerOutcome::Handled
    }

    pub(super) fn sidebar_width_event(
        &mut self,
        view: &mut ClientView,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Some(drag) = self.sidebar_width_preview() else {
            return PointerOutcome::Ignored;
        };
        if event.mods.shift || !self.sidebar_width_source_alive(view, &drag.target) {
            self.cancel_sidebar_width();
            if event.action == MouseAction::Release
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                self.cancelled_chrome_release = None;
            }
            return PointerOutcome::SidebarWidthEnded;
        }
        match event.action {
            MouseAction::Motion if event.button == MouseButton::Left => {
                if let Gesture::SidebarWidth(drag) = &mut self.gesture {
                    drag.update(event.column);
                }
            }
            MouseAction::Release
                if matches!(event.button, MouseButton::Left | MouseButton::None) =>
            {
                let Gesture::SidebarWidth(mut drag) = std::mem::take(&mut self.gesture) else {
                    unreachable!();
                };
                self.map = None;
                if event.button != MouseButton::Left || event.mods != Modifiers::NONE {
                    return PointerOutcome::SidebarWidthEnded;
                }
                drag.update(event.column);
                view.machines
                    .set_sidebar_width(drag.target.preferred(), drag.width);
                return PointerOutcome::SidebarWidthEnded;
            }
            _ => {}
        }
        PointerOutcome::Handled
    }
}
