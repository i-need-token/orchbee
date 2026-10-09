//! Copy cursor and search/status projection; no content, selection or effect authority.
use crate::{
    localization::Locale,
    terminal_copy::CopySearchDirection,
    terminal_copy_mode::CopyModeView,
    terminal_text::{display_width, truncate_display},
    tui_theme::{Role, Theme},
};
use ratatui::{Frame, layout::Rect, style::Modifier, widgets::Clear};
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn paint_cursor(
    frame: &mut Frame<'_>,
    area: Rect,
    mode: CopyModeView<'_>,
    theme: &Theme,
) {
    let Some(cursor) = mode.cursor.filter(|_| mode.search.is_none()) else {
        return;
    };
    if cursor.column >= area.width || cursor.row >= area.height {
        return;
    }
    let position = (area.x + cursor.column, area.y + cursor.row);
    if let Some(cell) = frame.buffer_mut().cell_mut(position) {
        cell.set_style(
            theme
                .style(Role::TextInverse)
                .bg(theme.color(Role::AccentFocus))
                .add_modifier(Modifier::BOLD),
        );
    }
    frame.set_cursor_position(position);
}

pub(super) fn render_status(
    frame: &mut Frame<'_>,
    area: Rect,
    mode: CopyModeView<'_>,
    locale: Locale,
    notice: Option<&str>,
    theme: &Theme,
) {
    if area.is_empty() {
        return;
    }
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceChrome));
    let label = locale.copy_mode_label(mode.status, mode.selecting);
    let search_warning = mode.search.is_some() && notice.is_some();
    let warning_reserve = if search_warning { 9 + 12 } else { 0 };
    let label =
        if display_width(label) + 3 + display_width(locale.copy_mode_exit_hint()) + warning_reserve
            > usize::from(area.width)
        {
            locale.copy_mode_label(crate::terminal_copy_mode::CopyModeStatus::Ready, false)
        } else {
            label
        };
    let prefix = format!("{label} · ");
    // Mode and search retain space before optional discovery as the status narrows.
    let minimum = display_width(&prefix) + if mode.search.is_some() { 9 } else { 0 };
    let notice_reserve = notice.filter(|_| !search_warning).map_or(0, display_width);
    let available = usize::from(area.width).saturating_sub(minimum + notice_reserve);
    let warning = notice.filter(|_| search_warning).map(|notice| {
        let exit = locale.copy_mode_exit_hint();
        let columns = available.saturating_sub(display_width(exit) + 3);
        if columns == 0 {
            exit.to_owned()
        } else {
            format!("{} · {exit}", truncate_display(notice, columns, "…"))
        }
    });
    let hint = if let Some(warning) = warning.as_deref() {
        warning
    } else {
        let full = locale.copy_mode_hint(mode.search.is_some(), true);
        let short = locale.copy_mode_hint(mode.search.is_some(), false);
        if display_width(full) <= available {
            full
        } else if display_width(short) <= available {
            short
        } else {
            locale.copy_mode_exit_hint()
        }
    };
    let hint_width = display_width(hint).min(usize::from(area.width).saturating_sub(minimum));
    // Status content keeps one blank cell ahead of the right-aligned hint.
    let content_width = usize::from(area.width)
        .saturating_sub(hint_width)
        .saturating_sub(usize::from(hint_width > 0));
    let label_width = display_width(&prefix).min(content_width);
    let shown_label = truncate_display(&prefix, label_width, "");
    frame.buffer_mut().set_stringn(
        area.x,
        area.y,
        shown_label,
        label_width,
        theme.style(Role::StatusInfo),
    );
    if let Some(search) = mode.search {
        let direction = match search.direction {
            CopySearchDirection::Forward => "/",
            CopySearchDirection::Backward => "?",
        };
        let left = area.x + u16::try_from(label_width).expect("status width is u16");
        let columns = content_width.saturating_sub(label_width);
        if columns > 1 {
            frame
                .buffer_mut()
                .set_stringn(left, area.y, direction, 1, theme.style(Role::HintKey));
            let (draft, cursor) = search_window(search.draft, search.cursor, columns - 1);
            frame.buffer_mut().set_stringn(
                left + 1,
                area.y,
                draft,
                columns - 1,
                theme.style(Role::TextPrimary),
            );
            frame.set_cursor_position((left + 1 + cursor, area.y));
        }
    } else if let Some(notice) = notice {
        let width = content_width.saturating_sub(label_width);
        let text = truncate_display(notice, width, "…");
        frame.buffer_mut().set_stringn(
            area.x + u16::try_from(label_width).expect("status width is u16"),
            area.y,
            text,
            width,
            theme.style(Role::StatusWarning),
        );
    }
    if hint_width > 0 {
        let hint = truncate_display(hint, hint_width, "…");
        frame.buffer_mut().set_stringn(
            area.right() - u16::try_from(hint_width).expect("status width is u16"),
            area.y,
            hint,
            hint_width,
            theme.style(if search_warning {
                Role::StatusWarning
            } else {
                Role::HintLabel
            }),
        );
    }
}

/// Horizontally scroll at grapheme boundaries, reserving one cell for the insertion caret.
fn search_window(draft: &str, cursor: usize, columns: usize) -> (String, u16) {
    if columns == 0 {
        return (String::new(), 0);
    }
    let before = &draft[..cursor];
    let mut start = 0;
    let mut width = display_width(before);
    for (offset, grapheme) in before.grapheme_indices(true) {
        if width < columns {
            break;
        }
        width = width.saturating_sub(display_width(grapheme));
        start = offset + grapheme.len();
    }
    (
        truncate_display(&draft[start..], columns, ""),
        u16::try_from(width).expect("caret is within u16 status width"),
    )
}

#[cfg(test)]
mod tests;
