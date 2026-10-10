//! Pure token projection from the already-fenced Agent row. No I/O or Agent authority.
use super::*;
use crate::machine_picker::Row;
use orchbee_local_site_runtime::{
    WindowsAgentSidebarToken, WindowsAgentSidebarTokenKind as Kind, WindowsSidebarTokenStyle,
};

struct Context<'a> {
    agent: &'a crate::machine_agents::Row,
    machine: String,
    state: &'a str,
}
impl<'a> Context<'a> {
    fn new(view: &ClientView, row: &'a Row, locale: Locale) -> Option<Self> {
        let agent = row.sidebar_agent()?;
        let (key, _) = super::super::sidebar_text::state_role(agent.display_state());
        Some(Self {
            agent,
            machine: machine_label(view, row, locale),
            state: agent
                .state_label()
                .unwrap_or(locale.machine_picker_text(key)),
        })
    }
    fn value(&self, kind: Kind<'_>) -> Option<&str> {
        match kind {
            Kind::StateIcon => None,
            Kind::StateText => Some(self.state),
            Kind::Machine => Some(&self.machine),
            Kind::Workspace => Some(&self.agent.workspace),
            Kind::Tab => self.agent.tab_label.as_deref(),
            Kind::Pane => Some(self.agent.title()),
            Kind::Agent => Some(self.agent.provider()),
            Kind::TerminalTitle | Kind::TerminalTitleStripped => {
                let orchbee_workbench_protocol::TerminalTitleObservation::Observed {
                    title, ..
                } = &self.agent.terminal_title
                else {
                    return None;
                };
                let value = title.as_deref()?;
                if kind == Kind::TerminalTitle {
                    Some(value)
                } else {
                    crate::terminal_title::stripped(value)
                }
            }
            Kind::Custom(key) => self
                .agent
                .tokens
                .as_map()
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v.as_str()),
        }
    }
    fn resolved(
        &self,
        token: &WindowsAgentSidebarToken,
    ) -> Option<(String, WindowsSidebarTokenStyle)> {
        let value = self.value(token.kind());
        let style = token.style_for_value(value)?;
        let text = if token.kind() == Kind::StateIcon {
            String::new()
        } else {
            display_label(value?)
        };
        (token.kind() == Kind::StateIcon || !text.is_empty()).then_some((text, style))
    }
}
fn configured<'a>(
    view: &'a ClientView,
    agent: &crate::machine_agents::Row,
) -> Option<&'a [Vec<WindowsAgentSidebarToken>]> {
    let canonical =
        orchbee_agent_observation_protocol::AgentReportProvider::lookup(&agent.target.agent)
            .map(|p| p.report_label());
    view.machines
        .sidebar_settings()
        .rows_for_agent(&agent.target.agent, canonical)
}
pub(in crate::tui_render) fn height(view: &ClientView, row: &Row, locale: Locale) -> u16 {
    let Some(context) = Context::new(view, row, locale) else {
        return 1 + u16::from(
            view.machines
                .rows
                .iter()
                .filter(|machine| machine.enabled)
                .count()
                > 1,
        );
    };
    let Some(rows) = configured(view, context.agent) else {
        return 2;
    };
    rows.iter()
        .filter(|tokens| tokens.iter().any(|token| context.resolved(token).is_some()))
        .count()
        .max(1) as u16
}
pub(in crate::tui_render) fn lines(
    view: &ClientView,
    row: &Row,
    ordinal: usize,
    locale: Locale,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let Some(context) = Context::new(view, row, locale) else {
        let mut lines = Vec::new();
        if view
            .machines
            .rows
            .iter()
            .filter(|machine| machine.enabled)
            .count()
            > 1
        {
            lines.push(Line::styled(
                machine_label(view, row, locale),
                theme.style(Role::TextDisabled),
            ));
        }
        lines.push(Line::styled(
            locale.machine_picker_text(row.sidebar_status().unwrap_or("agents-unavailable")),
            theme.style(Role::TextDisabled),
        ));
        return lines;
    };
    let (_, state_role) = super::super::sidebar_text::state_role(context.agent.display_state());
    let current = view.machines.current_agent(&row.choice, context.agent);
    let text_style = |role| {
        if current {
            theme.current_context_style(theme.style(role))
        } else {
            theme.style(role)
        }
    };
    let marker = if current {
        theme.current_context_marker()
    } else {
        " "
    };
    let ordinal = if ordinal < 9 {
        format!("{marker}{} ", ordinal + 1)
    } else {
        format!("{marker}  ")
    };
    let Some(rows) = configured(view, context.agent) else {
        return vec![
            Line::from(vec![
                Span::styled(ordinal, text_style(Role::HintLabel)),
                Span::styled(
                    format!("{} ", display_label(context.state)),
                    text_style(state_role),
                ),
                Span::styled(
                    display_label(context.agent.provider()),
                    if current {
                        text_style(Role::TextPrimary).add_modifier(Modifier::BOLD)
                    } else {
                        text_style(Role::TextPrimary)
                    },
                ),
            ]),
            Line::styled(
                format!(
                    "   {} / {} / {}",
                    context.machine,
                    display_label(&context.agent.workspace),
                    display_label(context.agent.title())
                ),
                text_style(Role::TextSecondary),
            ),
        ];
    };
    let mut lines = Vec::new();
    let mut continuation_indent = 3;
    for row in rows {
        let mut spans = Vec::new();
        let mut previous_icon = false;
        for token in row {
            let Some((mut text, style)) = context.resolved(token) else {
                continue;
            };
            if lines.is_empty() && spans.is_empty() && token.kind() == Kind::StateIcon {
                continuation_indent = 5;
            }
            if !spans.is_empty() {
                spans.push(Span::styled(
                    if previous_icon { " " } else { " · " },
                    text_style(Role::TextSecondary),
                ));
            }
            previous_icon = token.kind() == Kind::StateIcon;
            if previous_icon {
                text = theme.glyphs.status_bullet.into();
            }
            let token_role = if matches!(token.kind(), Kind::StateIcon | Kind::StateText) {
                state_role
            } else if lines.is_empty() {
                Role::TextPrimary
            } else {
                Role::TextSecondary
            };
            let mut projected = theme.sidebar_token_style(token_role, style, current);
            if current && token_role == Role::TextPrimary && style.bold.is_none() {
                projected = projected.add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(text, projected));
        }
        if !spans.is_empty() {
            if !lines.is_empty() {
                spans.insert(0, Span::raw(" ".repeat(continuation_indent)));
            }
            lines.push(Line::from(spans));
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
    }
    lines[0]
        .spans
        .insert(0, Span::styled(ordinal, text_style(Role::HintLabel)));
    lines
}
