//! Identity-based machine selection and painted input ownership; no storage or network effects.
use crate::client_view::{KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::localization::Locale;
use crate::machine_discovery::overview::{Context, Observation};
use crate::machines::{Profile, ProfileId};
use crate::startup_directory::ClientStartupTarget;
use crate::workbench_projection::{ClientContextTarget, ClientMemberTarget};
use std::{num::NonZeroU64, sync::Arc};
mod agents;
mod navigation;
mod pointer;
mod sidebar;
mod spaces;
mod workspace_query;
pub use agents::Control;
use agents::Navigation;
pub use navigation::SidebarTarget;
pub(crate) use navigation::{
    DrawnSidebarFixedTarget, DrawnSidebarTarget, SIDEBAR_BINDINGS, SidebarActivation,
    SidebarCommand, SidebarFixedAction, SidebarFixedActivation, SidebarIntent, SidebarPart,
    SidebarRow, SidebarSource, SidebarWindow, sidebar_binding_label,
};
pub(crate) use spaces::TabAgent;
pub(crate) use spaces::{most_urgent, urgency};

pub use crate::machines::Choice;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub machine: Choice,
    pub context: Option<ClientStartupTarget>,
}
impl From<Choice> for Selection {
    fn from(machine: Choice) -> Self {
        Self {
            machine,
            context: None,
        }
    }
}

/// A navigation episode outlives its request packet, independently of picker paint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionScope {
    generation: u64,
    pub origin: SelectionOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionOrigin {
    Picker,
    Sidebar,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Machines,
    Workspaces,
    Agents,
}

/// One live opening of the centered picker. It is minted by the existing
/// selection scope, rather than by paint or pointer motion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PickerEpisode(NonZeroU64);

/// Inputs from the accepted Agent view that can change source evaluation or
/// its stable ordering without appearing in a row's display copy.
#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentSourceBasis {
    session: orchbee_workbench_protocol::SessionId,
    view_revision: orchbee_workbench_protocol::WorkbenchRevision,
    source_order_locked: bool,
}

/// Exact nonvisual inputs used to project the Agent catalog in this frame.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PickerAgentBasis {
    view: crate::machine_agents::view::View,
    editing_query: bool,
    current: Choice,
    current_context: Option<ClientContextTarget>,
    current_member: Option<ClientMemberTarget>,
    source: Option<AgentSourceBasis>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PickerCatalogRow {
    selection: Selection,
    enabled: bool,
}

/// Value-semantic source shared by every target from one centered picker
/// candidate. Labels, transient status, pointer state and theme are excluded.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PickerCatalog {
    episode: PickerEpisode,
    mode: Mode,
    agent: Option<PickerAgentBasis>,
    workspace_query: Option<(String, usize)>,
    rows: Box<[PickerCatalogRow]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickerSource(Arc<PickerCatalog>);

/// A semantic action whose identity is closed over the exact painted catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickerAction {
    Close,
    Mode(Mode),
    Control(Control),
    Row(usize),
    CopyId(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnPickerTarget {
    source: PickerSource,
    action: PickerAction,
}

impl DrawnPickerTarget {
    pub(crate) const fn action(&self) -> PickerAction {
        self.action
    }

    pub(crate) fn source(&self) -> &PickerSource {
        &self.source
    }

    pub(crate) fn available(&self) -> bool {
        self.source.available(self.action)
    }
}

/// The exact list viewport published with one successfully painted picker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnPickerList {
    source: PickerSource,
    window: SidebarWindow,
}

impl DrawnPickerList {
    pub(crate) fn source(&self) -> &PickerSource {
        &self.source
    }

    pub(crate) const fn window(&self) -> SidebarWindow {
        self.window
    }
}

impl PickerSource {
    fn admits(&self, action: PickerAction) -> bool {
        match action {
            PickerAction::Close | PickerAction::Mode(_) => true,
            PickerAction::Control(control) => {
                (self.0.mode == Mode::Workspaces
                    && matches!(control, Control::Search | Control::Clear))
                    || self.0.mode == Mode::Agents
                        && match control {
                            Control::Done => self
                                .0
                                .agent
                                .as_ref()
                                .is_some_and(|basis| basis.editing_query),
                            Control::State
                            | Control::Scope
                            | Control::Order
                            | Control::Search
                            | Control::Clear => true,
                        }
            }
            PickerAction::Row(index) | PickerAction::CopyId(index) => index < self.0.rows.len(),
        }
    }

    fn available(&self, action: PickerAction) -> bool {
        match action {
            PickerAction::Row(index) => self.0.rows.get(index).is_some_and(|row| row.enabled),
            _ => self.admits(action),
        }
    }

    pub(crate) fn drawn(&self, action: PickerAction) -> Option<DrawnPickerTarget> {
        self.admits(action).then(|| DrawnPickerTarget {
            source: self.clone(),
            action,
        })
    }

    pub(crate) fn drawn_list(&self, window: SidebarWindow) -> Option<DrawnPickerList> {
        (window.capacity > 0
            && window.total == self.0.rows.len()
            && window.maximum == window.total.saturating_sub(window.capacity)
            && window.top <= window.maximum)
            .then(|| DrawnPickerList {
                source: self.clone(),
                window,
            })
    }
}

impl Mode {
    pub(crate) fn label(self, locale: Locale) -> &'static str {
        locale.machine_picker_text(match self {
            Self::Machines => "machines-mode",
            Self::Workspaces => "workspaces-mode",
            Self::Agents => "agents-mode",
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub(crate) choice: Choice,
    pub(crate) label: String,
    pub(crate) enabled: bool,
    pub(crate) observation: Option<Observation>,
    context: Option<Context>,
    agent: Option<crate::machine_agents::checkpoint::Row>,
    agent_status: Option<&'static str>,
}
impl Row {
    pub(crate) fn context(&self) -> Option<&Context> {
        self.context.as_ref()
    }
    pub(crate) fn agent(&self) -> Option<&crate::machine_agents::checkpoint::Row> {
        self.agent.as_ref()
    }
    pub(crate) const fn agent_status(&self) -> Option<&'static str> {
        self.agent_status
    }
    pub(crate) fn selection(&self) -> Selection {
        Selection {
            machine: self.choice.clone(),
            context: self
                .agent
                .as_ref()
                .map(|a| ClientStartupTarget::Agent(Box::new(a.target.clone())))
                .or_else(|| self.context.as_ref().map(|c| c.target.clone().into())),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Picker {
    pub available: bool,
    pub open: bool,
    pub current: Choice,
    pub current_label: String,
    pub(crate) rows: Vec<Row>,
    pub busy: bool,
    pub error: Option<&'static str>,
    pub request: Option<Selection>,
    pub(crate) copy_id_request: Option<String>,
    selection_generation: u64,
    selection_scope: Option<SelectionScope>,
    pub(crate) current_context: Option<ClientContextTarget>,
    local_contexts: std::sync::Arc<[Context]>,
    workspace_rows: std::sync::Arc<[Row]>,
    workspace_matches: std::sync::Arc<[Row]>,
    workspace_query: crate::terminal_text::line_editor::LineEditor,
    agent_rows: std::sync::Arc<[Row]>,
    agent_observations: Vec<(Choice, crate::machine_agents::checkpoint::Observation)>,
    agent_sources: Vec<(Choice, crate::machine_agents::checkpoint::Observation)>,
    agent_attention_key: Option<(Choice, Option<(u64, usize)>)>,
    agent_attention_refresh: bool,
    mode: Mode,
    agent_attention: crate::machine_agents::attention::Attention,
    agent_view: crate::machine_agents::view::View,
    agent_sidebar: sidebar::AgentSidebar,
    spaces: sidebar::Sidebar<orchbee_local_runtime::SpaceSidebarSettings>,
    sidebar_width: crate::sidebar_width::SidebarWidth,
    navigation: navigation::SidebarNavigation,
    working_motion: sidebar::Motion,
    space_observations: Vec<(Choice, crate::machine_spaces::checkpoint::Observation)>,
    space_updates: Vec<(Choice, crate::machine_spaces::checkpoint::Observation)>,
    space_stale: Vec<Choice>,
    /// Workspaces and directories that already asked for one Git read ahead of the cooldown.
    space_git_demanded: Vec<(
        Choice,
        orchbee_workbench_protocol::WorkbenchWorkspaceId,
        orchbee_workbench_protocol::WorkbenchDirectoryPath,
    )>,
    native_navigation: Option<(
        Choice,
        crate::machine_spaces::navigation_projection::SidebarNavigationProjection,
    )>,
    space_projection: Option<(Choice, Arc<crate::machine_spaces::checkpoint::Checkpoint>)>,
    editing_query: bool,
    current_member: Option<ClientMemberTarget>,
    pending_navigation: Option<Navigation>,
    selected: usize,
    top: usize,
    /// The painted list capacity while a wheel step owns the viewport; `None`
    /// while the window follows the selection.
    wheel_capacity: Option<usize>,
}
impl Default for Picker {
    fn default() -> Self {
        Self {
            available: false,
            open: false,
            current: Choice::Local,
            current_label: String::new(),
            rows: vec![Row {
                choice: Choice::Local,
                label: String::new(),
                enabled: true,
                observation: None,
                context: None,
                agent: None,
                agent_status: None,
            }],
            busy: false,
            error: None,
            request: None,
            copy_id_request: None,
            selection_generation: 0,
            selection_scope: None,
            current_context: None,
            local_contexts: std::sync::Arc::from([]),
            workspace_rows: std::sync::Arc::from([]),
            workspace_matches: std::sync::Arc::from([]),
            workspace_query: crate::terminal_text::line_editor::LineEditor::new(
                String::new(),
                crate::terminal_text::line_editor::LengthLimit::Scalars(128),
            ),
            agent_rows: std::sync::Arc::from([]),
            agent_observations: Vec::new(),
            agent_sources: Vec::new(),
            agent_attention_key: None,
            agent_attention_refresh: false,
            mode: Mode::Machines,
            agent_attention: crate::machine_agents::attention::Attention::default(),
            agent_view: crate::machine_agents::view::View::default(),
            agent_sidebar: sidebar::AgentSidebar::default(),
            spaces: sidebar::Sidebar::default(),
            sidebar_width: crate::sidebar_width::SidebarWidth::DEFAULT,
            navigation: navigation::SidebarNavigation::default(),
            working_motion: sidebar::Motion::default(),
            space_observations: Vec::new(),
            space_updates: Vec::new(),
            space_stale: Vec::new(),
            space_git_demanded: Vec::new(),
            native_navigation: None,
            space_projection: None,
            editing_query: false,
            current_member: None,
            pending_navigation: None,
            selected: 0,
            top: 0,
            wheel_capacity: None,
        }
    }
}
impl Picker {
    /// Read access for the picker painter in `tui_render`; only this module writes these fields.
    pub(crate) const fn mode(&self) -> Mode {
        self.mode
    }
    pub(crate) const fn selected(&self) -> usize {
        self.selected
    }
    pub(crate) const fn editing_query(&self) -> bool {
        self.editing_query
    }
    pub(crate) const fn workspace_query(&self) -> &crate::terminal_text::line_editor::LineEditor {
        &self.workspace_query
    }
    pub(crate) fn workspace_rows(&self) -> &[Row] {
        &self.workspace_rows
    }
    pub(crate) fn workspace_matches(&self) -> &[Row] {
        &self.workspace_matches
    }
    pub(crate) const fn agent_view(&self) -> &crate::machine_agents::view::View {
        &self.agent_view
    }
    pub(crate) fn visible_rows(&self) -> &[Row] {
        match self.mode {
            Mode::Machines => &self.rows,
            Mode::Workspaces => &self.workspace_matches,
            Mode::Agents => &self.agent_rows,
        }
    }
    fn selection(&self) -> Option<Selection> {
        self.visible_rows().get(self.selected).map(Row::selection)
    }

    fn agent_source_basis(&self) -> Option<AgentSourceBasis> {
        self.agent_observations
            .iter()
            .find(|(choice, _)| choice == &self.current)
            .and_then(|(_, observation)| match observation {
                crate::machine_agents::checkpoint::Observation::Ready(checkpoint) => {
                    Some(AgentSourceBasis {
                        session: checkpoint.session_id.clone(),
                        view_revision: checkpoint.view.revision,
                        source_order_locked: checkpoint
                            .view
                            .spec
                            .as_ref()
                            .is_some_and(|spec| !spec.sort.is_empty()),
                    })
                }
                _ => None,
            })
    }

    /// Projects the exact nonvisual authority behind the currently open
    /// centered picker. A candidate renderer clones this `Arc` for the frame.
    pub(crate) fn picker_source(&self) -> Option<PickerSource> {
        let scope = self.selection_scope.filter(|scope| {
            self.available && self.open && scope.origin == SelectionOrigin::Picker
        })?;
        let episode = PickerEpisode(NonZeroU64::new(scope.generation)?);
        let agent = (self.mode == Mode::Agents).then(|| PickerAgentBasis {
            view: self.agent_view.clone(),
            editing_query: self.editing_query,
            current: self.current.clone(),
            current_context: self.current_context.clone(),
            current_member: self.current_member.clone(),
            source: self.agent_source_basis(),
        });
        let rows = self
            .visible_rows()
            .iter()
            .map(|row| PickerCatalogRow {
                selection: row.selection(),
                enabled: self.workspace_row_available(row),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Some(PickerSource(Arc::new(PickerCatalog {
            episode,
            mode: self.mode,
            agent,
            workspace_query: (self.mode == Mode::Workspaces).then(|| {
                (
                    self.workspace_query.text().to_owned(),
                    self.workspace_query.cursor(),
                )
            }),
            rows,
        })))
    }

    pub(crate) fn picker_window(&self, capacity: usize) -> SidebarWindow {
        let total = self.visible_rows().len();
        let maximum = total.saturating_sub(capacity);
        let mut top = self.top.min(maximum);
        if capacity > 0 && self.wheel_capacity.is_none() {
            if self.selected < top {
                top = self.selected;
            } else if self.selected >= top.saturating_add(capacity) {
                top = self.selected.saturating_add(1).saturating_sub(capacity);
            }
        }
        top = top.min(maximum);
        SidebarWindow {
            top,
            capacity,
            total,
            maximum,
            insertion: None,
        }
    }

    fn close_picker(&mut self) {
        self.open = false;
        self.copy_id_request = None;
        self.editing_query = false;
        self.cancel_selection();
    }

    fn rebuild_workspaces(&mut self, selected: Option<Selection>) {
        self.rebuild_space_projection();
        let mut rows = Vec::new();
        for row in &self.rows {
            let checkpoint_contexts = self.checkpoint_contexts(&row.choice);
            let contexts = if let Some(contexts) = checkpoint_contexts.as_ref() {
                contexts.as_slice()
            } else if row.choice == Choice::Local {
                self.local_contexts.as_ref()
            } else if let Some(Observation::Online(o)) = &row.observation {
                o.contexts.as_ref()
            } else {
                &[]
            };
            rows.extend(contexts.iter().map(|context| Row {
                choice: row.choice.clone(),
                label: row.label.clone(),
                enabled: row.enabled,
                observation: None,
                context: Some(context.clone()),
                agent: None,
                agent_status: None,
            }));
        }
        self.cancel_sidebar_scroll();
        self.workspace_rows = rows.into();
        self.filter_workspaces(selected.clone());
        self.reconcile_sidebar_navigation();
        self.rebuild_agents();
        self.selected = selected
            .and_then(|s| {
                self.visible_rows()
                    .iter()
                    .position(|row| row.selection() == s)
            })
            .unwrap_or(0);
    }
    pub fn observe_local(&mut self, contexts: Vec<Context>) -> bool {
        if self.local_contexts.as_ref() == contexts {
            return false;
        }
        let selected = self.selection();
        self.local_contexts = contexts.into();
        self.rebuild_workspaces(selected);
        true
    }
    pub fn wants_agents(&self) -> bool {
        self.spaces.visible() || (self.open && matches!(self.mode, Mode::Agents | Mode::Workspaces))
    }
    /// The shared Tab strip shows unread Agents even while the Sidebar is hidden.
    pub(crate) fn wants_agent_attention(&self) -> bool {
        self.available
    }
    pub fn observe_agents(
        &mut self,
        observations: Vec<(Choice, crate::machine_agents::checkpoint::Observation)>,
    ) -> bool {
        self.agent_attention
            .retain(|id| self.rows.iter().any(|r| r.enabled && &r.choice == id));
        if self.agent_sources == observations {
            return false;
        }
        self.agent_sources = observations.clone();
        self.agent_attention_refresh = true;
        self.project_agent_observations(observations)
    }
    pub(super) fn project_agent_observations(
        &mut self,
        observations: Vec<(Choice, crate::machine_agents::checkpoint::Observation)>,
    ) -> bool {
        let observations = observations
            .into_iter()
            .map(|(id, o)| {
                let projected = match o {
                    crate::machine_agents::checkpoint::Observation::Ready(round) => {
                        self.agent_attention.project(&id, &round).map_or(
                            crate::machine_agents::checkpoint::Observation::Unavailable,
                            crate::machine_agents::checkpoint::Observation::Ready,
                        )
                    }
                    other => other,
                };
                (id, projected)
            })
            .collect::<Vec<_>>();
        if self.agent_observations == observations {
            return false;
        }
        let selected = self.selection();
        self.agent_observations = observations;
        self.rebuild_workspaces(selected);
        true
    }
    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.editing_query = false;
        self.pending_navigation = None;
        self.selected = 0;
        self.top = 0;
        self.wheel_capacity = None;
        self.error = None;
    }
    fn toggle_mode(&mut self) {
        self.set_mode(match self.mode {
            Mode::Machines => Mode::Workspaces,
            Mode::Workspaces => Mode::Agents,
            Mode::Agents => Mode::Machines,
        });
    }
    pub fn replace(&mut self, profiles: &[Profile]) {
        let selected = self.selection();
        let mut rows = vec![Row {
            choice: Choice::Local,
            label: String::new(),
            enabled: true,
            observation: None,
            context: None,
            agent: None,
            agent_status: None,
        }];
        rows.extend(profiles.iter().map(|p| {
            Row {
                choice: Choice::Remote(p.id.clone()),
                label: p.draft.label.clone(),
                enabled: p.enabled,
                context: None,
                agent: None,
                agent_status: None,
                observation: p
                    .enabled
                    .then(|| {
                        self.rows
                            .iter()
                            .find(|r| r.choice == Choice::Remote(p.id.clone()))
                            .and_then(|r| r.observation.clone())
                    })
                    .flatten(),
            }
        }));
        if rows != self.rows {
            self.rows = rows;
            self.space_observations.retain(|(choice, _)| {
                self.rows
                    .iter()
                    .any(|row| row.enabled && &row.choice == choice)
            });
            self.space_updates.retain(|(choice, _)| {
                self.rows
                    .iter()
                    .any(|row| row.enabled && &row.choice == choice)
            });
            self.space_stale.retain(|choice| {
                self.rows
                    .iter()
                    .any(|row| row.enabled && &row.choice == choice)
            });
            self.rebuild_workspaces(selected);
        }
    }
    pub fn observe(&mut self, observations: &[(ProfileId, Observation)]) -> bool {
        let selected = self.selection();
        let mut changed = false;
        for row in &mut self.rows {
            let next = if row.enabled {
                observations
                    .iter()
                    .find(|(id, _)| row.choice == Choice::Remote(id.clone()))
                    .map(|(_, o)| o.clone())
            } else {
                None
            };
            if row.observation != next {
                row.observation = next;
                changed = true;
            }
        }
        if changed {
            self.rebuild_workspaces(selected);
        }
        changed
    }
    pub(crate) fn show(&mut self) {
        if !self.begin_selection(SelectionOrigin::Picker) {
            return;
        }
        self.leave_sidebar_navigation();
        self.open = true;
        self.error = None;
        self.wheel_capacity = None;
        self.selected = self
            .visible_rows()
            .iter()
            .position(|r| r.choice == self.current)
            .unwrap_or(0);
    }
    fn begin_selection(&mut self, origin: SelectionOrigin) -> bool {
        self.cancel_selection();
        let Some(generation) = self.selection_generation.checked_add(1) else {
            self.error = Some("failed");
            return false;
        };
        self.selection_generation = generation;
        self.selection_scope = Some(SelectionScope { generation, origin });
        true
    }
    pub fn selection_scope(&self) -> Option<SelectionScope> {
        self.selection_scope
    }
    pub fn selection_current(&self, scope: SelectionScope) -> bool {
        self.selection_scope == Some(scope)
            && (scope.origin == SelectionOrigin::Sidebar || self.open)
    }
    pub fn cancel_selection(&mut self) {
        self.selection_scope = None;
        self.request = None;
        self.pending_navigation = None;
    }
    pub fn direct_selection_pending(&self) -> bool {
        self.selection_scope
            .is_some_and(|scope| scope.origin == SelectionOrigin::Sidebar)
    }

    /// All keys, paste and malformed events are consumed by the caller while open.
    pub(crate) fn row_id(&self, index: usize) -> Option<&str> {
        let row = self.visible_rows().get(index)?;
        if let Some(agent) = &row.agent {
            return Some(agent.target.member.as_str());
        }
        if let Some(context) = &row.context {
            return Some(match &context.target.location {
                crate::workbench_projection::ClientContextLocation::NoWorkspace => return None,
                crate::workbench_projection::ClientContextLocation::Workspace {
                    workspace_id,
                    ..
                } => workspace_id.as_str(),
            });
        }
        match &row.choice {
            Choice::Remote(id) => Some(id.as_str()),
            Choice::Local => None,
        }
    }
    pub(crate) fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyKind::Release {
            return;
        }
        self.pending_navigation = None;
        let hidden = self.reveal_selection();
        let (code, mods) = key.normalized();
        if code == KeyCode::Char('y') && mods == Modifiers::CTRL {
            self.copy_id_request = self.row_id(self.selected).map(str::to_owned);
            return;
        }
        if self.agent_key(code, mods) {
            return;
        }
        if self.workspace_query_key(code, mods) {
            return;
        }
        if (code == KeyCode::Char('i') && mods == Modifiers::CTRL)
            || (code == KeyCode::Tab && mods == Modifiers::NONE)
        {
            self.toggle_mode();
            return;
        }
        if code == KeyCode::Char('m') && mods == Modifiers::CTRL {
            if !hidden {
                self.choose(self.selected);
            }
            return;
        }
        if mods != Modifiers::NONE {
            return;
        }
        match code {
            KeyCode::Esc => self.close_picker(),
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('k') if self.mode != Mode::Workspaces => {
                self.selected = self.selected.saturating_sub(1)
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.visible_rows().len().saturating_sub(1))
            }
            KeyCode::Char('j') if self.mode != Mode::Workspaces => {
                self.selected = (self.selected + 1).min(self.visible_rows().len().saturating_sub(1))
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = self.visible_rows().len().saturating_sub(1),
            KeyCode::Enter if !hidden => self.choose(self.selected),
            _ => {}
        }
    }

    /// Returns the viewport to the selection after a wheel step. True when the
    /// selection was outside that painted viewport, so this key only reveals it.
    fn reveal_selection(&mut self) -> bool {
        let Some(capacity) = self.wheel_capacity.take() else {
            return false;
        };
        let top = self
            .top
            .min(self.visible_rows().len().saturating_sub(capacity));
        !(top..top.saturating_add(capacity)).contains(&self.selected)
    }
    fn choose(&mut self, index: usize) {
        if let Some(row) = self.visible_rows().get(index) {
            if self.workspace_row_available(row) {
                let selection = row.selection();
                if self.begin_selection(SelectionOrigin::Picker) {
                    self.request = Some(selection);
                    self.error = None;
                }
            } else {
                self.error = Some(row.agent_status.unwrap_or("disabled"));
            }
        }
    }
}

/// Crate-internal `DrawnPickerTarget` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait DrawnPickerTargetTestAccess {
    fn available(&self) -> bool;
}

#[cfg(any(test, feature = "test-support"))]
impl DrawnPickerTargetTestAccess for DrawnPickerTarget {
    fn available(&self) -> bool {
        DrawnPickerTarget::available(self)
    }
}

/// Crate-internal `PickerSource` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait PickerSourceTestAccess {
    fn drawn(&self, action: PickerAction) -> Option<DrawnPickerTarget>;
}

#[cfg(any(test, feature = "test-support"))]
impl PickerSourceTestAccess for PickerSource {
    fn drawn(&self, action: PickerAction) -> Option<DrawnPickerTarget> {
        PickerSource::drawn(self, action)
    }
}

/// Crate-internal `Picker` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait PickerTestAccess {
    fn rows(&self) -> &[Row];
    fn picker_source(&self) -> Option<PickerSource>;
    fn show(&mut self);
    fn key(&mut self, key: KeyEvent);
    fn enter_sidebar_navigation(&mut self) -> bool;
    fn sidebar_width(&self) -> crate::sidebar_width::SidebarWidth;
    fn activate_sidebar_agent(&mut self, selected: &Selection) -> bool;
    fn show_workspaces(&mut self);
    fn activate_sidebar_workspace(&mut self, selected: &Selection) -> bool;
}

#[cfg(any(test, feature = "test-support"))]
impl PickerTestAccess for Picker {
    fn rows(&self) -> &[Row] {
        &self.rows
    }
    fn picker_source(&self) -> Option<PickerSource> {
        Picker::picker_source(self)
    }
    fn show(&mut self) {
        Picker::show(self)
    }
    fn key(&mut self, key: KeyEvent) {
        Picker::key(self, key)
    }
    fn enter_sidebar_navigation(&mut self) -> bool {
        Picker::enter_sidebar_navigation(self)
    }
    fn sidebar_width(&self) -> crate::sidebar_width::SidebarWidth {
        Picker::sidebar_width(self)
    }
    fn activate_sidebar_agent(&mut self, selected: &Selection) -> bool {
        Picker::activate_sidebar_agent(self, selected)
    }
    fn show_workspaces(&mut self) {
        Picker::show_workspaces(self)
    }
    fn activate_sidebar_workspace(&mut self, selected: &Selection) -> bool {
        Picker::activate_sidebar_workspace(self, selected)
    }
}

#[cfg(test)]
mod tests;
