//! Paint-only pointer projection for specialized shell chrome.
//!
//! Divider and tab presses keep their existing drag state machines. This
//! module projects only exact hover/pressed presentation from their painted
//! semantic witness; it never emits an action or starts a gesture.

use super::{FrameHitMap, MotionObservability, MouseAction, MouseEvent};
use crate::tui_hit_map::chrome::{Fingerprint, Projection};

#[derive(Default)]
pub(super) struct ProjectionState {
    position: Option<(u16, u16)>,
    hovered: Option<Fingerprint>,
    redraw_after_publish: bool,
}

impl ProjectionState {
    fn replace_hover(&mut self, next: Option<Fingerprint>) -> bool {
        if self.hovered == next {
            return false;
        }
        self.hovered = next;
        true
    }

    pub(super) fn observe_motion(
        &mut self,
        map: Option<&FrameHitMap>,
        observability: MotionObservability,
        event: MouseEvent,
        suppress: bool,
    ) -> bool {
        if observability != MotionObservability::AnyMotion
            || event.mods.shift
            || (event.action != MouseAction::Motion && self.position.is_none())
        {
            return false;
        }
        self.position = Some((event.column, event.row));
        let next = (!suppress)
            .then(|| map.and_then(|map| map.chrome_fingerprint_at(event.column, event.row)))
            .flatten();
        self.replace_hover(next)
    }

    pub(super) fn snapshot(
        &self,
        pressed: Option<Fingerprint>,
        suppress_hover: bool,
    ) -> Projection {
        Projection {
            hovered: if suppress_hover {
                None
            } else {
                self.hovered.clone()
            },
            pressed,
        }
    }

    pub(super) fn publish(
        &mut self,
        map: &FrameHitMap,
        pressed: Option<Fingerprint>,
        suppress_hover: bool,
    ) -> bool {
        if pressed.is_none() {
            let next = (!suppress_hover)
                .then(|| {
                    self.position
                        .and_then(|(column, row)| map.chrome_fingerprint_at(column, row))
                })
                .flatten();
            self.replace_hover(next);
        }
        self.redraw_after_publish = map.chrome_projection != self.snapshot(pressed, suppress_hover);
        self.redraw_after_publish
    }

    pub(super) fn invalidate_map(&mut self) {
        self.hovered = None;
        self.redraw_after_publish = false;
    }

    pub(super) fn clear_transport(&mut self) {
        self.position = None;
        self.hovered = None;
        self.redraw_after_publish = false;
    }

    pub(super) fn take_redraw_after_publish(&mut self) -> bool {
        std::mem::take(&mut self.redraw_after_publish)
    }
}
