//! A resting pointer steers a cascading menu. A submenu row opens its child after a short rest;
//! another row of the parent takes the menu back after a grace that every movement restarts, so
//! a diagonal path toward the submenu may cross parent rows without closing it.
//!
//! This state only times rests over the published menu targets. The menu owner decides whether a
//! rested row opens anything and revalidates the exact painted token before it acts.

use std::time::{Duration, Instant};

use crate::action_menu::MenuControl;
use crate::tui_hit_map::InteractionTarget;

/// How long the pointer stays on a submenu row before its child opens.
const OPEN_DELAY: Duration = Duration::from_millis(200);
/// How long the pointer stays still over another parent row before the menu returns to it.
const RETURN_GRACE: Duration = Duration::from_millis(300);

#[derive(Debug, Default)]
pub(super) struct State {
    pending: Option<Pending>,
}

#[derive(Debug)]
struct Pending {
    target: InteractionTarget,
    due: Instant,
}

impl State {
    /// One admitted button-free movement report; `hovered` is the menu target now under the
    /// pointer.
    pub(super) fn motion(&mut self, hovered: Option<&InteractionTarget>, now: Instant) {
        let delay = match hovered {
            Some(InteractionTarget::ActionMenuRow { .. }) => OPEN_DELAY,
            Some(InteractionTarget::ActionMenuControl {
                control: MenuControl::ParentRow(_),
                ..
            }) => RETURN_GRACE,
            _ => {
                self.pending = None;
                return;
            }
        };
        let Some(hovered) = hovered else {
            return;
        };
        // A row opens a fixed time after the pointer reaches it; a parent row waits for stillness.
        if delay == OPEN_DELAY
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| same_place(&pending.target, hovered))
        {
            return;
        }
        self.pending = Some(Pending {
            target: hovered.clone(),
            due: now + delay,
        });
    }

    /// A press, a key, lost admission or transport: no rest survives it.
    pub(super) fn clear(&mut self) {
        self.pending = None;
    }

    /// The rested target once its time has come, while the pointer is still on it. `at` is the
    /// target under the pointer in the current published frame, and `enabled` whether it acts. A row
    /// keeps its rest while disabled and through a fresh token, so a submenu row the pointer reached
    /// before Git discovery finished opens when discovery enables it.
    pub(super) fn due(
        &mut self,
        at: Option<&InteractionTarget>,
        enabled: bool,
        now: Instant,
    ) -> Option<InteractionTarget> {
        let pending = self.pending.as_mut()?;
        let Some(at) = at.filter(|at| same_place(at, &pending.target)) else {
            self.pending = None;
            return None;
        };
        pending.target = at.clone();
        (enabled && now >= pending.due)
            .then(|| self.pending.take())
            .flatten()
            .map(|pending| pending.target)
    }
}

/// The same menu row or control, whatever token its menu has since taken.
fn same_place(left: &InteractionTarget, right: &InteractionTarget) -> bool {
    match (left, right) {
        (
            InteractionTarget::ActionMenuRow { source, row, .. },
            InteractionTarget::ActionMenuRow {
                source: other,
                row: other_row,
                ..
            },
        ) => source == other && row == other_row,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests;
