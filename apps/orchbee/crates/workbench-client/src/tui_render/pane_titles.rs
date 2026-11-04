//! Framed panes title themselves on the frame or divider line above them, so a title never takes a
//! terminal row. Titles paint after the focus outline, keeping a neighbor's title in its own color.
use super::{
    ClientTabView, Frame, HitMapBuilder, Locale, Rect, Role, ShellText, Theme, WorkbenchClientView,
    controls::ControlSurface, presentation_selector::PresentationSelector,
};

use crate::{
    terminal_text::{display_width, truncate_display},
    tui_hit_map::TooltipText,
    tui_layout::LeafArea,
};
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};

pub(super) struct PaneTitle<'a> {
    pub(super) tab: ClientTabView<'a>,
    /// The pane owns input, so its title carries focus emphasis.
    pub(super) focused: bool,
    /// A zoomed pane names how many panes the zoom hides behind it.
    pub(super) zoomed_panes: Option<usize>,
    pub(super) working_frame: usize,
    pub(super) text: ShellText<'a>,
    pub(super) locale: Locale,
}

/// A title needs its rule corner and one rule cell on each side.
const MINIMUM_LEAF_WIDTH: u16 = 4;
/// Narrower names drop the zoom badge before truncating further.
const MINIMUM_NAME_WIDTH: usize = 4;

pub(super) fn paint(
    frame: &mut Frame<'_>,
    leaf: &LeafArea,
    title: &PaneTitle<'_>,
    native: &WorkbenchClientView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let (text, locale) = (title.text, title.locale);
    let area = leaf.leaf;
    if area.y == 0 || area.width < MINIMUM_LEAF_WIDTH {
        return;
    }
    let line = Rect::new(area.x + 1, area.y - 1, area.width - 2, 1);
    let panel = title.tab.panel(leaf.panel_index);
    let selector = panel.and_then(|panel| PresentationSelector::new(line, panel, native));
    let flat = |role: Role| theme.style(role).remove_modifier(Modifier::BOLD);
    let name_style = if title.focused {
        theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
    } else {
        flat(Role::TextSecondary)
    };
    let ended = panel.and_then(|panel| native.terminal_end(panel.panel_id()));
    let name = match (ended, panel) {
        (Some(end), _) => locale.terminal_end(end),
        (None, Some(panel)) => name(
            panel,
            native,
            text,
            theme.glyphs.middle_dot,
            selector.is_some(),
        ),
        (None, None) => format!("{} {}", text.panel_title, leaf.panel_index + 1),
    };
    let name_style = if ended.is_some() {
        flat(Role::StatusWarning)
    } else {
        name_style
    };
    let (state, unread) = match (ended, panel) {
        (None, Some(panel)) => (
            super::panel_reports::agent_state(native.panel_report(panel)).map(|state| {
                if state == orchbee_workbench_protocol::MetadataState::Done
                    && native.panel_completion_edited(panel.panel_id())
                {
                    orchbee_workbench_protocol::MetadataState::Idle
                } else {
                    state
                }
            }),
            native.panel_unseen_count(panel.panel_id()) > 0,
        ),
        _ => (None, false),
    };
    let glyph = state.map(|state| {
        (
            super::sidebar_text::state_glyph(state, theme, title.working_frame),
            theme
                .agent_style(state, title.working_frame)
                .remove_modifier(Modifier::BOLD),
        )
    });
    let unread = unread.then(|| format!(" {}", theme.glyphs.status_bullet));
    let fixed = 2
        + glyph.map_or(0, |(glyph, _)| display_width(glyph) + 1)
        + unread.as_deref().map_or(0, display_width);
    let mut badge = title
        .zoomed_panes
        .filter(|_| selector.is_none())
        .map(|count| format!(" {} ", locale.zoomed_panes(count)));
    let width = usize::from(line.width);
    let right = |badge: &Option<String>| {
        selector.as_ref().map_or_else(
            || badge.as_deref().map_or(0, display_width),
            |selector| usize::from(selector.area.width),
        )
    };
    if width.saturating_sub(fixed + right(&badge)) < MINIMUM_NAME_WIDTH {
        badge = None;
    }
    let room = width.saturating_sub(fixed + right(&badge));
    let mut spans = vec![Span::styled(" ", name_style)];
    // Where the state glyph and unread dot land, the only cells that name them.
    let cells = |offset: usize, text: &str| {
        let x = line.x.saturating_add(u16::try_from(offset).ok()?);
        let width = u16::try_from(display_width(text)).ok()?;
        Some(Rect::new(x, line.y, width, 1)).filter(|rect| line.intersection(*rect) == *rect)
    };
    let glyph_cells = glyph.and_then(|(glyph, _)| cells(1, glyph));
    if let Some((glyph, style)) = glyph {
        spans.push(Span::styled(glyph, style));
        spans.push(Span::styled(" ", name_style));
    }
    let shown = truncate_display(&name, room, theme.glyphs.ellipsis);
    let cut = (shown != name).then(|| TooltipText {
        text: name.clone(),
        middle: false,
    });
    let dot_cells = unread.as_ref().and_then(|_| {
        let lead: usize = spans.iter().map(|span| display_width(&span.content)).sum();
        cells(lead + display_width(&shown) + 1, theme.glyphs.status_bullet)
    });
    spans.push(Span::styled(shown, name_style));
    if let Some(unread) = unread {
        spans.push(Span::styled(unread, flat(Role::StatusUnread)));
    }
    spans.push(Span::styled(" ", name_style));
    let left = Line::from(spans);
    frame
        .buffer_mut()
        .set_line(line.x, line.y, &left, line.width);
    // The state glyph names its state and the unread dot unread activity, each from its own
    // cells; the rest of the title gives only a cut name, in full. Neither records a target, so a
    // press there still reaches the divider beneath.
    if let Some(pane) = panel.and_then(|panel| native.pane_ref_for_panel(panel.panel_id())) {
        let width = u16::try_from(left.width())
            .unwrap_or(u16::MAX)
            .min(line.width);
        let text = Rect::new(line.x, line.y, width, 1);
        let legends = [
            glyph_cells.map(|rect| {
                (
                    rect,
                    super::tooltip::legend(state, false, title.working_frame, locale, theme),
                )
            }),
            dot_cells.map(|rect| {
                (
                    rect,
                    super::tooltip::legend(None, true, title.working_frame, locale, theme),
                )
            }),
        ];
        // The glyphs record first, so they win within their own cells.
        for (rect, legend) in legends.into_iter().flatten() {
            if text.intersection(rect) == rect
                && let Some(content) = super::tooltip::describe(None, legend)
            {
                hits.record_pane_title_tooltip(rect, pane.clone(), content);
            }
        }
        if let Some(content) = super::tooltip::describe(cut, Vec::new()) {
            hits.record_pane_title_tooltip(text, pane, content);
        }
    }
    if let Some(selector) = &selector {
        selector.paint(frame, ControlSurface::Canvas, theme, hits);
    } else if let Some(badge) = badge {
        let badge_width = u16::try_from(display_width(&badge)).unwrap_or(u16::MAX);
        frame.buffer_mut().set_stringn(
            line.right().saturating_sub(badge_width),
            line.y,
            &badge,
            usize::from(badge_width),
            flat(Role::TextSecondary),
        );
    }
}

/// Custom and metadata titles win over the creation label; a numeric creation label keeps the
/// localized Terminal prefix. Without room for the selector, a member still shows its position.
fn name(
    panel: &crate::workbench_projection::ClientPanel,
    native: &WorkbenchClientView<'_>,
    text: ShellText<'_>,
    separator: &str,
    selector: bool,
) -> String {
    let presentation = native.selected_presentation(panel);
    let name = super::panel_reports::title(
        presentation.custom_label(),
        presentation.display_label(),
        text.panel_title,
        native.panel_report(panel),
        separator,
    );

    if selector || panel.presentation_count() < 2 {
        return name;
    }
    let position = panel
        .presentation_index(presentation.presentation_id())
        .unwrap_or(0)
        + 1;
    format!("{name} [{position}/{}]", panel.presentation_count())
}
