//! The Start Agent picker's overlay owner and the Workbench presentation index.
//!
//! The owner holds one open picker episode: its exact-draw proof, the Session's Agent catalog and
//! start state the picker lists from, and which Panels the Session's Agents hold. The index maps
//! each Presentation to its Workbench placement for notification names and terminal titles.
//! Canonical Agent state stays Server-owned; nothing here synthesizes status of its own.

use crate::tui_presentation::{GeometrySettlement, PresentationPhase};

mod names;
pub(crate) use names::{LivePlacementNames, NameFacts};
pub(crate) mod picker;
pub(crate) mod session_agents;
pub(crate) mod start;
#[cfg(test)]
mod tests;
pub(crate) use session_agents::SessionAgents;
pub(crate) use start::CatalogEvent as AgentCatalogEvent;

/// Stable identity for one open picker incarnation.
///
/// Query, cursor and catalog changes deliberately do not advance this token. They belong to the
/// exact draw witness below; late work from another open incarnation remains excluded by this
/// episode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentBoardEpisodeToken(u64);

impl AgentBoardEpisodeToken {
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// Value-semantic source behind one picker draw.
#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentBoardDrawSource {
    episode: AgentBoardEpisodeToken,
    picker: picker::DrawSource,
}

/// Exact successful-paint evidence for one picker frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentBoardActualDraw {
    token: AgentBoardToken,
    visible_rows: usize,
}

impl AgentBoardActualDraw {
    pub(crate) const fn episode(&self) -> AgentBoardEpisodeToken {
        self.token.source.episode
    }

    pub(crate) const fn token(&self) -> &AgentBoardToken {
        &self.token
    }

    pub(crate) const fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    pub(crate) fn control_target(&self, control: AgentBoardControl) -> AgentBoardControlTarget {
        AgentBoardControlTarget {
            token: self.token.clone(),
            control,
        }
    }
}

/// What the renderer actually did with one exact picker episode.
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

    /// Whether a complete frame for this episode still holds its visibility lease. The
    /// compositor separately decides whether the picker is the top owner. Together those facts
    /// permit owner-local keys; activations and pointer effects also require exact current draw
    /// proof.
    pub(crate) const fn interactive(self) -> bool {
        self.interactive
    }
}

/// A workbench placement resolved from a `TerminalPresentationId`: which
/// context owns it, which tab, which panel — plus the display labels the
/// snapshot carried at index-build time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PresentationPlacement {
    pub(crate) location: crate::workbench_projection::ClientContextLocation,
    pub(crate) tab_id: orchbee_workbench_protocol::WorkbenchTabId,
    pub(crate) panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
    pub(crate) tab_label: String,
    pub(crate) panel_name: NameFacts,
    pub(crate) panel_position: usize,
    /// The attached Presentation's naming facts for the Tab strip's rule.
    pub(crate) presentation_name: NameFacts,
    pub(crate) project_label: Option<String>,
    pub(crate) workspace_label: Option<String>,
}

/// `presentation_id → placement` over the latest observed Workbench
/// snapshot. Rebuilt wholesale on every snapshot-bearing worker event —
/// the index is a function of canonical state, never a second authority.
#[derive(Default)]
pub(crate) struct PresentationIndex {
    placements: std::collections::HashMap<String, PresentationPlacement>,
    /// Every bound Presentation in canonical Workspace, Tab, Panel and member order.
    bound: Vec<orchbee_workbench_protocol::TerminalPresentationId>,
}

impl PresentationIndex {
    /// Rebuild the whole index from a freshly arrived canonical snapshot:
    /// every navigation location's tabs, every panel's presentation id.
    /// Called only after the snapshot passed the session's acceptance
    /// fences — the index is a function of accepted canonical state.
    pub(crate) fn rebuild(snapshot: &orchbee_workbench_protocol::Snapshot) -> Self {
        let mut index = Self::default();
        for location in navigation_locations(snapshot) {
            if let Ok(tabs) =
                crate::workbench_projection::project_native_context_tabs(snapshot, &location)
            {
                index.install_context(&tabs);
            }
        }
        index
    }

    /// Fold one context's projected tabs into the index (used for the
    /// bootstrap cache, which arrives already projected rather than as a
    /// raw snapshot).
    pub(crate) fn install_context(
        &mut self,
        tabs: &crate::workbench_projection::ClientContextTabs,
    ) {
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
                    if member.binding().is_some() {
                        self.bound.push(member.presentation_id().clone());
                    }
                    self.placements.insert(
                        member.presentation_id().as_str().to_owned(),
                        PresentationPlacement {
                            location: tabs.location().clone(),
                            tab_id: tab.tab_id().clone(),
                            panel_id: panel.panel_id().clone(),
                            tab_label: tab.display_label(tab_index).into_owned(),
                            panel_name: NameFacts::of(panel.primary()),
                            panel_position: panel_index + 1,
                            presentation_name: NameFacts::of(member),
                            project_label: project_label.clone(),
                            workspace_label: workspace_label.clone(),
                        },
                    );
                }
            }
        }
    }

    /// Resolve one stable presentation identity to a live placement — identity only; labels are
    /// never used to locate.
    pub(crate) fn placement_by_presentation(
        &self,
        presentation_id: &str,
    ) -> Option<&PresentationPlacement> {
        self.placements.get(presentation_id)
    }

    /// Every bound Presentation of every indexed Workspace, in canonical order: the ones whose
    /// terminal titles the Server publishes.
    pub(crate) fn bound_presentations(
        &self,
    ) -> &[orchbee_workbench_protocol::TerminalPresentationId] {
        &self.bound
    }

    /// A Presentation's binding: `None` when no indexed Workspace holds it, `Some(None)` while it
    /// is unbound.
    pub(crate) fn binding(
        &self,
        presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> Option<Option<&crate::workbench_projection::ClientTerminalBinding>> {
        self.placements
            .get(presentation.as_str())
            .map(|placement| placement.presentation_name.binding.as_ref())
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardControl {
    Close,
    /// One picker row, by its index in the drawn picker rows.
    Pick(usize),
    /// The picker's place row; a click moves to the next place.
    Place,
}

/// Exact picker control painted into one admitted frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentBoardControlTarget {
    token: AgentBoardToken,
    control: AgentBoardControl,
}

impl AgentBoardControlTarget {
    #[cfg(test)]
    pub(crate) const fn for_test(
        episode: AgentBoardEpisodeToken,
        control: AgentBoardControl,
    ) -> Self {
        Self {
            token: AgentBoardToken {
                source: AgentBoardDrawSource {
                    episode,
                    picker: picker::DrawSource::for_test(),
                },
                scroll: 0,
            },
            control,
        }
    }

    pub(crate) const fn token(&self) -> &AgentBoardToken {
        &self.token
    }

    pub(crate) const fn control(&self) -> AgentBoardControl {
        self.control
    }
}

/// The Start Agent picker's owner: open exactly while it holds a picker.
pub(crate) struct AgentBoard {
    /// Bump on every open/close — a draw receipt carrying an older episode is stale by
    /// construction and dropped.
    epoch: u64,
    presentation: PresentationPhase,
    /// Whole-surface visibility lease for ordinary owner-local keys. Only an exact successful
    /// paint grants it; cover, geometry block, reopen and close revoke it. Semantic state changes
    /// invalidate `shown` activation proof without pretending the still-visible picker vanished.
    visible: bool,
    shown: Option<AgentBoardActualDraw>,
    start: start::State,
    /// An opened picker asked for a new lookup; it holds until the composition sends it.
    catalog_refresh: bool,
    picker: Option<picker::Picker>,
    session_agents: SessionAgents,
}

impl AgentBoard {
    pub(crate) const fn episode(&self) -> AgentBoardEpisodeToken {
        AgentBoardEpisodeToken(self.epoch)
    }

    fn draw_source(&self) -> Option<AgentBoardDrawSource> {
        Some(AgentBoardDrawSource {
            episode: self.episode(),
            picker: self.picker_draw_source()?,
        })
    }

    fn token(&self) -> Option<AgentBoardToken> {
        Some(AgentBoardToken {
            source: self.draw_source()?,
            scroll: self.picker.as_ref()?.scroll(),
        })
    }

    fn current_draw(&self) -> Option<&AgentBoardActualDraw> {
        let draw = self.shown.as_ref()?;
        (Some(&draw.token) == self.token().as_ref()).then_some(draw)
    }

    pub(crate) fn input_context(&self) -> Option<AgentBoardInputContext> {
        self.is_open().then_some(AgentBoardInputContext {
            episode: self.episode(),
            presentation: self.presentation,
            interactive: self.interactive(),
        })
    }

    fn interactive(&self) -> bool {
        self.is_open() && self.presentation.interactive() && self.visible
    }

    fn actual_draw(&self, visible_rows: usize) -> Option<AgentBoardActualDraw> {
        if visible_rows == 0 {
            return None;
        }
        let token = AgentBoardToken {
            source: self.draw_source()?,
            scroll: self.picker_first_row(visible_rows),
        };
        Some(AgentBoardActualDraw {
            token,
            visible_rows,
        })
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
        self.picker_scroll_by(if up { -lines } else { lines });
        self.token()
    }

    pub(crate) fn new(launch_supported: bool) -> Self {
        Self {
            epoch: 0,
            presentation: PresentationPhase::Pending,
            visible: false,
            shown: None,
            start: start::State::new(launch_supported),
            catalog_refresh: false,
            picker: None,
            session_agents: SessionAgents::default(),
        }
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.picker.is_some()
    }

    /// Capture presence is synchronous from opening: a key arriving before the first paint still
    /// belongs to the picker and never falls through. Activation remains separately gated by
    /// exact successful-draw proof.
    #[cfg(test)]
    pub(crate) const fn owns_input(&self) -> bool {
        self.is_open()
    }

    /// Starts a new open episode around `picker`.
    fn open_with(&mut self, picker: picker::Picker) {
        self.epoch += 1;
        self.presentation = PresentationPhase::Pending;
        self.visible = false;
        self.shown = None;
        self.picker = Some(picker);
    }

    /// Ends the open episode. The epoch bump invalidates every draw receipt the episode still
    /// owes.
    pub(crate) fn close(&mut self) {
        if self.picker.take().is_some() {
            self.epoch += 1;
        }
        self.visible = false;
        self.shown = None;
    }

    /// Settles exact paint evidence. `None` means another composed surface covered the picker:
    /// revoke current-frame authority but retain the open episode. `NotPresented` is geometry
    /// evidence and therefore refuses only a never-presented episode.
    pub(crate) fn settle_draw(
        &mut self,
        draw: Option<AgentBoardDraw>,
    ) -> Option<GeometrySettlement> {
        let Some(draw) = draw else {
            self.visible = false;
            self.shown = None;
            return None;
        };
        if !self.is_open() || draw.episode() != self.episode() {
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
                let first = self.picker_first_row(visible);
                if Some(&actual.token.source) != self.draw_source().as_ref()
                    || actual.token.scroll != first
                {
                    return None;
                }
                self.settle_picker_viewport(visible, first);
                self.presentation.presented();
                self.visible = true;
                self.shown = Some(actual);
                None
            }
        }
    }

    /// One start drain: `jobs` settles its outcomes into the start state.
    pub(crate) fn drain_start(
        &mut self,
        now: std::time::Instant,
        jobs: impl FnOnce(&mut start::State) -> start::Drain,
    ) -> bool {
        let expired = self.start.expire(now);
        self.hold_picker_cursor();
        let drained = jobs(&mut self.start);
        drained.changed || expired
    }

    /// One report of the Session's catalog stream. Returns whether anything listed changed; a
    /// resting picker cursor stays on its provider.
    pub(crate) fn receive_agent_catalog(&mut self, event: AgentCatalogEvent) -> bool {
        self.hold_picker_cursor();
        self.start.receive_catalog(event)
    }

    /// The start state the composition's jobs run against.
    pub(crate) fn start_state(&mut self) -> &mut start::State {
        &mut self.start
    }

    /// A startable Session's catalog after a completed lookup that found `found`.
    #[cfg(test)]
    pub(crate) fn install_start_catalog_t(&mut self, found: &[&'static str]) {
        self.hold_picker_cursor();
        self.start.install_catalog(found);
    }

    #[cfg(test)]
    pub(crate) fn accept_start_without_task_t(&mut self) {
        self.start.accept_without_task();
    }

    #[cfg(test)]
    pub(crate) fn start_operation_panel_t(
        &self,
    ) -> Option<&orchbee_workbench_protocol::WorkbenchPanelId> {
        self.start.latest().map(|(_, panel)| panel)
    }

    /// Connection release: the catalog stream is closed, so nothing held belongs to a connection
    /// any more.
    pub(crate) fn retire_start_catalog(&mut self) {
        self.start.receive_catalog(AgentCatalogEvent::Reset);
        self.catalog_refresh = false;
    }
}

/// The renderer's read handle over an open picker owner, without handing out mutation.
#[derive(Clone, Copy)]
pub struct AgentBoardView<'a> {
    board: &'a AgentBoard,
}

impl<'a> AgentBoardView<'a> {
    pub(crate) const fn new(board: &'a AgentBoard) -> Self {
        Self { board }
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
            .expect("a AgentBoardView borrows an open picker")
    }
    /// Builds the exact successful-frame witness after the renderer has resolved the viewport.
    pub(crate) fn actual_draw(&self, visible_rows: usize) -> Option<AgentBoardActualDraw> {
        self.board.actual_draw(visible_rows)
    }
    pub(crate) fn picker_status(&self) -> Option<picker::Status> {
        self.board.picker_status()
    }
    /// The product name and command line the Session's catalog gives `id`.
    pub(crate) fn start_agent_text(&self, id: &str) -> Option<(&'a str, &'a str)> {
        self.board.start_agent_text(id)
    }
    pub(crate) const fn picker(&self) -> Option<&'a picker::Picker> {
        self.board.picker.as_ref()
    }
    pub(crate) fn picker_items(&self) -> Vec<picker::Item> {
        self.board.picker_items()
    }
    pub(crate) const fn can_start_agent(&self) -> bool {
        self.board.can_start_agent()
    }
}
