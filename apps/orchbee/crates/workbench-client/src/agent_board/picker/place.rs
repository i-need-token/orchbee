//! Where a picked start opens: the places one picker offers and the `Tab` cycle through them.
//!
//! The offer is fixed when the picker opens, from the focused Panel at that moment. The
//! automatic place is the first one offered, so nothing is asked unless the person cycles.

use orchbee_workbench_protocol::WorkbenchPanelId;

use super::super::start::StartPlace;

/// The places a picker offers, in cycle order: this Panel, a new Tab, the two splits, then a new
/// worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Places {
    /// The focused Panel while it may be an idle shell at an empty prompt.
    idle: Option<WorkbenchPanelId>,
    /// Whether a Panel is focused, so a split has a Panel to split.
    focused: bool,
    /// Whether the current Project can create a worktree now, by the New Worktree form's own rule.
    worktree: bool,
}

impl Places {
    /// The Client cannot see the prompt itself, so the focused Panel counts as idle unless an
    /// Agent is known to hold it or one of this Client's starts still guards it; the Host's
    /// prompt gate decides when a typed command is sent.
    pub(crate) fn new(
        focus: Option<&WorkbenchPanelId>,
        busy: impl Fn(&WorkbenchPanelId) -> bool,
    ) -> Self {
        Self {
            idle: focus.filter(|panel| !busy(panel)).cloned(),
            focused: focus.is_some(),
            worktree: false,
        }
    }

    /// Offers New Worktree too, last, when `available`.
    #[must_use]
    pub(crate) const fn with_worktree(mut self, available: bool) -> Self {
        self.worktree = available;
        self
    }

    pub(crate) fn offered(&self) -> Vec<StartPlace> {
        let mut places = Vec::with_capacity(5);
        places.extend(self.idle.clone().map(StartPlace::ThisPanel));
        places.push(StartPlace::NewTab);
        if self.focused {
            places.extend([StartPlace::SplitRight, StartPlace::SplitDown]);
        }
        if self.worktree {
            places.push(StartPlace::NewWorktree);
        }
        places
    }

    /// This Panel when it is offered, otherwise a new Tab.
    pub(crate) fn automatic(&self) -> StartPlace {
        self.idle
            .clone()
            .map_or(StartPlace::NewTab, StartPlace::ThisPanel)
    }

    /// The place after `current`, wrapping; `forward` is `Tab`, otherwise `Shift+Tab`.
    pub(crate) fn next(&self, current: &StartPlace, forward: bool) -> StartPlace {
        let offered = self.offered();
        let len = offered.len();
        let index = offered
            .iter()
            .position(|place| place == current)
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % len
        } else {
            (index + len - 1) % len
        };
        offered[next].clone()
    }
}
