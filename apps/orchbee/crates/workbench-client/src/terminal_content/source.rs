//! Painted terminal provenance and exact Workbench membership checks shared by read-only users.

use std::num::NonZeroU64;

use orchbee_client_server_protocol::HostTerminalId;
use orchbee_terminal_protocol::{AttachmentId, HostEpoch, TerminalTextFrame};
use orchbee_workbench_protocol::{
    PanelNode, Presentation, SessionId, Snapshot, TerminalId, TerminalPresentationId,
    WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId, WorkerId,
};

use crate::workbench_state::PaneRef;

/// The canonical member whose cells one successful native draw exposed.
/// Every axis is retained so a later snapshot can reject a moved, rebound,
/// or re-created pane instead of following its logical label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotMember {
    pub session: SessionId,
    pub context: WorkbenchTerminalContextId,
    pub tab: WorkbenchTabId,
    pub panel: WorkbenchPanelId,
    pub presentation: TerminalPresentationId,
    pub worker: WorkerId,
    pub terminal: TerminalId,
}

impl SnapshotMember {
    /// Re-resolve this exact member in a fresh canonical snapshot. No suffix,
    /// current-focus, or replacement-presentation lookup is permitted.
    pub(crate) fn snapshot_member<'a>(
        &self,
        snapshot: &'a Snapshot,
    ) -> Option<(&'a orchbee_workbench_protocol::Tab, &'a Presentation)> {
        if snapshot.session_id != self.session {
            return None;
        }
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == self.context)?;
        let tab = context.tabs.iter().find(|tab| tab.tab_id == self.tab)?;
        let presentation = presentation(&tab.root, &self.panel, &self.presentation)?;
        let binding = presentation.binding.as_ref()?;
        (binding.worker_id == self.worker && binding.terminal_id == self.terminal)
            .then_some((tab, presentation))
    }
}

/// Which Host screen supplied the admitted surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalScreen {
    Primary,
    Alternate,
}

impl TerminalScreen {
    /// Capture the explicit screen kind carried by one validated frame.
    pub(crate) const fn from_frame(frame: &TerminalTextFrame) -> Self {
        if frame.alternate_screen {
            Self::Alternate
        } else {
            Self::Primary
        }
    }
}

/// The exact terminal row surface behind the painted pane. A fresh viewer has
/// a different attachment, so the attachment and sequence are a local paint
/// witness while epoch/generation/revision/geometry are the cross-attach
/// content fence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaintedRows {
    pub(crate) attachment: AttachmentId,
    pub(crate) host_epoch: HostEpoch,
    pub(crate) sequence: NonZeroU64,
    pub(crate) generation: NonZeroU64,
    pub(crate) revision: u64,
    pub(crate) columns: u16,
    pub(crate) start_column: u16,
    pub(crate) total_rows: u64,
    pub(crate) start_row: u64,
    pub(crate) row_count: u16,
    pub(crate) history: bool,
}

impl PaintedRows {
    /// Capture the complete row fence for one live projection frame.
    pub(crate) fn live(frame: &TerminalTextFrame) -> Option<Self> {
        let total_rows = frame
            .scrollback_rows
            .checked_add(u64::from(frame.geometry.rows))?;
        Some(Self {
            attachment: frame.route.attachment_id,
            host_epoch: frame.route.host_epoch,
            sequence: frame.route.sequence,
            generation: frame.route.generation,
            revision: frame.route.revision,
            columns: frame.geometry.columns,
            start_column: 0,
            total_rows,
            start_row: frame.scrollback_rows,
            row_count: frame.geometry.rows,
            history: false,
        })
    }

    /// Map one painted local cell to its exact terminal column and absolute row.
    pub(crate) fn point(&self, column: u16, row: u16) -> Option<(u16, u64)> {
        let column = column.checked_add(self.start_column)?;
        if column >= self.columns || row >= self.row_count {
            return None;
        }
        // Retained-history chrome overwrites the first and last painted rows.
        // Those rows remain deliberately inert until the hit map carries
        // cell-level chrome holes.
        if self.history && (row == 0 || row.saturating_add(1) == self.row_count) {
            return None;
        }
        let absolute = self.start_row.checked_add(u64::from(row))?;
        (absolute < self.total_rows).then_some((column, absolute))
    }
}

/// One exact pane/content source captured from an admitted surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSource {
    pub(crate) pane: PaneRef,
    pub(crate) member: SnapshotMember,
    pub(crate) host_terminal: HostTerminalId,
    pub(crate) screen: TerminalScreen,
    pub(crate) rows: PaintedRows,
}

impl ContentSource {
    /// Re-resolve the exact member in a fresh canonical snapshot. No suffix,
    /// current-focus, or replacement-presentation lookup is permitted.
    pub(crate) fn snapshot_member<'a>(
        &self,
        snapshot: &'a Snapshot,
    ) -> Option<(&'a orchbee_workbench_protocol::Tab, &'a Presentation)> {
        if self.pane.anchor().tab_id() != &self.member.tab
            || self.pane.pane().panel() != &self.member.panel
        {
            return None;
        }
        self.member.snapshot_member(snapshot)
    }

    /// Require a fresh independent attachment to reproduce the complete cross-attach fence.
    pub(crate) fn matches_frame(
        &self,
        host_terminal: &HostTerminalId,
        frame: &TerminalTextFrame,
        total_rows: u64,
    ) -> bool {
        host_terminal == &self.host_terminal
            && TerminalScreen::from_frame(frame) == self.screen
            && frame.route.host_epoch == self.rows.host_epoch
            && frame.route.generation == self.rows.generation
            && frame.route.revision == self.rows.revision
            && frame.geometry.columns == self.rows.columns
            && total_rows == self.rows.total_rows
    }
}

fn presentation<'a>(
    node: &'a PanelNode,
    panel: &WorkbenchPanelId,
    presentation_id: &TerminalPresentationId,
) -> Option<&'a Presentation> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } if panel_id == panel => std::iter::once(primary_presentation)
            .chain(child_presentations)
            .find(|member| &member.presentation_id == presentation_id),
        PanelNode::Panel { .. } => None,
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| presentation(&slot.node, panel, presentation_id)),
    }
}

#[cfg(test)]
mod tests;
