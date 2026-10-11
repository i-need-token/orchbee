//! Identity-based machine selection and painted input ownership; no storage or network effects.
use crate::{
    client_view::{KeyCode, KeyEvent, KeyKind, Modifiers},
    localization::Locale,
    machine_discovery::{Context, Observation, display_label},
    machines::{Profile, ProfileId},
    terminal_input::{MouseAction, MouseButton, MouseEvent},
    terminal_session::{NativeContextTarget, NativeMemberTarget, NativeStartupTarget},
    terminal_text::display_width,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};
mod agents;
mod navigation;
mod sidebar;
mod spaces;
pub(crate) use agents::Control;
use agents::Navigation;
pub(crate) use navigation::{
    SIDEBAR_BINDINGS, SidebarCommand, SidebarIntent, SidebarRow, SidebarTarget,
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
enum Mode {
    #[default]
    Machines,
    Workspaces,
    Agents,
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
    agent_rows: std::sync::Arc<[Row]>,
    agent_observations: Vec<(Choice, crate::machine_agents::Observation)>,
    agent_sources: Vec<(Choice, crate::machine_agents::Observation)>,
    agent_attention_key: Option<(Choice, Option<(u64, usize)>)>,
    agent_attention_refresh: bool,
    mode: Mode,
    mode_hits: Vec<(Rect, Mode)>,
    agent_attention: crate::machine_agents::Attention,
    agent_view: crate::machine_agents::view::View,
    agent_sidebar: sidebar::AgentSidebar,
    spaces: sidebar::Sidebar<orchbee_local_site_runtime::WindowsSpaceSidebarSettings>,
    sidebar_width: crate::tui_layout::SidebarWidth,
    navigation: navigation::SidebarNavigation,
    space_observations: Vec<(Choice, crate::machine_spaces::Observation)>,
    editing_query: bool,
    control_hits: Vec<(Rect, Control)>,
    current_member: Option<NativeMemberTarget>,
    pending_navigation: Option<Navigation>,
    selected: usize,
    top: usize,
    painted: Vec<(Rect, Selection)>,
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
            agent_rows: std::sync::Arc::from([]),
            agent_observations: Vec::new(),
            agent_sources: Vec::new(),
            agent_attention_key: None,
            agent_attention_refresh: false,
            mode: Mode::Machines,
            mode_hits: Vec::new(),
            agent_attention: crate::machine_agents::Attention::default(),
            agent_view: crate::machine_agents::view::View::default(),
            agent_sidebar: sidebar::AgentSidebar::default(),
            spaces: sidebar::Sidebar::default(),
            sidebar_width: crate::tui_layout::SidebarWidth::DEFAULT,
            navigation: navigation::SidebarNavigation::default(),
            space_observations: Vec::new(),
            editing_query: false,
            control_hits: Vec::new(),
            current_member: None,
            pending_navigation: None,
            selected: 0,
            top: 0,
            painted: Vec::new(),
        }
    }
}
impl Picker {
    fn visible_rows(&self) -> &[Row] {
        match self.mode {
            Mode::Machines => &self.rows,
            Mode::Workspaces => &self.workspace_rows,
            Mode::Agents => &self.agent_rows,
        }
    }
    fn selection(&self) -> Option<Selection> {
        self.visible_rows().get(self.selected).map(Row::selection)
    }
    fn rebuild_workspaces(&mut self, selected: Option<Selection>) {
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
        if self
            .workspace_rows
            .iter()
            .map(Row::selection)
            .ne(rows.iter().map(Row::selection))
        {
            self.spaces.reset();
        }
        self.cancel_sidebar_scroll();
        self.workspace_rows = rows.into();
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
        let before =
            self.agent_sources
                .iter()
                .find_map(|(choice, observation)| match observation {
                    crate::machine_agents::Observation::Ready(round) if choice == &self.current => {
                        Some(round)
                    }
                    _ => None,
                });
        let after = observations
            .iter()
            .find_map(|(choice, observation)| match observation {
                crate::machine_agents::Observation::Ready(round) if choice == &self.current => {
                    Some(round)
                }
                _ => None,
            });
        if match (before, after) {
            (Some(before), Some(after)) => {
                before.site_id != after.site_id || before.view != after.view
            }
            (None, None) => false,
            _ => true,
        } {
            self.spaces.reset();
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
    pub(crate) fn invalidate(&mut self) {
        self.painted.clear();
        self.mode_hits.clear();
        self.control_hits.clear();
    }
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
            KeyCode::Esc => {
                self.open = false;
                self.cancel_selection();
                self.invalidate();
            }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
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
            if row.enabled {
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
    /// Returns whether this drawn surface owns the pointer packet.
    pub(crate) fn pointer(&mut self, event: MouseEvent) -> bool {
        let point = (event.column, event.row).into();
        if !self.open {
            return false;
        }
        self.pending_navigation = None;
        if event.action == MouseAction::Press
            && event.button == MouseButton::Left
            && let Some((_, control)) = self.control_hits.iter().find(|(r, _)| r.contains(point))
        {
            self.agent_control(*control);
            return true;
        }
        if event.action == MouseAction::Press && event.button == MouseButton::WheelUp {
            self.selected = self.selected.saturating_sub(1);
        }
        if event.action == MouseAction::Press && event.button == MouseButton::WheelDown {
            self.selected = (self.selected + 1).min(self.visible_rows().len().saturating_sub(1));
        }
        if event.action == MouseAction::Press
            && event.button == MouseButton::Left
            && let Some((_, mode)) = self.mode_hits.iter().find(|(r, _)| r.contains(point))
        {
            self.set_mode(*mode);
            return true;
        }
        if event.action == MouseAction::Press
            && event.button == MouseButton::Left
            && let Some((_, choice)) = self.painted.iter().find(|(r, _)| r.contains(point))
            && let Some(index) = self
                .visible_rows()
                .iter()
                .position(|r| &r.selection() == choice)
        {
            self.selected = index;
            self.choose(index);
        }
        true
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
        let label = if editing {
            locale.machine_picker_text("agent-search")
        } else {
            self.mode.label(locale)
        };
        let full = locale.machine_picker_text(if editing {
            "query-capture-hint"
        } else {
            "capture-hint"
        });
        let short = locale.machine_picker_text(if editing {
            "query-capture-short-hint"
        } else {
            "capture-short-hint"
        });
        let key = locale.machine_picker_text("capture-key");
        let hint = [full, short, key].into_iter().find(|hint| {
            display_width(label) + 1 + display_width(hint) <= usize::from(status.width)
        });
        let show_label = hint.is_some();
        let hint = hint.unwrap_or(key);
        let Ok(width) = u16::try_from(display_width(hint)) else {
            return;
        };
        if width > status.width {
            return;
        }
        if show_label {
            frame.render_widget(
                Paragraph::new(label)
                    .style(theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD)),
                Rect::new(status.x, status.y, status.width - width - 1, 1),
            );
        }
        frame.render_widget(
            Paragraph::new(hint).style(theme.style(Role::HintKey)),
            Rect::new(status.right() - width, status.y, width, 1),
        );
    }

    pub(crate) fn render(&mut self, frame: &mut Frame<'_>, locale: Locale, theme: &Theme) {
        self.painted.clear();
        self.mode_hits.clear();
        self.control_hits.clear();
        if !self.available {
            return;
        }
        let area = frame.area();
        if !self.open {
            return;
        }
        self.render_capture_status(frame, locale, theme);
        let width = area.width.saturating_sub(4).min(72);
        let height = area.height.saturating_sub(3).min(
            (self.visible_rows().len() + if self.mode == Mode::Agents { 13 } else { 10 }).min(24)
                as u16,
        );
        if width < 34 || height < 5 {
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
        let mut inner = panel.inner(ratatui::layout::Margin::new(1, 1));
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        if inner.height >= 4 {
            for (index, mode) in [Mode::Machines, Mode::Workspaces, Mode::Agents]
                .into_iter()
                .enumerate()
            {
                let column = inner.width / 3;
                let rect = Rect::new(inner.x + index as u16 * column, inner.y, column, 1);
                frame.render_widget(
                    Paragraph::new(mode.label(locale)).style(theme.style(if self.mode == mode {
                        Role::AccentFocus
                    } else {
                        Role::TextSecondary
                    })),
                    rect,
                );
                self.mode_hits.push((rect, mode));
            }
            inner.y += 1;
            inner.height -= 1;
        }
        self.render_agent_controls(frame, &mut inner, locale, theme);
        let details = if inner.height >= 9 { 5 } else { 0 };
        let count = usize::from(inner.height.saturating_sub(2 + details));
        if self.selected < self.top {
            self.top = self.selected;
        }
        if self.selected >= self.top + count && count > 0 {
            self.top = self.selected + 1 - count;
        }
        let mut painted = Vec::new();
        for (index, row) in self
            .visible_rows()
            .iter()
            .enumerate()
            .skip(self.top)
            .take(count)
        {
            let rect = Rect::new(inner.x, inner.y + (index - self.top) as u16, inner.width, 1);
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
                    orchbee_workbench_protocol::MetadataState::Working => "agent-working",
                    orchbee_workbench_protocol::MetadataState::Blocked => "agent-blocked",
                    orchbee_workbench_protocol::MetadataState::Unknown => "agent-unknown",
                });
                // Sanitize the owned label before it enters a terminal render widget.
                agent
                    .state_label()
                    .map(display_label)
                    .unwrap_or_else(|| fallback.into())
            } else if !row.enabled {
                locale.machine_picker_text("disabled").to_owned()
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
            let style = if index == self.selected {
                theme.style(Role::SurfaceSelection)
            } else if !row.enabled {
                theme.style(Role::TextDisabled)
            } else {
                theme.style(Role::TextPrimary)
            };
            let status_width = match self.mode {
                Mode::Machines => rect.width.min(16),
                Mode::Agents => rect.width.min(18),
                Mode::Workspaces => 0,
            };
            let name_rect = Rect::new(rect.x, rect.y, rect.width.saturating_sub(status_width), 1);
            let status_rect = Rect::new(name_rect.right(), rect.y, status_width, 1);
            let current = if row
                .agent
                .as_ref()
                .is_some_and(|a| self.current_agent(&row.choice, a))
                || (self.mode != Mode::Agents
                    && row.choice == self.current
                    && row
                        .context
                        .as_ref()
                        .is_none_or(|c| self.current_context.as_ref() == Some(&c.target)))
            {
                "•"
            } else {
                " "
            };
            frame.render_widget(
                Paragraph::new(format!("{current}{id}{name}")).style(style),
                name_rect,
            );
            frame.render_widget(Paragraph::new(suffix).style(style), status_rect);
            painted.push((rect, row.selection()));
        }
        self.painted = painted;
        if self.mode == Mode::Workspaces && self.workspace_rows.is_empty() && count > 0 {
            frame.render_widget(
                Paragraph::new(locale.machine_picker_text("no-workspaces"))
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
                lines.push(
                    context
                        .project
                        .as_deref()
                        .unwrap_or(locale.machine_picker_text("standalone"))
                        .to_owned(),
                );
                lines.push(context.workspace.clone().unwrap_or_default());
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
            .map(|e| locale.machine_picker_text(e))
            .unwrap_or_else(|| {
                if self.busy {
                    locale.machine_picker_text("connecting")
                } else {
                    locale.machine_picker_text(if self.mode == Mode::Agents {
                        if self.editing_query {
                            "agent-query-hint"
                        } else {
                            "agent-controls-hint"
                        }
                    } else {
                        "hint"
                    })
                }
            });
        frame.render_widget(
            Paragraph::new(status).style(theme.style(if self.error.is_some() {
                Role::StatusWarning
            } else {
                Role::HintLabel
            })),
            Rect::new(inner.x, inner.bottom() - 2, inner.width, 2),
        );
    }
}

#[cfg(test)]
mod tests;
