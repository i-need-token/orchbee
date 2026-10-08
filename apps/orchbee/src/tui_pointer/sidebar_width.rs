//! Local width gestures share successful-frame admission with keyboard adjustment.
use super::*;
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
            // The preview covered canonical status controls. They cannot
            // become clickable again until a clean frame is published.
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
        view.machines.available
            && !view.machines.open
            && !view.resize_mode()
            && matches!(view.overlay(), None | Some(Overlay::BusinessSidebar))
            && DrawnSidebarWidth::new(target.viewport, view.sidebar_surface()) == Some(*target)
            && self
                .map
                .as_ref()
                .is_some_and(|map| map.exposes_sidebar_width(target))
    }

    /// Does not require mouse capture: keyboard uses the same successful paint.
    pub(crate) fn sidebar_width_target(&self, view: &ClientView) -> Option<DrawnSidebarWidth> {
        if !self.can_open_action_menu() || view.prefix_active() {
            return None;
        }
        self.map.as_ref()?.regions.iter().find_map(|region| {
            if let HitTarget::SidebarResize(target) = &region.target {
                self.sidebar_width_source_alive(view, target)
                    .then_some(*target)
            } else {
                None
            }
        })
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
