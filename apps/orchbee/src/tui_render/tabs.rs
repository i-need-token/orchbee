use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
#[cfg(windows)]
pub(super) mod groups;

#[cfg(windows)]
pub(crate) fn render_tab_drag_preview(
    frame: &mut Frame<'_>,
    preview: &crate::tui_pointer::TabDragPreview,
    locale: crate::localization::Locale,
    theme: &Theme,
) {
    for rect in &preview.grabbed {
        let rect = rect.intersection(frame.area());
        frame.buffer_mut().set_style(
            rect,
            theme
                .style(Role::SurfaceSelection)
                .add_modifier(Modifier::UNDERLINED),
        );
    }
    if let Some((x, y)) = preview.marker
        && frame.area().contains((x, y).into())
    {
        frame.render_widget(
            Paragraph::new(theme.glyphs.scrollbar_track)
                .style(theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD)),
            Rect::new(x, y, 1, 1),
        );
    }
    let area = frame.area();
    if area.height > 0 {
        frame.render_widget(
            Paragraph::new(truncate_display(
                &locale.tab_drag_hint(preview.count, preview.marker.is_some()),
                usize::from(area.width),
                theme.glyphs.ellipsis,
            ))
            .style(
                theme
                    .style(Role::SurfaceSelection)
                    .patch(theme.style(Role::TextPrimary)),
            ),
            Rect::new(area.x, area.bottom() - 1, area.width, 1),
        );
    }
}

use crate::{
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::ProjectedPointer,
    tui_theme::{Role, Theme},
};

use super::controls::{
    Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
    NavigationState, OperationState, PointerPresentation, RowContentBand, RowFocusCue, RowTracks,
    paint_row_interaction,
};

#[derive(Clone)]
pub(super) struct TabDisplay<'a> {
    pub(super) position: usize,
    pub(super) active: bool,
    pub(super) label: std::borrow::Cow<'a, str>,
}

/// Metadata decorates display only. Sanitizing, clipping and hit width still
/// consume the complete label through the same rendering path as plain Tabs.
pub(super) fn label_with_metadata<'a>(
    label: std::borrow::Cow<'a, str>,
    pinned: Option<&str>,
    group: Option<&str>,
) -> std::borrow::Cow<'a, str> {
    if let Some(pinned) = pinned {
        format!("{pinned} · {label}").into()
    } else if let Some(group) = group {
        format!("{group} / {label}").into()
    } else {
        label
    }
}

/// Paints the compact tab strip and reports each complete token's exact
/// rectangle to `on_token` in the same pass, so pointer projection and the
/// recorded hit region cannot disagree with what was drawn.
#[cfg(any(not(windows), test))]
pub(super) fn render_compact<'a, C, P, R>(
    frame: &mut Frame<'_>,
    area: Rect,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
    mut interactions: CompactInteractions<'_, C, P, R>,
) where
    P: FnMut(&C, usize, Rect) -> ProjectedPointer,
    R: FnMut(&mut C, usize, Rect),
{
    let projection = compact_line(
        usize::from(area.width),
        tab_count,
        active_position,
        &tab_at,
        theme,
    );
    let tokens = projection
        .tokens
        .into_iter()
        .filter_map(|token| {
            let rect = Rect::new(
                area.x
                    .saturating_add(u16::try_from(token.x).unwrap_or(u16::MAX)),
                area.y,
                u16::try_from(token.width).unwrap_or(u16::MAX),
                1,
            );
            (frame.area().intersection(rect) == rect).then(|| CompactPaint {
                index: token.index,
                rect,
                pointer: interactions.project(token.index, rect),
            })
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(projection.line), area);
    for token in tokens {
        if paint_token_pointer(frame, token.rect, token.pointer, theme).is_some() {
            interactions.record(token.index, token.rect);
        }
    }
}

/// Paints the navigator's tab column and reports each complete row's exact
/// rectangle to `on_row` in the same pass.
#[expect(
    clippy::too_many_arguments,
    reason = "paint data, active selection and keyboard focus have distinct owners"
)]
pub(super) fn render_navigator<'a, C>(
    frame: &mut Frame<'_>,
    area: Rect,
    tab_count: usize,
    active_position: Option<usize>,
    focused_position: Option<usize>,
    tab_at: impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
    interaction_context: &mut C,
    project_pointer: &mut impl FnMut(&C, usize, Rect) -> ProjectedPointer,
    on_row: &mut impl FnMut(&mut C, usize, Rect),
) {
    let projection = navigator_projection(
        usize::from(area.width),
        usize::from(area.height),
        tab_count,
        active_position,
        focused_position,
        &tab_at,
        theme,
    );
    let Some(layout) = NavigatorRowLayout::new(area, tab_count) else {
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
            pointer_presentation(pointer),
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

#[cfg(any(not(windows), test))]
fn compact_line<'a>(
    width: usize,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: &impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
) -> CompactProjection {
    if width == 0 || tab_count == 0 {
        return CompactProjection::default();
    }

    let start = compact_window_start(width, tab_count, active_position, tab_at);
    let mut spans = Vec::new();
    let mut tokens = Vec::new();
    let mut used = 0_usize;
    for index in start..tab_count {
        let Some(tab) = tab_at(index) else {
            break;
        };
        if !spans.is_empty() {
            if used >= width {
                break;
            }
            spans.push(Span::raw(" "));
            used = used.saturating_add(1);
        }
        let remaining = width.saturating_sub(used);
        let Some((token, token_width)) = compact_token(&tab, remaining, theme) else {
            break;
        };
        let token_x = used;
        spans.push(Span::styled(token, compact_style(tab.active, theme)));
        tokens.push(CompactToken {
            index,
            x: token_x,
            width: token_width,
        });
        used = used.saturating_add(token_width);
        if used >= width {
            break;
        }
    }
    CompactProjection {
        line: Line::from(spans),
        tokens,
    }
}

pub(super) const fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

/// Applies only the transient pointer surface to an already painted compact
/// token. Its current, checked, group-color, and content styles remain the
/// token owner's independent axes.
pub(super) fn paint_token_pointer(
    frame: &mut Frame<'_>,
    rect: Rect,
    pointer: ProjectedPointer,
    theme: &Theme,
) -> Option<Rect> {
    if rect.is_empty() || frame.area().intersection(rect) != rect {
        return None;
    }
    let interaction = match pointer {
        ProjectedPointer::Rest => return Some(rect),
        ProjectedPointer::Hovered => Role::SurfaceHover,
        ProjectedPointer::Pressed => Role::SurfacePressed,
    };
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            let content = cell.style();
            cell.set_style(theme.content_on_surface(
                Role::SurfaceChrome,
                Some(interaction),
                None,
                content,
            ));
        }
    }
    Some(rect)
}

#[cfg(any(not(windows), test))]
pub(super) struct CompactInteractions<'a, C, P, R> {
    context: &'a mut C,
    project_pointer: &'a mut P,
    record_token: &'a mut R,
}

#[cfg(any(not(windows), test))]
impl<'a, C, P, R> CompactInteractions<'a, C, P, R>
where
    P: for<'context> FnMut(&'context C, usize, Rect) -> ProjectedPointer,
    R: for<'context> FnMut(&'context mut C, usize, Rect),
{
    pub(super) const fn new(
        context: &'a mut C,
        project_pointer: &'a mut P,
        record_token: &'a mut R,
    ) -> Self {
        Self {
            context,
            project_pointer,
            record_token,
        }
    }
}

#[cfg(any(not(windows), test))]
impl<C, P, R> CompactInteractions<'_, C, P, R>
where
    P: FnMut(&C, usize, Rect) -> ProjectedPointer,
    R: FnMut(&mut C, usize, Rect),
{
    fn project(&mut self, index: usize, rect: Rect) -> ProjectedPointer {
        (self.project_pointer)(&*self.context, index, rect)
    }

    fn record(&mut self, index: usize, rect: Rect) {
        (self.record_token)(self.context, index, rect);
    }
}

#[cfg(any(not(windows), test))]
#[derive(Debug, Default)]
struct CompactProjection {
    line: Line<'static>,
    tokens: Vec<CompactToken>,
}

#[cfg(any(not(windows), test))]
#[derive(Clone, Copy, Debug)]
struct CompactToken {
    index: usize,
    x: usize,
    width: usize,
}

#[cfg(any(not(windows), test))]
#[derive(Clone, Copy, Debug)]
struct CompactPaint {
    index: usize,
    rect: Rect,
    pointer: ProjectedPointer,
}

#[cfg(any(not(windows), test))]
fn compact_window_start<'a>(
    width: usize,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: &impl Fn(usize) -> Option<TabDisplay<'a>>,
) -> usize {
    let Some(active_index) = active_position
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < tab_count)
    else {
        return 0;
    };
    let Some(active) = tab_at(active_index) else {
        return 0;
    };
    let active_width = compact_token_width(&active);
    if active_width > width {
        return active_index;
    }

    let mut start = active_index;
    let mut occupied = active_width;
    while start > 0 {
        let Some(previous) = tab_at(start - 1) else {
            break;
        };
        let additional = compact_token_width(&previous).saturating_add(1);
        if additional > width.saturating_sub(occupied) {
            break;
        }
        start -= 1;
        occupied = occupied.saturating_add(additional);
    }
    start
}

fn navigator_projection<'a>(
    width: usize,
    height: usize,
    tab_count: usize,
    active_position: Option<usize>,
    focused_position: Option<usize>,
    tab_at: &impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
) -> NavigatorProjection {
    if height == 0 || width == 0 || tab_count == 0 {
        return NavigatorProjection::default();
    }
    // A clipped window always keeps its last row for the current/total
    // position so hidden entries are presented as clipped, not as absent.
    let item_rows = if tab_count > height {
        height.saturating_sub(1)
    } else {
        height
    };
    let active_index = focused_position
        .or(active_position)
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < tab_count);
    let start = active_index
        .map(|index| index.saturating_sub(item_rows.saturating_sub(1)))
        .unwrap_or(0)
        .min(tab_count.saturating_sub(item_rows));
    let end = start.saturating_add(item_rows).min(tab_count);

    let Some(layout) = NavigatorRowLayout::from_width(width, tab_count) else {
        return NavigatorProjection {
            rows: Vec::new(),
            position: (tab_count > height).then(|| {
                let current = active_index.map_or(end, |index| index.saturating_add(1));
                (
                    u16::try_from(item_rows).unwrap_or(u16::MAX),
                    format!("{current}/{tab_count}"),
                )
            }),
        };
    };
    let mut rows = Vec::with_capacity(end.saturating_sub(start));
    for (offset, index) in (start..end).enumerate() {
        let Some(tab) = tab_at(index) else {
            return NavigatorProjection::default();
        };
        let focused = focused_position == Some(tab.position);
        rows.push(ProjectedNavigatorRow {
            index,
            offset: u16::try_from(offset).unwrap_or(u16::MAX),
            line: navigator_row(&tab, layout, theme),
            active: tab.active,
            focused,
        });
    }
    let position = if tab_count > height {
        let current = active_index.map_or(end, |index| index.saturating_add(1));
        Some((
            u16::try_from(item_rows).unwrap_or(u16::MAX),
            format!("{current}/{tab_count}"),
        ))
    } else {
        None
    };
    NavigatorProjection { rows, position }
}

fn compact_token(tab: &TabDisplay<'_>, max_width: usize, theme: &Theme) -> Option<(String, usize)> {
    let safe_label = sanitize_terminal_text(&tab.label);
    let marker = if tab.active {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let prefix = format!(" {marker}{} ", tab.position);
    let prefix_width = display_width(&prefix);
    let token = format!("{prefix}{safe_label} ");
    let width = display_width(&token);
    if width <= max_width {
        return Some((token, width));
    }
    if let Some(label_budget) = max_width
        .checked_sub(prefix_width)
        .and_then(|rest| rest.checked_sub(1))
    {
        let truncated = truncate_display(&safe_label, label_budget, theme.glyphs.ellipsis);
        let token = format!("{prefix}{truncated} ");
        let width = display_width(&token);
        if width <= max_width {
            return Some((token, width));
        }
    }
    let clipped = truncate_display(&token, max_width, "");
    let clipped_width = display_width(&clipped);
    (clipped_width > 0).then_some((clipped, clipped_width))
}

fn compact_token_width(tab: &TabDisplay<'_>) -> usize {
    let safe_label = sanitize_terminal_text(&tab.label);
    let prefix_width = tab.position.to_string().len().saturating_add(3);
    prefix_width
        .saturating_add(display_width(safe_label.as_ref()))
        .saturating_add(1)
}

fn navigator_row(tab: &TabDisplay<'_>, layout: NavigatorRowLayout, theme: &Theme) -> Line<'static> {
    let safe_label = sanitize_terminal_text(&tab.label);
    let marker = if tab.active {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let prefix = format!(
        "  {marker} {:>width$} ",
        tab.position,
        width = layout.ordinal_width,
    );
    let label = truncate_display(&safe_label, layout.primary_width, theme.glyphs.ellipsis);
    Line::from(vec![
        Span::styled(prefix, theme.style(Role::TextSecondary)),
        Span::styled(label, theme.style(Role::TextPrimary)),
    ])
}

fn compact_style(active: bool, theme: &Theme) -> Style {
    if active {
        theme
            .current_context_style(theme.style(Role::TextPrimary))
            .add_modifier(Modifier::BOLD)
    } else {
        theme.style(Role::TextSecondary)
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
