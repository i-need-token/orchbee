//! Pointer tooltips: when the explanation a painted region offers opens, and what closes it.
//!
//! The renderer attaches [`TooltipContent`] to an exact recorded region while it paints a candidate
//! frame; the published map keeps only which regions offer one. This state decides from real
//! movement reports and time whether one region's tooltip is open. A key never passes through here:
//! it only closes the tooltip, and the dispatcher routes it to its owner unchanged.

use std::time::{Duration, Instant};

use super::FrameHitMap;
use super::hit_map::tooltip::{Key, Shown};

/// How long the pointer rests on a target before its tooltip opens.
const OPEN_DELAY: Duration = Duration::from_millis(300);
/// After a tooltip closes because the pointer left its target, the next target's opens at once.
const ADJACENT_GRACE: Duration = Duration::from_millis(400);

/// Tooltip state owned by `PointerState`. Every transition that changes what is painted asks for
/// one redraw, which the dispatcher's next tick reports.
#[derive(Debug, Default)]
pub(super) struct State {
    /// The cell of the last real movement report.
    position: Option<(u16, u16)>,
    /// When the pointer last moved. One open attempt consumes it, so content that later moves
    /// under a still pointer never opens a tooltip.
    rest_since: Option<Instant>,
    open: Option<Shown>,
    /// Until then the next target's tooltip opens without the rest.
    warm_until: Option<Instant>,
    /// A pressed target stays quiet until the pointer leaves it.
    quiet: Option<Key>,
    /// A movement report has not met a published frame yet: what that frame reveals beneath the
    /// pointer is the same arrival, not content moving under a still pointer.
    unsettled: bool,
    /// The painted tooltip changed outside a frame: the next tick asks for one.
    redraw: bool,
    /// A presented frame showed a tooltip that has since closed or changed target.
    redraw_after_publish: bool,
}

impl State {
    pub(super) fn projection(&self) -> Option<Shown> {
        self.open.clone()
    }

    fn close(&mut self) {
        if self.open.take().is_some() {
            self.redraw = true;
        }
    }

    fn warm(&self, now: Instant) -> bool {
        self.warm_until.is_some_and(|until| now < until)
    }

    /// A real button-free movement report.
    pub(super) fn motion(&mut self, map: Option<&FrameHitMap>, cell: (u16, u16), now: Instant) {
        self.position = Some(cell);
        self.unsettled = true;
        let at = map.and_then(|map| map.tooltip_at(cell.0, cell.1));
        if self.quiet.is_some() && self.quiet != at {
            self.quiet = None;
        }
        if let Some(open) = &self.open {
            if Some(&open.key) == at.as_ref() {
                // It stays put while the pointer moves within its target.
                return;
            }
            self.close();
            self.warm_until = Some(now + ADJACENT_GRACE);
        }
        match at.filter(|key| self.quiet.as_ref() != Some(key)) {
            Some(key) if self.warm(now) => {
                self.open = Some(Shown { key, pointer: cell });
                self.redraw = true;
                self.rest_since = None;
            }
            _ => self.rest_since = Some(now),
        }
    }

    /// A press or wheel notch: the tooltip closes and the target under it stays quiet.
    pub(super) fn press(&mut self, map: Option<&FrameHitMap>, cell: (u16, u16)) {
        self.quiet = map.and_then(|map| map.tooltip_at(cell.0, cell.1));
        self.dismiss();
    }

    /// A key, a drag, or a structural change: nothing stays open, pending or warm.
    pub(super) fn dismiss(&mut self) {
        self.close();
        self.rest_since = None;
        self.warm_until = None;
    }

    /// Focus, capture, or any-motion loss: the old position no longer describes the pointer.
    pub(super) fn clear_transport(&mut self) {
        self.dismiss();
        self.position = None;
        self.quiet = None;
        self.unsettled = false;
    }

    /// Opens the tooltip under a pointer that has rested long enough, while nothing else owns the
    /// screen. Returns whether the painted tooltip changed since the last tick.
    pub(super) fn tick(&mut self, map: Option<&FrameHitMap>, now: Instant, admitted: bool) -> bool {
        if !admitted {
            self.dismiss();
        } else if let (Some(since), Some(map), Some(cell)) = (self.rest_since, map, self.position)
            && now.saturating_duration_since(since) >= OPEN_DELAY
        {
            self.rest_since = None;
            if self.open.is_none()
                && let Some(key) = map
                    .tooltip_at(cell.0, cell.1)
                    .filter(|key| self.quiet.as_ref() != Some(key))
            {
                self.open = Some(Shown { key, pointer: cell });
                self.redraw = true;
            }
        }
        if !self.warm(now) {
            self.warm_until = None;
        }
        std::mem::take(&mut self.redraw)
    }

    /// Reconciles the open tooltip with a successfully presented frame: it closes once its target
    /// moved, went away or changed what it does.
    pub(super) fn publish(&mut self, map: &FrameHitMap) {
        let unsettled = std::mem::take(&mut self.unsettled);
        let Some(open) = &mut self.open else {
            return;
        };
        let Some(position) = self.position else {
            self.open = None;
            self.redraw_after_publish = true;
            return;
        };
        let at = map.tooltip_at(position.0, position.1);
        if at.as_ref() == Some(&open.key) {
            return;
        }
        // What the frame after a movement reveals beneath the pointer is where it arrived.
        match at.filter(|key| unsettled && self.quiet.as_ref() != Some(key)) {
            Some(key) => {
                *open = Shown {
                    key,
                    pointer: position,
                };
            }
            None => self.open = None,
        }
        self.redraw_after_publish = true;
    }

    pub(super) fn take_redraw_after_publish(&mut self) -> bool {
        std::mem::take(&mut self.redraw_after_publish)
    }
}

#[cfg(test)]
mod tests;
