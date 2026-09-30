//! Pure token projection from the already-fenced Agent row. No I/O or Agent authority.
use super::*;
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
        let (key, _) = state_role(agent);
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
pub(super) fn height(view: &ClientView, row: &Row, locale: Locale) -> u16 {
    let Some(context) = Context::new(view, row, locale) else {
        return 2;
    };
    let Some(rows) = configured(view, context.agent) else {
        return 2;
    };
    rows.iter()
        .filter(|tokens| tokens.iter().any(|token| context.resolved(token).is_some()))
        .count()
        .max(1) as u16
}
pub(super) fn lines(
    view: &ClientView,
    row: &Row,
    index: usize,
    locale: Locale,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let Some(context) = Context::new(view, row, locale) else {
        return vec![
            Line::styled(
                machine_label(view, row, locale),
                theme.style(Role::TextDisabled),
            ),
            Line::styled(
                locale.machine_picker_text(row.sidebar_status().unwrap_or("agents-unavailable")),
                theme.style(Role::TextDisabled),
            ),
        ];
    };
    let (_, state_role) = state_role(context.agent);
    let role = if view.machines.current_agent(&row.choice, context.agent) {
        Role::AccentFocus
    } else {
        state_role
    };
    let marker = if view.machines.current_agent(&row.choice, context.agent) {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let ordinal = if index < 9 {
        format!("{} ", index + 1)
    } else {
        String::new()
    };
    let identity = format!("{marker}{ordinal}");
    let Some(rows) = configured(view, context.agent) else {
        return vec![
            Line::styled(
                format!(
                    "{identity}{} {}",
                    display_label(context.state),
                    display_label(context.agent.provider())
                ),
                theme.style(role),
            ),
            Line::styled(
                format!(
                    "{} / {} / {}",
                    context.machine,
                    display_label(&context.agent.workspace),
                    display_label(context.agent.title())
                ),
                theme.style(role),
            ),
        ];
    };
    let mut lines = Vec::new();
    for row in rows {
        let mut spans = Vec::new();
        let mut previous_icon = false;
        for token in row {
            let Some((mut text, style)) = context.resolved(token) else {
                continue;
            };
            if !spans.is_empty() {
                spans.push(Span::styled(
                    if previous_icon { " " } else { " · " },
                    theme.style(Role::TextSecondary),
                ));
            }
            previous_icon = token.kind() == Kind::StateIcon;
            if previous_icon {
                text = theme.glyphs.status_bullet.into();
            }
            let token_role = if matches!(token.kind(), Kind::StateIcon | Kind::StateText) {
                state_role
            } else {
                Role::TextPrimary
            };
            spans.push(Span::styled(
                text,
                theme.sidebar_token_style(token_role, style),
            ));
        }
        if !spans.is_empty() {
            lines.push(Line::from(spans));
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
    }
    lines[0].spans.insert(
        0,
        Span::styled(
            identity,
            theme.style(if role == Role::AccentFocus {
                Role::AccentFocus
            } else {
                Role::HintKey
            }),
        ),
    );
    lines
}
fn state_role(agent: &crate::machine_agents::Row) -> (&'static str, Role) {
    match agent.display_state() {
        MetadataState::Blocked => ("agent-blocked", Role::StatusWarning),
        MetadataState::Done => ("agent-done", Role::StatusSuccess),
        MetadataState::Working => ("agent-working", Role::StatusInfo),
        MetadataState::Idle => ("agent-idle", Role::TextPrimary),
        MetadataState::Unknown => ("agent-unknown", Role::TextSecondary),
    }
}
/// Preserve style boundaries while truncating the entire line at a grapheme boundary.
pub(super) fn fit(line: Line<'static>, width: usize, ellipsis: &str) -> Line<'static> {
    let text = line
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>();
    let clipped = truncate_display(&text, width, ellipsis);
    if clipped == text {
        return line;
    }
    let mut remaining = clipped
        .strip_suffix(ellipsis)
        .map_or(clipped.len(), str::len);
    let mut spans = Vec::new();
    for span in line.spans {
        let length = remaining.min(span.content.len());
        if length > 0 {
            spans.push(Span::styled(span.content[..length].to_owned(), span.style));
        }
        remaining -= length;
        if remaining == 0 {
            break;
        }
    }
    if clipped.ends_with(ellipsis) {
        spans.push(Span::styled(ellipsis.to_owned(), line.style));
    }
    Line { spans, ..line }
}
