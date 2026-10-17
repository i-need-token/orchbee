use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_pointer::ProjectedPointer,
    tui_theme::{Role, Theme},
};

use super::controls::{
    Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
    NavigationState, OperationState, RowContentBand, RowFocusCue, RowTracks, paint_row_interaction,
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

/// Paints the navigator's context column and reports each complete row's exact
/// rectangle to `on_row` in the same pass, so pointer projection and the
/// recorded hit region cannot disagree with what was drawn.
#[expect(
    clippy::too_many_arguments,
    reason = "paint data, active selection and keyboard focus have distinct owners"
)]
pub(super) fn render_navigator<'a, C>(
    frame: &mut Frame<'_>,
    area: Rect,
    context_count: usize,
    active_position: Option<usize>,
    focused_position: Option<usize>,
    context_at: impl Fn(usize) -> Option<ContextDisplay<'a>>,
    theme: &Theme,
    interaction_context: &mut C,
    project_pointer: &mut impl FnMut(&C, usize, Rect) -> ProjectedPointer,
    on_row: &mut impl FnMut(&mut C, usize, Rect),
) {
    let projection = navigator_projection(
        usize::from(area.width),
        usize::from(area.height),
        context_count,
        active_position,
        focused_position,
        &context_at,
        theme,
    );
    let Some(layout) = NavigatorRowLayout::new(area, context_count) else {
        if let Some((offset, position)) = projection.position {
            frame.render_widget(
                Paragraph::new(position).style(theme.style(Role::TextSecondary)),
                Rect::new(area.x, area.y.saturating_add(offset), area.width, 1),
            );
        }
        return;
    };
    for row in projection.rows {
        let rect = Rect::new(area.x, area.y.saturating_add(row.offset), area.width, 1);
        let pointer = project_pointer(&*interaction_context, row.index, rect);
        frame.render_widget(Paragraph::new(row.line), rect);
        let state = InteractionState::new(
            FocusState::Focused,
            super::tabs::pointer_presentation(pointer),
            if row.focused {
                NavigationState::Provisional
            } else {
                NavigationState::None
            },
            if row.active {
                CurrentState::Current
            } else {
                CurrentState::NotCurrent
            },
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Idle,
        );
        let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, layout.primary(rect))
            .with_provisional(layout.navigation(rect));
        let bands = [
            RowContentBand::semantic(layout.current(rect), Role::TextPrimary),
            RowContentBand::semantic(layout.ordinal(rect), Role::TextSecondary),
        ];
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
            on_row(interaction_context, row.index, rect);
        }
    }
    if let Some((offset, position)) = projection.position {
        frame.render_widget(
            Paragraph::new(position).style(theme.style(Role::TextSecondary)),
            Rect::new(area.x, area.y.saturating_add(offset), area.width, 1),
        );
    }
}

fn navigator_projection<'a>(
    width: usize,
    height: usize,
    context_count: usize,
    active_position: Option<usize>,
    focused_position: Option<usize>,
    context_at: &impl Fn(usize) -> Option<ContextDisplay<'a>>,
    theme: &Theme,
) -> NavigatorProjection {
    let visible = height.min(MAX_VISIBLE_CONTEXTS).min(context_count);
    if visible == 0 || width == 0 {
        return NavigatorProjection::default();
    }
    // A clipped window always keeps its last row for the current/total
    // position so hidden entries are presented as clipped, not as absent.
    let item_rows = if context_count > visible {
        visible.saturating_sub(1)
    } else {
        visible
    };

    let active_index = focused_position
        .or(active_position)
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < context_count);
    let max_start = context_count.saturating_sub(item_rows);
    let start = active_index
        .map(|index| index.saturating_sub(item_rows / 2))
        .unwrap_or(0)
        .min(max_start);
    let end = start.saturating_add(item_rows).min(context_count);

    let Some(layout) = NavigatorRowLayout::from_width(width, context_count) else {
        return NavigatorProjection {
            rows: Vec::new(),
            position: (context_count > visible).then(|| {
                let current = active_index.map_or(end, |index| index.saturating_add(1));
                (
                    u16::try_from(item_rows).unwrap_or(u16::MAX),
                    format!("{current}/{context_count}"),
                )
            }),
        };
    };
    let mut rows = Vec::with_capacity(end.saturating_sub(start));
    for (offset, index) in (start..end).enumerate() {
        let Some(context) = context_at(index) else {
            return NavigatorProjection::default();
        };
        let focused = focused_position == Some(context.position);
        rows.push(ProjectedNavigatorRow {
            index,
            offset: u16::try_from(offset).unwrap_or(u16::MAX),
            line: context_row(context, layout, theme),
            active: context.active,
            focused,
        });
    }
    let position = if context_count > visible {
        let current = active_index.map_or(end, |index| index.saturating_add(1));
        Some((
            u16::try_from(item_rows).unwrap_or(u16::MAX),
            format!("{current}/{context_count}"),
        ))
    } else {
        None
    };
    NavigatorProjection { rows, position }
}

fn context_row(
    context: ContextDisplay<'_>,
    layout: NavigatorRowLayout,
    theme: &Theme,
) -> Line<'static> {
    let marker = if context.active {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let prefix = format!(
        "  {marker} {:>width$} ",
        context.position,
        width = layout.ordinal_width,
    );
    let label = context_label(context);
    let label = truncate_display(&label, layout.primary_width, theme.glyphs.ellipsis);
    Line::from(vec![
        Span::styled(prefix, theme.style(Role::TextSecondary)),
        Span::styled(label, theme.style(Role::TextPrimary)),
    ])
}

pub(super) fn context_label(context: ContextDisplay<'_>) -> String {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NavigatorRowLayout {
    ordinal_width: usize,
    primary_width: usize,
}

impl NavigatorRowLayout {
    fn new(area: Rect, count: usize) -> Option<Self> {
        Self::from_width(usize::from(area.width), count)
    }

    fn from_width(width: usize, count: usize) -> Option<Self> {
        let ordinal_width = count.max(1).to_string().len();
        let primary_width = width.checked_sub(ordinal_width.checked_add(5)?)?;
        (primary_width > 0).then_some(Self {
            ordinal_width,
            primary_width,
        })
    }

    const fn navigation(self, row: Rect) -> Rect {
        Rect::new(row.x, row.y, 1, 1)
    }

    const fn current(self, row: Rect) -> Rect {
        Rect::new(row.x.saturating_add(2), row.y, 1, 1)
    }

    fn ordinal(self, row: Rect) -> Rect {
        Rect::new(
            row.x.saturating_add(4),
            row.y,
            u16::try_from(self.ordinal_width).unwrap_or(u16::MAX),
            1,
        )
    }

    fn primary(self, row: Rect) -> Rect {
        let x = row
            .x
            .saturating_add(u16::try_from(self.ordinal_width + 5).unwrap_or(u16::MAX));
        Rect::new(x, row.y, row.right().saturating_sub(x), 1)
    }
}

#[derive(Debug, Default)]
struct NavigatorProjection {
    rows: Vec<ProjectedNavigatorRow>,
    position: Option<(u16, String)>,
}

#[derive(Debug)]
struct ProjectedNavigatorRow {
    index: usize,
    offset: u16,
    line: Line<'static>,
    active: bool,
    focused: bool,
}

#[cfg(test)]
mod tests;
