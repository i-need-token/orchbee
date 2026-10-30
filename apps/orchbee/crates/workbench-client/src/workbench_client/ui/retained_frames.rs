//! Bound immutable live pixels by canonical presentation identity.
//! These images never participate in attachment, input, selection, or feedback.

use crate::localization::ClientProjectionStatus;
use crate::terminal_projection::TerminalProjection;
use crate::workbench_client::{
    state::{ClientPhaseKind, PaneRef, WorkbenchClientState},
    workbench_projection::ClientTerminalBinding,
};
use orchbee_terminal_protocol::{TerminalTextCell, TerminalTextFrame, TerminalTextRow};
use orchbee_workbench_protocol::{
    SessionId, TerminalPresentationId, WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::sync::Arc;

const MAXIMUM_PANES: usize = 8;
const MAXIMUM_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
struct Identity {
    session: SessionId,
    context: WorkbenchTerminalContextId,
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
    presentation: TerminalPresentationId,
    binding: ClientTerminalBinding,
}

impl Identity {
    fn selected(state: &WorkbenchClientState, panel: &WorkbenchPanelId) -> Option<Self> {
        let cache = state.cache();
        let tab = cache.tabs().get(state.active()?.index())?;
        let panel = tab.panel(tab.panel_index(panel)?)?;
        let presentation = state.selected_presentation(panel);
        Some(Self {
            session: cache.session_id().clone(),
            context: cache.terminal_context_id()?.clone(),
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: presentation.presentation_id().clone(),
            binding: presentation.binding()?.clone(),
        })
    }

    fn remains_member(&self, state: &WorkbenchClientState) -> bool {
        let cache = state.cache();
        cache.contains_terminal_presentation(
            &self.session,
            &self.context,
            &self.tab,
            &self.panel,
            &self.presentation,
            &self.binding,
        )
    }
}

#[derive(Clone)]
struct Pixels {
    identity: Identity,
    frame: Arc<TerminalTextFrame>,
    bytes: usize,
    host_terminal: Option<Box<str>>,
}

#[derive(Clone, Default)]
pub(crate) struct RetainedFrames {
    // Least recently used first; both bounds apply even when frames share text allocations.
    entries: Vec<Pixels>,
}

/// The draw facts of one projected pane that the bank may retain.
pub(super) struct DrawnPane<'a> {
    pub(super) key: &'a PaneRef,
    pub(super) status: ClientProjectionStatus,
    /// The pane shows retained history instead of its live frame.
    pub(super) history: bool,
    pub(super) projection: &'a TerminalProjection,
}

impl RetainedFrames {
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn prune(&mut self, state: &WorkbenchClientState) {
        self.entries
            .retain(|pixels| pixels.identity.remains_member(state));
    }

    /// Merge pixels captured by an older surface. Entries already captured by
    /// this surface are replayed last, so a fresh frame always wins an exact
    /// identity collision. All images remain subject to the shared LRU bounds.
    pub(super) fn inherit_older(&mut self, older: Self, state: &WorkbenchClientState) {
        self.prune(state);
        let newer = std::mem::take(&mut self.entries);
        for pixels in older
            .entries
            .into_iter()
            .filter(|pixels| pixels.identity.remains_member(state))
        {
            self.retain_host(pixels.identity, pixels.frame, pixels.host_terminal);
        }
        for pixels in newer {
            self.retain_host(pixels.identity, pixels.frame, pixels.host_terminal);
        }
    }

    pub(super) fn invalidate(&mut self, state: &WorkbenchClientState, panel: &WorkbenchPanelId) {
        if let Some(identity) = Identity::selected(state, panel) {
            self.entries.retain(|pixels| pixels.identity != identity);
        }
    }

    /// A verified physical replacement cannot reuse pixels from an earlier Host incarnation.
    pub(super) fn fence_selection(
        &mut self,
        context: &crate::workbench_client::ClientContextTarget,
        selection: &crate::workbench_client::workbench_projection::ClientMemberSelection,
    ) {
        let Some(expected) = &selection.expected_host else {
            return;
        };
        self.entries.retain(|pixels| {
            let id = &pixels.identity;
            id.session != context.session_id
                || Some(&id.context) != context.location.terminal_context_id()
                || id.tab != selection.member.tab
                || id.panel != selection.member.panel
                || id.presentation != selection.member.presentation
                || (pixels.frame.route.host_epoch.to_text() == expected.host_epoch.as_str()
                    && pixels.host_terminal.as_deref() == Some(expected.host_terminal_id.as_str()))
        });
    }

    pub(super) fn after_draw<'a>(
        &mut self,
        state: &WorkbenchClientState,
        panes: impl IntoIterator<Item = DrawnPane<'a>>,
    ) {
        self.prune(state);
        if state.ready().is_none() {
            return;
        }
        for pane in panes {
            if pane.status == ClientProjectionStatus::Blocked {
                self.invalidate(state, pane.key.pane().panel());
                continue;
            }
            if pane.status != ClientProjectionStatus::FrameAvailable
                || pane.history
                || !state.pane_projected(pane.key)
            {
                continue;
            }
            let Some(identity) = Identity::selected(state, pane.key.pane().panel()) else {
                continue;
            };
            if state.pane_by_panel(&identity.panel).is_none_or(|record| {
                record.pane_ref() != *pane.key || record.binding() != &identity.binding
            }) {
                continue;
            }
            let Some(frame) = pane.projection.share_frame() else {
                continue;
            };
            self.retain_host(
                identity,
                frame,
                pane.projection.host_terminal_id().map(Into::into),
            );
        }
    }

    #[cfg(test)]
    fn retain(&mut self, identity: Identity, frame: Arc<TerminalTextFrame>) {
        self.retain_host(identity, frame, None);
    }

    fn retain_host(
        &mut self,
        identity: Identity,
        frame: Arc<TerminalTextFrame>,
        host_terminal: Option<Box<str>>,
    ) {
        if let Some(index) = self.entries.iter().position(|old| {
            old.identity == identity
                && old.host_terminal == host_terminal
                && Arc::ptr_eq(&old.frame, &frame)
        }) {
            let pixels = self.entries.remove(index);
            self.entries.push(pixels);
            return;
        }
        self.entries.retain(|old| old.identity != identity);
        let bytes =
            retained_bytes(&frame).saturating_add(host_terminal.as_ref().map_or(0, |id| id.len()));
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
            host_terminal,
        });
    }

    pub(super) fn view<'a>(&'a self, state: &'a WorkbenchClientState) -> RetainedView<'a> {
        RetainedView {
            frames: self,
            state,
        }
    }
}

pub(super) struct RetainedView<'a> {
    frames: &'a RetainedFrames,
    state: &'a WorkbenchClientState,
}

impl<'a> RetainedView<'a> {
    /// A last observed title, fenced by its canonical binding and the attached Host epoch. A
    /// `held` normal exit keeps its title after its viewer settles or is lost.
    pub(super) fn title(
        &self,
        tab: &WorkbenchTabId,
        panel: &crate::workbench_client::ClientPanel,
        epoch: Option<&orchbee_terminal_protocol::HostEpoch>,
        held: bool,
    ) -> Option<&'a str> {
        if !held
            && self
                .state
                .pane_by_panel(panel.panel_id())
                .is_some_and(|record| record.fault().is_some())
        {
            return None;
        }
        let member = self.state.selected_presentation(panel);
        self.frames
            .entries
            .iter()
            .rev()
            .find(|pixels| {
                let id = &pixels.identity;
                &id.tab == tab
                    && &id.panel == panel.panel_id()
                    && &id.presentation == member.presentation_id()
                    && Some(&id.binding) == member.binding()
                    && id.remains_member(self.state)
                    && epoch.is_none_or(|epoch| *epoch == pixels.frame.route.host_epoch)
            })?
            .frame
            .title
            .as_deref()
    }

    /// Retained pixels for a connecting pane. A `held` normal exit keeps its final pixels in any
    /// phase until it leaves membership; they are never live content.
    pub(super) fn frame(
        &self,
        panel: &WorkbenchPanelId,
        columns: u16,
        rows: u16,
        held: bool,
    ) -> Option<&'a TerminalTextFrame> {
        match self.state.phase_kind() {
            _ if held => {}
            ClientPhaseKind::Starting | ClientPhaseKind::Switching | ClientPhaseKind::Ready => {
                if self
                    .state
                    .pane_by_panel(panel)
                    .is_some_and(|record| record.fault().is_some())
                {
                    return None;
                }
            }
            ClientPhaseKind::Empty
            | ClientPhaseKind::Creating
            | ClientPhaseKind::CreateCommittedNeedDetach
            | ClientPhaseKind::ProjectionUnavailable
            | ClientPhaseKind::MoveProjectionUnavailable
            | ClientPhaseKind::PanelProjectionUnavailable
            | ClientPhaseKind::RenameProjectionUnavailable
            | ClientPhaseKind::SelectedWithoutAttachment
            | ClientPhaseKind::RetryableAttachUnavailable
            | ClientPhaseKind::Retired => return None,
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
