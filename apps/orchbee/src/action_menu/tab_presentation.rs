//! Read-only Tab menu facts. The captured action owner, never these labels, resolves targets.

use orchbee_workbench_protocol::TabGroupColor;
use std::num::NonZeroUsize;

/// The scope behind a displayed row, independent of the current terminal or menu candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TabMenuScope {
    Addressed,
    Selected(NonZeroUsize),
    Group(NonZeroUsize),
    Other(NonZeroUsize),
    Trailing(NonZeroUsize),
    Workspace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TabMenuPresentation {
    pub(crate) scope: TabMenuScope,
    /// One canonical value, not one mutable check flag per palette option.
    pub(crate) applied_color: Option<TabGroupColor>,
    pub(crate) rows: Vec<TabMenuScope>,
}

impl TabMenuPresentation {
    pub(crate) fn checked(&self, action: &super::MenuAction) -> bool {
        matches!(action, super::MenuAction::GroupColor(color) if self.applied_color == Some(*color))
    }
}
