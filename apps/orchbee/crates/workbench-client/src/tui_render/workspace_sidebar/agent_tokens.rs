//! Pure token projection from the already-fenced Agent row. No I/O or Agent authority.
use super::{
    ClientView, Line, Locale, ProjectedSidebarLines, Role, Span, Theme, display_label,
    display_width, full_name, machine_label, truncate_display,
};
use crate::machine_picker::Row;
use orchbee_local_runtime::{AgentSidebarToken, AgentSidebarTokenKind as Kind, SidebarTokenStyle};

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
    fn resolved(&self, token: &AgentSidebarToken) -> Option<(String, SidebarTokenStyle)> {
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
) -> Option<&'a [Vec<AgentSidebarToken>]> {
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
        return 1;
    };
    rows.iter()
        .filter(|tokens| tokens.iter().any(|token| context.resolved(token).is_some()))
        .count()
        .max(1) as u16
}
/// A cut default title keeps at least this many cells before its ellipsis beside the provider.
const MINIMUM_TITLE_CELLS: usize = 3;

/// The default row: what names the Agent, then its provider in quieter text, or the provider
/// alone; its leading glyph and pointer tooltip carry state. Within `width` the title gives way
/// first, so the provider stays whole while the title keeps its first few cells; narrower still,
/// the title alone tells sibling Agents apart.
fn default_row(
    agent: &crate::machine_agents::Row,
    width: usize,
    theme: &Theme,
) -> ProjectedSidebarLines {
    let style = |role| theme.sidebar_token_style(role, Default::default(), false, false);
    let provider = display_label(agent.provider());
    let Some(title) = agent.identity_title() else {
        return ProjectedSidebarLines::new(vec![Line::from(Span::styled(
            provider.clone(),
            style(Role::TextPrimary),
        ))])
        .with_names(vec![(0, full_name(provider))]);
    };
    let separator = format!(" {} ", theme.glyphs.middle_dot);
    let whole = format!("{title}{separator}{provider}");
    let ellipsis = theme.glyphs.ellipsis;
    let trailing = display_width(&separator) + display_width(&provider);
    let kept = display_width(&title).min(MINIMUM_TITLE_CELLS + display_width(ellipsis));
    let spans = if width >= trailing + kept {
        vec![
            Span::styled(
                truncate_display(&title, width - trailing, ellipsis),
                style(Role::TextPrimary),
            ),
            Span::styled(separator, style(Role::TextSecondary)),
            Span::styled(provider, style(Role::TextSecondary)),
        ]
    } else {
        vec![Span::styled(title, style(Role::TextPrimary))]
    };
    ProjectedSidebarLines::new(vec![Line::from(spans)]).with_names(vec![(0, full_name(whole))])
}

/// `width` is the cells the first line may take; configured rows are cut to it later.
pub(in crate::tui_render) fn projection(
    view: &ClientView,
    row: &Row,
    locale: Locale,
    theme: &Theme,
    width: usize,
) -> ProjectedSidebarLines {
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
        return ProjectedSidebarLines::new(lines);
    };
    let (_, state_role) = super::super::sidebar_text::state_role(context.agent.display_state());
    let text_style =
        |role, _first_line| theme.sidebar_token_style(role, Default::default(), false, false);
    let Some(rows) = configured(view, context.agent) else {
        return default_row(context.agent, width, theme);
    };
    let mut lines = Vec::new();
    let mut names = Vec::new();
    for row in rows {
        let mut spans = Vec::new();
        let mut previous_icon = false;
        for token in row {
            if token.kind() == Kind::StateIcon {
                continue;
            }
            let Some((mut text, style)) = context.resolved(token) else {
                continue;
            };
            // A Workspace, Tab, pane or terminal title is a name the row may cut.
            if matches!(
                token.kind(),
                Kind::Workspace
                    | Kind::Tab
                    | Kind::Pane
                    | Kind::TerminalTitle
                    | Kind::TerminalTitleStripped
            ) {
                names.push((lines.len(), full_name(text.clone())));
            }
            if !spans.is_empty() {
                spans.push(Span::styled(
                    if previous_icon {
                        " ".into()
                    } else {
                        format!(" {} ", theme.glyphs.middle_dot)
                    },
                    text_style(Role::TextSecondary, lines.is_empty()),
                ));
            }
            previous_icon = token.kind() == Kind::StateIcon;
            if previous_icon {
                text = theme.agent_glyph(context.agent.display_state()).into();
            }
            let token_role = if matches!(token.kind(), Kind::StateIcon | Kind::StateText) {
                state_role
            } else if lines.is_empty() {
                Role::TextPrimary
            } else {
                Role::TextSecondary
            };
            let projected = theme.sidebar_token_style(token_role, style, false, false);
            spans.push(Span::styled(text, projected));
        }
        if !spans.is_empty() {
            lines.push(Line::from(spans));
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
    }
    ProjectedSidebarLines::new(lines).with_names(names)
}

#[cfg(test)]
pub(in crate::tui_render) fn lines(
    view: &ClientView,
    row: &Row,
    locale: Locale,
    theme: &Theme,
) -> Vec<Line<'static>> {
    projection(view, row, locale, theme, usize::MAX)
        .into_parts()
        .0
}
