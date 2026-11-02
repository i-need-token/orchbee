//! The action-menu episode: the menu a person opened with its confirmation stage, the parents of an
//! open submenu, the observed geometry, the Tab-close resume handoff, and the sidebar worktree
//! discovery that the open menu waits on.
//!
//! This module is the episode's only writer. The UI composition evaluates availability and target
//! validity against the other owners, advances navigation, and drives the worktree lane.

use crate::action_menu::{
    ActionMenu, MenuAction, MenuAnchor, MenuInputContext, MenuTarget, MenuView, RetainedMenu,
};
use crate::tui_capabilities::Capabilities;
use crate::workbench_projection::{ClientContextLocation, ClientTab, MenuContextSource};
use crate::workbench_state::worktree_launch::addressed::{
    AddressedWorktreeSource, SidebarDiscovery,
};
use crate::workbench_state::{PaneKey, PaneRef, TabCloseResumePermit, WorkbenchClientState};
use orchbee_client_server_protocol::Geometry;
use std::time::{Duration, Instant};
mod tab_source;
pub(super) use tab_source::TabResolverSource;

/// The Client facts a menu opened against; a changed origin retires an unconfirmed menu.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Origin {
    Unavailable,
    Context(Box<ContextOrigin>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ContextOrigin {
    selected: Vec<ClientTab>,
    groups: Vec<orchbee_workbench_protocol::TabGroup>,
    session: orchbee_workbench_protocol::SessionId,
    location: ClientContextLocation,
    desired: Option<PaneKey>,
    focus: Option<PaneRef>,
    tab: Option<ClientTab>,
    source: Option<MenuContextSource>,
}

impl Origin {
    pub(super) fn capture(state: Option<&WorkbenchClientState>) -> Self {
        state.map_or(Self::Unavailable, |state| {
            Self::Context(Box::new(ContextOrigin {
                selected: state
                    .selected_tab_ids()
                    .iter()
                    .filter_map(|id| state.cache().tab_index(id))
                    .filter_map(|index| state.cache().tabs().get(index))
                    .cloned()
                    .collect(),
                groups: state.cache().groups().to_vec(),
                session: state.cache().session_id().clone(),
                location: state.cache().location().clone(),
                desired: state.desired().cloned(),
                focus: state.focus_pane_ref(),
                tab: state
                    .desired()
                    .and_then(|key| state.cache().tab_index(key.anchor().tab_id()))
                    .and_then(|i| state.cache().tabs().get(i))
                    .cloned(),
                source: state.cache().menu_context_source(state.cache().location()),
            }))
        })
    }
}

/// The subject a menu acts on, with the canonical facts that prove it unchanged.
#[derive(Clone)]
pub(super) enum Target {
    Global,
    Pane {
        pane: PaneRef,
        presentation: orchbee_workbench_protocol::TerminalPresentationId,
        source: MenuContextSource,
    },
    Tab {
        location: ClientContextLocation,
        tab: Box<ClientTab>,
        source: MenuContextSource,
        resolver: TabResolverSource,
    },
    Workspace {
        location: ClientContextLocation,
        source: MenuContextSource,
    },
    Project {
        project: Box<orchbee_workbench_protocol::Project>,
        context: Option<Box<ProjectContext>>,
    },
}

#[derive(Clone)]
pub(super) struct ProjectContext {
    pub(super) location: ClientContextLocation,
    pub(super) source: MenuContextSource,
}

pub(super) enum Stage {
    Open,
    Confirmed(Instant),
    Context {
        generation: u64,
        prepared: bool,
        deadline: Instant,
    },
    Tab {
        generation: u64,
        prepared: bool,
        deadline: Instant,
    },
    Wait(Origin, Instant),
    Offered(Origin, Instant),
}

impl Stage {
    pub(super) fn deadline(&self) -> Option<Instant> {
        match self {
            Self::Open => None,
            Self::Confirmed(deadline)
            | Self::Context { deadline, .. }
            | Self::Tab { deadline, .. }
            | Self::Wait(_, deadline)
            | Self::Offered(_, deadline) => Some(*deadline),
        }
    }
}

/// Refresh and attachment generations belong to separate monotonic domains.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::workbench_client::ui) enum NavigationKey {
    Context(u64),
    Tab(u64),
}

/// The open menu's subject, the origin it opened against, and its confirmation stage.
pub(super) struct Intent {
    capabilities: Capabilities,
    entry: MenuTarget,
    target: Target,
    origin: Origin,
    stage: Stage,
    zoomed: bool,
    pointer_released: bool,
    resume_permit: Option<TabCloseResumePermit>,
    project_choices: Option<Vec<orchbee_workbench_protocol::Project>>,
}

impl Intent {
    /// An open menu that captures its origin and zoom from `state`.
    pub(super) fn open(
        capabilities: Capabilities,
        entry: MenuTarget,
        target: Target,
        state: Option<&WorkbenchClientState>,
        pointer_released: bool,
        resume_permit: Option<TabCloseResumePermit>,
        project_choices: Option<Vec<orchbee_workbench_protocol::Project>>,
    ) -> Self {
        Self {
            capabilities,
            entry,
            target,
            origin: Origin::capture(state),
            stage: Stage::Open,
            zoomed: state.is_some_and(WorkbenchClientState::zoom_active),
            pointer_released,
            resume_permit,
            project_choices,
        }
    }

    pub(super) const fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub(super) const fn entry(&self) -> &MenuTarget {
        &self.entry
    }

    pub(super) const fn target(&self) -> &Target {
        &self.target
    }

    pub(super) const fn origin(&self) -> &Origin {
        &self.origin
    }

    pub(super) const fn stage(&self) -> &Stage {
        &self.stage
    }

    pub(super) const fn zoomed(&self) -> bool {
        self.zoomed
    }

    pub(super) const fn pointer_released(&self) -> bool {
        self.pointer_released
    }

    pub(super) const fn resume_permit(&self) -> Option<&TabCloseResumePermit> {
        self.resume_permit.as_ref()
    }

    pub(super) fn project_choices(&self) -> Option<&[orchbee_workbench_protocol::Project]> {
        self.project_choices.as_deref()
    }

    /// Whether the navigation that `navigation` names belongs to this intent.
    pub(super) fn owns_navigation(&self, navigation: NavigationKey) -> bool {
        matches!(
            (&self.stage, navigation),
            (Stage::Context { generation: owned, .. }, NavigationKey::Context(generation))
                | (Stage::Tab { generation: owned, .. }, NavigationKey::Tab(generation))
                if *owned == generation
        )
    }

    /// Whether this intent owns `navigation` and its candidate was prepared.
    pub(super) fn navigation_prepared(&self, navigation: NavigationKey) -> bool {
        matches!(
            (&self.stage, navigation),
            (
                Stage::Context { generation: owned, prepared: true, .. },
                NavigationKey::Context(generation)
            ) | (
                Stage::Tab { generation: owned, prepared: true, .. },
                NavigationKey::Tab(generation)
            ) if *owned == generation
        )
    }

    /// Moves a confirmed or waiting intent on; the composition chooses `next` from the confirmed
    /// action's subject and the navigation it started.
    pub(super) fn advance(&mut self, next: Stage) {
        self.stage = next;
    }
}

/// A menu beneath an open submenu, restored when the submenu returns.
pub(super) struct ParentMenu {
    pub(super) target: MenuTarget,
    pub(super) anchor: Option<MenuAnchor>,
    pub(super) selected: MenuAction,
    pub(super) query: Option<String>,
    pub(super) resume_permit: Option<TabCloseResumePermit>,
    /// The painted parent of a cascaded context submenu; a replaced page retains none.
    pub(super) retained: Option<RetainedMenu>,
}

/// A confirmed Resume Tab Close, handed to the action that executes it.
pub(super) struct ResumeHandoff {
    pub(super) deadline: Instant,
    pub(super) location: ClientContextLocation,
    pub(super) source: MenuContextSource,
    pub(super) tab: Box<ClientTab>,
    pub(super) permit: TabCloseResumePermit,
}

/// The worktree discovery that a sidebar menu waits on, correlated by its own identity.
pub(super) struct Discovery {
    id: u64,
    source: AddressedWorktreeSource,
    deadline: Instant,
    state: DiscoveryState,
    /// The last success for this source the rows show until `state` is answered, kept only while
    /// the first terminal stands in the checkout it describes.
    earlier: Option<DiscoveryState>,
    /// The menu's row showed a branch for this directory or one above it, so New Worktree… stays
    /// enabled until the first answer.
    known_git: bool,
}

pub(super) enum DiscoveryState {
    Waiting,
    Querying,
    Ready(Box<SidebarDiscovery>),
    Absent,
}

impl Discovery {
    pub(super) const fn id(&self) -> u64 {
        self.id
    }

    pub(super) const fn source(&self) -> &AddressedWorktreeSource {
        &self.source
    }

    pub(super) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) const fn state(&self) -> &DiscoveryState {
        &self.state
    }

    /// Whether New Worktree… stays enabled until the first answer.
    pub(super) const fn known_git(&self) -> bool {
        self.known_git
    }

    /// The answer about where the first terminal stands now, which an action runs against.
    pub(super) fn ready(&self) -> Option<&SidebarDiscovery> {
        match &self.state {
            DiscoveryState::Ready(discovery) => Some(discovery),
            _ => None,
        }
    }

    /// What the rows show: the current answer or, until it arrives, the earlier one.
    pub(super) fn shown(&self) -> &DiscoveryState {
        match (&self.state, &self.earlier) {
            (DiscoveryState::Waiting | DiscoveryState::Querying, Some(earlier)) => earlier,
            (state, _) => state,
        }
    }

    /// The checkouts the rows list, under the same rule as `shown`.
    pub(super) fn recognized(&self) -> Option<&SidebarDiscovery> {
        match self.shown() {
            DiscoveryState::Ready(discovery) => Some(discovery),
            _ => None,
        }
    }

    /// Whether the seed still speaks for `next`, where the first terminal moved.
    fn known_git_for(&self, next: &AddressedWorktreeSource) -> bool {
        self.known_git
            && self
                .source
                .stands_in()
                .is_some_and(|seeded| next.stands_under(seeded))
    }

    fn settle(&mut self, state: DiscoveryState) {
        self.state = state;
        self.earlier = None;
    }
}

/// Recent sources whose last success a later menu may show, across menus and moves.
const REMEMBERED_ANSWERS: usize = 16;

/// Whether `answer` still speaks for `next`: the same source, standing in the checkout it describes.
/// A failed answer speaks only for its own directory, so it is never kept.
fn speaks_for(answer: &SidebarDiscovery, next: &AddressedWorktreeSource) -> bool {
    answer.addressed.moved_to(next) && next.stands_under(&answer.discovery.source.checkout_path)
}

#[derive(Default)]
pub(in crate::workbench_client::ui) struct MenuEpisode {
    pub(super) menu: ActionMenu,
    intent: Option<Intent>,
    geometry: Option<Geometry>,
    parents: Vec<ParentMenu>,
    resume_handoff: Option<ResumeHandoff>,
    discovery: Option<Discovery>,
    next_discovery: u64,
    /// The newest success for each of the most recent sources, newest first.
    remembered: Vec<SidebarDiscovery>,
    /// Where the sidebar row of the menu about to open stands, while it shows a branch for it.
    sidebar_git: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
}

impl MenuEpisode {
    pub(super) fn view(&self) -> Option<MenuView<'_>> {
        self.intent.as_ref()?;
        self.menu.view().map(|mut menu| {
            menu.has_parent = !self.parents.is_empty();
            if let Some(parent) = self.parents.first() {
                menu.context_menu = parent.target.context_menu();
            }
            menu
        })
    }

    /// The painted parents of a cascade, root first; empty while a child replaces its parent.
    pub(super) fn cascade(&self) -> Vec<MenuView<'_>> {
        if self.intent.is_none() {
            return Vec::new();
        }
        self.parents
            .iter()
            .enumerate()
            .map(|(level, parent)| parent.retained.as_ref().map(|menu| menu.view(level > 0)))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    /// The target and anchor of the menu a person opened, beneath any submenu.
    pub(super) fn root(&self) -> Option<(&MenuTarget, Option<MenuAnchor>)> {
        self.intent.as_ref()?;
        match self.parents.first() {
            Some(parent) => Some((&parent.target, parent.anchor)),
            None => self.menu.view().map(|menu| (menu.source, menu.anchor)),
        }
    }

    pub(super) fn input_context(&self) -> Option<MenuInputContext> {
        let intent = self.intent.as_ref()?;
        let context = self.menu.input_context()?;
        Some(if matches!(intent.stage, Stage::Open) {
            context
        } else {
            context.recovery_only()
        })
    }

    pub(super) const fn intent(&self) -> Option<&Intent> {
        self.intent.as_ref()
    }

    /// The open menu for its own transitions, beside the episode facts its rows read.
    pub(super) fn menu_with_facts(
        &mut self,
    ) -> (&mut ActionMenu, Option<&Intent>, Option<&Discovery>) {
        (
            &mut self.menu,
            self.intent.as_ref(),
            self.discovery.as_ref(),
        )
    }

    /// Records the geometry a frame used; a change revokes the painted menu's activation proof.
    pub(super) fn observe_geometry(&mut self, geometry: Geometry) {
        if self
            .geometry
            .replace(geometry)
            .is_some_and(|old| old != geometry)
        {
            self.menu.not_presented();
        }
    }

    /// Adopts `latest` when no frame has reported geometry; reports whether geometry is known.
    pub(super) fn ensure_geometry(&mut self, latest: impl FnOnce() -> Option<Geometry>) -> bool {
        if self.geometry.is_none() {
            self.geometry = latest();
        }
        self.geometry.is_some()
    }

    /// Opens the episode of a menu whose rows are already published.
    pub(super) fn begin(&mut self, intent: Intent) {
        self.intent = Some(intent);
    }

    /// Closes the menu and everything the episode retained for it.
    pub(super) fn cancel(&mut self) {
        self.menu.cancel();
        self.intent = None;
        self.parents.clear();
        self.resume_handoff = None;
        self.discovery = None;
    }

    /// Ends the episode after its menu closed; a resume handoff stays for its action.
    pub(super) fn end(&mut self) {
        self.intent = None;
        self.parents.clear();
        self.discovery = None;
    }

    /// Drops the intent while its menu is replaced by a parent or child page.
    pub(super) fn release_intent(&mut self) {
        self.intent = None;
    }

    /// Moves the intent out while the composition validates it against other owners.
    pub(super) fn take_intent(&mut self) -> Option<Intent> {
        self.intent.take()
    }

    /// Returns a validated intent; a menu that closed meanwhile keeps none.
    pub(super) fn restore_intent(&mut self, intent: Intent) {
        if self.menu.active() {
            self.intent = Some(intent);
        }
    }

    /// Confirms the open intent's candidate until `deadline`.
    pub(super) fn confirm(&mut self, deadline: Instant) {
        if let Some(intent) = &mut self.intent
            && matches!(intent.stage, Stage::Open)
        {
            intent.stage = Stage::Confirmed(deadline);
        }
    }

    /// Marks the owned navigation candidate prepared.
    pub(super) fn mark_navigation_prepared(&mut self) {
        if let Some(Intent {
            stage: Stage::Context { prepared, .. } | Stage::Tab { prepared, .. },
            ..
        }) = &mut self.intent
        {
            *prepared = true;
        }
    }

    /// Waits for the installed navigation to settle at `expected` before offering the action.
    pub(super) fn await_origin(&mut self, expected: Origin) {
        if let Some(intent) = &mut self.intent
            && let Some(deadline) = intent.stage.deadline()
        {
            intent.stage = Stage::Wait(expected, deadline);
        }
    }

    /// Names the open menu and the painted parents of its cascade `title`, which all address one
    /// Workspace; reports whether the menu needs a new draw.
    pub(super) fn set_workspace_title(&mut self, title: &str) -> bool {
        for parent in &mut self.parents {
            if let Some(retained) = &mut parent.retained {
                retained.set_title(title);
            }
        }
        self.menu.set_title(title)
    }

    pub(super) fn push_parent(&mut self, parent: ParentMenu) {
        self.parents.push(parent);
    }

    pub(super) fn pop_parent(&mut self) -> Option<ParentMenu> {
        self.parents.pop()
    }

    pub(super) fn clear_parents(&mut self) {
        self.parents.clear();
    }

    pub(super) fn clear_resume_handoff(&mut self) {
        self.resume_handoff = None;
    }

    pub(super) fn take_resume_handoff(&mut self) -> Option<ResumeHandoff> {
        self.resume_handoff.take()
    }

    /// Closes the menu after taking its confirmed action; the handoff travels with that action.
    pub(super) fn finish(&mut self, handoff: Option<ResumeHandoff>) {
        self.end();
        self.resume_handoff = handoff;
    }

    pub(super) const fn discovery(&self) -> Option<&Discovery> {
        self.discovery.as_ref()
    }

    pub(super) fn clear_discovery(&mut self) {
        self.discovery = None;
    }

    pub(super) fn observe_sidebar_git(
        &mut self,
        directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    ) {
        self.sidebar_git = directory;
    }

    /// Starts discovering `source`, keeping from before its first terminal moved only what still
    /// speaks for where it stands; false when its identity or deadline is unrepresentable. The
    /// clock is read only once a new identity exists.
    pub(super) fn begin_discovery(
        &mut self,
        source: AddressedWorktreeSource,
        now: impl FnOnce() -> Instant,
    ) -> bool {
        let Some(id) = self.next_discovery.checked_add(1) else {
            return false;
        };
        let Some(deadline) = now().checked_add(Duration::from_secs(10)) else {
            return false;
        };
        self.next_discovery = id;
        let moved = self
            .discovery
            .take()
            .filter(|discovery| discovery.source.moved_to(&source));
        let seeded = self
            .sidebar_git
            .take()
            .is_some_and(|directory| source.stands_in() == Some(&directory));
        let known_git = seeded
            || moved
                .as_ref()
                .is_some_and(|moved| moved.known_git_for(&source));
        let earlier = self
            .remembered
            .iter()
            .find(|answer| speaks_for(answer, &source))
            .map(|answer| DiscoveryState::Ready(Box::new(answer.clone())));
        self.discovery = Some(Discovery {
            id,
            source,
            deadline,
            state: DiscoveryState::Waiting,
            earlier,
            known_git,
        });
        true
    }

    /// Gives up a discovery still pending at its deadline; reports whether it did.
    pub(super) fn expire_discovery(&mut self, now: Instant) -> bool {
        let Some(discovery) = self.discovery.as_mut().filter(|discovery| {
            now >= discovery.deadline
                && matches!(
                    discovery.state,
                    DiscoveryState::Waiting | DiscoveryState::Querying
                )
        }) else {
            return false;
        };
        discovery.settle(DiscoveryState::Absent);
        true
    }

    /// Records that the worktree lane accepted the discovery query.
    pub(super) fn mark_discovery_querying(&mut self) {
        if let Some(discovery) = self.discovery.as_mut() {
            discovery.state = DiscoveryState::Querying;
        }
    }

    /// Settles the querying discovery `id`: a result for its source, before its deadline, that
    /// `valid` admits becomes ready, and anything else absent. The outcome replaces what is
    /// remembered for the source. Reports whether `id` was current.
    pub(super) fn settle_discovery(
        &mut self,
        id: u64,
        result: Option<Box<SidebarDiscovery>>,
        now: impl Fn() -> Instant,
        valid: impl Fn(&SidebarDiscovery) -> bool,
    ) -> bool {
        let Some(discovery) = self.discovery.as_mut().filter(|discovery| {
            discovery.id == id && matches!(discovery.state, DiscoveryState::Querying)
        }) else {
            return false;
        };
        let settled = match result {
            Some(found)
                if now() < discovery.deadline
                    && found.addressed == discovery.source
                    && valid(&found) =>
            {
                DiscoveryState::Ready(found)
            }
            _ => DiscoveryState::Absent,
        };
        let target = &discovery.source.target;
        self.remembered
            .retain(|answer| &answer.addressed.target != target);
        if let DiscoveryState::Ready(found) = &settled {
            self.remembered.insert(0, found.as_ref().clone());
            self.remembered.truncate(REMEMBERED_ANSWERS);
        }
        discovery.settle(settled);
        true
    }

    #[cfg(test)]
    pub(super) fn parent_count(&self) -> usize {
        self.parents.len()
    }

    #[cfg(test)]
    pub(super) fn forget_geometry(&mut self) {
        self.geometry = None;
    }
}
