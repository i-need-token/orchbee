//! Identity-based machine selection and painted input ownership; no storage or network effects.
use crate::{
    client_view::{KeyCode, KeyEvent, KeyKind, Modifiers},
    localization::Locale,
    machine_discovery::{Context, Observation, display_label},
    machines::{Profile, ProfileId},
    terminal_input::{MouseAction, MouseButton, MouseEvent},
    terminal_session::{NativeContextTarget, NativeStartupTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Mode {
    #[default]
    Machines,
    Workspaces,
    Agents,
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
    fn selection(&self) -> Selection {
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
    pub(crate) current_context: Option<NativeContextTarget>,
    local_contexts: std::sync::Arc<[Context]>,
    workspace_rows: std::sync::Arc<[Row]>,
    agent_rows: std::sync::Arc<[Row]>,
    agent_observations: Vec<(Choice, crate::machine_agents::Observation)>,
    mode: Mode,
    mode_hits: Vec<(Rect, Mode)>,
    selected: usize,
    top: usize,
    painted: Vec<(Rect, Selection)>,
    indicator: Option<Rect>,
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
            current_context: None,
            local_contexts: std::sync::Arc::from([]),
            workspace_rows: std::sync::Arc::from([]),
            agent_rows: std::sync::Arc::from([]),
            agent_observations: Vec::new(),
            mode: Mode::Machines,
            mode_hits: Vec::new(),
            selected: 0,
            top: 0,
            painted: Vec::new(),
            indicator: None,
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
            let contexts = if row.choice == Choice::Local {
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
        self.workspace_rows = rows.into();
        let mut agents = Vec::new();
        for machine in self.rows.iter().filter(|r| r.enabled) {
            let observation = self
                .agent_observations
                .iter()
                .find(|(id, _)| id == &machine.choice)
                .map(|(_, o)| o);
            if let Some(crate::machine_agents::Observation::Ready(rows)) = observation
                && !rows.is_empty()
            {
                agents.extend(rows.iter().map(|agent| Row {
                    choice: machine.choice.clone(),
                    label: machine.label.clone(),
                    enabled: true,
                    observation: None,
                    context: None,
                    agent: Some(agent.clone()),
                    agent_status: None,
                }));
            } else {
                let status = match observation {
                    Some(crate::machine_agents::Observation::Ready(_)) => "no-agents",
                    Some(crate::machine_agents::Observation::Loading) => "agents-loading",
                    _ => "agents-unavailable",
                };
                agents.push(Row {
                    choice: machine.choice.clone(),
                    label: machine.label.clone(),
                    enabled: false,
                    observation: None,
                    context: None,
                    agent: None,
                    agent_status: Some(status),
                });
            }
        }
        self.agent_rows = agents.into();
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
        self.open && self.mode == Mode::Agents
    }
    pub(crate) fn observe_agents(
        &mut self,
        observations: Vec<(Choice, crate::machine_agents::Observation)>,
    ) -> bool {
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
        self.open = true;
        self.error = None;
        self.selected = self
            .visible_rows()
            .iter()
            .position(|r| r.choice == self.current)
            .unwrap_or(0);
        self.invalidate();
    }
    pub(crate) fn invalidate(&mut self) {
        self.painted.clear();
        self.mode_hits.clear();
        self.indicator = None;
    }
    /// All keys, paste and malformed events are consumed by the caller while open.
    pub(crate) fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyKind::Release {
            return;
        }
        let (code, mods) = key.normalized();
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
                self.request = None;
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
                self.request = Some(row.selection());
                self.error = None;
            } else {
                self.error = Some(row.agent_status.unwrap_or("disabled"));
            }
        }
    }
    /// Returns whether this drawn surface owns the pointer packet.
    pub(crate) fn pointer(&mut self, event: MouseEvent) -> bool {
        let point = (event.column, event.row).into();
        if !self.open {
            if self.indicator.is_some_and(|r| r.contains(point)) {
                if event.action == MouseAction::Press && event.button == MouseButton::Left {
                    self.show();
                }
                return true;
            }
            return false;
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
    pub(crate) fn render(&mut self, frame: &mut Frame<'_>, locale: Locale, theme: &Theme) {
        self.painted.clear();
        self.mode_hits.clear();
        if !self.available {
            self.indicator = None;
            return;
        }
        let area = frame.area();
        if area.width == 0 || area.height == 0 {
            self.indicator = None;
            return;
        }
        let label = if self.current == Choice::Local {
            locale.machine_picker_text("local")
        } else {
            &display_label(&self.current_label)
        };
        let width = area.width.min(28);
        let indicator = Rect::new(area.right() - width, area.bottom() - 1, width, 1);
        frame.render_widget(Clear, indicator);
        frame.render_widget(
            Paragraph::new(format!(" ◇ {label} ▾ ")).style(theme.style(Role::AccentFocus)),
            indicator,
        );
        self.indicator = Some(indicator);
        if !self.open {
            return;
        }
        let width = area.width.saturating_sub(4).min(72);
        let height = area
            .height
            .saturating_sub(3)
            .min((self.visible_rows().len() + 10).min(24) as u16);
        if width < 34 || height < 5 {
            frame.render_widget(
                Paragraph::new(locale.machine_picker_text("compact"))
                    .style(theme.style(Role::HintLabel)),
                indicator,
            );
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
                .title(format!(
                    "{} {}/{} ",
                    locale.machine_picker_text("title"),
                    (self.selected + 1).min(self.visible_rows().len()),
                    self.visible_rows().len()
                )),
            panel,
        );
        let mut inner = panel.inner(ratatui::layout::Margin::new(1, 1));
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        for (index, mode) in [Mode::Machines, Mode::Workspaces, Mode::Agents]
            .into_iter()
            .enumerate()
        {
            let column = inner.width / 3;
            let rect = Rect::new(inner.x + index as u16 * column, inner.y, column, 1);
            frame.render_widget(
                Paragraph::new(locale.machine_picker_text(match mode {
                    Mode::Machines => "machines-mode",
                    Mode::Workspaces => "workspaces-mode",
                    Mode::Agents => "agents-mode",
                }))
                .style(theme.style(if self.mode == mode {
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
                    "{name} / {} · {}",
                    display_label(&agent.target.agent),
                    agent.label
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
                locale.machine_picker_text(status)
            } else if let Some(agent) = &row.agent {
                locale.machine_picker_text(match agent.state {
                    orchbee_workbench_protocol::PanelAgentState::Idle => "agent-idle",
                    orchbee_workbench_protocol::PanelAgentState::Working => "agent-working",
                    orchbee_workbench_protocol::PanelAgentState::Blocked => "agent-blocked",
                    orchbee_workbench_protocol::PanelAgentState::Unknown => "agent-unknown",
                })
            } else if !row.enabled {
                locale.machine_picker_text("disabled")
            } else if let Some(observation) = &row.observation {
                locale.machine_picker_text(observation.key())
            } else if row.choice == self.current {
                locale.machine_picker_text("current")
            } else {
                ""
            };
            let id = match &row.choice {
                Choice::Local => String::new(),
                Choice::Remote(id) => {
                    let width = (8..=32).step_by(4).find(|width| self.rows.iter().filter(|r| matches!(&r.choice, Choice::Remote(other) if other.as_str()[..*width] == id.as_str()[..*width])).count() == 1).unwrap_or(32);
                    format!("{} · ", &id.as_str()[..width])
                }
            };
            let style = if index == self.selected {
                theme
                    .style(Role::SurfaceSelection)
                    .patch(theme.style(Role::TextInverse))
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
            let current = if self.mode != Mode::Agents
                && row.choice == self.current
                && row
                    .context
                    .as_ref()
                    .is_none_or(|c| self.current_context.as_ref() == Some(&c.target))
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
                lines.push(agent.label.clone());
                lines.push(format!("ID: {}", agent.target.member.as_str()));
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
                    locale.machine_picker_text("hint")
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
mod tests {
    use super::*;
    use crate::machines::Draft;
    fn profile(n: u8, label: &str, enabled: bool) -> Profile {
        Profile {
            id: ProfileId::parse(&format!("{n:032x}")).unwrap(),
            draft: Draft::parse(label, "example.invalid", "default").unwrap(),
            enabled,
        }
    }
    fn paint(picker: &mut Picker, locale: Locale, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| picker.render(frame, locale, &Theme::terminal_native()))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect()
    }
    fn click(rect: Rect) -> MouseEvent {
        MouseEvent {
            action: MouseAction::Press,
            button: MouseButton::Left,
            mods: Modifiers::NONE,
            column: rect.x,
            row: rect.y,
        }
    }
    #[test]
    fn rendered_rows_select_exact_ids_and_reordering_invalidates_old_hits() {
        let a = profile(1, "same", true);
        let b = profile(2, "same", true);
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(&[a.clone(), b.clone()]);
        picker.show();
        let text = paint(&mut picker, Locale::EnUs, 100, 30);
        assert!(text.contains("Machines"));
        let rect = picker.painted[2].0;
        picker.pointer(click(rect));
        assert_eq!(
            picker.request.take(),
            Some(Choice::Remote(b.id.clone()).into())
        );
        picker.replace(&[b.clone(), a]);
        picker.pointer(click(rect));
        assert!(picker.request.is_none());
        assert!(
            paint(&mut picker, Locale::ZhCn, 100, 30)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .contains("机器")
        );
        picker.pointer(click(picker.painted[1].0));
        assert_eq!(picker.request.take(), Some(Choice::Remote(b.id).into()));
    }
    #[test]
    fn disabled_selection_and_small_resize_never_fall_through_to_a_terminal() {
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(&[profile(1, "offline", false)]);
        picker.show();
        paint(&mut picker, Locale::EnUs, 80, 24);
        assert!(picker.pointer(click(picker.painted[1].0)));
        assert_eq!(picker.error, Some("disabled"));
        assert!(picker.request.is_none());
        picker.invalidate();
        paint(&mut picker, Locale::ZhCn, 1, 1);
        assert!(picker.painted.is_empty());
        assert!(picker.pointer(click(Rect::new(0, 0, 1, 1))));
    }
    #[test]
    fn machine_overview_keeps_status_visible_and_localizes_without_claiming_stale_online() {
        let p = profile(1, &"long-name".repeat(10), true);
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(std::slice::from_ref(&p));
        picker.show();
        picker.selected = 1;
        let overview = crate::machine_discovery::Overview {
            site: orchbee_workbench_protocol::SiteId::parse(p.id.as_str()).unwrap(),
            revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
            projects: 2,
            workspaces: 3,
            tabs: 7,
            labels: vec!["Project A".into(), "Workspace B".into()],
            contexts: std::sync::Arc::from([]),
        };
        picker.observe(&[(p.id.clone(), Observation::Online(overview))]);
        let text = paint(&mut picker, Locale::EnUs, 80, 24);
        assert!(text.contains("Online"));
        assert!(text.contains("3 workspaces"));
        assert!(text.contains("Project A"));
        let text = paint(&mut picker, Locale::ZhCn, 80, 24);
        assert!(
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .contains("在线")
        );
        let rect = picker.painted[1].0;
        picker.observe(&[(p.id, Observation::Stale)]);
        assert!(picker.pointer(click(rect)));
        assert!(
            picker.request.is_none(),
            "old painted rows are invalid after a status update"
        );
        let text = paint(&mut picker, Locale::EnUs, 80, 24);
        assert!(text.contains("Stale"));
        assert!(text.contains("0 machines"));
        assert!(!text.contains("Workspace B"));
    }
    fn context(n: u8) -> Context {
        Context {
            target: NativeContextTarget {
                site_id: orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", 9)).unwrap(),
                location: crate::terminal_session::NativeContextLocation::Standalone(
                    orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&format!(
                        "{n:032x}"
                    ))
                    .unwrap(),
                ),
            },
            project: None,
            workspace: Some("same workspace".into()),
        }
    }
    #[test]
    fn workspace_mode_routes_full_local_remote_identity_and_preserves_selection_on_reorder() {
        let a = profile(1, "same machine", true);
        let b = profile(2, "same machine", true);
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(&[a.clone(), b.clone()]);
        picker.observe_local(vec![context(11)]);
        let overview = crate::machine_discovery::Overview {
            site: context(11).target.site_id,
            revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
            projects: 1,
            workspaces: 2,
            tabs: 0,
            labels: Vec::new(),
            contexts: vec![context(11), context(12)].into(),
        };
        picker.observe(&[
            (a.id.clone(), Observation::Online(overview.clone())),
            (b.id.clone(), Observation::Online(overview.clone())),
        ]);
        picker.show();
        picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
        assert_eq!(picker.mode, Mode::Workspaces);
        assert_eq!(picker.visible_rows().len(), 5);
        assert!(paint(&mut picker, Locale::EnUs, 90, 26).contains("same workspace"));
        picker.key(KeyEvent::press(KeyCode::End, Modifiers::NONE));
        picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
        let expected = Selection {
            machine: Choice::Remote(b.id.clone()),
            context: Some(context(12).target.into()),
        };
        assert_eq!(picker.request.take(), Some(expected.clone()));
        let mut reordered = overview.clone();
        reordered.contexts = vec![context(12), context(11)].into();
        picker.observe(&[
            (a.id.clone(), Observation::Online(overview)),
            (b.id.clone(), Observation::Online(reordered)),
        ]);
        assert_eq!(picker.selection(), Some(expected));
        paint(&mut picker, Locale::EnUs, 90, 26);
        let old = picker.painted[picker.selected].0;
        picker.observe(&[(a.id, Observation::Stale), (b.id, Observation::Stale)]);
        picker.pointer(click(old));
        assert!(picker.request.is_none());
        assert_eq!(picker.visible_rows().len(), 1);
        paint(&mut picker, Locale::ZhCn, 90, 26);
        picker.pointer(click(picker.painted[0].0));
        assert_eq!(
            picker.request.take(),
            Some(Selection {
                machine: Choice::Local,
                context: Some(context(11).target.into())
            })
        );
        picker.pointer(click(picker.mode_hits[0].0));
        assert_eq!(picker.mode, Mode::Machines);
        assert!(picker.painted.is_empty());
    }
    #[test]
    fn workspace_mode_scrolls_and_refuses_stale_hits_after_small_resize() {
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.observe_local((1..=30).map(context).collect());
        picker.show();
        paint(&mut picker, Locale::EnUs, 80, 24);
        picker.pointer(click(picker.mode_hits[1].0));
        picker.key(KeyEvent::press(KeyCode::End, Modifiers::NONE));
        let text = paint(&mut picker, Locale::EnUs, 80, 24);
        assert!(text.contains("30/30"));
        let last = picker.painted.last().unwrap().0;
        assert_eq!(
            picker.painted.last().unwrap().1.context,
            Some(context(30).target.into())
        );
        paint(&mut picker, Locale::ZhCn, 20, 5);
        assert!(picker.painted.is_empty());
        picker.pointer(click(last));
        assert!(picker.request.is_none());
        picker.observe_local(Vec::new());
        assert!(paint(&mut picker, Locale::EnUs, 80, 24).contains("No available workspaces"));
    }
    fn agent_row() -> crate::machine_agents::Row {
        crate::machine_agents::Row {
            target: crate::machine_agents::tests::target(),
            label: "same terminal".into(),
            workspace: "project / workspace".into(),
            state: orchbee_workbench_protocol::PanelAgentState::Working,
        }
    }
    #[test]
    fn agent_mode_routes_duplicate_local_and_remote_identities_and_invalidates_stale_hits() {
        use crate::machine_agents::Observation as Agents;
        let a = profile(1, "same machine", true);
        let b = profile(2, "same machine", true);
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(&[a.clone(), b.clone()]);
        let rows: std::sync::Arc<[_]> = vec![agent_row()].into();
        picker.observe_agents(vec![
            (Choice::Local, Agents::Ready(rows.clone())),
            (Choice::Remote(a.id.clone()), Agents::Ready(rows.clone())),
            (Choice::Remote(b.id.clone()), Agents::Ready(rows)),
        ]);
        picker.show();
        picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
        picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
        assert!(picker.wants_agents());
        assert_eq!(picker.visible_rows().len(), 3);
        let text = paint(&mut picker, Locale::EnUs, 100, 30);
        assert!(text.contains("Working"));
        picker.pointer(click(picker.painted[2].0));
        let expected = Selection {
            machine: Choice::Remote(b.id.clone()),
            context: Some(NativeStartupTarget::Agent(Box::new(agent_row().target))),
        };
        assert_eq!(picker.request.take(), Some(expected.clone()));
        picker.replace(&[b.clone(), a.clone()]);
        assert_eq!(picker.selection(), Some(expected));
        paint(&mut picker, Locale::ZhCn, 100, 30);
        let old = picker.painted[picker.selected].0;
        picker.observe_agents(vec![(
            Choice::Local,
            Agents::Ready(vec![agent_row()].into()),
        )]);
        picker.pointer(click(old));
        assert!(
            picker.request.is_none(),
            "old frame is no authority for activation"
        );
        picker.key(KeyEvent::press(KeyCode::Home, Modifiers::NONE));
        picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
        assert_eq!(picker.request.take().unwrap().machine, Choice::Local);
        picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
        assert_eq!(picker.mode, Mode::Machines);
    }
    #[test]
    fn agent_mode_distinguishes_empty_loading_and_unavailable_without_fallback_selection() {
        use crate::machine_agents::Observation as Agents;
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(&[profile(1, "remote", true)]);
        picker.show();
        paint(&mut picker, Locale::EnUs, 100, 30);
        picker.pointer(click(picker.mode_hits[2].0));
        for (observation, key) in [
            (Agents::Loading, "agents-loading"),
            (Agents::Ready(std::sync::Arc::from([])), "no-agents"),
            (Agents::Unavailable, "agents-unavailable"),
        ] {
            picker.observe_agents(vec![(Choice::Local, observation)]);
            picker.key(KeyEvent::press(KeyCode::Home, Modifiers::NONE));
            picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
            assert!(picker.request.is_none());
            assert_eq!(picker.error, Some(key));
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let text = paint(&mut picker, locale, 100, 30);
                let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                let expected: String = locale
                    .machine_picker_text(key)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                assert!(compact.contains(&expected));
            }
        }
        let old = picker.painted[0].0;
        paint(&mut picker, Locale::ZhCn, 20, 5);
        assert!(picker.painted.is_empty());
        picker.pointer(click(old));
        assert!(picker.request.is_none());
    }
}
