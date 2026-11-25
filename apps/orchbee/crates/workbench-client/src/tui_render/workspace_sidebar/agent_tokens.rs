//! Pure projection of the already-fenced Agent row: one fixed line. No I/O or Agent authority.
use super::{
    ClientView, Line, Locale, Modifier, ProjectedSidebarLines, Role, Span, Theme, display_label,
    display_width, full_name, machine_label, truncate_display,
};
use crate::machine_picker::Row;

pub(in crate::tui_render) fn height(view: &ClientView, row: &Row) -> u16 {
    if row.sidebar_agent().is_some() {
        return 1;
    }
    1 + u16::from(
        view.machines
            .rows
            .iter()
            .filter(|machine| machine.enabled)
            .count()
            > 1,
    )
}
/// A cut default title keeps at least this many cells before its ellipsis beside the provider.
const MINIMUM_TITLE_CELLS: usize = 3;

/// The separator between an Agent's identity title and its provider.
fn identity_separator(theme: &Theme) -> String {
    format!(" {} ", theme.glyphs.middle_dot)
}

/// The whole name an Agent row shows: its identity title, the separator, then its provider; the
/// provider alone without an identity title. A folded Workspace's roster names Agents the same way.
pub(super) fn identity_label(agent: &crate::machine_agents::Row, theme: &Theme) -> String {
    let provider = display_label(agent.provider());
    match agent.identity_title() {
        Some(title) => format!("{title}{}{provider}", identity_separator(theme)),
        None => provider,
    }
}

/// The default row: what names the Agent, then its provider in quieter text, or the provider
/// alone; its leading glyph and pointer tooltip carry state. Within `width` the title gives way
/// first, so the provider stays whole while the title keeps its first few cells; narrower still,
/// the title alone tells sibling Agents apart. The current Agent's title is bold primary text;
/// every other row is muted, as the row takes no current fill.
fn default_row(
    agent: &crate::machine_agents::Row,
    width: usize,
    theme: &Theme,
    current: bool,
) -> ProjectedSidebarLines {
    let style = |role| theme.sidebar_token_style(role, Default::default(), false, false);
    let (title_style, quiet) = if current {
        (
            style(Role::TextPrimary).add_modifier(Modifier::BOLD),
            style(Role::TextSecondary),
        )
    } else {
        (style(Role::TextMuted), style(Role::TextMuted))
    };
    let provider = display_label(agent.provider());
    let Some(title) = agent.identity_title() else {
        return ProjectedSidebarLines::new(vec![Line::from(Span::styled(
            provider.clone(),
            title_style,
        ))])
        .with_names(vec![(0, full_name(provider))]);
    };
    let separator = identity_separator(theme);
    let whole = identity_label(agent, theme);
    let ellipsis = theme.glyphs.ellipsis;
    let trailing = display_width(&separator) + display_width(&provider);
    let kept = display_width(&title).min(MINIMUM_TITLE_CELLS + display_width(ellipsis));
    let spans = if width >= trailing + kept {
        vec![
            Span::styled(
                truncate_display(&title, width - trailing, ellipsis),
                title_style,
            ),
            Span::styled(separator, quiet),
            Span::styled(provider, quiet),
        ]
    } else {
        vec![Span::styled(title, title_style)]
    };
    ProjectedSidebarLines::new(vec![Line::from(spans)]).with_names(vec![(0, full_name(whole))])
}

/// A pointed noncurrent Agent row's title, the first span of its line, rises to primary text.
pub(in crate::tui_render) fn raise_title(lines: &mut [Line<'static>], theme: &Theme) {
    if let Some(title) = lines.first_mut().and_then(|line| line.spans.first_mut()) {
        title.style =
            theme.sidebar_token_style(Role::TextPrimary, Default::default(), false, false);
    }
}

/// `width` is the cells the first line may take; `current` says whether the row is the current
/// Agent's.
pub(in crate::tui_render) fn projection(
    view: &ClientView,
    row: &Row,
    locale: Locale,
    theme: &Theme,
    width: usize,
    current: bool,
) -> ProjectedSidebarLines {
    if let Some(agent) = row.sidebar_agent() {
        return default_row(agent, width, theme, current);
    }
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
    ProjectedSidebarLines::new(lines)
}

#[cfg(test)]
pub(in crate::tui_render) fn lines(
    view: &ClientView,
    row: &Row,
    locale: Locale,
    theme: &Theme,
) -> Vec<Line<'static>> {
    projection(view, row, locale, theme, usize::MAX, false)
        .into_parts()
        .0
}
