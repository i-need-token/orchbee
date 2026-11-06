//! Projects Tab labels, navigation controls, and drag previews into one bounded strip or
//! navigator surface. It does not reorder canonical Tabs or change the Client's selected Tab.

use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};
pub(super) mod groups;
pub(super) mod list;

pub fn render_tab_drag_preview(
    frame: &mut Frame<'_>,
    preview: &crate::tui_hit_map::TabDragPreview,
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
    if let (Some((x, y)), Some(width)) = (preview.marker, preview.marker_line) {
        // Between the Tab list's rows: a rule over blank cells, an underline under text.
        let rule = Rect::new(x, y, width, 1).intersection(frame.area());
        let style = theme
            .style(Role::TextPrimary)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        let glyph = match theme.glyphs.mode() {
            crate::tui_theme::GlyphMode::Unicode => "─",
            #[cfg(any(test, feature = "test-support"))]
            crate::tui_theme::GlyphMode::Simplified => "-",
        };
        for column in rule.x..rule.right() {
            let cell = &mut frame.buffer_mut()[(column, rule.y)];
            if cell.symbol().trim().is_empty() {
                cell.set_symbol(glyph);
            }
            cell.set_style(style);
        }
    } else if let Some((x, y)) = preview.marker
        && frame.area().contains((x, y).into())
    {
        frame.render_widget(
            Paragraph::new(theme.glyphs.scrollbar_track).style(
                theme
                    .style(Role::TextPrimary)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
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
    client_view::{ClientAction, ClientView},
    localization::{Locale, MenuLabel, TooltipLabel},
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_hit_map::{ProjectedPointer, TooltipContent},
    tui_theme::{Role, Theme},
};

use super::controls::{
    Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
    NavigationState, OperationState, PointerPresentation, RowContentBand, RowFocusCue, RowTracks,
    paint_row_interaction,
};

/// The Tabs the strip shows, by canonical index, with what each one displays. A Tab whose close is
/// progressing leaves at once; the Server's publication removes it from the canonical list
/// shortly after.
pub(super) fn shown_tabs<'a>(
    native: &crate::workbench_client::WorkbenchClientView<'a>,
    view: &ClientView,
    locale: Locale,
) -> (Vec<usize>, Vec<groups::GroupTabDisplay<'a>>) {
    let selected = native.selected_tab_ids();
    let (shown, displayed) = (0..native.tab_count())
        .filter_map(|index| {
            let tab = native.tab(index)?;
            (!native.tab_leaving(tab.tab_id())).then(|| {
                (
                    index,
                    groups::GroupTabDisplay {
                        selected: selected.len() > 1 && selected.contains(tab.tab_id()),
                        tab: TabDisplay {
                            position: tab.position(),
                            active: tab.active(),
                            label: tab.display_label(locale),
                        },
                        group: tab.group(),
                        collapsed: tab
                            .group()
                            .is_some_and(|group| native.group_collapsed(&group.group_id)),
                        pinned: tab.pinned(),
                        agent: view.machines.tab_agent(tab.tab_id()),
                    },
                )
            })
        })
        .unzip();
    (shown, displayed)
}

#[derive(Clone)]
pub(super) struct TabDisplay<'a> {
    pub(super) position: usize,
    pub(super) active: bool,
    pub(super) label: std::borrow::Cow<'a, str>,
}

/// What a pointer resting on one strip part reads: a Tab or group header gives only the name it
/// cuts, in full, close names its key only on the current Tab, the one the key closes, and an edge
/// marker names the direction it pages. Its glyphs name themselves from their own cells, through
/// [`strip_glyph_tooltips`].
pub(super) fn strip_tooltip(
    view: &ClientView,
    active: bool,
    part: groups::StripPart,
    cue: groups::PartCue<'_>,
    locale: Locale,
    theme: &Theme,
) -> Option<TooltipContent> {
    match part {
        groups::StripPart::Tab | groups::StripPart::Header => super::tooltip::describe(
            cue.name.map(|name| crate::tui_hit_map::TooltipText {
                text: name.to_owned(),
                middle: false,
            }),
            Vec::new(),
        ),
        groups::StripPart::Close => Some(TooltipContent::Action {
            label: locale.menu_action(MenuLabel::CloseSelectedTabs).to_owned(),
            key: active
                .then(|| super::tooltip::action_key(view, ClientAction::RequestCloseTab, theme))
                .flatten(),
        }),
        groups::StripPart::Page { .. } => Some(TooltipContent::Action {
            label: locale
                .tooltip_label(if cue.later {
                    TooltipLabel::LaterTabs
                } else {
                    TooltipLabel::EarlierTabs
                })
                .to_owned(),
            key: None,
        }),
    }
}

/// A Tab's or collapsed header's state glyph names its states, and its unread dot unread activity,
/// each only while the pointer rests on its cells.
pub(super) fn strip_glyph_tooltips(
    view: &ClientView,
    cue: groups::PartCue<'_>,
    locale: Locale,
    theme: &Theme,
) -> impl Iterator<Item = (ratatui::layout::Rect, TooltipContent)> {
    let working_frame = view.machines.working_frame();
    [
        cue.glyph.map(|rect| {
            (
                rect,
                super::tooltip::legend(
                    cue.states.iter().copied(),
                    false,
                    working_frame,
                    locale,
                    theme,
                ),
            )
        }),
        cue.dot.filter(|_| cue.unread).map(|rect| {
            (
                rect,
                super::tooltip::legend([], true, working_frame, locale, theme),
            )
        }),
    ]
    .into_iter()
    .flatten()
    .filter_map(|(rect, legend)| Some((rect, super::tooltip::describe(None, legend)?)))
}

/// The create control names its action and the key that does the same.
pub(super) fn create_tooltip(view: &ClientView, locale: Locale, theme: &Theme) -> TooltipContent {
    TooltipContent::Action {
        label: locale.menu_action(MenuLabel::NewTab).to_owned(),
        key: super::tooltip::action_key(view, ClientAction::CreateTab, theme),
    }
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

pub(super) const fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
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
