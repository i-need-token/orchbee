//! Drag coordinates remain tied to the painted copy episode and content surface.
use super::{
    ClientView, FrameHitMap, HitResolution, HitTarget, Modifiers, MouseAction, MouseButton,
    MouseEvent, PointerState, Rect, TerminalInputPort, plain_primary_press,
};
use crate::terminal_copy_mode::{CopyModeCursor, CopySurfaceToken};

#[derive(Clone, Copy)]
pub(super) struct Drag {
    token: CopySurfaceToken,
    area: Rect,
    origin: CopyModeCursor,
}
impl Drag {
    pub(super) fn painted_in(&self, map: &FrameHitMap) -> bool {
        !map.overlay_active
            && map.regions.iter().any(|region| {
                region.target
                    == HitTarget::CopySurface {
                        token: self.token,
                        area: self.area,
                        origin: self.origin,
                    }
            })
    }
    fn cell(self, event: MouseEvent) -> CopyModeCursor {
        CopyModeCursor {
            column: event
                .column
                .saturating_sub(self.area.x)
                .min(self.area.width.saturating_sub(1))
                .saturating_add(self.origin.column),
            row: event
                .row
                .saturating_sub(self.area.y)
                .min(self.area.height.saturating_sub(1))
                .saturating_add(self.origin.row),
        }
    }
}

impl PointerState {
    pub(super) fn cancel_copy_drag(&mut self) {
        self.copy_drag = None;
    }

    pub(crate) fn copy_event(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        if self.queue_list_scroll(event) {
            return false;
        }
        if self.copy_control_event(view, port, event) {
            return true;
        }
        if !self.capture || event.mods != Modifiers::NONE {
            self.cancel_copy_drag();
            return false;
        }
        if let Some(drag) = self.copy_drag {
            if matches!(event.action, MouseAction::Motion | MouseAction::Release)
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                if self.map.as_ref().is_none_or(|map| !drag.painted_in(map))
                    || !port.copy_pointer_cell(drag.token, drag.cell(event), false)
                    || event.action == MouseAction::Release
                {
                    self.cancel_copy_drag();
                }
                return event.action == MouseAction::Release;
            }
            if event.action == MouseAction::Press {
                self.cancel_copy_drag();
            }
        }
        if !plain_primary_press(event) {
            return false;
        }
        self.cancel_list_scroll();
        if let Some(map) = &self.map
            && let HitResolution::Region(HitTarget::CopySurface {
                token,
                area,
                origin,
            }) = map.hit(event.column, event.row)
        {
            let drag = Drag {
                token: *token,
                area: *area,
                origin: *origin,
            };
            if port.copy_pointer_cell(drag.token, drag.cell(event), true) {
                self.copy_drag = Some(drag);
            }
        }
        false
    }
}
