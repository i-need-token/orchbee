//! The open Find bar owns its controls, its body and the content of the Panel it searches. Every
//! other report keeps its ordinary route, so a press on another Panel, the Tab strip or the
//! sidebar does what it always does, and the bar closes when its Panel loses focus.
use super::{
    FindAction, Gesture, HitResolution, HitTarget, Modifiers, MouseAction, MouseButton, MouseEvent,
    PointerState, ReleaseOwnership, SurfaceAction, TerminalInputPort,
};
use crate::terminal_find::FindToken;

impl PointerState {
    /// Whether the open Find bar owns this report; it also settles hover over the bar's controls.
    /// A press already held by a bar control keeps its release, and a gesture that started
    /// elsewhere keeps its own route.
    pub(crate) fn find_owns(&mut self, event: MouseEvent) -> bool {
        self.observe_find_motion(event);
        if self.modal_press.is_some() {
            return true;
        }
        if !matches!(self.gesture, Gesture::Idle)
            || self.interactions.release_ownership() != ReleaseOwnership::None
        {
            return false;
        }
        self.find_target(event).is_some()
    }

    /// The bar a report lands on, and whether it lands on the searched content.
    fn find_target(&self, event: MouseEvent) -> Option<(FindToken, bool)> {
        match self.map.as_ref()?.hit(event.column, event.row) {
            HitResolution::Region(
                HitTarget::FindControl { token, .. } | HitTarget::FindSurface(token),
            ) => Some((*token, false)),
            HitResolution::Region(HitTarget::FindContent(token)) => Some((*token, true)),
            _ => None,
        }
    }

    /// One report the bar owns: a control acts on its plain left release, the wheel scrolls the
    /// searched history, and a plain press on the searched content closes the bar. Nothing
    /// reaches the program. Returns `true` when the report ended the bar's packet.
    pub(crate) fn find_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        if let Some(settled) = self.find_control_event(port, event) {
            return settled;
        }
        if !self.capture || event.action != MouseAction::Press || event.mods != Modifiers::NONE {
            return false;
        }
        let Some((token, content)) = self.find_target(event) else {
            return false;
        };
        match event.button {
            MouseButton::WheelUp | MouseButton::WheelDown => {
                port.surface_action(SurfaceAction::Find(FindAction::Scroll {
                    token,
                    up: event.button == MouseButton::WheelUp,
                }));
                false
            }
            MouseButton::Left | MouseButton::Right | MouseButton::Middle if content => {
                port.surface_action(SurfaceAction::Find(FindAction::Close(token)));
                self.stale_map();
                true
            }
            _ => false,
        }
    }
}
