//! Cell controls return only the exact, fully painted cells they expose.
use crate::{
    terminal_text::display_width,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::Text,
    widgets::{Paragraph, Wrap},
};

pub(crate) fn button(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    theme: &Theme,
    selected: bool,
) -> Option<Rect> {
    let width = u16::try_from(display_width(label)).ok()?;
    if area.height == 0 || width == 0 || width > area.width {
        return None;
    }
    let rect = Rect::new(area.x, area.y, width, 1);
    frame.render_widget(
        Paragraph::new(label).style(theme.style(if selected {
            Role::AccentFocus
        } else {
            Role::TextSecondary
        })),
        rect,
    );
    Some(rect)
}

pub(crate) fn close(frame: &mut Frame<'_>, area: Rect, theme: &Theme) -> Option<Rect> {
    if area.is_empty() {
        return None;
    }
    let (label, width) = if area.width >= 3 {
        ("[x]", 3)
    } else {
        ("x", 1)
    };
    let inset = u16::from(area.width >= 5);
    button(
        frame,
        Rect::new(area.right() - width - inset, area.y, width, 1),
        label,
        theme,
        false,
    )
}

/// The wrapped position of a logical text row, shared by fields and actions.
pub(super) fn text_row(text: &Text<'_>, inner: Rect, index: usize) -> Rect {
    let offset = Paragraph::new(Text::from(text.lines[..index].to_vec()))
        .wrap(Wrap { trim: false })
        .line_count(inner.width);
    let y = inner
        .y
        .saturating_add(u16::try_from(offset).unwrap_or(u16::MAX));
    Rect::new(inner.x, y, inner.width, u16::from(y < inner.bottom()))
}

pub(super) fn pair(
    frame: &mut Frame<'_>,
    area: Rect,
    labels: [&str; 2],
    theme: &Theme,
) -> Option<[Rect; 2]> {
    let first_width = u16::try_from(display_width(labels[0])).ok()?;
    let second_width = u16::try_from(display_width(labels[1])).ok()?;
    let width = first_width.checked_add(2)?.checked_add(second_width)?;
    if area.height == 0 || width > area.width {
        return None;
    }
    let first = button(frame, area, labels[0], theme, false)?;
    let second = button(
        frame,
        Rect::new(area.x + first_width + 2, area.y, second_width, 1),
        labels[1],
        theme,
        false,
    )?;
    Some([first, second])
}
#[cfg(windows)]
pub(super) fn mode_bar(
    viewport: ratatui::layout::Rect,
    sidebar: crate::client_view::SidebarSurface,
) -> ratatui::layout::Rect {
    let areas = crate::tui_layout::project_shell_areas(viewport, sidebar);
    if areas.tabs.is_empty() {
        ratatui::layout::Rect::new(
            areas.workbench.x,
            areas.workbench.y,
            areas.workbench.width,
            areas.workbench.height.min(1),
        )
    } else {
        areas.tabs
    }
}
