//! Painted terminal provenance and exact Workbench membership checks for link activation; this
//! module maps cells to rows but neither reads terminal bytes nor performs external effects.

use std::num::NonZeroU64;

use orchbee_terminal_protocol::{AttachmentId, HostEpoch, TerminalTextFrame};
use orchbee_workbench_protocol::{
    PanelNode, Presentation, SiteId, Snapshot, TerminalId, TerminalPresentationId,
    WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId, WorkerId,
};

use crate::terminal_session::state::PaneRef;

/// The canonical member whose cells one successful native draw exposed.
/// Every axis is retained so a later snapshot can reject a moved, rebound,
/// or re-created pane instead of following its logical label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotMember {
    pub(crate) site: SiteId,
    pub(crate) context: WorkbenchTerminalContextId,
    pub(crate) tab: WorkbenchTabId,
    pub(crate) panel: WorkbenchPanelId,
    pub(crate) presentation: TerminalPresentationId,
    pub(crate) worker: WorkerId,
    pub(crate) terminal: TerminalId,
}

impl SnapshotMember {
    /// Re-resolve this exact member in a fresh canonical snapshot. No suffix,
    /// current-focus, or replacement-presentation lookup is permitted.
    pub(crate) fn snapshot_member<'a>(
        &self,
        snapshot: &'a Snapshot,
    ) -> Option<(&'a orchbee_workbench_protocol::Tab, &'a Presentation)> {
        if snapshot.site_id != self.site {
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
            total_rows,
            start_row: frame.scrollback_rows,
            row_count: frame.geometry.rows,
            history: false,
        })
    }

    /// Map one painted local cell to its exact terminal column and absolute row.
    pub(crate) fn point(&self, column: u16, row: u16) -> Option<(u16, u64)> {
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

/// One pane hit source from the same draw that produced the published map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaintedLinkSource {
    pub(crate) pane: PaneRef,
    pub(crate) member: SnapshotMember,
    pub(crate) rows: PaintedRows,
}

impl PaintedLinkSource {
    /// Re-resolve the exact member in a fresh canonical snapshot. No suffix,
    /// current-focus, or replacement-presentation lookup is permitted.
    pub(crate) fn snapshot_member<'a>(
        &self,
        snapshot: &'a Snapshot,
    ) -> Option<(&'a orchbee_workbench_protocol::Tab, &'a Presentation)> {
        self.member.snapshot_member(snapshot)
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
