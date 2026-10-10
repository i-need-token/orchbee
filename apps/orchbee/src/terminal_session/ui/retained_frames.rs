//! Bound immutable live pixels by canonical presentation identity.
//! These images never participate in attachment, input, selection, or feedback.

use super::{KeyedProjection, NativeProjectionStatus};
use crate::terminal_session::{
    state::{NativeSessionState, SessionPhaseKind},
    workbench_projection::NativeTerminalBinding,
};
use orchbee_terminal_protocol::{TerminalTextCell, TerminalTextFrame, TerminalTextRow};
use orchbee_workbench_protocol::{
    SiteId, TerminalPresentationId, WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::sync::Arc;

const MAXIMUM_PANES: usize = 8;
const MAXIMUM_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
struct Identity {
    site: SiteId,
    context: WorkbenchTerminalContextId,
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
    presentation: TerminalPresentationId,
    binding: NativeTerminalBinding,
}

impl Identity {
    fn selected(state: &NativeSessionState, panel: &WorkbenchPanelId) -> Option<Self> {
        let cache = state.cache();
        let tab = cache.tabs().get(state.active()?.index())?;
        let panel = tab.panel(tab.panel_index(panel)?)?;
        let presentation = state.selected_presentation(panel);
        Some(Self {
            site: cache.site_id().clone(),
            context: cache.terminal_context_id().clone(),
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: presentation.presentation_id().clone(),
            binding: presentation.binding()?.clone(),
        })
    }

    fn remains_member(&self, state: &NativeSessionState) -> bool {
        let Some(tabs) = state.retained_frame_tabs(&self.site, &self.context) else {
            return false;
        };
        tabs.iter().any(|tab| {
            tab.tab_id() == &self.tab
                && tab
                    .panel_index(&self.panel)
                    .and_then(|index| tab.panel(index))
                    .is_some_and(|panel| {
                        panel.presentations().any(|presentation| {
                            presentation.presentation_id() == &self.presentation
                                && presentation.binding() == Some(&self.binding)
                        })
                    })
        })
    }
}

struct Pixels {
    identity: Identity,
    frame: Arc<TerminalTextFrame>,
    bytes: usize,
}

#[derive(Default)]
pub(super) struct RetainedFrames {
    // Least recently used first; both bounds apply even when frames share text allocations.
    entries: Vec<Pixels>,
}

impl RetainedFrames {
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn prune(&mut self, state: &NativeSessionState) {
        self.entries
            .retain(|pixels| pixels.identity.remains_member(state));
    }

    pub(super) fn invalidate(&mut self, state: &NativeSessionState, panel: &WorkbenchPanelId) {
        if let Some(identity) = Identity::selected(state, panel) {
            self.entries.retain(|pixels| pixels.identity != identity);
        }
    }

    pub(super) fn after_draw(&mut self, state: &NativeSessionState, panes: &[KeyedProjection]) {
        self.prune(state);
        if state.ready().is_none() {
            return;
        }
        for pane in panes {
            if pane.status == NativeProjectionStatus::Blocked {
                self.invalidate(state, pane.key.pane().panel());
                continue;
            }
            if pane.status != NativeProjectionStatus::FrameAvailable
                || pane.viewport.is_some()
                || !state.pane_projected(&pane.key)
            {
                continue;
            }
            let Some(identity) = Identity::selected(state, pane.key.pane().panel()) else {
                continue;
            };
            if state.pane_by_panel(&identity.panel).is_none_or(|record| {
                record.pane_ref() != pane.key || record.binding() != &identity.binding
            }) {
                continue;
            }
            let Some(frame) = pane.value.share_frame() else {
                continue;
            };
            self.retain(identity, frame);
        }
    }

    fn retain(&mut self, identity: Identity, frame: Arc<TerminalTextFrame>) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|old| old.identity == identity && Arc::ptr_eq(&old.frame, &frame))
        {
            let pixels = self.entries.remove(index);
            self.entries.push(pixels);
            return;
        }
        self.entries.retain(|old| old.identity != identity);
        let bytes = retained_bytes(&frame);
        if bytes > MAXIMUM_BYTES {
            return;
        }
        while self.entries.len() >= MAXIMUM_PANES
            || self.entries.iter().map(|old| old.bytes).sum::<usize>() + bytes > MAXIMUM_BYTES
        {
            self.entries.remove(0);
        }
        self.entries.push(Pixels {
            identity,
            frame,
            bytes,
        });
    }

    pub(super) fn view<'a>(&'a self, state: &'a NativeSessionState) -> RetainedView<'a> {
        RetainedView {
            frames: self,
            state,
        }
    }
}

pub(super) struct RetainedView<'a> {
    frames: &'a RetainedFrames,
    state: &'a NativeSessionState,
}

impl<'a> RetainedView<'a> {
    pub(super) fn frame(
        &self,
        panel: &WorkbenchPanelId,
        columns: u16,
        rows: u16,
    ) -> Option<&'a TerminalTextFrame> {
        match self.state.phase_kind() {
            SessionPhaseKind::Starting | SessionPhaseKind::Switching | SessionPhaseKind::Ready => {
                if self
                    .state
                    .pane_by_panel(panel)
                    .is_some_and(|record| record.fault().is_some())
                {
                    return None;
                }
            }
            SessionPhaseKind::Empty
            | SessionPhaseKind::Creating
            | SessionPhaseKind::CreateCommittedNeedDetach
            | SessionPhaseKind::ProjectionUnavailable
            | SessionPhaseKind::MoveProjectionUnavailable
            | SessionPhaseKind::PanelProjectionUnavailable
            | SessionPhaseKind::RenameProjectionUnavailable
            | SessionPhaseKind::SelectedWithoutAttachment
            | SessionPhaseKind::RetryableAttachUnavailable
            | SessionPhaseKind::Retired => return None,
        }
        let identity = Identity::selected(self.state, panel)?;
        let pixels = self
            .frames
            .entries
            .iter()
            .find(|pixels| pixels.identity == identity)?;
        let frame = &pixels.frame;
        (frame.geometry.columns == columns && frame.geometry.rows == rows).then_some(frame)
    }
}

fn retained_bytes(frame: &TerminalTextFrame) -> usize {
    // Conservatively count repeated Arc text as distinct, including its counters.
    // Oversized images are evicted, never partially retained or truncated.
    let mut bytes = std::mem::size_of::<TerminalTextFrame>()
        + 2 * std::mem::size_of::<usize>()
        + frame.rows.capacity() * std::mem::size_of::<TerminalTextRow>();
    for text in [&frame.title, &frame.current_directory]
        .into_iter()
        .flatten()
    {
        bytes = bytes.saturating_add(text.len() + 2 * std::mem::size_of::<usize>());
    }
    for row in &frame.rows {
        bytes =
            bytes.saturating_add(row.cells.capacity() * std::mem::size_of::<TerminalTextCell>());
        for cell in &row.cells {
            bytes = bytes.saturating_add(cell.text.len() + 2 * std::mem::size_of::<usize>());
        }
    }
    bytes
}

#[cfg(test)]
mod tests;
