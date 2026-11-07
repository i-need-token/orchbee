//! Release-activation authority for generic painted interactions.
//!
//! Hover and armed presses project into the hit map's pointer projection.
//! Feature effects remain with the dispatcher and their domain owners, while
//! specialized drags keep their dedicated state machines.

use super::{FrameHitMap, InteractionTarget, PrimaryContract};
use crate::tui_hit_map::interaction::{Fingerprint, PointerProjection};
use crate::{
    client_view::Modifiers,
    terminal_events::{MouseAction, MouseButton, MouseEvent},
};
use std::time::Instant;

/// Whether the settled outer transport can report button-free movement.
///
/// Lifecycle code changes this only after its mode effect settles. Re-admission
/// never supplies a synthetic position; a real movement report must arrive.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MotionObservability {
    #[default]
    ButtonOnly,
    AnyMotion,
}

#[derive(Clone, Debug)]
struct Arm {
    fingerprint: Fingerprint,
    backdrop: bool,
    column: u16,
    row: u16,
    cancelled: bool,
    second_click: bool,
}

impl Arm {
    fn matches_map(&self, map: &FrameHitMap, column: u16, row: u16) -> bool {
        let current = if self.backdrop {
            map.backdrop_interaction_at(column, row)
        } else {
            map.interaction_at(column, row)
        };
        current.as_ref() == Some(&self.fingerprint)
    }

    fn active(&self) -> bool {
        !self.cancelled
    }

    /// Takes the exposure `map` recorded for the pressed interaction when that frame painted the
    /// press and then revealed the interaction's pointer-only children: a press read together
    /// with the motion onto its target precedes the frame that reveals them. Whether the
    /// interaction is still current is decided afterwards, as for any frame.
    fn follow_reveal(&mut self, map: &FrameHitMap) -> bool {
        if !map.revealed_for_pointer(&self.fingerprint) {
            return false;
        }
        let current = if self.backdrop {
            map.backdrop_interaction_at(self.column, self.row)
        } else {
            map.interaction_at(self.column, self.row)
        };
        let Some(current) = current.filter(|current| {
            current.same_interaction(&self.fingerprint) && *current != self.fingerprint
        }) else {
            return false;
        };
        self.fingerprint = current;
        true
    }

    fn cancel(&mut self) -> bool {
        let changed = !self.cancelled;
        self.cancelled = true;
        changed
    }
}

pub(super) enum Event {
    Ignored,
    Captured,
    /// An already accepted primary press reached its cleanup boundary
    /// without activating. The dispatcher must fence the physical packet
    /// that carried the matching release.
    Ended,
    Intent {
        target: Box<InteractionTarget>,
        contract: PrimaryContract,
    },
}

/// Pointer-side ownership of a packet-level primary-release latch.
///
/// Active owners must see the release before the dispatcher fences its packet.
/// Cleanup owners have already lost semantic authority but still consume that
/// exact release once without consulting current hit geometry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ReleaseOwnership {
    #[default]
    None,
    Active,
    Cleanup,
}

impl ReleaseOwnership {
    pub(super) const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Active, _) | (_, Self::Active) => Self::Active,
            (Self::Cleanup, _) | (_, Self::Cleanup) => Self::Cleanup,
            (Self::None, Self::None) => Self::None,
        }
    }
}

/// One generic pointer owner. A cancelled arm remains as a cleanup tombstone
/// until its matching release or a later press retires it.
#[derive(Default)]
pub(super) struct State {
    position: Option<(u16, u16)>,
    hovered: Option<Fingerprint>,
    arm: Option<Arm>,
    last_click: Option<(Fingerprint, u16, u16, Instant)>,
    revision: u64,
    redraw_after_publish: bool,
}

impl State {
    /// The cell of the last admitted movement report.
    pub(super) const fn position(&self) -> Option<(u16, u16)> {
        self.position
    }

    pub(super) fn hovered_target(&self) -> Option<&InteractionTarget> {
        self.hovered.as_ref().map(|fingerprint| &fingerprint.target)
    }

    pub(super) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn projection(&self) -> PointerProjection {
        PointerProjection {
            revision: self.revision,
            hovered: self.hovered.clone(),
            pressed: self
                .arm
                .as_ref()
                .filter(|arm| arm.active() && !arm.backdrop)
                .map(|arm| arm.fingerprint.clone()),
            modal: crate::tui_hit_map::modal::Projection::empty(),
            chrome: crate::tui_hit_map::chrome::Projection::empty(),
            tooltip: None,
        }
    }

    pub(super) fn release_ownership(&self) -> ReleaseOwnership {
        match self.arm.as_ref() {
            Some(arm) if arm.active() => ReleaseOwnership::Active,
            Some(_) => ReleaseOwnership::Cleanup,
            None => ReleaseOwnership::None,
        }
    }

    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn replace_hover(&mut self, next: Option<Fingerprint>) -> bool {
        if self.hovered == next {
            return false;
        }
        self.hovered = next;
        self.changed();
        true
    }

    fn cancel_arm(&mut self) -> bool {
        let changed = self.arm.as_mut().is_some_and(Arm::cancel);
        if changed {
            self.changed();
        }
        changed
    }

    /// Applies a real pointer report to hover presentation. Only movement
    /// can establish position after transport loss; later reports may refine
    /// an already observed position while any-motion remains admitted.
    pub(super) fn observe_motion(
        &mut self,
        map: Option<&FrameHitMap>,
        observability: MotionObservability,
        event: MouseEvent,
        accepts: fn(&InteractionTarget) -> bool,
    ) -> bool {
        if observability != MotionObservability::AnyMotion
            || event.mods.shift
            || (event.action != MouseAction::Motion && self.position.is_none())
        {
            return false;
        }
        self.position = Some((event.column, event.row));
        let next = map
            .and_then(|map| map.interaction_at(event.column, event.row))
            .filter(|fingerprint| accepts(&fingerprint.target))
            .filter(Fingerprint::enabled);
        self.replace_hover(next)
    }

    /// Gives a declared generic interaction first refusal over its press,
    /// ordered boundary crossings, and matching release.
    pub(super) fn event(
        &mut self,
        map: Option<&FrameHitMap>,
        event: MouseEvent,
        now: Instant,
        admit_new: bool,
        accepts: fn(&InteractionTarget) -> bool,
    ) -> Event {
        if self
            .arm
            .as_ref()
            .is_some_and(|arm| !accepts(&arm.fingerprint.target))
        {
            self.cancel_arm();
        }
        if let Some(arm) = &mut self.arm {
            let was_active = arm.active();
            if event.mods != Modifiers::NONE {
                arm.cancel();
            }
            if event.action == MouseAction::Motion
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                if !was_active {
                    return Event::Captured;
                }
                let moved = (arm.column, arm.row) != (event.column, event.row);
                arm.column = event.column;
                arm.row = event.row;
                let current = map.is_some_and(|map| arm.matches_map(map, event.column, event.row));
                if !current
                    || (arm.fingerprint.spec.primary == PrimaryContract::ActivateOnDoubleClick
                        && moved)
                {
                    arm.cancel();
                }
                if was_active && !arm.active() {
                    self.changed();
                }
                return Event::Captured;
            }
            if event.action == MouseAction::Release
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                let arm = self.arm.take().expect("checked above");
                let current = map.is_some_and(|map| arm.matches_map(map, event.column, event.row));
                let admitted = arm.active()
                    && (arm.fingerprint.spec.primary != PrimaryContract::ActivateOnDoubleClick
                        || (arm.column, arm.row) == (event.column, event.row))
                    && current
                    && (arm.backdrop
                        || super::contains(arm.fingerprint.paint_rect, event.column, event.row));
                if was_active {
                    self.changed();
                }
                if admitted
                    && arm.fingerprint.spec.primary == PrimaryContract::ActivateOnDoubleClick
                {
                    if arm.second_click {
                        return Event::Intent {
                            target: Box::new(arm.fingerprint.target),
                            contract: PrimaryContract::ActivateOnRelease,
                        };
                    }
                    self.last_click = Some((arm.fingerprint, event.column, event.row, now));
                    return Event::Ended;
                }
                self.last_click = None;
                return if admitted
                    && arm.fingerprint.spec.primary == PrimaryContract::ActivateOnRelease
                {
                    Event::Intent {
                        target: Box::new(arm.fingerprint.target),
                        contract: PrimaryContract::ActivateOnRelease,
                    }
                } else {
                    Event::Ended
                };
            }
            if event.action == MouseAction::Press
                && matches!(
                    event.button,
                    MouseButton::WheelUp
                        | MouseButton::WheelDown
                        | MouseButton::WheelLeft
                        | MouseButton::WheelRight
                )
            {
                arm.cancel();
                if was_active && !arm.active() {
                    self.changed();
                }
                return Event::Captured;
            }
            let remains_active = arm.active();
            if was_active && !remains_active {
                self.changed();
            }
            if event.action != MouseAction::Press {
                return if remains_active {
                    Event::Ignored
                } else {
                    Event::Captured
                };
            }
            let previous = self.arm.take().expect("checked above");
            if previous.active() {
                self.changed();
            }
        }

        let previous_click = if event.action == MouseAction::Press {
            self.last_click.take()
        } else {
            None
        };
        if event.action != MouseAction::Press
            || event.button != MouseButton::Left
            || event.mods != Modifiers::NONE
            || !admit_new
        {
            return Event::Ignored;
        }
        let Some(map) = map else {
            return Event::Ignored;
        };
        let backdrop = matches!(
            map.hit(event.column, event.row),
            super::HitResolution::OverlayBackdrop
        );
        let Some(fingerprint) = map
            .interaction_at(event.column, event.row)
            .or_else(|| map.backdrop_interaction_at(event.column, event.row))
            .filter(|fingerprint| accepts(&fingerprint.target))
        else {
            return Event::Ignored;
        };
        if !fingerprint.enabled() {
            // A disabled painted control remains an explicit interaction
            // boundary. Retain cleanup-only ownership so its matching
            // release cannot fall through to a containing row or pane, but
            // never project Pressed or emit a semantic intent.
            self.arm = Some(Arm {
                fingerprint,
                backdrop,
                column: event.column,
                row: event.row,
                cancelled: true,
                second_click: false,
            });
            return Event::Captured;
        }
        if fingerprint.spec.primary == PrimaryContract::Specialized {
            return Event::Ignored;
        }

        let second_click = previous_click.is_some_and(|(previous, column, row, at)| {
            previous == fingerprint
                && (column, row) == (event.column, event.row)
                && now.saturating_duration_since(at) <= super::DOUBLE_CLICK_INTERVAL
        });
        let contract = fingerprint.spec.primary;
        let target = fingerprint.target.clone();
        self.arm = Some(Arm {
            fingerprint,
            backdrop,
            column: event.column,
            row: event.row,
            cancelled: false,
            second_click,
        });
        self.changed();
        if contract == PrimaryContract::SelectOnPress {
            Event::Intent {
                target: Box::new(target),
                contract,
            }
        } else {
            Event::Captured
        }
    }

    /// Settles only cleanup ownership after the transport or host focus has
    /// gone away. It cannot re-hit, arm, or activate a target. A fresh
    /// non-wheel press retires the old debt and belongs to the new transport
    /// state; a matching late release ends the accepted gesture exactly once.
    pub(super) fn cleanup_event(&mut self, event: MouseEvent) -> Event {
        let Some(arm) = self.arm.as_ref() else {
            return Event::Ignored;
        };
        if arm.active() {
            return Event::Ignored;
        }
        if event.action == MouseAction::Press
            && !matches!(
                event.button,
                MouseButton::WheelUp
                    | MouseButton::WheelDown
                    | MouseButton::WheelLeft
                    | MouseButton::WheelRight
            )
        {
            self.arm = None;
            return Event::Ignored;
        }
        if event.action == MouseAction::Release
            && matches!(event.button, MouseButton::Left | MouseButton::None)
        {
            self.arm = None;
            return Event::Ended;
        }
        Event::Captured
    }

    /// Reconciles only after the candidate frame was successfully presented.
    /// Returns whether that frame used an older projection and needs one later
    /// bounded repaint.
    pub(super) fn publish(&mut self, map: &FrameHitMap) -> bool {
        if self
            .last_click
            .as_ref()
            .is_some_and(|(fingerprint, column, row, _)| {
                map.interaction_at(*column, *row).as_ref() != Some(fingerprint)
            })
        {
            self.last_click = None;
        }
        if self
            .arm
            .as_mut()
            .is_some_and(|arm| arm.active() && arm.follow_reveal(map))
        {
            self.changed();
        }
        if self.arm.as_ref().is_some_and(Arm::active) {
            let still_current = self
                .arm
                .as_ref()
                .is_some_and(|arm| arm.matches_map(map, arm.column, arm.row));
            if !still_current {
                self.cancel_arm();
            }
        }
        if self.arm.as_ref().is_none_or(|arm| !arm.active()) {
            let next = self
                .position
                .and_then(|(column, row)| map.interaction_at(column, row))
                .filter(Fingerprint::enabled);
            self.replace_hover(next);
        }
        self.redraw_after_publish = map.projection_revision != self.revision;
        self.redraw_after_publish
    }

    /// Structural invalidation clears presented authority but retains an
    /// observed position for resolution against the next successful frame.
    pub(super) fn invalidate_map(&mut self) {
        self.last_click = None;
        self.replace_hover(None);
        self.cancel_arm();
        self.redraw_after_publish = false;
    }

    /// Capture, focus, or motion-observability loss also forgets position so
    /// restoration cannot synthesize hover.
    pub(super) fn clear_transport(&mut self) {
        self.position = None;
        self.last_click = None;
        self.replace_hover(None);
        self.cancel_arm();
        self.redraw_after_publish = false;
    }

    pub(super) fn take_redraw_after_publish(&mut self) -> bool {
        std::mem::take(&mut self.redraw_after_publish)
    }
}
