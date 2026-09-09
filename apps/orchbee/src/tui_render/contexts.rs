use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::terminal_text::sanitize_terminal_text;

use super::{EMPHASIS, TEXT};

pub(super) const MAX_VISIBLE_CONTEXTS: usize = 9;

#[derive(Clone, Copy)]
pub(super) struct ContextDisplay<'a> {
    pub(super) position: usize,
    pub(super) active: bool,
    pub(super) project_label: Option<&'a str>,
    pub(super) workspace_label: Option<&'a str>,
    pub(super) standalone_label: &'a str,
}

pub(super) fn render_navigator<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    context_count: usize,
    active_position: Option<usize>,
    context_at: impl Fn(usize) -> Option<ContextDisplay<'a>>,
) {
    let lines = navigator_lines(
        usize::from(area.height),
        context_count,
        active_position,
        &context_at,
    );
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn location_label(context: ContextDisplay<'_>) -> String {
    context_label(context)
}

fn navigator_lines<'a>(
    height: usize,
    context_count: usize,
    active_position: Option<usize>,
    context_at: &impl Fn(usize) -> Option<ContextDisplay<'a>>,
) -> Vec<Line<'static>> {
    let visible = height.min(MAX_VISIBLE_CONTEXTS).min(context_count);
    if visible == 0 {
        return Vec::new();
    }

    let active_index = active_position
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < context_count);
    let max_start = context_count.saturating_sub(visible);
    let start = active_index
        .map(|index| index.saturating_sub(visible / 2))
        .unwrap_or(0)
        .min(max_start);
    let end = start.saturating_add(visible).min(context_count);

    (start..end)
        .map_while(context_at)
        .map(|context| {
            Line::from(Span::styled(
                context_row(context),
                navigator_style(context.active),
            ))
        })
        .collect()
}

fn context_row(context: ContextDisplay<'_>) -> String {
    let marker = if context.active { '*' } else { ' ' };
    format!("[{marker}{}] {}", context.position, context_label(context))
}

fn context_label(context: ContextDisplay<'_>) -> String {
    match (context.project_label, context.workspace_label) {
        (Some(project), Some(workspace)) => format!(
            "{} / {}",
            sanitize_terminal_text(project),
            sanitize_terminal_text(workspace)
        ),
        (Some(project), None) => sanitize_terminal_text(project).into_owned(),
        (None, Some(workspace)) => sanitize_terminal_text(workspace).into_owned(),
        (None, None) => sanitize_terminal_text(context.standalone_label).into_owned(),
    }
}

fn navigator_style(active: bool) -> Style {
    if active {
        Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(TEXT)
    }
}

#[cfg(test)]
mod tests;
