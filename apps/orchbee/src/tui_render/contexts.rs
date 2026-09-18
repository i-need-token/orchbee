use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_theme::{Role, Theme},
};

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
    theme: &Theme,
) {
    let lines = navigator_lines(
        usize::from(area.width),
        usize::from(area.height),
        context_count,
        active_position,
        &context_at,
        theme,
    );
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn location_label(context: ContextDisplay<'_>) -> String {
    context_label(context)
}

fn navigator_lines<'a>(
    width: usize,
    height: usize,
    context_count: usize,
    active_position: Option<usize>,
    context_at: &impl Fn(usize) -> Option<ContextDisplay<'a>>,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let visible = height.min(MAX_VISIBLE_CONTEXTS).min(context_count);
    if visible == 0 || width == 0 {
        return Vec::new();
    }
    // A clipped window always keeps its last row for the current/total
    // position so hidden entries are presented as clipped, not as absent.
    let item_rows = if context_count > visible {
        visible.saturating_sub(1)
    } else {
        visible
    };

    let active_index = active_position
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < context_count);
    let max_start = context_count.saturating_sub(item_rows);
    let start = active_index
        .map(|index| index.saturating_sub(item_rows / 2))
        .unwrap_or(0)
        .min(max_start);
    let end = start.saturating_add(item_rows).min(context_count);

    let mut lines: Vec<Line<'static>> = (start..end)
        .map_while(context_at)
        .map(|context| {
            Line::from(Span::styled(
                context_row(context, width, theme),
                navigator_style(context.active, theme),
            ))
        })
        .collect();
    if context_count > visible {
        let current = active_index.map_or(end, |index| index.saturating_add(1));
        lines.push(Line::styled(
            format!("{current}/{context_count}"),
            theme.style(Role::TextSecondary),
        ));
    }
    lines
}

fn context_row(context: ContextDisplay<'_>, width: usize, theme: &Theme) -> String {
    let marker = if context.active {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let prefix = format!("[{marker}{}] ", context.position);
    let prefix_width = display_width(&prefix);
    let label = context_label(context);
    let row = format!("{prefix}{label}");
    if display_width(&row) <= width {
        return row;
    }
    let Some(label_budget) = width.checked_sub(prefix_width) else {
        return truncate_display(&row, width, "");
    };
    format!(
        "{prefix}{}",
        truncate_display(&label, label_budget, theme.glyphs.ellipsis)
    )
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

fn navigator_style(active: bool, theme: &Theme) -> Style {
    if active {
        theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
    } else {
        theme.style(Role::TextPrimary)
    }
}

#[cfg(test)]
mod tests;
