//! Ephemeral Agent view controls and a bounded global-navigation intent.
use super::*;
use crate::machine_agents::{
    Observation as Agents,
    view::{Entry, Scope},
};
use std::time::{Duration, Instant};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Control {
    State,
    Scope,
    Order,
    Search,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Navigation {
    target: NavigationTarget,
    deadline: Instant,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NavigationTarget {
    Cycle { reverse: bool },
    Index(u8),
}
impl Picker {
    pub(super) fn source_spec(&self) -> Option<&orchbee_workbench_protocol::AgentViewSpec> {
        self.agent_observations
            .iter()
            .find(|(id, _)| id == &self.current)
            .and_then(|(_, o)| match o {
                Agents::Ready(c) => c.view.spec.as_ref(),
                _ => None,
            })
    }
    pub(super) fn rebuild_agents(&mut self) {
        let source = self
            .agent_observations
            .iter()
            .find(|(id, _)| id == &self.current)
            .and_then(|(_, o)| match o {
                Agents::Ready(checkpoint) => Some(&checkpoint.view),
                _ => None,
            });
        if source.is_none() {
            let status = if self
                .agent_observations
                .iter()
                .any(|(id, o)| id == &self.current && matches!(o, Agents::Unavailable))
            {
                "agents-unavailable"
            } else {
                "agents-loading"
            };
            self.agent_rows = vec![Row {
                choice: self.current.clone(),
                label: self.current_label.clone(),
                enabled: false,
                observation: None,
                context: None,
                agent: None,
                agent_status: Some(status),
            }]
            .into();
            return;
        }
        let candidates = self.rows.iter().filter(|m| m.enabled).flat_map(|machine| {
            let rows = self
                .agent_observations
                .iter()
                .find(|(id, _)| id == &machine.choice)
                .and_then(|(_, o)| match o {
                    Agents::Ready(rows) => Some(rows.rows.as_ref()),
                    _ => None,
                })
                .unwrap_or(&[]);
            rows.iter().map(move |row| Entry {
                machine: &machine.choice,
                label: &machine.label,
                row,
            })
        });
        let eligible = self.agent_view.project_source(
            candidates.filter(|_| source.is_some()),
            &self.current,
            self.current_context.as_ref(),
            self.current_member.as_ref(),
            source.and_then(|s| s.spec.as_ref()),
        );
        let mut rows: Vec<_> = eligible
            .into_iter()
            .map(|e| Row {
                choice: e.machine.clone(),
                label: e.label.into(),
                enabled: true,
                observation: None,
                context: None,
                agent: Some(e.row.clone()),
                agent_status: None,
            })
            .collect();
        for machine in self.rows.iter().filter(|m| {
            m.enabled && (self.agent_view.scope == Scope::All || m.choice == self.current)
        }) {
            if rows.iter().any(|r| r.choice == machine.choice) {
                continue;
            }
            let status = match self
                .agent_observations
                .iter()
                .find(|(id, _)| id == &machine.choice)
                .map(|(_, o)| o)
            {
                Some(Agents::Ready(rows)) if rows.is_empty() => "no-agents",
                Some(Agents::Ready(_)) => "agents-no-matches",
                Some(Agents::Loading) => "agents-loading",
                _ => "agents-unavailable",
            };
            rows.push(Row {
                choice: machine.choice.clone(),
                label: machine.label.clone(),
                enabled: false,
                observation: None,
                context: None,
                agent: None,
                agent_status: Some(status),
            });
        }
        self.agent_rows = rows.into();
    }
    pub(crate) fn observe_focus(
        &mut self,
        machine: Choice,
        context: Option<NativeContextTarget>,
        member: Option<NativeMemberTarget>,
    ) -> bool {
        if self.current == machine
            && self.current_context == context
            && self.current_member == member
        {
            return false;
        }
        let selected = self.selection();
        self.agent_attention_refresh = true;
        self.current = machine;
        self.current_context = context;
        self.current_member = member;
        self.sidebar.reset();
        self.rebuild_workspaces(selected);
        true
    }
    pub(crate) fn agent_attention_bindings(
        &self,
    ) -> Vec<orchbee_agent_observation_protocol::AgentLaunchBinding> {
        self.agent_observations
            .iter()
            .find(|(id, _)| id == &self.current)
            .and_then(|(_, o)| match o {
                Agents::Ready(round) => Some(
                    round
                        .rows
                        .iter()
                        .map(|r| r.target.physical.clone())
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default()
    }
    pub(crate) fn reset_agent_attention_read(&mut self) {
        self.agent_attention_key = None;
        self.agent_attention_refresh = true;
    }
    pub(crate) fn needs_agent_attention(&mut self, version: Option<(u64, usize)>) -> bool {
        if !self.wants_agents() {
            return false;
        }
        let key = (self.current.clone(), version);
        let changed = self.agent_attention_key.as_ref() != Some(&key);
        self.agent_attention_key = Some(key);
        if version.is_none() {
            return false;
        }
        let refresh = std::mem::take(&mut self.agent_attention_refresh);
        changed || refresh
    }
    pub(crate) fn observe_agent_attention(
        &mut self,
        snapshots: &[orchbee_workbench_protocol::ClientTerminalAgentAttentionSnapshot],
    ) -> bool {
        let mut changed = false;
        for snapshot in snapshots {
            changed |= self.agent_attention.acknowledge(&self.current, snapshot);
        }
        changed && self.project_agent_observations(self.agent_sources.clone())
    }
    pub(crate) fn current_agent(&self, machine: &Choice, row: &crate::machine_agents::Row) -> bool {
        machine == &self.current
            && self.current_context.as_ref() == Some(&row.target.context)
            && self.current_member.as_ref().is_some_and(|m| {
                m.tab == row.target.tab
                    && m.panel == row.target.panel
                    && m.presentation == row.target.member
            })
    }
    pub(crate) fn navigate_agent(&mut self, reverse: bool, now: Instant) {
        self.request_agent(NavigationTarget::Cycle { reverse }, now);
    }
    pub(crate) fn focus_agent(&mut self, index: u8, now: Instant) {
        self.request_agent(NavigationTarget::Index(index), now);
    }
    fn request_agent(&mut self, target: NavigationTarget, now: Instant) {
        self.show();
        self.set_mode(Mode::Agents);
        self.pending_navigation = Some(Navigation {
            target,
            deadline: now + Duration::from_secs(8),
        });
    }
    pub(crate) fn drive_agent_navigation(&mut self, now: Instant) -> bool {
        let Some(intent) = self.pending_navigation else {
            return false;
        };
        if !self.open || self.mode != Mode::Agents {
            self.pending_navigation = None;
            return false;
        }
        if now >= intent.deadline {
            self.pending_navigation = None;
            self.error = Some("agents-unavailable");
            return true;
        }
        if self
            .agent_rows
            .iter()
            .any(|r| r.agent_status == Some("agents-loading"))
        {
            return false;
        }
        self.pending_navigation = None;
        let rows: Vec<_> = self
            .agent_rows
            .iter()
            .filter(|r| r.enabled && r.agent.is_some())
            .collect();
        if rows.is_empty() {
            if matches!(intent.target, NavigationTarget::Index(_))
                && self
                    .agent_rows
                    .iter()
                    .all(|r| matches!(r.agent_status, Some("no-agents" | "agents-no-matches")))
            {
                self.open = false;
                self.invalidate();
                return true;
            }
            self.error = Some(
                if self
                    .agent_rows
                    .iter()
                    .all(|r| r.agent_status == Some("agents-unavailable"))
                {
                    "agents-unavailable"
                } else {
                    "agents-no-matches"
                },
            );
            return true;
        }
        let current = rows
            .iter()
            .position(|r| self.current_agent(&r.choice, r.agent.as_ref().unwrap()));
        let index = match intent.target {
            NavigationTarget::Index(index) if usize::from(index) >= rows.len() => {
                self.open = false;
                self.invalidate();
                return true;
            }
            NavigationTarget::Index(index) => usize::from(index),
            NavigationTarget::Cycle { reverse } => match (current, reverse) {
                (Some(i), true) => (i + rows.len() - 1) % rows.len(),
                (Some(i), false) => (i + 1) % rows.len(),
                (None, true) => rows.len() - 1,
                (None, false) => 0,
            },
        };
        if current == Some(index) {
            self.open = false;
            self.invalidate();
        } else {
            self.request = Some(rows[index].selection());
        }
        true
    }
    pub(super) fn agent_control(&mut self, control: Control) {
        self.pending_navigation = None;
        let selected = self.selection();
        match control {
            Control::State => self.agent_view.cycle_state(),
            Control::Scope => self.agent_view.cycle_scope(),
            Control::Order if self.source_spec().is_some_and(|s| !s.sort.is_empty()) => {
                self.error = Some("agent-source-sort");
                return;
            }
            Control::Order => self.agent_view.cycle_order(),
            Control::Search => self.editing_query = true,
        };
        self.error = None;
        self.sidebar.reset();
        self.rebuild_workspaces(selected);
    }
    pub(super) fn agent_key(&mut self, code: KeyCode, mods: Modifiers) -> bool {
        if self.mode != Mode::Agents {
            return false;
        }
        if self.editing_query {
            match (code, mods) {
                (KeyCode::Esc, Modifiers::NONE) | (KeyCode::Char('m'), Modifiers::CTRL) => {
                    self.editing_query = false
                }
                (KeyCode::Backspace, Modifiers::NONE) => {
                    self.agent_view.query.pop();
                }
                (KeyCode::Char('u'), Modifiers::CTRL) => self.agent_view.query.clear(),
                (KeyCode::Char(c), m)
                    if m == Modifiers::NONE || m == Modifiers::NONE.with_shift() =>
                {
                    self.agent_view
                        .push(if m.shift { c.to_ascii_uppercase() } else { c })
                }
                _ => {}
            }
            let selected = self.selection();
            self.error = None;
            self.sidebar.reset();
            self.rebuild_workspaces(selected);
            return true;
        }
        if mods != Modifiers::NONE {
            return false;
        }
        let control = match code {
            KeyCode::Char('f') => Control::State,
            KeyCode::Char('s') => Control::Order,
            KeyCode::Char('c') => Control::Scope,
            KeyCode::Char('/') => Control::Search,
            _ => return false,
        };
        self.agent_control(control);
        true
    }
    pub(super) fn render_agent_controls(
        &mut self,
        frame: &mut Frame<'_>,
        inner: &mut Rect,
        locale: Locale,
        theme: &Theme,
    ) {
        if self.mode != Mode::Agents || inner.height < 4 {
            return;
        }
        for (index, control) in [Control::State, Control::Order, Control::Scope]
            .into_iter()
            .enumerate()
        {
            let width = inner.width / 3;
            let rect = Rect::new(inner.x + index as u16 * width, inner.y, width, 1);
            frame.render_widget(
                Paragraph::new(
                    if control == Control::Order
                        && self.source_spec().is_some_and(|s| !s.sort.is_empty())
                    {
                        format!("s {}", locale.machine_picker_text("agent-source-sort"))
                    } else {
                        locale.machine_agent_control(control, &self.agent_view)
                    },
                )
                .style(theme.style(Role::AccentFocus)),
                rect,
            );
            self.control_hits.push((rect, control));
        }
        let rect = Rect::new(inner.x, inner.y + 1, inner.width, 1);
        let heading = format!("/ {}: ", locale.machine_picker_text("agent-search"));
        let mut query = self.agent_view.query.as_str();
        if self.editing_query {
            let width = usize::from(rect.width)
                .saturating_sub(ratatui::text::Line::from(heading.as_str()).width() + 1);
            while ratatui::text::Line::from(query).width() > width {
                let Some(first) = query.chars().next() else {
                    break;
                };
                query = &query[first.len_utf8()..];
            }
        }
        frame.render_widget(
            Paragraph::new(format!(
                "{heading}{query}{}{}",
                if self.editing_query { "▏" } else { "" },
                if self.editing_query {
                    String::new()
                } else {
                    self.source_spec().map_or(String::new(), |s| {
                        format!(
                            " · {}: {}",
                            locale.machine_picker_text("agent-source"),
                            crate::machine_discovery::display_label(
                                s.label.as_deref().unwrap_or(&s.source)
                            )
                        )
                    })
                }
            ))
            .style(theme.style(if self.editing_query {
                Role::AccentFocus
            } else {
                Role::HintLabel
            })),
            rect,
        );
        self.control_hits.push((rect, Control::Search));
        inner.y += 2;
        inner.height -= 2;
    }
}
