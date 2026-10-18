//! Identity-based machine selection and painted input ownership; no storage or network effects.
use crate::{
    client_view::{
        Binding, KeyCode, KeyEvent, KeyKind, KeyLabelGlyphs, LabelStyle, Modifiers,
        format_binding_with_glyphs, join_key_labels,
    },
    localization::Locale,
    machine_discovery::{Context, Observation, display_label},
    machines::{Profile, ProfileId},
    terminal_session::{NativeContextTarget, NativeMemberTarget, NativeStartupTarget},
    terminal_text::display_width,
    tui_layout::ViewportTier,
    tui_pointer::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, ListScrollTarget,
        PrimaryContract, ProjectedPointer,
    },
    tui_render::controls::{
        Availability, CheckedState, ControlSurface, CurrentState, FocusState, IconActionGlyph,
        IconActionSpec, InteractionState, KeyHint, NavigationState, OperationState,
        PointerPresentation, RowContentBand, RowFocusCue, RowTracks, close_rect, measure_key_hints,
        paint_row_interaction, render_icon_action, render_key_hints,
    },
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::{num::NonZeroU64, sync::Arc};
mod agents;
mod navigation;
mod sidebar;
mod spaces;
mod workspace_query;
pub(crate) use agents::Control;
use agents::Navigation;
pub(crate) use agents::{AgentSourceCoverage, CompactAgentSummary};
pub(crate) use navigation::{
    DrawnSidebarFixedTarget, DrawnSidebarTarget, SIDEBAR_BINDINGS, SidebarActivation,
    SidebarCommand, SidebarFixedAction, SidebarFixedActivation, SidebarIntent, SidebarPart,
    SidebarRow, SidebarSource, SidebarTarget, sidebar_binding_label,
};
pub(crate) use sidebar::SidebarWindow;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Choice {
    Local,
    Remote(ProfileId),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Selection {
    pub(crate) machine: Choice,
    pub(crate) context: Option<NativeStartupTarget>,
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
pub(crate) struct SelectionScope {
    generation: u64,
    pub(crate) origin: SelectionOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionOrigin {
    Picker,
    Sidebar,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Mode {
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
    site: orchbee_workbench_protocol::SiteId,
    view_revision: orchbee_workbench_protocol::WorkbenchRevision,
    source_order_locked: bool,
}

/// Exact nonvisual inputs used to project the Agent catalog in this frame.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PickerAgentBasis {
    view: crate::machine_agents::view::View,
    editing_query: bool,
    current: Choice,
    current_context: Option<NativeContextTarget>,
    current_member: Option<NativeMemberTarget>,
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
pub(crate) struct PickerSource(Arc<PickerCatalog>);

/// A semantic action whose identity is closed over the exact painted catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PickerAction {
    Close,
    Mode(Mode),
    Control(Control),
    Row(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DrawnPickerTarget {
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
pub(crate) struct DrawnPickerList {
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
            PickerAction::Row(index) => index < self.0.rows.len(),
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

fn key_label_glyphs(theme: &Theme) -> KeyLabelGlyphs {
    match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(test)]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    }
}

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

fn interaction_spec(target: &DrawnPickerTarget) -> InteractionSpec {
    if target.available() {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    }
}

pub(super) fn picker_interaction(
    source: &PickerSource,
    action: PickerAction,
) -> Option<(InteractionTarget, InteractionSpec)> {
    let drawn = source.drawn(action)?;
    let spec = interaction_spec(&drawn);
    Some((InteractionTarget::MachinePicker(drawn), spec))
}

fn component_state(
    pointer: PointerPresentation,
    navigation: NavigationState,
    current: CurrentState,
    checked: CheckedState,
    availability: Availability,
    operation: OperationState,
) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        pointer,
        navigation,
        current,
        checked,
        availability,
        operation,
    )
}

fn render_picker_close(
    frame: &mut Frame<'_>,
    area: Rect,
    source: &PickerSource,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let rect = close_rect(frame.area(), area)?;
    let (target, spec) = picker_interaction(source, PickerAction::Close)?;
    let pointer =
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec));
    let painted = render_icon_action(
        frame,
        rect,
        IconActionSpec::new(
            IconActionGlyph::Close,
            ControlSurface::Overlay,
            Availability::Enabled,
            pointer,
        ),
        theme,
    )?;
    hits.record_overlay_dismissal(&target);
    hits.record_interaction(HitLayer::Overlay, painted, painted, target, spec);
    Some(painted)
}
impl Mode {
    fn label(self, locale: Locale) -> &'static str {
        locale.machine_picker_text(match self {
            Self::Machines => "machines-mode",
            Self::Workspaces => "workspaces-mode",
            Self::Agents => "agents-mode",
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) choice: Choice,
    pub(crate) label: String,
    pub(crate) enabled: bool,
    pub(crate) observation: Option<Observation>,
    context: Option<Context>,
    agent: Option<crate::machine_agents::Row>,
    agent_status: Option<&'static str>,
}
impl Row {
    pub(crate) fn selection(&self) -> Selection {
        Selection {
            machine: self.choice.clone(),
            context: self
                .agent
                .as_ref()
                .map(|a| NativeStartupTarget::Agent(Box::new(a.target.clone())))
                .or_else(|| self.context.as_ref().map(|c| c.target.clone().into())),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Picker {
    pub(crate) available: bool,
    pub(crate) open: bool,
    pub(crate) current: Choice,
    pub(crate) current_label: String,
    pub(crate) rows: Vec<Row>,
    pub(crate) busy: bool,
    pub(crate) error: Option<&'static str>,
    pub(crate) request: Option<Selection>,
    selection_generation: u64,
    selection_scope: Option<SelectionScope>,
    pub(crate) current_context: Option<NativeContextTarget>,
    local_contexts: std::sync::Arc<[Context]>,
    workspace_rows: std::sync::Arc<[Row]>,
    workspace_matches: std::sync::Arc<[Row]>,
    workspace_query: crate::terminal_text::line_editor::LineEditor,
    agent_rows: std::sync::Arc<[Row]>,
    agent_observations: Vec<(Choice, crate::machine_agents::Observation)>,
    agent_sources: Vec<(Choice, crate::machine_agents::Observation)>,
    compact_agent_summary: bool,
    agent_attention_key: Option<(Choice, Option<(u64, usize)>)>,
    agent_attention_refresh: bool,
    mode: Mode,
    agent_attention: crate::machine_agents::Attention,
    agent_view: crate::machine_agents::view::View,
    agent_sidebar: sidebar::AgentSidebar,
    spaces: sidebar::Sidebar<orchbee_local_site_runtime::WindowsSpaceSidebarSettings>,
    sidebar_width: crate::tui_layout::SidebarWidth,
    navigation: navigation::SidebarNavigation,
    space_observations: Vec<(Choice, crate::machine_spaces::Observation)>,
    space_updates: Vec<(Choice, crate::machine_spaces::Observation)>,
    space_stale: Vec<Choice>,
    native_navigation: Option<(Choice, crate::terminal_session::SidebarNavigationProjection)>,
    space_projection: Option<(Choice, Arc<crate::machine_spaces::Checkpoint>)>,
    editing_query: bool,
    current_member: Option<NativeMemberTarget>,
    pending_navigation: Option<Navigation>,
    selected: usize,
    top: usize,
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
            compact_agent_summary: false,
            agent_attention_key: None,
            agent_attention_refresh: false,
            mode: Mode::Machines,
            agent_attention: crate::machine_agents::Attention::default(),
            agent_view: crate::machine_agents::view::View::default(),
            agent_sidebar: sidebar::AgentSidebar::default(),
            spaces: sidebar::Sidebar::default(),
            sidebar_width: crate::tui_layout::SidebarWidth::DEFAULT,
            navigation: navigation::SidebarNavigation::default(),
            space_observations: Vec::new(),
            space_updates: Vec::new(),
            space_stale: Vec::new(),
            native_navigation: None,
            space_projection: None,
            editing_query: false,
            current_member: None,
            pending_navigation: None,
            selected: 0,
            top: 0,
        }
    }
}
impl Picker {
    fn visible_rows(&self) -> &[Row] {
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
                crate::machine_agents::Observation::Ready(checkpoint) => Some(AgentSourceBasis {
                    site: checkpoint.site_id.clone(),
                    view_revision: checkpoint.view.revision,
                    source_order_locked: checkpoint
                        .view
                        .spec
                        .as_ref()
                        .is_some_and(|spec| !spec.sort.is_empty()),
                }),
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

    fn picker_window(&self, capacity: usize) -> SidebarWindow {
        let total = self.visible_rows().len();
        let maximum = total.saturating_sub(capacity);
        let mut top = self.top.min(maximum);
        if capacity > 0 {
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
        self.editing_query = false;
        self.cancel_selection();
    }

    /// Applies a release only when its complete catalog still describes the
    /// live picker. Visual-only refreshes deliberately preserve that source.
    pub(crate) fn activate_drawn_picker(&mut self, target: &DrawnPickerTarget) -> bool {
        let Some(source) = self.picker_source() else {
            return false;
        };
        if &source != target.source() || !source.available(target.action()) {
            return false;
        }
        self.pending_navigation = None;
        match target.action() {
            PickerAction::Close => self.close_picker(),
            PickerAction::Mode(mode) => self.set_mode(mode),
            PickerAction::Control(control) if self.mode == Mode::Workspaces => {
                if control == Control::Clear {
                    self.workspace_query.clear();
                    self.filter_workspaces(self.selection());
                }
            }
            PickerAction::Control(control) => self.agent_control(control),
            PickerAction::Row(index) => {
                let Some(catalog_row) = source.0.rows.get(index) else {
                    return false;
                };
                if self
                    .visible_rows()
                    .get(index)
                    .is_none_or(|row| row.selection() != catalog_row.selection)
                {
                    return false;
                }
                self.selected = index;
                self.choose(index);
                return self.request.is_some();
            }
        }
        true
    }

    /// Moves at most one painted viewport and returns its replacement witness.
    /// Stale geometry and edge movement fail closed without retaining work.
    pub(crate) fn scroll_drawn_picker(
        &mut self,
        target: &DrawnPickerList,
        up: bool,
        lines: usize,
    ) -> Option<DrawnPickerList> {
        let source = self.picker_source()?;
        let window = target.window();
        if &source != target.source() || self.picker_window(window.capacity) != window {
            return None;
        }
        let amount = lines.min(window.capacity);
        let top = if up {
            window.top.saturating_sub(amount)
        } else {
            window.top.saturating_add(amount).min(window.maximum)
        };
        if top == window.top {
            return None;
        }
        self.top = top;
        if window.total > 0 {
            self.selected = self.selected.clamp(
                top,
                top.saturating_add(window.capacity.saturating_sub(1))
                    .min(window.total - 1),
            );
        }
        source.drawn_list(SidebarWindow { top, ..window })
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
        self.invalidate();
    }
    pub(crate) fn observe_local(&mut self, contexts: Vec<Context>) -> bool {
        if self.local_contexts.as_ref() == contexts {
            return false;
        }
        let selected = self.selection();
        self.local_contexts = contexts.into();
        self.rebuild_workspaces(selected);
        true
    }
    pub(crate) fn wants_agents(&self) -> bool {
        self.spaces.visible()
            || self.compact_agent_summary
            || (self.open && matches!(self.mode, Mode::Agents | Mode::Workspaces))
    }
    pub(crate) fn wants_agent_attention(&self) -> bool {
        self.spaces.visible() || (self.open && matches!(self.mode, Mode::Agents | Mode::Workspaces))
    }
    pub(crate) fn observe_agents(
        &mut self,
        observations: Vec<(Choice, crate::machine_agents::Observation)>,
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
        observations: Vec<(Choice, crate::machine_agents::Observation)>,
    ) -> bool {
        let observations = observations
            .into_iter()
            .map(|(id, o)| {
                let projected = match o {
                    crate::machine_agents::Observation::Ready(round) => {
                        self.agent_attention.project(&id, &round).map_or(
                            crate::machine_agents::Observation::Unavailable,
                            crate::machine_agents::Observation::Ready,
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
        self.error = None;
        self.invalidate();
    }
    fn toggle_mode(&mut self) {
        self.set_mode(match self.mode {
            Mode::Machines => Mode::Workspaces,
            Mode::Workspaces => Mode::Agents,
            Mode::Agents => Mode::Machines,
        });
    }
    pub(crate) fn replace(&mut self, profiles: &[Profile]) {
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
    pub(crate) fn observe(&mut self, observations: &[(ProfileId, Observation)]) -> bool {
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
        self.selected = self
            .visible_rows()
            .iter()
            .position(|r| r.choice == self.current)
            .unwrap_or(0);
        self.invalidate();
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
    pub(crate) fn selection_scope(&self) -> Option<SelectionScope> {
        self.selection_scope
    }
    pub(crate) fn selection_current(&self, scope: SelectionScope) -> bool {
        self.selection_scope == Some(scope)
            && (scope.origin == SelectionOrigin::Sidebar || self.open)
    }
    pub(crate) fn cancel_selection(&mut self) {
        self.selection_scope = None;
        self.request = None;
        self.pending_navigation = None;
    }
    pub(crate) fn direct_selection_pending(&self) -> bool {
        self.selection_scope
            .is_some_and(|scope| scope.origin == SelectionOrigin::Sidebar)
    }
    /// Centered-picker pointer authority is the successfully published frame
    /// hit map. Callers may keep using this transition hook while they migrate;
    /// there is no feature-local geometry to clear.
    pub(crate) fn invalidate(&mut self) {}
    /// All keys, paste and malformed events are consumed by the caller while open.
    pub(crate) fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyKind::Release {
            return;
        }
        self.pending_navigation = None;
        let (code, mods) = key.normalized();
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
            self.choose(self.selected);
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
            KeyCode::Enter => self.choose(self.selected),
            _ => {}
        }
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
    fn render_capture_status(&self, frame: &mut Frame<'_>, locale: Locale, theme: &Theme) {
        let area = frame.area();
        if area.is_empty() {
            return;
        }
        let status = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        frame.render_widget(Clear, status);
        frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), status);
        let editing = self.mode == Mode::Agents && self.editing_query;
        let label = if self.mode == Mode::Workspaces {
            locale.machine_picker_text("workspace-search")
        } else if editing {
            locale.machine_picker_text("agent-search")
        } else {
            self.mode.label(locale)
        };
        let glyphs = key_label_glyphs(theme);
        let style = LabelStyle::for_host();
        let close =
            format_binding_with_glyphs(&Binding::new(KeyCode::Esc, Modifiers::NONE), style, glyphs);
        let enter = format_binding_with_glyphs(
            &Binding::new(KeyCode::Enter, Modifiers::NONE),
            style,
            glyphs,
        );
        let tab =
            format_binding_with_glyphs(&Binding::new(KeyCode::Tab, Modifiers::NONE), style, glyphs);
        let up =
            format_binding_with_glyphs(&Binding::new(KeyCode::Up, Modifiers::NONE), style, glyphs);
        let down = format_binding_with_glyphs(
            &Binding::new(KeyCode::Down, Modifiers::NONE),
            style,
            glyphs,
        );
        let choose = join_key_labels(&[up, down], style, "/");
        let clear = format_binding_with_glyphs(
            &Binding::new(KeyCode::Char('u'), Modifiers::CTRL),
            style,
            glyphs,
        );
        let hints = if editing {
            vec![
                KeyHint::new(&close, locale.machine_picker_text("picker-finish-hint")),
                KeyHint::new(&enter, locale.machine_picker_text("picker-finish-hint")),
                KeyHint::new(&clear, locale.machine_picker_text("picker-clear-hint")),
            ]
        } else {
            vec![
                KeyHint::new(&close, locale.machine_picker_text("picker-close-hint")),
                KeyHint::new(&choose, locale.machine_picker_text("picker-choose-hint")),
                KeyHint::new(&enter, locale.machine_picker_text("picker-open-hint")),
                KeyHint::new(&tab, locale.machine_picker_text("picker-mode-hint")),
            ]
        };
        let full_width = measure_key_hints(&hints, theme);
        let label_width = u16::try_from(display_width(label)).unwrap_or(u16::MAX);
        let show_label = full_width.is_some_and(|width| {
            usize::from(label_width)
                .saturating_add(1)
                .saturating_add(width)
                <= usize::from(status.width)
        });
        let hint_area = if show_label {
            let width = u16::try_from(full_width.unwrap_or(0)).unwrap_or(status.width);
            frame.render_widget(
                Paragraph::new(label)
                    .style(theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD)),
                Rect::new(status.x, status.y, label_width.min(status.width), 1),
            );
            Rect::new(status.right().saturating_sub(width), status.y, width, 1)
        } else {
            status
        };
        let _ = render_key_hints(frame, hint_area, &hints, theme);
    }

    pub(crate) fn render(
        &self,
        frame: &mut Frame<'_>,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        let Some(source) = self.picker_source() else {
            return;
        };
        let area = frame.area();
        self.render_capture_status(frame, locale, theme);
        if ViewportTier::from_area(area) == ViewportTier::TooSmall {
            return;
        }
        hits.start_modal();
        let width = area.width.saturating_sub(4).min(72);
        let height = area.height.saturating_sub(3).min(
            ((if self.mode == Mode::Workspaces {
                self.workspace_rows.len()
            } else {
                self.visible_rows().len()
            }) + match self.mode {
                Mode::Agents => 13,
                Mode::Workspaces => 11,
                Mode::Machines => 10,
            })
            .min(24) as u16,
        );
        if width < 5 || height == 0 {
            let _ = render_picker_close(frame, area, &source, theme, hits);
            return;
        }
        let panel = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, panel);
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .border_type(theme.corners)
                .border_style(theme.style(Role::FrameFocused))
                .style(theme.style(Role::SurfaceOverlay))
                .title(Span::styled(
                    format!(
                        " {} {}/{} ",
                        self.mode.label(locale),
                        (self.selected + 1).min(self.visible_rows().len()),
                        self.visible_rows().len()
                    ),
                    theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
                )),
            panel,
        );
        let _ = render_picker_close(frame, panel, &source, theme, hits);
        if width < 34 || height < 5 {
            return;
        }
        let mut inner = panel.inner(ratatui::layout::Margin::new(1, 1));
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        if inner.height >= 4 {
            let mut x = inner.x;
            for mode in [Mode::Machines, Mode::Workspaces, Mode::Agents] {
                let label = mode.label(locale);
                let Ok(label_width) = u16::try_from(display_width(label)) else {
                    continue;
                };
                let Some(width) = label_width.checked_add(1) else {
                    continue;
                };
                if width == 1 || x.saturating_add(width) > inner.right() {
                    continue;
                }
                let rect = Rect::new(x, inner.y, width, 1);
                let checked = Rect::new(rect.x, rect.y, 1, 1);
                let primary = Rect::new(rect.x + 1, rect.y, label_width, 1);
                let Some((target, spec)) = picker_interaction(&source, PickerAction::Mode(mode))
                else {
                    continue;
                };
                let pointer = pointer_presentation(hits.projected_pointer(
                    HitLayer::Overlay,
                    rect,
                    rect,
                    &target,
                    spec,
                ));
                frame.render_widget(
                    Paragraph::new(label).style(theme.style(Role::TextPrimary)),
                    primary,
                );
                let state = component_state(
                    pointer,
                    NavigationState::None,
                    CurrentState::NotCurrent,
                    if self.mode == mode {
                        CheckedState::Checked
                    } else {
                        CheckedState::Unchecked
                    },
                    Availability::Enabled,
                    OperationState::Idle,
                );
                if paint_row_interaction(
                    frame,
                    rect,
                    RowTracks::new(RowFocusCue::EnclosingFrame, primary)
                        .with_checked(checked, theme.glyphs.active_marker),
                    &[],
                    ControlSurface::Overlay,
                    state,
                    theme,
                )
                .is_some()
                {
                    hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
                    x = rect.right().saturating_add(1);
                }
            }
            inner.y += 1;
            inner.height -= 1;
        }
        self.render_workspace_query(frame, &mut inner, locale, theme, &source, hits);
        self.render_agent_controls(frame, &mut inner, locale, theme, &source, hits);
        let details = if inner.height >= 9 { 5 } else { 0 };
        let count = usize::from(inner.height.saturating_sub(2 + details));
        let window = self.picker_window(count);
        let list = Rect::new(inner.x, inner.y, inner.width, count as u16);
        for (index, row) in self
            .visible_rows()
            .iter()
            .enumerate()
            .skip(window.top)
            .take(count)
        {
            let enabled = self.workspace_row_available(row);
            let rect = Rect::new(
                inner.x,
                inner.y + (index - window.top) as u16,
                inner.width,
                1,
            );
            let name = if row.choice == Choice::Local {
                locale.machine_picker_text("local")
            } else {
                &display_label(&row.label)
            };
            let name = if let Some(agent) = &row.agent {
                format!(
                    "{}{name} / {} · {}",
                    if index < 9 {
                        format!("{} ", index + 1)
                    } else {
                        String::new()
                    },
                    display_label(agent.provider()),
                    display_label(agent.title())
                )
            } else if let Some(context) = &row.context {
                format!(
                    "{name} / {}",
                    context
                        .workspace
                        .as_deref()
                        .unwrap_or(locale.machine_picker_text("standalone"))
                )
            } else {
                name.to_owned()
            };
            let suffix = if let Some(status) = row.agent_status {
                locale.machine_picker_text(status).to_owned()
            } else if let Some(agent) = &row.agent {
                let fallback = locale.machine_picker_text(match agent.display_state() {
                    orchbee_workbench_protocol::MetadataState::Idle => "agent-idle",
                    orchbee_workbench_protocol::MetadataState::Done => "agent-done",
                    orchbee_workbench_protocol::MetadataState::Error => "agent-error",
                    orchbee_workbench_protocol::MetadataState::Working => "agent-working",
                    orchbee_workbench_protocol::MetadataState::Blocked => "agent-blocked",
                    orchbee_workbench_protocol::MetadataState::Unknown => "agent-unknown",
                });
                // Sanitize the owned label before it enters a terminal render widget.
                agent
                    .state_label()
                    .map(display_label)
                    .unwrap_or_else(|| fallback.into())
            } else if !enabled {
                locale.machine_picker_text("disabled").to_owned()
            } else if row.context.is_some() && self.space_checkpoint_stale(row) {
                locale.machine_picker_text("stale").to_owned()
            } else if let Some(observation) = &row.observation {
                locale.machine_picker_text(observation.key()).to_owned()
            } else if row.choice == self.current {
                locale.machine_picker_text("current").to_owned()
            } else {
                String::new()
            };
            let id = match &row.choice {
                Choice::Local => String::new(),
                Choice::Remote(id) => {
                    let width = (8..=32).step_by(4).find(|width| self.rows.iter().filter(|r| matches!(&r.choice, Choice::Remote(other) if other.as_str()[..*width] == id.as_str()[..*width])).count() == 1).unwrap_or(32);
                    format!("{} · ", &id.as_str()[..width])
                }
            };
            let status_width = match self.mode {
                Mode::Machines => rect.width.min(16),
                Mode::Agents => rect.width.min(18),
                Mode::Workspaces => 0,
            }
            .min(rect.width.saturating_sub(4));
            let current_rect = Rect::new(rect.x, rect.y, 1, 1);
            let navigation_rect = Rect::new(rect.x + 1, rect.y, 1, 1);
            let operation_rect = Rect::new(rect.right() - 1, rect.y, 1, 1);
            let primary_width = rect
                .width
                .saturating_sub(2)
                .saturating_sub(status_width)
                .saturating_sub(1);
            let primary_rect = Rect::new(rect.x + 2, rect.y, primary_width, 1);
            let status_rect = Rect::new(primary_rect.right(), rect.y, status_width, 1);
            let current = row
                .agent
                .as_ref()
                .is_some_and(|a| self.current_agent(&row.choice, a))
                || (self.mode != Mode::Agents
                    && row.choice == self.current
                    && row
                        .context
                        .as_ref()
                        .is_none_or(|c| self.current_context.as_ref() == Some(&c.target)));
            let Some((target, spec)) = picker_interaction(&source, PickerAction::Row(index)) else {
                continue;
            };
            let pointer = pointer_presentation(hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                spec,
            ));
            let primary_role = if enabled {
                Role::TextPrimary
            } else {
                Role::TextDisabled
            };
            frame.render_widget(
                Paragraph::new(if current {
                    theme.glyphs.active_marker
                } else {
                    " "
                })
                .style(theme.style(if current {
                    Role::AccentSelection
                } else {
                    Role::TextSecondary
                })),
                current_rect,
            );
            frame.render_widget(
                Paragraph::new(format!("{id}{name}")).style(theme.style(primary_role)),
                primary_rect,
            );
            let status_role = if !enabled {
                Role::TextDisabled
            } else if row.agent.as_ref().is_some_and(|agent| {
                matches!(
                    agent.display_state(),
                    orchbee_workbench_protocol::MetadataState::Error
                        | orchbee_workbench_protocol::MetadataState::Blocked
                )
            }) || matches!(
                row.observation.as_ref(),
                Some(Observation::Attention | Observation::Stale)
            ) {
                Role::StatusWarning
            } else if row.agent.as_ref().is_some_and(|agent| {
                agent.display_state() == orchbee_workbench_protocol::MetadataState::Working
            }) || matches!(
                row.observation.as_ref(),
                Some(Observation::Waiting | Observation::Connecting | Observation::Reconnecting)
            ) {
                Role::StatusWorking
            } else if matches!(row.observation.as_ref(), Some(Observation::Online(_))) {
                Role::StatusSuccess
            } else {
                Role::TextSecondary
            };
            frame.render_widget(
                Paragraph::new(suffix).style(theme.style(status_role)),
                status_rect,
            );
            let availability = if enabled {
                Availability::Enabled
            } else {
                Availability::Disabled
            };
            let state = component_state(
                pointer,
                if index == self.selected {
                    NavigationState::Provisional
                } else {
                    NavigationState::None
                },
                if current {
                    CurrentState::Current
                } else {
                    CurrentState::NotCurrent
                },
                CheckedState::Unchecked,
                availability,
                if self.busy && index == self.selected {
                    OperationState::Pending
                } else {
                    OperationState::Idle
                },
            );
            let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary_rect)
                .with_primary_role(primary_role)
                .with_provisional(navigation_rect)
                .with_status(operation_rect);
            let mut bands = vec![RowContentBand::semantic(
                current_rect,
                if current {
                    Role::AccentSelection
                } else {
                    Role::TextSecondary
                },
            )];
            if !status_rect.is_empty() {
                bands.push(RowContentBand::semantic(status_rect, status_role));
            }
            if paint_row_interaction(
                frame,
                rect,
                tracks,
                &bands,
                ControlSurface::Overlay,
                state,
                theme,
            )
            .is_some()
            {
                hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
            }
        }
        if let Some(list_source) = source.drawn_list(window) {
            hits.record(
                HitLayer::Overlay,
                list,
                HitTarget::ListScroll(ListScrollTarget::MachinePicker(list_source)),
            );
        }
        if self.mode == Mode::Workspaces && self.workspace_matches.is_empty() && count > 0 {
            frame.render_widget(
                Paragraph::new(locale.machine_picker_text(
                    if self.workspace_query.text().is_empty() {
                        "no-workspaces"
                    } else {
                        "workspaces-no-matches"
                    },
                ))
                .style(theme.style(Role::TextSecondary)),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
        }
        if details > 0 {
            let online: Vec<_> = self
                .rows
                .iter()
                .filter_map(|r| match &r.observation {
                    Some(Observation::Online(o)) => Some(o),
                    _ => None,
                })
                .collect();
            let mut lines = vec![locale.machine_overview(
                online.len(),
                online.iter().map(|o| o.workspaces).sum(),
                online.iter().map(|o| o.tabs).sum(),
            )];
            if let Some(agent) = self
                .visible_rows()
                .get(self.selected)
                .and_then(|r| r.agent.as_ref())
            {
                lines.push(if agent.workspace.is_empty() {
                    locale.machine_picker_text("standalone").into()
                } else {
                    agent.workspace.clone()
                });
                lines.push(display_label(agent.title()));
                lines.push(format!("ID: {}", agent.target.member.as_str()));
                lines.push(
                    agent
                        .tokens
                        .as_map()
                        .iter()
                        .take(3)
                        .map(|(k, v)| {
                            format!(
                                "{}: {}",
                                display_label(k.as_str()),
                                display_label(v.as_str())
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" · "),
                );
            } else if let Some(row) = self.visible_rows().get(self.selected)
                && let Some(context) = &row.context
            {
                if let Some(project) = &context.project {
                    lines.push(project.clone());
                }
                lines.push(
                    context
                        .workspace
                        .clone()
                        .unwrap_or_else(|| locale.machine_picker_text("standalone").to_owned()),
                );
                let id = match &context.target.location {
                    crate::terminal_session::NativeContextLocation::Standalone(id) => id.as_str(),
                    crate::terminal_session::NativeContextLocation::Workspace {
                        workspace_id,
                        ..
                    } => workspace_id.as_str(),
                };
                lines.push(format!("ID: {id}"));
            } else if let Some(Observation::Online(o)) = self
                .visible_rows()
                .get(self.selected)
                .and_then(|r| r.observation.as_ref())
            {
                lines.push(locale.machine_details(o.projects, o.workspaces, o.tabs));
                lines.extend(o.labels.iter().take(3).map(|label| format!("  {label}")));
            }
            frame.render_widget(
                Paragraph::new(lines.join("\n")).style(theme.style(Role::TextSecondary)),
                Rect::new(inner.x, inner.bottom() - 2 - details, inner.width, details),
            );
        }
        let status = self
            .error
            .map(|error| (locale.machine_picker_text(error), Role::StatusWarning));
        let status = status.or_else(|| {
            self.busy.then(|| {
                (
                    locale.machine_picker_text("connecting"),
                    Role::StatusPending,
                )
            })
        });
        if let Some((status, role)) = status {
            frame.render_widget(
                Paragraph::new(status).style(theme.style(role)),
                Rect::new(inner.x, inner.bottom() - 2, inner.width, 2),
            );
        }
    }
}

#[cfg(test)]
mod tests;
