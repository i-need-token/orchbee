//! The Agent board's session-side presentation state: the rows the last
//! completed OBAP list read returned, the board's own interaction state
//! (selection, scroll, notice), the queued read/jump intents, and the
//! in-flight jump's stage bookkeeping. Canonical Agent state stays
//! server-owned — this module keeps only what the last completed read
//! delivered and never synthesizes status of its own.
//!
//! The lane (Linux) runs the OBAP reads on a dedicated Client Session
//! spawned from `LocalClientOrigin` — never the native worker's
//! connection, so a post-submission ambiguity there cannot retire the
//! primary session.

use std::time::{Duration, Instant};

use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentLifecycle, AgentObservationFreshness, AgentPresentationAttachment,
    AgentPresentationVisibility, AgentProviderId, AgentSemanticStatus, AgentSessionId,
    AgentSessionRejectionReason, AgentSessionSummary,
};

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use super::agent_read_lane as lane;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use lane::{AgentReadLane, LaneCommand, LaneOutcome};

#[cfg(test)]
mod tests;

/// One bounded read round's absolute deadline — connect admission, the
/// whole page walk, and the single allowed restart all live inside it.
pub(super) const AGENT_READ_DEADLINE: Duration = Duration::from_secs(3);
/// One jump Get's own deadline inside the jump's total budget.
pub(super) const AGENT_GET_DEADLINE: Duration = Duration::from_secs(3);
/// One jump's total budget across the Get plus every navigation stage.
pub(super) const AGENT_JUMP_DEADLINE: Duration = Duration::from_secs(8);
/// The read cooldown: a completed list round — success or failure —
/// refuses further reads until this much time has passed; `r` does not
/// bypass it.
pub(super) const AGENT_READ_COOLDOWN: Duration = Duration::from_secs(2);
/// The 512-session runtime bound at 100 items per page, rounded up.
const MAXIMUM_ROWS: usize = 600;

/// A canonical Agent row: the summary the last completed list read
/// returned, kept verbatim — the board renders it, never reinterprets it.
#[derive(Clone, Debug)]
pub(crate) struct AgentBoardRow {
    session_id: AgentSessionId,
    provider_id: AgentProviderId,
    status: AgentSemanticStatus,
    lifecycle: AgentLifecycle,
    freshness: AgentObservationFreshness,
    attachments: Box<[AgentPresentationAttachment]>,
}

impl AgentBoardRow {
    fn from_summary(summary: AgentSessionSummary) -> Self {
        Self {
            session_id: summary.session_id,
            provider_id: summary.provider_id,
            status: summary.status,
            lifecycle: summary.lifecycle,
            freshness: summary.observation.freshness,
            attachments: summary.presentation_attachments.into_boxed_slice(),
        }
    }

    pub(crate) const fn session_id(&self) -> &AgentSessionId {
        &self.session_id
    }
    pub(crate) const fn provider_id(&self) -> &AgentProviderId {
        &self.provider_id
    }
    pub(crate) const fn status(&self) -> &AgentSemanticStatus {
        &self.status
    }
    pub(crate) const fn lifecycle(&self) -> AgentLifecycle {
        self.lifecycle
    }
    pub(crate) const fn freshness(&self) -> AgentObservationFreshness {
        self.freshness
    }
    pub(crate) fn attachments(&self) -> &[AgentPresentationAttachment] {
        &self.attachments
    }
    /// Whether the canonical observation advertises at least one navigable
    /// attachment — a display hint only; the jump itself re-verifies.
    pub(crate) fn navigable(&self) -> bool {
        self.attachments
            .iter()
            .any(|attachment| attachment.navigable)
    }
}

/// The renderer-facing read state of the board's last list attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardRead {
    /// No completed read yet — never rendered as "no Agents".
    Unread,
    InFlight,
    /// `complete` marks the page walk reached the canonical end marker;
    /// `false` means the walk exhausted its budget — shown, never hidden.
    Ready {
        complete: bool,
        at: Instant,
    },
    Failed(AgentBoardReadFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardReadFailure {
    /// The dedicated read lane could not be established at all.
    LaneUnavailable,
    Denied,
    /// The Agent runtime answered `unavailable`.
    Unavailable,
    /// Transport/connection failure, including post-submission ambiguity.
    Transport,
}

/// Why a jump refused; every variant maps to a localized board notice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JumpFailure {
    /// Get found the session but no resolvable navigable attachment, or the
    /// session disappeared / its presentation identity vanished.
    Gone,
    /// Get could not produce an observation (transport or runtime
    /// unavailability) — the jump never proceeds from unverified state.
    Unavailable,
    Denied,
    /// The canonical lifecycle says the session already ended.
    Ended,
    /// The session has no navigable presentation attachment at all.
    NoPlacement,
    /// The jump's total budget elapsed before landing.
    Expired,
}

/// What the last draw actually did with the board, reported back through
/// `NativeDrawReceipt` so input capture settles on the painted truth.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AgentBoardDraw {
    pub(crate) presented: bool,
    pub(crate) viewport_rows: usize,
}

/// The sidebar's honest summary of the last completed read — `None` means
/// no completed read ever ran and the sidebar shows no count (never zero).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentBoardSummary {
    pub(crate) sessions: usize,
    pub(crate) complete: bool,
    /// When that read completed — the sidebar renders its age.
    pub(crate) at: Instant,
}

/// A workbench placement resolved from a `TerminalPresentationId`: which
/// context owns it, which tab, which panel — plus the display labels the
/// snapshot carried at index-build time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentationPlacement {
    pub(crate) location: super::workbench_projection::NativeContextLocation,
    pub(crate) tab_id: orchbee_workbench_protocol::WorkbenchTabId,
    pub(crate) panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
    pub(crate) tab_position: usize,
    pub(crate) tab_label: String,
    pub(crate) panel_label: String,
    pub(crate) project_label: Option<String>,
    pub(crate) workspace_label: Option<String>,
}

/// `presentation_id → placement` over the latest observed Workbench
/// snapshot. Rebuilt wholesale on every snapshot-bearing worker event —
/// the index is a function of canonical state, never a second authority.
#[derive(Default)]
pub(super) struct PresentationIndex {
    /// `false` until a canonical snapshot (or the bootstrap cache) has
    /// actually fed the index — a jump resolving against a never-fed
    /// index must wait for one, never conclude `Gone` from an empty map.
    ready: bool,
    placements: std::collections::HashMap<String, PresentationPlacement>,
}

impl PresentationIndex {
    /// Rebuild the whole index from a freshly arrived canonical snapshot:
    /// every navigation location's tabs, every panel's presentation id.
    /// Called only after the snapshot passed the session's acceptance
    /// fences — the index is a function of accepted canonical state.
    pub(super) fn rebuild(snapshot: &orchbee_workbench_protocol::Snapshot) -> Self {
        let mut index = Self {
            ready: true,
            ..Self::default()
        };
        for location in navigation_locations(snapshot) {
            if let Ok(tabs) =
                super::workbench_projection::project_native_context_tabs(snapshot, &location)
            {
                index.install_context(&tabs);
            }
        }
        index
    }

    /// Whether any canonical feed has ever built this index — the
    /// jump's honesty gate: `false` means "no snapshot yet", never
    /// "no placements exist".
    pub(super) const fn ready(&self) -> bool {
        self.ready
    }

    /// Fold one context's projected tabs into the index (used for the
    /// bootstrap cache, which arrives already projected rather than as a
    /// raw snapshot).
    pub(super) fn install_context(
        &mut self,
        tabs: &super::workbench_projection::NativeContextTabs,
    ) {
        self.ready = true;
        let (project_label, workspace_label) = tabs
            .navigation()
            .iter()
            .find(|context| context.location == *tabs.location())
            .map(|context| {
                (
                    context
                        .project_label
                        .as_ref()
                        .map(|label| label.as_str().to_owned()),
                    context
                        .workspace_label
                        .as_ref()
                        .map(|label| label.as_str().to_owned()),
                )
            })
            .unwrap_or((None, None));
        for (tab_index, tab) in tabs.tabs().iter().enumerate() {
            for panel_index in 0..tab.panel_count() {
                let Some(panel) = tab.panel(panel_index) else {
                    continue;
                };
                for member in panel.presentations() {
                    self.placements.insert(
                        member.presentation_id().as_str().to_owned(),
                        PresentationPlacement {
                            location: tabs.location().clone(),
                            tab_id: tab.tab_id().clone(),
                            panel_id: panel.panel_id().clone(),
                            tab_position: tab_index + 1,
                            tab_label: tab.display_label(tab_index).into_owned(),
                            panel_label: member.display_label().to_owned(),
                            project_label: project_label.clone(),
                            workspace_label: workspace_label.clone(),
                        },
                    );
                }
            }
        }
    }

    /// Resolve one attachment's stable presentation identity to a live
    /// placement — identity only; labels are never used to locate.
    pub(super) fn placement(
        &self,
        attachment: &AgentPresentationAttachment,
    ) -> Option<&PresentationPlacement> {
        self.placements.get(attachment.presentation_id.as_str())
    }

    pub(super) fn placement_by_presentation(
        &self,
        presentation_id: &str,
    ) -> Option<&PresentationPlacement> {
        self.placements.get(presentation_id)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.placements.len()
    }
}

/// Every `NativeContextLocation` a snapshot's navigation declares, in
/// canonical order (standalone first, then all Workspaces).
fn navigation_locations(
    snapshot: &orchbee_workbench_protocol::Snapshot,
) -> Vec<super::workbench_projection::NativeContextLocation> {
    use super::workbench_projection::NativeContextLocation;
    let mut locations = Vec::with_capacity(snapshot.navigation.workspaces.len() + 1);
    locations.push(NativeContextLocation::Standalone(
        snapshot.navigation.standalone_terminal_context_id.clone(),
    ));
    if let Ok(ordered) = snapshot.navigation.ordered_workspaces() {
        for (_, workspace) in ordered {
            locations.push(NativeContextLocation::Workspace {
                workspace_id: workspace.workspace_id.clone(),
                terminal_context_id: workspace.terminal_context_id.clone(),
            });
        }
    }
    locations
}

/// The board's key map, resolved in one place so the capture handler and
/// its tests share the exact binding set. All bindings are plain (no
/// modifiers); `Ctrl+B g` reaches the dispatcher before the board layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AgentBoardKeyAction {
    SelectUp,
    SelectDown,
    Refresh,
    Jump,
    Close,
}

pub(super) fn board_key_action(event: crate::client_view::KeyEvent) -> Option<AgentBoardKeyAction> {
    use crate::client_view::{Binding, KeyCode, Modifiers};
    const UP: Binding = Binding::new(KeyCode::Up, Modifiers::NONE);
    const UP_VIM: Binding = Binding::new(KeyCode::Char('k'), Modifiers::NONE);
    const DOWN: Binding = Binding::new(KeyCode::Down, Modifiers::NONE);
    const DOWN_VIM: Binding = Binding::new(KeyCode::Char('j'), Modifiers::NONE);
    const REFRESH: Binding = Binding::new(KeyCode::Char('r'), Modifiers::NONE);
    const JUMP: Binding = Binding::new(KeyCode::Enter, Modifiers::NONE);
    const CLOSE: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);
    if UP.matches(event) || UP_VIM.matches(event) {
        Some(AgentBoardKeyAction::SelectUp)
    } else if DOWN.matches(event) || DOWN_VIM.matches(event) {
        Some(AgentBoardKeyAction::SelectDown)
    } else if REFRESH.matches(event) {
        Some(AgentBoardKeyAction::Refresh)
    } else if JUMP.matches(event) {
        Some(AgentBoardKeyAction::Jump)
    } else if CLOSE.matches(event) {
        Some(AgentBoardKeyAction::Close)
    } else {
        None
    }
}

/// One step of the staged jump, decided purely from observed state — the
/// driver re-plans against live facts every tick and never caches indices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum JumpStep {
    /// The owning context is not active; select it (0-based position).
    SelectContext(usize),
    /// In the right context; select the tab (1-based `SelectTab` position).
    SelectTab(u8),
    /// Right context and tab; focus the panel if the record is ready.
    FocusPanel(orchbee_workbench_protocol::WorkbenchPanelId),
    /// Right context and tab, but the target panel's viewer record is
    /// dead — reattach through the same switch admission, still naming
    /// the stable panel identity (1-based `SelectTab` position + panel).
    Reattach {
        position: u8,
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
    },
    /// Already focused on the exact target *and* its viewer is live —
    /// the jump landed.
    Landed,
    /// This candidate's identity no longer resolves (or its owning context
    /// or tab vanished); try the next attachment.
    Advance,
    /// An admission already owns the mailbox (context selection or a tab
    /// switch is in flight) — observe again next tick.
    Wait,
}

#[expect(clippy::too_many_arguments)]
pub(super) fn plan_jump_step(
    placement: &PresentationPlacement,
    active_location: Option<&super::workbench_projection::NativeContextLocation>,
    context_locations: &[super::workbench_projection::NativeContextLocation],
    target_tab_index: Option<usize>,
    active_tab: Option<&orchbee_workbench_protocol::WorkbenchTabId>,
    focus_panel: Option<&orchbee_workbench_protocol::WorkbenchPanelId>,
    target_live: bool,
    context_selecting: bool,
    switching: bool,
) -> JumpStep {
    if active_location != Some(&placement.location) {
        if context_selecting {
            return JumpStep::Wait;
        }
        return match context_locations
            .iter()
            .position(|location| location == &placement.location)
        {
            Some(index) => JumpStep::SelectContext(index),
            None => JumpStep::Advance,
        };
    }
    let Some(tab_index) = target_tab_index else {
        return JumpStep::Advance;
    };
    if active_tab != Some(&placement.tab_id) {
        return if switching {
            JumpStep::Wait
        } else {
            JumpStep::SelectTab((tab_index + 1) as u8)
        };
    }
    if switching {
        return JumpStep::Wait;
    }
    if !target_live {
        // The resolved panel has no live viewer under the current
        // anchor — landing here would strand the user on a dead pane.
        // Reattach on the same tab instead of ever reporting `Landed`.
        return JumpStep::Reattach {
            position: (tab_index + 1) as u8,
            panel_id: placement.panel_id.clone(),
        };
    }
    if focus_panel == Some(&placement.panel_id) {
        JumpStep::Landed
    } else {
        JumpStep::FocusPanel(placement.panel_id.clone())
    }
}

/// A queued board intent — the arbitration model's single intent slot:
/// Enter replaces an unissued refresh, while a refresh request is
/// consumed rather than displacing a queued jump.
#[derive(Clone, Debug)]
enum BoardIntent {
    Refresh,
    Jump { target: AgentSessionId },
}

/// The one outstanding lane request, correlated by kind (and jump serial).
#[derive(Clone, Debug)]
enum BoardInflight {
    ListRound,
    JumpGet { serial: u64 },
}

/// An accepted jump in progress. `serial` is the generation the lane's
/// `Get` outcome must match; a new jump replaces the serial so a stale
/// in-flight stage is dropped at its next boundary check.
#[derive(Clone, Debug)]
pub(super) struct BoardJump {
    pub(super) serial: u64,
    pub(super) stage: JumpStage,
    pub(super) deadline: Instant,
}

#[derive(Clone, Debug)]
pub(super) enum JumpStage {
    /// The lane Get is in flight — the canonical attachment set is not yet
    /// verified, so no navigation may begin.
    Checking,
    /// Ordered navigable candidates, resolved fresh against the live
    /// presentation index on every attempt.
    Navigating {
        candidates: Box<[AgentPresentationAttachment]>,
        index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentBoardToken {
    epoch: u64,
    selected: Option<AgentSessionId>,
    scroll: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardControl {
    Open,
    Refresh,
    Close,
}

pub(super) struct AgentBoard {
    open: bool,
    /// Bump on every open/close — lane outcomes carrying an older epoch
    /// are stale by construction and dropped.
    epoch: u64,
    rows: Vec<AgentBoardRow>,
    selection: usize,
    scroll: usize,
    /// Rows the last paint could show at once — the selection-visibility
    /// window, settled from the draw receipt rather than guessed.
    viewport_rows: usize,
    read: AgentBoardRead,
    /// Whether the rows on screen came from a complete read — the
    /// last_good marker F6's incomplete path preserves.
    rows_from_complete: bool,
    /// When the rows on screen were installed — held independently of
    /// the read state so a partial round landing on last_good never
    /// rewrites the kept rows' age.
    rows_at: Option<Instant>,
    /// The sidebar's summary of the last completed read — survives
    /// failed attempts, absent until the first completed read ever.
    last_summary: Option<AgentBoardSummary>,
    intent: Option<BoardIntent>,
    inflight: Option<BoardInflight>,
    jump: Option<BoardJump>,
    jump_serial: u64,
    notice: Option<JumpFailure>,
    /// When the last list round finished — success or failure — the
    /// source of the read cooldown `r` never bypasses.
    last_read_end: Option<Instant>,
    clock: crate::local_client::LocalOperationClock,
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    lane_origin: Option<crate::local_client::LocalClientOrigin>,
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    lane: Option<AgentReadLane>,
    /// Lane handles whose threads are still exiting after a close or a
    /// failed send — kept until `finished` so a spawn never overlaps a
    /// live predecessor and teardown joins whatever remains instead of
    /// leaving a detached thread.
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    dying_lanes: Vec<AgentReadLane>,
}

impl AgentBoard {
    fn token(&self) -> AgentBoardToken {
        AgentBoardToken {
            epoch: self.epoch,
            selected: self.selected_row().map(|row| row.session_id.clone()),
            scroll: self.scroll,
        }
    }

    pub(super) fn control(&mut self, token: &AgentBoardToken, control: AgentBoardControl) {
        if !self.open || *token != self.token() {
            return;
        }
        match control {
            AgentBoardControl::Open => self.request_jump(),
            AgentBoardControl::Refresh => self.request_refresh(),
            AgentBoardControl::Close => self.close(),
        }
    }

    pub(super) fn scroll_painted(
        &mut self,
        token: &AgentBoardToken,
        up: bool,
        lines: usize,
    ) -> Option<AgentBoardToken> {
        if !self.open || *token != self.token() {
            return None;
        }
        let lines = isize::try_from(lines).ok()?;
        self.scroll_by(if up { -lines } else { lines });
        Some(self.token())
    }

    pub(super) fn new(clock: crate::local_client::LocalOperationClock) -> Self {
        Self {
            open: false,
            epoch: 0,
            rows: Vec::new(),
            selection: 0,
            scroll: 0,
            viewport_rows: 0,
            read: AgentBoardRead::Unread,
            rows_from_complete: false,
            rows_at: None,
            last_summary: None,
            intent: None,
            inflight: None,
            jump: None,
            jump_serial: 0,
            notice: None,
            last_read_end: None,
            clock,
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            lane_origin: None,
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            lane: None,
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            dying_lanes: Vec::new(),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(super) fn set_origin(&mut self, origin: crate::local_client::LocalClientOrigin) {
        self.lane_origin = Some(origin);
    }

    pub(super) const fn is_open(&self) -> bool {
        self.open
    }
    /// Input ownership is synchronous from `open`: a key arriving before
    /// the first paint still belongs to the board — it can never fall
    /// through to a pane or an overlay underneath. A paint that cannot
    /// present the board at all closes it (see `settle_draw`), so the
    /// only invisible capture window is the one frame that carries that
    /// verdict — and that same frame already shows the refusal notice.
    pub(super) const fn owns_input(&self) -> bool {
        self.open
    }

    pub(super) fn open(&mut self) {
        self.open = true;
        self.epoch += 1;
        self.intent = Some(BoardIntent::Refresh);
        self.inflight = None;
        self.jump = None;
        self.notice = None;
        self.clamp_selection();
    }

    pub(super) fn close(&mut self) {
        self.open = false;
        self.epoch += 1;
        self.intent = None;
        self.inflight = None;
        self.jump = None;
        self.notice = None;
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        if let Some(mut lane) = self.lane.take() {
            // Cancel order: the epoch bump above already invalidates
            // outcomes and stops scheduling; now the in-flight exchange
            // is interrupted and the thread asked to exit. The handle
            // moves to `dying_lanes` and is released only after the
            // thread actually exits — never a UI join, never detached.
            lane.interrupt();
            self.dying_lanes.push(lane);
        }
    }

    /// Settle the renderer's verdict: a board the draw could not present
    /// at all closes itself instead of surviving as an invisible input
    /// owner — the same frame already carries the `overlay_refused`
    /// status line as the honest notice. `None` (the draw ran before
    /// the board's paint site, or the board is closed) changes nothing.
    pub(super) fn settle_draw(&mut self, draw: Option<AgentBoardDraw>) {
        let Some(draw) = draw else {
            return;
        };
        if !draw.presented {
            if self.open {
                self.close();
            }
            return;
        }
        if draw.viewport_rows > 0 {
            self.viewport_rows = draw.viewport_rows;
        }
        self.clamp_selection();
    }

    pub(super) fn rows(&self) -> &[AgentBoardRow] {
        &self.rows
    }
    pub(super) const fn selection(&self) -> usize {
        self.selection
    }
    pub(super) const fn scroll(&self) -> usize {
        self.scroll
    }
    pub(super) const fn read(&self) -> AgentBoardRead {
        self.read
    }
    pub(super) const fn notice(&self) -> Option<JumpFailure> {
        self.notice
    }
    pub(super) fn jump(&self) -> Option<&BoardJump> {
        self.jump.as_ref()
    }
    pub(super) const fn jumping(&self) -> bool {
        self.jump.is_some()
    }

    /// The sidebar's summary of the last completed read — `None` until
    /// the first completed read ever, so a failed or never-run read
    /// shows no count at all rather than implying zero Agents.
    pub(super) fn summary(&self) -> Option<&AgentBoardSummary> {
        self.last_summary.as_ref()
    }

    fn selected_row(&self) -> Option<&AgentBoardRow> {
        self.rows.get(self.selection)
    }

    fn clamp_selection(&mut self) {
        if self.rows.is_empty() {
            self.selection = 0;
            self.scroll = 0;
            return;
        }
        self.selection = self.selection.min(self.rows.len() - 1);
        let visible = self.viewport_rows.max(1);
        if self.selection < self.scroll {
            self.scroll = self.selection;
        } else if self.selection >= self.scroll + visible {
            self.scroll = self.selection + 1 - visible;
        }
        let maximum = self.rows.len().saturating_sub(visible);
        self.scroll = self.scroll.min(maximum);
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        self.selection = self
            .selection
            .saturating_add_signed(delta)
            .min(self.rows.len() - 1);
        self.clamp_selection();
    }

    /// Pointer row select — resolves by stable session identity, so a
    /// stale click can only ever pick a row that still exists.
    pub(super) fn select_session(&mut self, session: &AgentSessionId) -> bool {
        let Some(index) = self.rows.iter().position(|row| row.session_id() == session) else {
            return false;
        };
        self.selection = index;
        self.clamp_selection();
        true
    }

    pub(super) fn scroll_by(&mut self, delta: isize) {
        let visible = self.viewport_rows.max(1);
        let maximum = self.rows.len().saturating_sub(visible);
        self.scroll = self.scroll.saturating_add_signed(delta).min(maximum);
        // Keep the focus inside the scrolled window so the next draw does not
        // pull the viewport back to its previous selection.
        self.selection = self
            .selection
            .max(self.scroll)
            .min((self.scroll + visible - 1).min(self.rows.len().saturating_sub(1)));
    }

    pub(super) fn request_refresh(&mut self) {
        // A completed round — success or failure — imposes the cooldown;
        // `r` inside the window is a no-op, not a queued refresh.
        if self
            .last_read_end
            .is_some_and(|at| self.clock.now() < at + AGENT_READ_COOLDOWN)
        {
            return;
        }
        // A queued jump holds the single intent slot — a refresh
        // request is consumed without displacing it.
        if matches!(self.intent, Some(BoardIntent::Jump { .. })) {
            return;
        }
        self.notice = None;
        self.intent = Some(BoardIntent::Refresh);
    }

    /// Enter on the selected row. A row without a navigable attachment
    /// refuses here — before any lane request — with the honest notice,
    /// and never disturbs a jump already in progress. An accepted jump
    /// supersedes the previous one *at request time*: the superseded
    /// jump dies now, while its in-flight Get keeps the physical lane
    /// credit until the matching outcome drains or the lane thread's
    /// exit releases it — a stale outcome can never displace the newer
    /// jump (the serial check in `apply_get_outcome`).
    pub(super) fn request_jump(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if !row.navigable() {
            self.notice = Some(JumpFailure::NoPlacement);
            return;
        }
        let target = row.session_id.clone();
        self.notice = None;
        self.jump = None;
        self.intent = Some(BoardIntent::Jump { target });
    }

    /// Issue the queued intent through the lane — one outstanding lane
    /// request at a time (the model's read serialization).
    pub(super) fn drive_intent(&mut self, now: Instant) -> bool {
        if !self.open || self.inflight.is_some() {
            return false;
        }
        match self.intent.take() {
            Some(BoardIntent::Refresh) => {
                // A queued refresh still honors the read cooldown — the
                // intent waits out the remainder instead of issuing early
                // or being dropped. `open()` queues its first refresh even
                // while a previous round's cooldown still runs; rearming
                // each tick is what makes the residual honest.
                if self
                    .last_read_end
                    .is_some_and(|at| now < at + AGENT_READ_COOLDOWN)
                {
                    self.intent = Some(BoardIntent::Refresh);
                    return false;
                }
                self.issue_list_round(now);
                true
            }
            Some(BoardIntent::Jump { target }) => {
                self.issue_jump_get(target, now);
                true
            }
            None => false,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    fn lane_or_spawn(&mut self) -> Option<&mut AgentReadLane> {
        if self.lane.is_none() {
            // Release finished predecessors first, then hold the spawn
            // while any predecessor thread is still exiting — at most
            // one lane thread is alive at a time, and the UI never
            // blocks on a join here.
            self.dying_lanes.retain(|lane| !lane.finished());
            if !self.dying_lanes.is_empty() {
                return None;
            }
            let origin = self.lane_origin.clone()?;
            match AgentReadLane::spawn(origin, self.clock.clone()) {
                Ok(lane) => self.lane = Some(lane),
                Err(_) => return None,
            }
        }
        self.lane.as_mut()
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    fn issue_list_round(&mut self, now: Instant) {
        let Some(deadline) = now.checked_add(AGENT_READ_DEADLINE) else {
            self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
            return;
        };
        let epoch = self.epoch;
        let Some(lane) = self.lane_or_spawn() else {
            self.read = AgentBoardRead::Failed(AgentBoardReadFailure::LaneUnavailable);
            return;
        };
        if lane.send(LaneCommand::ListRound { epoch, deadline }) {
            self.inflight = Some(BoardInflight::ListRound);
            self.read = AgentBoardRead::InFlight;
        } else {
            self.dying_lanes.extend(self.lane.take());
            self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    fn issue_list_round(&mut self, _now: Instant) {
        self.read = AgentBoardRead::Failed(AgentBoardReadFailure::LaneUnavailable);
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    fn issue_jump_get(&mut self, target: AgentSessionId, now: Instant) {
        self.jump_serial += 1;
        let serial = self.jump_serial;
        let Some(deadline) = now.checked_add(AGENT_JUMP_DEADLINE) else {
            self.fail_jump(JumpFailure::Expired);
            return;
        };
        self.jump = Some(BoardJump {
            serial,
            stage: JumpStage::Checking,
            deadline,
        });
        // The Get's own deadline sits inside the jump's total budget —
        // it cannot outlive either.
        let get_deadline = now
            .checked_add(AGENT_GET_DEADLINE)
            .map_or(deadline, |get| get.min(deadline));
        let epoch = self.epoch;
        let Some(lane) = self.lane_or_spawn() else {
            self.fail_jump(JumpFailure::Unavailable);
            return;
        };
        if lane.send(LaneCommand::GetSession {
            epoch,
            serial,
            session: target,
            deadline: get_deadline,
        }) {
            self.inflight = Some(BoardInflight::JumpGet { serial });
        } else {
            self.dying_lanes.extend(self.lane.take());
            self.fail_jump(JumpFailure::Unavailable);
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    fn issue_jump_get(&mut self, _target: AgentSessionId, _now: Instant) {
        self.fail_jump(JumpFailure::Unavailable);
    }

    /// Drain every arrived lane outcome. The list path is board-local;
    /// the Get path needs the caller's live-context preference to order
    /// candidates, so it takes that as a closure.
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(super) fn drain_lane(
        &mut self,
        now: Instant,
        mut prefer: impl FnMut(&AgentPresentationAttachment) -> bool,
    ) -> bool {
        let mut changed = false;
        while let Some(outcome) = self.lane.as_mut().and_then(AgentReadLane::take_outcome) {
            changed = true;
            match outcome {
                #[cfg(windows)]
                LaneOutcome::PanelReports { .. } | LaneOutcome::Graphics { .. } => {
                    self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
                }
                LaneOutcome::Attention { .. } | LaneOutcome::PhysicalAttention { .. } => {
                    self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
                }
                LaneOutcome::List { epoch, result } => {
                    self.apply_list_outcome(epoch, result, now);
                }
                LaneOutcome::Get {
                    epoch,
                    serial,
                    result,
                } => {
                    self.apply_get_outcome(epoch, serial, result, &mut prefer);
                }
            }
        }
        // Dead-lane honesty: every queued outcome drained above; the
        // thread's exit is the only remaining witness, and it proves no
        // outcome will ever arrive for a residual credit. Release that
        // credit into the honest failure now — never leave the board
        // waiting on a lane that no longer exists.
        if self.lane.as_ref().is_some_and(AgentReadLane::finished)
            && let Some(lane) = self.lane.take()
        {
            lane.close();
            match self.inflight.take() {
                Some(BoardInflight::ListRound) => {
                    self.last_read_end = Some(now);
                    self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Unavailable);
                }
                Some(BoardInflight::JumpGet { serial })
                    if self.jump.as_ref().is_some_and(|jump| jump.serial == serial) =>
                {
                    self.fail_jump(JumpFailure::Unavailable);
                }
                Some(BoardInflight::JumpGet { .. }) | None => {}
            }
            changed = true;
        }
        changed
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    pub(super) fn drain_lane(
        &mut self,
        _now: Instant,
        _prefer: impl FnMut(&AgentPresentationAttachment) -> bool,
    ) -> bool {
        false
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    fn apply_list_outcome(
        &mut self,
        epoch: u64,
        result: Result<lane::ListRound, lane::LaneFailure>,
        now: Instant,
    ) {
        if epoch != self.epoch || !matches!(self.inflight, Some(BoardInflight::ListRound)) {
            return;
        }
        self.inflight = None;
        // Every completed attempt starts the cooldown — failure counts.
        self.last_read_end = Some(now);
        match result {
            Ok(round) => {
                // An incomplete walk never overwrites a complete read's
                // rows: last_good stays on screen with the incomplete
                // mark and its original age. Without any last_good the
                // single-revision partial rows may show — marked, never
                // reported as a full count.
                let replace = round.complete || !self.rows_from_complete;
                if replace {
                    let held = self.selected_row().map(|row| row.session_id.clone());
                    let mut rows: Vec<AgentBoardRow> = round
                        .items
                        .into_iter()
                        .map(AgentBoardRow::from_summary)
                        .collect();
                    // Needs-attention rows surface first — a stable lift,
                    // never a reorder: the canonical order inside each
                    // group stays exactly what the Server returned.
                    rows.sort_by_key(|row| {
                        usize::from(!matches!(
                            row.status(),
                            AgentSemanticStatus::NeedsAttention { .. }
                        ))
                    });
                    rows.truncate(MAXIMUM_ROWS);
                    self.rows = rows;
                    // The selection follows the opaque session identity —
                    // never a stale row index. A vanished selection
                    // repairs deterministically to the first row in the
                    // current display order.
                    if let Some(held) = held {
                        self.selection = self
                            .rows
                            .iter()
                            .position(|row| row.session_id() == &held)
                            .unwrap_or(0);
                    }
                    self.rows_from_complete = round.complete;
                    self.rows_at = Some(now);
                    self.clamp_selection();
                }
                // When last_good rows are kept, their install time is the
                // honest freshness — not the moment this partial landed.
                let at = if replace {
                    now
                } else {
                    self.rows_at.unwrap_or(now)
                };
                self.read = AgentBoardRead::Ready {
                    complete: round.complete,
                    at,
                };
                self.last_summary = Some(AgentBoardSummary {
                    sessions: self.rows.len(),
                    complete: round.complete,
                    at,
                });
                self.notice = None;
            }
            Err(lane::LaneFailure::Denied) => {
                self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Denied);
            }
            Err(lane::LaneFailure::Unavailable) => {
                self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Unavailable);
            }
            Err(lane::LaneFailure::Transport) => {
                self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
            }
        }
    }

    /// Apply the verified Get outcome. `prefer` reports whether an
    /// attachment currently resolves into the caller's active context and
    /// tab — the deterministic pick's "current-tab usable" preference.
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    fn apply_get_outcome(
        &mut self,
        epoch: u64,
        serial: u64,
        result: Result<AgentGetOutcome, lane::LaneFailure>,
        prefer: &mut impl FnMut(&AgentPresentationAttachment) -> bool,
    ) {
        // The physical lane credit is bound to the outcome itself: a
        // matching serial always releases it — even when the jump that
        // issued it was superseded at request time or already failed.
        // What the credit must never do is clear a *different* jump:
        // that check happens only after the release.
        if epoch != self.epoch
            || !matches!(self.inflight, Some(BoardInflight::JumpGet { serial: held }) if held == serial)
        {
            return;
        }
        self.inflight = None;
        let Some(jump) = self.jump.as_mut() else {
            return;
        };
        if jump.serial != serial {
            return;
        }
        match result {
            Ok(AgentGetOutcome::Found { session, .. }) => {
                if session.snapshot.lifecycle == AgentLifecycle::Ended {
                    self.fail_jump(JumpFailure::Ended);
                    return;
                }
                let mut candidates: Vec<AgentPresentationAttachment> = session
                    .snapshot
                    .presentation_attachments
                    .iter()
                    .filter(|attachment| attachment.navigable)
                    .cloned()
                    .collect();
                // Deterministic pick: attachments already usable on the
                // active tab first, then Visible over hidden/unknown, then
                // stable presentation identity — the same observation always
                // produces the same first attempt.
                candidates.sort_by(|left, right| {
                    prefer(right)
                        .cmp(&prefer(left))
                        .then(
                            (right.visibility == AgentPresentationVisibility::Visible)
                                .cmp(&(left.visibility == AgentPresentationVisibility::Visible)),
                        )
                        .then(
                            left.presentation_id
                                .as_str()
                                .cmp(right.presentation_id.as_str()),
                        )
                });
                if candidates.is_empty() {
                    self.fail_jump(JumpFailure::NoPlacement);
                    return;
                }
                jump.stage = JumpStage::Navigating {
                    candidates: candidates.into_boxed_slice(),
                    index: 0,
                };
            }
            Ok(AgentGetOutcome::Rejected {
                reason: AgentSessionRejectionReason::NotFound,
            }) => self.fail_jump(JumpFailure::Gone),
            Ok(AgentGetOutcome::Rejected {
                reason: AgentSessionRejectionReason::ReadDenied,
            }) => self.fail_jump(JumpFailure::Denied),
            Ok(AgentGetOutcome::Unavailable { .. }) | Err(_) => {
                self.fail_jump(JumpFailure::Unavailable);
            }
        }
    }

    /// Drop the current candidate and try the next one; exhaustion means
    /// no attachment resolved to a live workbench target — `Gone`.
    pub(super) fn advance_jump_candidate(&mut self) -> bool {
        let Some(jump) = self.jump.as_mut() else {
            return false;
        };
        let JumpStage::Navigating { candidates, index } = &mut jump.stage else {
            return false;
        };
        *index += 1;
        if *index >= candidates.len() {
            self.fail_jump(JumpFailure::Gone);
        }
        true
    }

    /// Whether the jump's total budget elapsed; caller checks each tick.
    pub(super) fn expire_jump(&mut self, now: Instant) -> bool {
        if self.jump.as_ref().is_some_and(|jump| now >= jump.deadline) {
            self.fail_jump(JumpFailure::Expired);
            return true;
        }
        false
    }

    /// A failed jump is finished logically, but its lane credit is
    /// physical: an in-flight Get keeps the credit until the matching
    /// outcome drains (released by `apply_get_outcome`'s serial match)
    /// or the dead-lane witness releases it — never an unconditional
    /// clear that would let a fresh request overlap the lane's work.
    pub(super) fn fail_jump(&mut self, kind: JumpFailure) {
        self.notice = Some(kind);
        self.jump = None;
    }

    /// The jump landed on the exact panel — the user is where they asked
    /// to be, so the board closes.
    pub(super) fn jump_landed(&mut self) {
        self.jump = None;
        self.close();
    }

    /// Test seam: install a jump in a chosen stage without driving the
    /// lane — the UI drive tests stage navigation directly.
    #[cfg(test)]
    pub(super) fn force_jump_t(&mut self, jump: BoardJump) {
        self.jump = Some(jump);
    }

    /// Test seam: hand the board a prepared lane (queued outcomes or a
    /// real spawned thread) so drains run against it without an origin.
    #[cfg(all(test, any(target_os = "linux", target_os = "macos", windows)))]
    pub(in crate::terminal_session::agent_board) fn install_lane_t(&mut self, lane: AgentReadLane) {
        self.lane = Some(lane);
    }

    /// Bounded lane close for session teardown: the wake write interrupts
    /// an in-flight exchange and the join returns without waiting out the
    /// read deadline. Must run before the mailbox is released.
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(super) fn release_lane(&mut self) {
        if let Some(lane) = self.lane.take() {
            lane.close();
        }
        for lane in self.dying_lanes.drain(..) {
            lane.close();
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    pub(super) fn release_lane(&mut self) {}
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
impl Drop for AgentBoard {
    fn drop(&mut self) {
        if let Some(lane) = self.lane.take() {
            lane.close();
        }
        for lane in self.dying_lanes.drain(..) {
            lane.close();
        }
    }
}

/// The renderer's read handle over a live `AgentBoard` — borrows the
/// board and the freshest Workbench presentation index inside
/// `NativeSessionView`, exposes rows and placement lookups without
/// handing out mutation.
#[derive(Clone, Copy)]
pub(crate) struct NativeAgentBoardView<'a> {
    board: &'a AgentBoard,
    index: &'a PresentationIndex,
    #[cfg(windows)]
    attention: Option<&'a super::agent_attention::AgentAttention>,
}

impl<'a> NativeAgentBoardView<'a> {
    pub(crate) fn token(&self) -> AgentBoardToken {
        self.board.token()
    }
    pub(super) const fn new(board: &'a AgentBoard, index: &'a PresentationIndex) -> Self {
        Self {
            board,
            index,
            #[cfg(windows)]
            attention: None,
        }
    }
    #[cfg(windows)]
    pub(super) fn with_attention(
        mut self,
        attention: &'a super::agent_attention::AgentAttention,
    ) -> Self {
        self.attention = Some(attention);
        self
    }
    pub(crate) fn unread(&self, row: &AgentBoardRow) -> bool {
        #[cfg(windows)]
        {
            self.attention
                .is_some_and(|attention| attention.session_unread(row.session_id()))
        }
        #[cfg(not(windows))]
        {
            let _ = row;
            false
        }
    }
    pub(crate) fn rows(&self) -> &'a [AgentBoardRow] {
        self.board.rows()
    }
    pub(crate) const fn selection(&self) -> usize {
        self.board.selection()
    }
    pub(crate) const fn scroll(&self) -> usize {
        self.board.scroll()
    }
    pub(crate) const fn read(&self) -> AgentBoardRead {
        self.board.read()
    }
    pub(crate) const fn notice(&self) -> Option<JumpFailure> {
        self.board.notice()
    }
    pub(crate) const fn jumping(&self) -> bool {
        self.board.jumping()
    }
    /// Where one attachment currently resolves in the live Workbench —
    /// `None` when its presentation identity has no painted panel.
    pub(crate) fn placement(
        &self,
        attachment: &AgentPresentationAttachment,
    ) -> Option<&'a PresentationPlacement> {
        self.index.placement(attachment)
    }
}
