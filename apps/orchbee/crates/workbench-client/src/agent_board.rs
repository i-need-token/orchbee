//! The Agent board's session-side presentation state: the rows the last
//! completed OBAP list read returned, the board's own interaction state
//! (selection, scroll, notice), the queued read/jump intents, and the
//! in-flight jump's stage bookkeeping. Canonical Agent state stays
//! server-owned — this module keeps only what the last completed read
//! delivered and never synthesizes status of its own.
//!
//! The Board holds no read lane. It issues each OBAP read as a typed
//! [`AgentBoardReadRequest`] and settles each [`AgentBoardReadEvent`] the
//! composition hands back; the composition owns the lane thread that runs
//! the reads on a logical scope of the shared carrier.

use std::time::{Duration, Instant};

use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentLifecycle, AgentObservationFreshness, AgentPresentationAttachment,
    AgentPresentationVisibility, AgentProviderId, AgentSemanticStatus, AgentSessionId,
    AgentSessionRejectionReason, AgentSessionSummary,
};

use crate::tui_presentation::{GeometrySettlement, PresentationPhase};

mod names;
pub(crate) use names::{LivePlacementNames, NameFacts};
pub(crate) mod start;
#[cfg(test)]
mod tests;
pub(crate) use start::{CatalogStatus as AgentStartCatalogStatus, Phase as AgentStartPhase};

/// One bounded read round's absolute deadline — connect admission, the
/// whole page walk, and the single allowed restart all live inside it.
pub(crate) const AGENT_READ_DEADLINE: Duration = Duration::from_secs(3);
/// One jump Get's own deadline inside the jump's total budget.
pub(crate) const AGENT_GET_DEADLINE: Duration = Duration::from_secs(3);
/// One jump's total budget across the Get plus every navigation stage.
pub(crate) const AGENT_JUMP_DEADLINE: Duration = Duration::from_secs(8);
/// The read cooldown: a completed list round — success or failure —
/// refuses further reads until this much time has passed; `r` does not
/// bypass it.
pub(crate) const AGENT_READ_COOLDOWN: Duration = Duration::from_secs(2);
/// The 512-session runtime bound at 100 items per page, rounded up.
const MAXIMUM_ROWS: usize = 600;

/// A canonical Agent row: the summary the last completed list read
/// returned, kept verbatim — the board renders it, never reinterprets it.
#[derive(Clone, Debug)]
pub struct AgentBoardRow {
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
    pub const fn status(&self) -> &AgentSemanticStatus {
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

pub(crate) use crate::localization::AgentBoardRead;

pub(crate) use crate::localization::AgentBoardReadFailure;

pub(crate) use crate::localization::JumpFailure;

/// Stable identity for one open Board incarnation.
///
/// Selection, viewport and row-source changes deliberately do not advance this
/// token. They belong to the exact draw witness below; late work from another
/// open incarnation remains excluded by this episode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentBoardEpisodeToken(u64);

impl AgentBoardEpisodeToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// Value-semantic source behind one Board draw.
///
/// `rows_revision` changes whenever the installed visible row set changes;
/// selection and jump availability remain explicit so a pointer release cannot
/// act on a different projection that happens to share a viewport offset.
#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentBoardDrawSource {
    episode: AgentBoardEpisodeToken,
    rows_revision: u64,
    selected: Option<AgentSessionId>,
    jumping: bool,
    start: start::DrawSource,
}

/// Exact successful-paint evidence for one Board frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentBoardActualDraw {
    token: AgentBoardToken,
    visible_rows: usize,
    first: Option<AgentSessionId>,
    selected: Option<AgentSessionId>,
}

impl AgentBoardActualDraw {
    fn new(
        token: AgentBoardToken,
        visible_rows: usize,
        first: Option<AgentSessionId>,
        selected: Option<AgentSessionId>,
    ) -> Option<Self> {
        (visible_rows > 0).then_some(Self {
            token,
            visible_rows,
            first,
            selected,
        })
    }

    pub(crate) const fn episode(&self) -> AgentBoardEpisodeToken {
        self.token.source.episode
    }

    pub(crate) const fn token(&self) -> &AgentBoardToken {
        &self.token
    }

    pub(crate) const fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    pub(crate) const fn first(&self) -> Option<&AgentSessionId> {
        self.first.as_ref()
    }

    pub(crate) const fn selected(&self) -> Option<&AgentSessionId> {
        self.selected.as_ref()
    }

    pub(crate) fn row_target(&self, session: AgentSessionId) -> AgentBoardRowTarget {
        AgentBoardRowTarget {
            token: self.token.clone(),
            session,
        }
    }

    pub(crate) fn control_target(&self, control: AgentBoardControl) -> AgentBoardControlTarget {
        AgentBoardControlTarget {
            token: self.token.clone(),
            control,
        }
    }
}

/// What the renderer actually did with one exact Board episode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardDraw {
    Presented(AgentBoardActualDraw),
    NotPresented(AgentBoardEpisodeToken),
}

impl AgentBoardDraw {
    pub(crate) const fn is_presented(&self) -> bool {
        matches!(self, Self::Presented(_))
    }

    pub(crate) const fn episode(&self) -> AgentBoardEpisodeToken {
        match self {
            Self::Presented(draw) => draw.episode(),
            Self::NotPresented(episode) => *episode,
        }
    }
}

/// Owner projection consumed by the top-input compositor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentBoardInputContext {
    episode: AgentBoardEpisodeToken,
    presentation: PresentationPhase,
    interactive: bool,
}

impl AgentBoardInputContext {
    #[cfg(test)]
    pub(crate) const fn for_test(
        episode: AgentBoardEpisodeToken,
        presentation: PresentationPhase,
        interactive: bool,
    ) -> Self {
        Self {
            episode,
            presentation,
            interactive,
        }
    }

    pub(crate) const fn episode(self) -> AgentBoardEpisodeToken {
        self.episode
    }

    #[cfg(test)]
    pub(crate) const fn presentation(self) -> PresentationPhase {
        self.presentation
    }

    /// Whether a complete frame for this episode still holds its visibility
    /// lease. The compositor separately decides whether the Board is the top
    /// owner. Together those facts permit owner-local navigation; activations
    /// and pointer effects also require exact current draw proof.
    pub(crate) const fn interactive(self) -> bool {
        self.interactive
    }
}

pub(crate) use crate::localization::AgentBoardSummary;

/// A workbench placement resolved from a `TerminalPresentationId`: which
/// context owns it, which tab, which panel — plus the display labels the
/// snapshot carried at index-build time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentationPlacement {
    pub(crate) location: crate::workbench_projection::ClientContextLocation,
    pub(crate) tab_id: orchbee_workbench_protocol::WorkbenchTabId,
    pub(crate) panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
    pub(crate) tab_position: usize,
    pub(crate) tab_label: String,
    pub(crate) panel_label: String,
    /// The attached Presentation's naming facts for the Tab strip's rule.
    pub(crate) presentation_name: NameFacts,
    /// For an automatic Tab, the facts of the member the strip names it after when no live view
    /// selects one: the first Panel's primary Presentation. `None` keeps a fixed name exact.
    pub(crate) automatic_tab_name: Option<NameFacts>,
    pub(crate) project_label: Option<String>,
    pub(crate) workspace_label: Option<String>,
}

/// `presentation_id → placement` over the latest observed Workbench
/// snapshot. Rebuilt wholesale on every snapshot-bearing worker event —
/// the index is a function of canonical state, never a second authority.
#[derive(Default)]
pub(crate) struct PresentationIndex {
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
    pub(crate) fn rebuild(snapshot: &orchbee_workbench_protocol::Snapshot) -> Self {
        let mut index = Self {
            ready: true,
            ..Self::default()
        };
        for location in navigation_locations(snapshot) {
            if let Ok(tabs) =
                crate::workbench_projection::project_native_context_tabs(snapshot, &location)
            {
                index.install_context(&tabs);
            }
        }
        index
    }

    /// Whether any canonical feed has ever built this index — the
    /// jump's honesty gate: `false` means "no snapshot yet", never
    /// "no placements exist".
    pub(crate) const fn ready(&self) -> bool {
        self.ready
    }

    /// Fold one context's projected tabs into the index (used for the
    /// bootstrap cache, which arrives already projected rather than as a
    /// raw snapshot).
    pub(crate) fn install_context(
        &mut self,
        tabs: &crate::workbench_projection::ClientContextTabs,
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
            let automatic_tab_name = tab
                .automatic_label()
                .then(|| tab.panel(0).map(|panel| NameFacts::of(panel.primary())))
                .map(Option::unwrap_or_default);
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
                            presentation_name: NameFacts::of(member),
                            automatic_tab_name: automatic_tab_name.clone(),
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
    pub(crate) fn placement(
        &self,
        attachment: &AgentPresentationAttachment,
    ) -> Option<&PresentationPlacement> {
        self.placements.get(attachment.presentation_id.as_str())
    }

    pub(crate) fn placement_by_presentation(
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

/// Every `ClientContextLocation` a snapshot's navigation declares, in
/// canonical Workspace order.
fn navigation_locations(
    snapshot: &orchbee_workbench_protocol::Snapshot,
) -> Vec<crate::workbench_projection::ClientContextLocation> {
    use crate::workbench_projection::ClientContextLocation;
    let mut locations = Vec::with_capacity(snapshot.navigation.workspaces.len());
    if let Ok(ordered) = snapshot.navigation.ordered_workspaces() {
        for (_, workspace) in ordered {
            locations.push(ClientContextLocation::Workspace {
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
const BOARD_SELECT_UP_KEYS: [crate::client_view::Binding; 2] = [
    crate::client_view::Binding::new(
        crate::client_view::KeyCode::Up,
        crate::client_view::Modifiers::NONE,
    ),
    crate::client_view::Binding::new(
        crate::client_view::KeyCode::Char('k'),
        crate::client_view::Modifiers::NONE,
    ),
];
const BOARD_SELECT_DOWN_KEYS: [crate::client_view::Binding; 2] = [
    crate::client_view::Binding::new(
        crate::client_view::KeyCode::Down,
        crate::client_view::Modifiers::NONE,
    ),
    crate::client_view::Binding::new(
        crate::client_view::KeyCode::Char('j'),
        crate::client_view::Modifiers::NONE,
    ),
];
const BOARD_REFRESH_KEY: crate::client_view::Binding = crate::client_view::Binding::new(
    crate::client_view::KeyCode::Char('r'),
    crate::client_view::Modifiers::NONE,
);
const BOARD_JUMP_KEY: crate::client_view::Binding = crate::client_view::Binding::new(
    crate::client_view::KeyCode::Enter,
    crate::client_view::Modifiers::NONE,
);
const BOARD_CLOSE_KEY: crate::client_view::Binding = crate::client_view::Binding::new(
    crate::client_view::KeyCode::Esc,
    crate::client_view::Modifiers::NONE,
);
const BOARD_START_ACCEPT_KEY: crate::client_view::Binding = crate::client_view::Binding::new(
    crate::client_view::KeyCode::Enter,
    crate::client_view::Modifiers::NONE,
);
const BOARD_START_PREVIOUS_PROVIDER_KEY: crate::client_view::Binding =
    crate::client_view::Binding::new(
        crate::client_view::KeyCode::Up,
        crate::client_view::Modifiers::NONE,
    );
const BOARD_START_NEXT_PROVIDER_KEY: crate::client_view::Binding = crate::client_view::Binding::new(
    crate::client_view::KeyCode::Down,
    crate::client_view::Modifiers::NONE,
);
const BOARD_START_EDIT_KEY: crate::client_view::Binding = crate::client_view::Binding::new(
    crate::client_view::KeyCode::Backspace,
    crate::client_view::Modifiers::NONE,
);
const BOARD_START_CANCEL_KEY: crate::client_view::Binding = crate::client_view::Binding::new(
    crate::client_view::KeyCode::Char('c'),
    crate::client_view::Modifiers::NONE,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardKeyAction {
    SelectUp,
    SelectDown,
    Refresh,
    Jump,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentStartKeyAction {
    Start,
    PreviousProvider,
    NextProvider,
    Cancel,
}

pub(crate) fn board_key_action(event: crate::client_view::KeyEvent) -> Option<AgentBoardKeyAction> {
    if BOARD_SELECT_UP_KEYS
        .iter()
        .any(|binding| binding.matches(event))
    {
        Some(AgentBoardKeyAction::SelectUp)
    } else if BOARD_SELECT_DOWN_KEYS
        .iter()
        .any(|binding| binding.matches(event))
    {
        Some(AgentBoardKeyAction::SelectDown)
    } else if BOARD_REFRESH_KEY.matches(event) {
        Some(AgentBoardKeyAction::Refresh)
    } else if BOARD_JUMP_KEY.matches(event) {
        Some(AgentBoardKeyAction::Jump)
    } else if BOARD_CLOSE_KEY.matches(event) {
        Some(AgentBoardKeyAction::Close)
    } else {
        None
    }
}

pub(crate) fn agent_start_key_action(
    event: crate::client_view::KeyEvent,
    running: bool,
) -> Option<AgentStartKeyAction> {
    if running && BOARD_START_CANCEL_KEY.matches(event) {
        Some(AgentStartKeyAction::Cancel)
    } else if BOARD_START_ACCEPT_KEY.matches(event) {
        Some(AgentStartKeyAction::Start)
    } else if BOARD_START_PREVIOUS_PROVIDER_KEY.matches(event) {
        Some(AgentStartKeyAction::PreviousProvider)
    } else if BOARD_START_NEXT_PROVIDER_KEY.matches(event) {
        Some(AgentStartKeyAction::NextProvider)
    } else {
        None
    }
}

/// One step of the staged jump, decided purely from observed state — the
/// driver re-plans against live facts every tick and never caches indices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum JumpStep {
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
pub(crate) fn plan_jump_step(
    placement: &PresentationPlacement,
    active_location: Option<&crate::workbench_projection::ClientContextLocation>,
    context_locations: &[crate::workbench_projection::ClientContextLocation],
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

/// The one outstanding read request, correlated by kind (and jump serial).
#[derive(Clone, Debug)]
enum BoardInflight {
    ListRound,
    JumpGet { serial: u64 },
}

/// One canonical read the open Board asks for. The composition runs it on
/// the Board's read lane; the answer carries `episode` and `serial` back
/// unchanged, and the Board alone interprets them.
#[derive(Debug)]
pub(crate) enum AgentBoardReadRequest {
    /// Walk the whole Agent list within `deadline`.
    List {
        episode: AgentBoardEpisodeToken,
        deadline: Instant,
    },
    /// Verify one session's attachments for jump `serial` within `deadline`.
    Get {
        episode: AgentBoardEpisodeToken,
        serial: u64,
        session: AgentSessionId,
        deadline: Instant,
    },
}

impl AgentBoardReadRequest {
    /// The open Board episode that issued the request.
    pub(crate) const fn episode(&self) -> AgentBoardEpisodeToken {
        match self {
            Self::List { episode, .. } | Self::Get { episode, .. } => *episode,
        }
    }
}

/// Why the composition could not hand a request to a read lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardReadRefusal {
    /// No lane could serve it: no origin yet, a retired lane's thread is
    /// still exiting, or the lane thread could not start.
    LaneUnavailable,
    /// The live lane refused the command because its thread is gone; the
    /// composition released that lane.
    Rejected,
}

/// Why a read the lane ran produced no canonical answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentReadFailure {
    /// The runtime answered `unavailable`.
    Unavailable,
    /// The runtime denied the read.
    Denied,
    /// Connect, transport, or post-submission ambiguity.
    Transport,
}

/// One completed list walk. `complete` is false when the page bound or the
/// restart budget was hit; the accumulated pages are still canonical and may
/// be shown with the incomplete marker.
#[derive(Debug)]
pub(crate) struct AgentListRound {
    pub(crate) items: Vec<AgentSessionSummary>,
    pub(crate) complete: bool,
}

/// The read lane's answer to one [`AgentBoardReadRequest`].
#[derive(Debug)]
pub(crate) enum AgentBoardReadAnswer {
    List {
        episode: AgentBoardEpisodeToken,
        result: Result<AgentListRound, AgentReadFailure>,
    },
    Get {
        episode: AgentBoardEpisodeToken,
        serial: u64,
        result: Result<AgentGetOutcome, AgentReadFailure>,
    },
}

/// What the composition settles into the Board from its read lane, in
/// delivery order.
#[derive(Debug)]
pub(crate) enum AgentBoardReadEvent {
    Answer(AgentBoardReadAnswer),
    /// The lane delivered an outcome of a kind the Board never requests.
    Foreign,
    /// The lane serving this episode ended after delivering every answer it
    /// had queued, so no answer will arrive for an outstanding request.
    LaneEnded,
}

/// Attention facts that the Board view overlays on canonical rows. The
/// composition lends the attention owner; the Board never reaches it.
pub(crate) trait AgentRowAttention {
    /// Whether the session has attention the user has not seen.
    fn session_unread(&self, session: &AgentSessionId) -> bool;
    /// The Server's completion fact for this exact presentation generation.
    fn presentation_completion(
        &self,
        attachment: &AgentPresentationAttachment,
    ) -> Option<orchbee_workbench_protocol::AgentCompletion>;
}

/// An accepted jump in progress. `serial` is the generation the lane's
/// `Get` outcome must match; a new jump replaces the serial so a stale
/// in-flight stage is dropped at its next boundary check.
#[derive(Clone, Debug)]
pub(crate) struct BoardJump {
    pub(crate) serial: u64,
    pub(crate) stage: JumpStage,
    pub(crate) deadline: Instant,
}

#[derive(Clone, Debug)]
pub(crate) enum JumpStage {
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
pub struct AgentBoardToken {
    source: AgentBoardDrawSource,
    scroll: usize,
}

impl AgentBoardToken {
    pub(crate) const fn scroll(&self) -> usize {
        self.scroll
    }
}

/// Exact Board row painted into one admitted frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentBoardRowTarget {
    token: AgentBoardToken,
    session: AgentSessionId,
}

impl AgentBoardRowTarget {
    #[cfg(test)]
    pub(crate) fn for_test(episode: AgentBoardEpisodeToken, session: AgentSessionId) -> Self {
        Self {
            token: AgentBoardToken {
                source: AgentBoardDrawSource {
                    episode,
                    rows_revision: 0,
                    selected: Some(session.clone()),
                    jumping: false,
                    start: start::State::new(true).draw_source(),
                },
                scroll: 0,
            },
            session,
        }
    }

    pub(crate) const fn token(&self) -> &AgentBoardToken {
        &self.token
    }

    pub(crate) const fn session(&self) -> &AgentSessionId {
        &self.session
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardControl {
    Open,
    Refresh,
    Close,
    Start,
    CancelStart,
    StartProvider(usize),
}

/// Exact Board control painted into one admitted frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentBoardControlTarget {
    token: AgentBoardToken,
    control: AgentBoardControl,
}

impl AgentBoardControlTarget {
    pub(crate) const fn token(&self) -> &AgentBoardToken {
        &self.token
    }

    pub(crate) const fn control(&self) -> AgentBoardControl {
        self.control
    }
}

pub(crate) struct AgentBoard {
    open: bool,
    /// Bump on every open/close — read answers carrying an older episode
    /// are stale by construction and dropped.
    epoch: u64,
    presentation: PresentationPhase,
    /// Whole-surface visibility lease for ordinary owner-local navigation.
    /// Only an exact successful paint grants it; cover, geometry block,
    /// reopen and close revoke it. Semantic state changes invalidate `shown`
    /// activation proof without pretending the still-visible Board vanished.
    visible: bool,
    shown: Option<AgentBoardActualDraw>,
    rows: Vec<AgentBoardRow>,
    rows_revision: u64,
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
    start: start::State,
    start_catalog_path: Option<std::ffi::OsString>,
    start_catalog_ready_epoch: Option<u64>,
}

impl AgentBoard {
    pub(crate) const fn episode(&self) -> AgentBoardEpisodeToken {
        AgentBoardEpisodeToken(self.epoch)
    }

    fn draw_source(&self) -> AgentBoardDrawSource {
        AgentBoardDrawSource {
            episode: self.episode(),
            rows_revision: self.rows_revision,
            selected: self.selected_row().map(|row| row.session_id.clone()),
            jumping: self.jumping(),
            start: self.start.draw_source(),
        }
    }

    fn token(&self) -> AgentBoardToken {
        AgentBoardToken {
            source: self.draw_source(),
            scroll: self.scroll,
        }
    }

    fn current_draw(&self) -> Option<&AgentBoardActualDraw> {
        let draw = self.shown.as_ref()?;
        (draw.token == self.token()).then_some(draw)
    }

    pub(crate) fn input_context(&self) -> Option<AgentBoardInputContext> {
        self.open.then_some(AgentBoardInputContext {
            episode: self.episode(),
            presentation: self.presentation,
            interactive: self.interactive(),
        })
    }

    fn interactive(&self) -> bool {
        self.open && self.presentation.interactive() && self.visible
    }

    pub(crate) fn selected_is_shown(&self) -> bool {
        let Some(draw) = self.current_draw() else {
            return false;
        };
        self.selected_row()
            .is_some_and(|row| draw.selected() == Some(row.session_id()))
    }

    fn actual_draw(&self, visible_rows: usize) -> Option<AgentBoardActualDraw> {
        if !self.open || visible_rows == 0 {
            return None;
        }
        let first_index = self
            .scroll
            .min(self.rows.len().saturating_sub(visible_rows));
        let token = AgentBoardToken {
            source: self.draw_source(),
            scroll: first_index,
        };
        let first = self
            .rows
            .get(first_index)
            .map(|row| row.session_id().clone());
        let selected = self
            .rows
            .get(self.selection)
            .filter(|_| {
                self.selection >= first_index
                    && self.selection < first_index.saturating_add(visible_rows)
            })
            .map(|row| row.session_id().clone());
        AgentBoardActualDraw::new(token, visible_rows, first, selected)
    }

    pub(crate) fn control_drawn(&mut self, target: &AgentBoardControlTarget, now: Instant) -> bool {
        if !self.interactive()
            || self
                .current_draw()
                .is_none_or(|draw| draw.token() != target.token())
        {
            return false;
        }
        match target.control() {
            AgentBoardControl::Open if self.selected_is_shown() => self.request_jump(),
            AgentBoardControl::Open => return false,
            AgentBoardControl::Refresh => self.request_refresh(now),
            AgentBoardControl::Close => self.close(),
            AgentBoardControl::Start => return false,
            AgentBoardControl::CancelStart => return self.start.cancel(),
            AgentBoardControl::StartProvider(index) => {
                if !self.rows.is_empty()
                    || !matches!(self.read, AgentBoardRead::Ready { complete: true, .. })
                {
                    return false;
                }
                return self.start.select_provider(index);
            }
        }
        true
    }

    pub(crate) fn scroll_painted(
        &mut self,
        token: &AgentBoardToken,
        up: bool,
        lines: usize,
    ) -> Option<AgentBoardToken> {
        if !self.interactive() || self.current_draw().is_none_or(|draw| draw.token() != token) {
            return None;
        }
        let lines = isize::try_from(lines).ok()?;
        self.scroll_by(if up { -lines } else { lines });
        Some(self.token())
    }

    pub(crate) fn new(launch_supported: bool) -> Self {
        Self {
            open: false,
            epoch: 0,
            presentation: PresentationPhase::Pending,
            visible: false,
            shown: None,
            rows: Vec::new(),
            rows_revision: 0,
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
            start: start::State::new(launch_supported),
            start_catalog_path: None,
            start_catalog_ready_epoch: None,
        }
    }

    /// Whether the Client Connection's origin is this machine; only then is
    /// the Client's own PATH evidence for the start catalog.
    pub(crate) fn set_origin_locality(&mut self, local: bool) {
        self.start.set_origin_locality(local);
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }

    /// The open episode that the composition's read lane may serve; `None`
    /// while the Board is closed.
    pub(crate) const fn open_episode(&self) -> Option<AgentBoardEpisodeToken> {
        if self.open {
            Some(self.episode())
        } else {
            None
        }
    }
    /// Capture presence is synchronous from `open`: a key arriving before
    /// the first paint still belongs to the board and never falls through.
    /// Activation remains separately gated by exact successful-draw proof.
    #[cfg(test)]
    pub(crate) const fn owns_input(&self) -> bool {
        self.open
    }

    pub(crate) fn open(&mut self) {
        self.open = true;
        self.epoch += 1;
        self.start.opened(self.epoch);
        {
            self.start_catalog_ready_epoch = None;
            self.start.retire_catalog();
        }
        self.presentation = PresentationPhase::Pending;
        self.visible = false;
        self.shown = None;
        self.intent = Some(BoardIntent::Refresh);
        self.inflight = None;
        self.jump = None;
        self.notice = None;
        self.clamp_viewport();
    }

    pub(crate) fn open_with_start_catalog(&mut self, path: Option<std::ffi::OsString>) {
        self.open();
        // PATH is only captured here. Probe admission waits for this episode's
        // complete empty List result and is fenced again in `drain_start`.
        self.start_catalog_path = path;
    }

    /// Ends the open episode. The epoch bump invalidates every answer the
    /// episode's requests still owe and stops scheduling; the composition then
    /// retires the read lane that served the episode.
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.epoch += 1;
        self.visible = false;
        self.shown = None;
        self.intent = None;
        self.inflight = None;
        self.jump = None;
        self.notice = None;
        {
            self.start_catalog_path = None;
            self.start_catalog_ready_epoch = None;
            self.start.retire_catalog();
        }
    }

    /// Settles exact paint evidence. `None` means another composed surface
    /// covered the Board: revoke current-frame authority but retain the open
    /// episode and all domain work. `NotPresented` is geometry evidence and
    /// therefore refuses only a never-presented episode.
    pub(crate) fn settle_draw(
        &mut self,
        draw: Option<AgentBoardDraw>,
    ) -> Option<GeometrySettlement> {
        let Some(draw) = draw else {
            self.visible = false;
            self.shown = None;
            return None;
        };
        if !self.open || draw.episode() != self.episode() {
            return None;
        }
        match draw {
            AgentBoardDraw::NotPresented(_) => {
                self.visible = false;
                self.shown = None;
                let settlement = self.presentation.geometry_blocked();
                if settlement == GeometrySettlement::Refused {
                    self.close();
                }
                Some(settlement)
            }
            AgentBoardDraw::Presented(actual) => {
                let visible = actual.visible_rows();
                let maximum = self.rows.len().saturating_sub(visible);
                let first_index = self.scroll.min(maximum);
                let first = self.rows.get(first_index).map(AgentBoardRow::session_id);
                let selected = self
                    .rows
                    .get(self.selection)
                    .filter(|_| {
                        self.selection >= first_index
                            && self.selection < first_index.saturating_add(visible)
                    })
                    .map(AgentBoardRow::session_id);
                if actual.token.source != self.draw_source()
                    || actual.token.scroll != first_index
                    || actual.first() != first
                    || actual.selected() != selected
                {
                    return None;
                }
                self.viewport_rows = visible;
                self.scroll = first_index;
                self.presentation.presented();
                self.visible = true;
                self.shown = Some(actual);
                None
            }
        }
    }

    pub(crate) fn rows(&self) -> &[AgentBoardRow] {
        &self.rows
    }
    pub(crate) const fn selection(&self) -> usize {
        self.selection
    }
    pub(crate) const fn read(&self) -> AgentBoardRead {
        self.read
    }
    pub(crate) const fn notice(&self) -> Option<JumpFailure> {
        self.notice
    }
    pub(crate) fn jump(&self) -> Option<&BoardJump> {
        self.jump.as_ref()
    }
    pub(crate) const fn jumping(&self) -> bool {
        self.jump.is_some()
    }

    /// The sidebar's summary of the last completed read — `None` until
    /// the first completed read ever, so a failed or never-run read
    /// shows no count at all rather than implying zero Agents.
    pub(crate) fn summary(&self) -> Option<&AgentBoardSummary> {
        self.last_summary.as_ref()
    }

    fn selected_row(&self) -> Option<&AgentBoardRow> {
        self.rows.get(self.selection)
    }

    fn clamp_viewport(&mut self) {
        if self.rows.is_empty() {
            self.selection = 0;
            self.scroll = 0;
            return;
        }
        self.selection = self.selection.min(self.rows.len() - 1);
        let visible = self.viewport_rows.max(1);
        let maximum = self.rows.len().saturating_sub(visible);
        self.scroll = self.scroll.min(maximum);
    }

    fn reveal_selection(&mut self) {
        self.clamp_viewport();
        if self.rows.is_empty() {
            return;
        }
        let visible = self.viewport_rows.max(1);
        if self.selection < self.scroll {
            self.scroll = self.selection;
        } else if self.selection >= self.scroll + visible {
            self.scroll = self.selection + 1 - visible;
        }
        self.clamp_viewport();
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        self.selection = self
            .selection
            .saturating_add_signed(delta)
            .min(self.rows.len() - 1);
        self.shown = None;
        self.reveal_selection();
    }

    /// Pointer row selection commits on press, but only against the exact
    /// successfully painted source and visible range carried by the target.
    pub(crate) fn select_drawn_row(&mut self, target: &AgentBoardRowTarget) -> bool {
        if !self.interactive()
            || self
                .current_draw()
                .is_none_or(|draw| draw.token() != target.token())
        {
            return false;
        }
        let Some(draw) = self.current_draw() else {
            return false;
        };
        let end = draw
            .token
            .scroll
            .saturating_add(draw.visible_rows)
            .min(self.rows.len());
        let Some(index) = self.rows[draw.token.scroll..end]
            .iter()
            .position(|row| row.session_id() == target.session())
            .map(|index| index + draw.token.scroll)
        else {
            return false;
        };
        self.selection = index;
        self.shown = None;
        self.clamp_viewport();
        true
    }

    /// Internal stable-id selection seam retained for non-pointer tests and
    /// owner operations that do not claim painted pointer authority.
    #[cfg(test)]
    pub(crate) fn select_session(&mut self, session: &AgentSessionId) -> bool {
        let Some(index) = self.rows.iter().position(|row| row.session_id() == session) else {
            return false;
        };
        self.selection = index;
        self.shown = None;
        self.reveal_selection();
        true
    }

    pub(crate) fn scroll_by(&mut self, delta: isize) {
        let visible = self.viewport_rows.max(1);
        let maximum = self.rows.len().saturating_sub(visible);
        self.scroll = self.scroll.saturating_add_signed(delta).min(maximum);
        self.shown = None;
    }

    pub(crate) fn request_refresh(&mut self, now: Instant) {
        // A completed round — success or failure — imposes the cooldown;
        // `r` inside the window is a no-op, not a queued refresh.
        if self
            .last_read_end
            .is_some_and(|at| now < at + AGENT_READ_COOLDOWN)
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
    pub(crate) fn request_jump(&mut self) {
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
        self.shown = None;
        self.intent = Some(BoardIntent::Jump { target });
    }

    /// Admits a start against the current draw; `jobs` accepts it in the start
    /// state for this episode and runs its workflow.
    pub(crate) fn start_agent(
        &mut self,
        panel: orchbee_workbench_protocol::WorkbenchPanelId,
        jobs: impl FnOnce(&mut start::State, u64, orchbee_workbench_protocol::WorkbenchPanelId) -> bool,
    ) -> bool {
        if !self.interactive()
            || !self.rows.is_empty()
            || !matches!(self.read, AgentBoardRead::Ready { complete: true, .. })
            || self.current_draw().is_none()
        {
            return false;
        }
        jobs(&mut self.start, self.epoch, panel)
    }

    pub(crate) fn start_agent_drawn(
        &mut self,
        target: &AgentBoardControlTarget,
        panel: orchbee_workbench_protocol::WorkbenchPanelId,
        jobs: impl FnOnce(&mut start::State, u64, orchbee_workbench_protocol::WorkbenchPanelId) -> bool,
    ) -> bool {
        if target.control() != AgentBoardControl::Start
            || !self.interactive()
            || self
                .current_draw()
                .is_none_or(|draw| draw.token() != target.token())
        {
            return false;
        }
        self.start_agent(panel, jobs)
    }

    pub(crate) fn edit_start_draft(&mut self, event: crate::client_view::KeyEvent) -> bool {
        if !self.interactive()
            || !self.rows.is_empty()
            || !matches!(self.read, AgentBoardRead::Ready { complete: true, .. })
        {
            return false;
        }
        self.start.edit(event)
    }

    pub(crate) fn move_start_provider(&mut self, delta: isize) -> bool {
        if !self.interactive()
            || !self.rows.is_empty()
            || !matches!(self.read, AgentBoardRead::Ready { complete: true, .. })
        {
            return false;
        }
        self.start.move_provider(delta)
    }

    pub(crate) fn cancel_start_drawn(&mut self) -> bool {
        if !self.interactive() || self.current_draw().is_none() {
            return false;
        }
        self.start.cancel()
    }

    pub(crate) fn start_form_active(&self) -> bool {
        self.start.catalog_status() != start::CatalogStatus::Unsupported
            && self.rows.is_empty()
            && matches!(self.read, AgentBoardRead::Ready { complete: true, .. })
    }

    pub(crate) fn start_phase(&self) -> Option<start::Phase> {
        self.start.operation_phase()
    }

    pub(crate) fn start_running(&self) -> bool {
        self.start
            .phase(self.epoch)
            .is_some_and(start::Phase::running)
    }

    /// The start state the jobs reconcile after an explicit cancellation.
    pub(crate) const fn start_state(&self) -> &start::State {
        &self.start
    }

    /// One start drain: `jobs` settles its outcomes into the start state and may
    /// begin the catalog only while the empty Board's form admits it.
    pub(crate) fn drain_start(
        &mut self,
        now: Instant,
        jobs: impl FnOnce(&mut start::State, start::CatalogAdmission) -> start::Drain,
    ) -> bool {
        let admission = if self.open
            && self.start_catalog_ready_epoch == Some(self.epoch)
            && self.rows.is_empty()
            && matches!(self.read, AgentBoardRead::Ready { complete: true, .. })
            && self.start.catalog_unloaded()
        {
            start::CatalogAdmission::Open(self.start_catalog_path.clone())
        } else {
            start::CatalogAdmission::Closed
        };
        let drained = jobs(&mut self.start, admission);
        if drained.started {
            self.request_refresh(now);
        }
        drained.changed
    }

    #[cfg(test)]
    pub(crate) fn install_start_draft_t(&mut self, providers: &[&'static str], name: &str) {
        self.start.install_catalog(providers);
        self.start.set_name(name);
    }

    #[cfg(test)]
    pub(crate) fn accept_start_without_task_t(&mut self) {
        self.start.accept_without_task();
    }

    #[cfg(test)]
    pub(crate) fn start_operation_panel_t(
        &self,
    ) -> Option<&orchbee_workbench_protocol::WorkbenchPanelId> {
        self.start.operation_panel()
    }

    /// Issue the queued intent as a typed read request — one outstanding
    /// request at a time (the model's read serialization). `issue` hands the
    /// request to the composition's read lane and reports a refusal; the
    /// request's answer arrives later through `settle_read`.
    pub(crate) fn drive_intent(
        &mut self,
        now: Instant,
        issue: impl FnOnce(AgentBoardReadRequest) -> Result<(), AgentBoardReadRefusal>,
    ) -> bool {
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
                self.issue_list_round(now, issue);
                true
            }
            Some(BoardIntent::Jump { target }) => {
                self.issue_jump_get(target, now, issue);
                true
            }
            None => false,
        }
    }

    fn issue_list_round(
        &mut self,
        now: Instant,
        issue: impl FnOnce(AgentBoardReadRequest) -> Result<(), AgentBoardReadRefusal>,
    ) {
        {
            // A new canonical read fences both a queued catalog answer and
            // the previous episode/round's empty-list admission.
            self.start_catalog_ready_epoch = None;
            self.start.retire_catalog();
        }
        let Some(deadline) = now.checked_add(AGENT_READ_DEADLINE) else {
            self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
            return;
        };
        let request = AgentBoardReadRequest::List {
            episode: self.episode(),
            deadline,
        };
        self.read = match issue(request) {
            Ok(()) => {
                self.inflight = Some(BoardInflight::ListRound);
                AgentBoardRead::InFlight
            }
            Err(AgentBoardReadRefusal::LaneUnavailable) => {
                AgentBoardRead::Failed(AgentBoardReadFailure::LaneUnavailable)
            }
            Err(AgentBoardReadRefusal::Rejected) => {
                AgentBoardRead::Failed(AgentBoardReadFailure::Transport)
            }
        };
    }

    fn issue_jump_get(
        &mut self,
        target: AgentSessionId,
        now: Instant,
        issue: impl FnOnce(AgentBoardReadRequest) -> Result<(), AgentBoardReadRefusal>,
    ) {
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
        self.shown = None;
        // The Get's own deadline sits inside the jump's total budget —
        // it cannot outlive either.
        let get_deadline = now
            .checked_add(AGENT_GET_DEADLINE)
            .map_or(deadline, |get| get.min(deadline));
        let request = AgentBoardReadRequest::Get {
            episode: self.episode(),
            serial,
            session: target,
            deadline: get_deadline,
        };
        match issue(request) {
            Ok(()) => self.inflight = Some(BoardInflight::JumpGet { serial }),
            Err(_) => self.fail_jump(JumpFailure::Unavailable),
        }
    }

    /// Settle one read event in the order the composition's lane delivered
    /// it. The list path is board-local; the Get path needs the caller's
    /// live-context preference to order candidates, so it takes that as a
    /// closure.
    pub(crate) fn settle_read(
        &mut self,
        event: AgentBoardReadEvent,
        now: Instant,
        prefer: &mut impl FnMut(&AgentPresentationAttachment) -> bool,
    ) {
        match event {
            AgentBoardReadEvent::Answer(AgentBoardReadAnswer::List { episode, result }) => {
                self.apply_list_outcome(episode, result, now);
            }
            AgentBoardReadEvent::Answer(AgentBoardReadAnswer::Get {
                episode,
                serial,
                result,
            }) => self.apply_get_outcome(episode, serial, result, prefer),
            AgentBoardReadEvent::Foreign => {
                self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
            }
            // Dead-lane honesty: the lane's end follows every answer it
            // queued, so it proves no answer will ever arrive for a residual
            // credit. Release that credit into the honest failure now —
            // never leave the board waiting on a lane that no longer exists.
            AgentBoardReadEvent::LaneEnded => match self.inflight.take() {
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
            },
        }
    }

    fn apply_list_outcome(
        &mut self,
        episode: AgentBoardEpisodeToken,
        result: Result<AgentListRound, AgentReadFailure>,
        now: Instant,
    ) {
        if episode != self.episode() || !matches!(self.inflight, Some(BoardInflight::ListRound)) {
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
                    let anchored = self.rows.get(self.scroll).map(|row| row.session_id.clone());
                    let old_scroll = self.scroll;
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
                    self.rows_revision = self.rows_revision.wrapping_add(1);
                    self.shown = None;
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
                    // A background refresh preserves the manual reading
                    // anchor independently of the logical selection. When
                    // that row vanished, retain the bounded viewport offset;
                    // never snap back to selection merely because data landed.
                    self.scroll = anchored
                        .and_then(|anchor| {
                            self.rows.iter().position(|row| row.session_id() == &anchor)
                        })
                        .unwrap_or(old_scroll);
                    self.rows_from_complete = round.complete;
                    self.rows_at = Some(now);
                    self.clamp_viewport();
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
                if round.complete && self.rows.is_empty() {
                    self.start_catalog_ready_epoch = Some(self.epoch);
                } else {
                    self.start_catalog_ready_epoch = None;
                    self.start.retire_catalog();
                }
            }
            Err(AgentReadFailure::Denied) => {
                {
                    self.start_catalog_ready_epoch = None;
                    self.start.retire_catalog();
                }
                self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Denied);
            }
            Err(AgentReadFailure::Unavailable) => {
                {
                    self.start_catalog_ready_epoch = None;
                    self.start.retire_catalog();
                }
                self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Unavailable);
            }
            Err(AgentReadFailure::Transport) => {
                {
                    self.start_catalog_ready_epoch = None;
                    self.start.retire_catalog();
                }
                self.read = AgentBoardRead::Failed(AgentBoardReadFailure::Transport);
            }
        }
    }

    /// Apply the verified Get outcome. `prefer` reports whether an
    /// attachment currently resolves into the caller's active context and
    /// tab — the deterministic pick's "current-tab usable" preference.
    fn apply_get_outcome(
        &mut self,
        episode: AgentBoardEpisodeToken,
        serial: u64,
        result: Result<AgentGetOutcome, AgentReadFailure>,
        prefer: &mut impl FnMut(&AgentPresentationAttachment) -> bool,
    ) {
        // The physical lane credit is bound to the outcome itself: a
        // matching serial always releases it — even when the jump that
        // issued it was superseded at request time or already failed.
        // What the credit must never do is clear a *different* jump:
        // that check happens only after the release.
        if episode != self.episode()
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
    pub(crate) fn advance_jump_candidate(&mut self) -> bool {
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
    pub(crate) fn expire_jump(&mut self, now: Instant) -> bool {
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
    pub(crate) fn fail_jump(&mut self, kind: JumpFailure) {
        self.notice = Some(kind);
        self.jump = None;
        self.shown = None;
    }

    /// The jump landed on the exact panel — the user is where they asked
    /// to be, so the board closes.
    pub(crate) fn jump_landed(&mut self) {
        self.jump = None;
        self.close();
    }

    /// Test seam: install a jump in a chosen stage without driving the
    /// lane — the UI drive tests stage navigation directly.
    #[cfg(test)]
    pub(crate) fn force_jump_t(&mut self, jump: BoardJump) {
        self.jump = Some(jump);
        self.shown = None;
    }

    /// Connection release: the start jobs are closed, so no catalog answer
    /// can still arrive for this connection.
    pub(crate) fn retire_start_catalog(&mut self) {
        self.start.retire_catalog();
    }
}

/// The renderer's read handle over a live `AgentBoard` — borrows the
/// board and the freshest Workbench presentation index inside
/// `WorkbenchClientView`, exposes rows and placement lookups without
/// handing out mutation.
#[derive(Clone, Copy)]
pub struct AgentBoardView<'a> {
    board: &'a AgentBoard,
    index: &'a PresentationIndex,
    attention: Option<&'a dyn AgentRowAttention>,
    names: Option<&'a dyn LivePlacementNames>,
}

impl<'a> AgentBoardView<'a> {
    pub(crate) const fn new(board: &'a AgentBoard, index: &'a PresentationIndex) -> Self {
        Self {
            board,
            index,
            attention: None,
            names: None,
        }
    }
    pub(crate) fn with_attention(mut self, attention: &'a dyn AgentRowAttention) -> Self {
        self.attention = Some(attention);
        self
    }
    /// Names rows after the Tab strip's live names wherever the view shows the row's Tab.
    pub(crate) fn with_names(mut self, names: &'a dyn LivePlacementNames) -> Self {
        self.names = Some(names);
        self
    }
    /// The row title and Tab name for an attachment's placement, by the Tab strip's rule.
    pub(crate) fn placement_names(
        &self,
        attachment: &AgentPresentationAttachment,
        placement: &PresentationPlacement,
        locale: crate::localization::Locale,
    ) -> (String, String) {
        placement.names(attachment.presentation_id.as_str(), self.names, locale)
    }
    pub(crate) fn unread(&self, row: &AgentBoardRow) -> bool {
        {
            self.attention
                .is_some_and(|attention| attention.session_unread(row.session_id()))
        }
    }
    pub(crate) fn display_status<'row>(
        &self,
        row: &'row AgentBoardRow,
    ) -> &'row AgentSemanticStatus {
        if matches!(
            row.status(),
            AgentSemanticStatus::Succeeded {} | AgentSemanticStatus::Idle {}
        ) && let Some(completion) = self.attention.and_then(|attention| {
            row.attachments()
                .iter()
                .find_map(|attachment| attention.presentation_completion(attachment))
        }) {
            match completion {
                orchbee_workbench_protocol::AgentCompletion::AwaitingInput => {
                    return &AgentSemanticStatus::Succeeded {};
                }
                orchbee_workbench_protocol::AgentCompletion::Acknowledged => {
                    return &AgentSemanticStatus::Idle {};
                }
                orchbee_workbench_protocol::AgentCompletion::None => {}
            }
        }
        row.status()
    }
    pub fn rows(&self) -> &'a [AgentBoardRow] {
        self.board.rows()
    }
    pub(crate) const fn episode(&self) -> AgentBoardEpisodeToken {
        self.board.episode()
    }
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.board.presentation
    }
    pub(crate) fn input_context(&self) -> AgentBoardInputContext {
        self.board
            .input_context()
            .expect("a AgentBoardView borrows an open Board")
    }
    /// Builds the exact successful-frame witness after the renderer has
    /// resolved the viewport. The owner supplies the source revision,
    /// logical selection proof and stable top-row identity in one value.
    pub(crate) fn actual_draw(&self, visible_rows: usize) -> Option<AgentBoardActualDraw> {
        self.board.actual_draw(visible_rows)
    }
    pub(crate) const fn close_binding(&self) -> crate::client_view::Binding {
        BOARD_CLOSE_KEY
    }
    pub(crate) const fn jump_binding(&self) -> crate::client_view::Binding {
        BOARD_JUMP_KEY
    }
    pub(crate) const fn refresh_binding(&self) -> crate::client_view::Binding {
        BOARD_REFRESH_KEY
    }
    pub(crate) const fn navigation_bindings(&self) -> [crate::client_view::Binding; 4] {
        [
            BOARD_SELECT_UP_KEYS[0],
            BOARD_SELECT_DOWN_KEYS[0],
            BOARD_SELECT_DOWN_KEYS[1],
            BOARD_SELECT_UP_KEYS[1],
        ]
    }
    pub(crate) const fn start_accept_binding(&self) -> crate::client_view::Binding {
        BOARD_START_ACCEPT_KEY
    }
    pub(crate) const fn start_provider_bindings(&self) -> [crate::client_view::Binding; 2] {
        [
            BOARD_START_PREVIOUS_PROVIDER_KEY,
            BOARD_START_NEXT_PROVIDER_KEY,
        ]
    }
    pub(crate) const fn start_edit_binding(&self) -> crate::client_view::Binding {
        BOARD_START_EDIT_KEY
    }
    pub(crate) const fn start_cancel_binding(&self) -> crate::client_view::Binding {
        BOARD_START_CANCEL_KEY
    }
    pub(crate) const fn selection(&self) -> usize {
        self.board.selection()
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
    pub(crate) fn start_catalog_status(&self) -> start::CatalogStatus {
        self.board.start.catalog_status()
    }
    pub(crate) fn start_providers(&self) -> &[&'static str] {
        self.board.start.providers()
    }
    pub(crate) fn detected_start_providers(&self) -> &[&'static str] {
        self.board.start.detected()
    }
    pub(crate) fn selected_start_provider(&self) -> Option<&'static str> {
        self.board.start.selected_provider()
    }
    pub(crate) fn start_name(&self) -> &str {
        self.board.start.name()
    }
    pub(crate) const fn start_name_cursor(&self) -> usize {
        self.board.start.cursor()
    }
    pub(crate) fn start_phase(&self) -> Option<start::Phase> {
        self.board.start.phase(self.board.epoch)
    }
    pub(crate) fn start_running(&self) -> bool {
        self.board.start_running()
    }
    pub(crate) fn start_operation_identity(
        &self,
    ) -> Option<(
        &orchbee_agent_observation_protocol::AgentName,
        &AgentProviderId,
    )> {
        self.board.start.operation_identity(self.board.epoch)
    }
    pub(crate) fn can_start_agent(&self) -> bool {
        self.board.start.can_start()
    }
    pub(crate) fn start_draft_editable(&self) -> bool {
        self.board.start.draft_editable()
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

impl From<AgentStartPhase> for crate::localization::AgentStartStatus {
    fn from(phase: AgentStartPhase) -> Self {
        match phase {
            AgentStartPhase::Starting => Self::Starting,
            AgentStartPhase::WaitingInput => Self::WaitingInput,
            AgentStartPhase::Unconfirmed => Self::Unconfirmed,
            AgentStartPhase::Cancelling => Self::Cancelling,
            AgentStartPhase::Started => Self::Started,
            AgentStartPhase::Failed => Self::Failed,
            AgentStartPhase::Unknown => Self::Unknown,
            AgentStartPhase::Cancelled => Self::Cancelled,
        }
    }
}
