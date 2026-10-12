//! Scrollable common settings; only rows from the last successful frame authorize clicks.
#[cfg(test)]
use crate::native_settings::Row;
use crate::{
    localization::Locale,
    native_settings::{DraftView, Token, View},
    terminal_session::ModalPresentation,
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    widgets::{Clear, Padding, Paragraph},
};
#[cfg(test)]
mod tests;

pub(super) fn render(
    frame: &mut Frame<'_>,
    content: Rect,
    view: View<'_>,
    pending: bool,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<Token> {
    hits.start_settings();
    if content.width < 36 || content.height < 10 {
        return ModalPresentation::NotPresented;
    }
    let width = content.width.min(92);
    let height = content.height.min(26);
    let area = Rect::new(
        content.x + (content.width - width) / 2,
        content.y + (content.height - height) / 2,
        width,
        height,
    );
    let block =
        super::overlay_surface(theme, true, Padding::horizontal(1)).title(locale.settings_title());
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    if let View::Draft(draft) = view {
        render_draft(frame, inner, draft, locale, theme, hits);
    } else if pending {
        frame.render_widget(
            Paragraph::new(locale.settings_pending(matches!(view, View::Saving { .. })))
                .style(theme.style(Role::StatusPending)),
            inner,
        );
    }
    frame.render_widget(
        Paragraph::new(locale.settings_footer(view)).style(theme.style(Role::HintKey)),
        Rect::new(inner.x, inner.bottom() - 2, inner.width, 1),
    );
    if !matches!(view, View::Saving { .. }) {
        frame.render_widget(
            Paragraph::new(locale.settings_cancel()).style(theme.style(Role::HintKey)),
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
        );
    }
    ModalPresentation::Presented(view.token())
}
fn render_draft(
    frame: &mut Frame<'_>,
    inner: Rect,
    draft: DraftView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let rows = draft.page.rows();
    let selected = rows
        .iter()
        .position(|row| *row == draft.selected)
        .unwrap_or(0);
    let visible = usize::from(inner.height.saturating_sub(5));
    let start = selected
        .saturating_sub(visible.saturating_sub(1))
        .min(rows.len().saturating_sub(visible));
    let heading = format!(
        "{}  {}/{}",
        locale.settings_category(draft.selected),
        selected + 1,
        rows.len()
    );
    frame.render_widget(
        Paragraph::new(heading).style(theme.style(Role::TextSecondary)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    for (index, row) in rows.iter().copied().enumerate().skip(start).take(visible) {
        let value = locale.settings_value(draft, row);
        let label = if value.is_empty() {
            locale.settings_row(row).to_owned()
        } else {
            format!("{}: {value}", locale.settings_row(row))
        };
        let label = sanitize_terminal_text(&label);
        let style = if !draft.enabled(row) {
            theme.style(Role::TextDisabled)
        } else if row == draft.selected {
            theme
                .style(Role::SurfaceSelection)
                .fg(theme.color(Role::TextPrimary))
                .add_modifier(Modifier::BOLD)
        } else {
            theme.style(Role::TextPrimary)
        };
        let row_area = Rect::new(
            inner.x,
            inner.y + 1 + (index - start) as u16,
            inner.width,
            1,
        );
        frame.render_widget(
            Paragraph::new(format!(
                "{} {}",
                if row == draft.selected { ">" } else { " " },
                truncate_display(&label, usize::from(inner.width.saturating_sub(2)), "…")
            ))
            .style(style),
            row_area,
        );
        if draft.text.is_none() && draft.confirmation.is_none() && draft.enabled(row) {
            hits.record(
                HitLayer::Overlay,
                row_area,
                HitTarget::SettingsRow {
                    token: draft.token,
                    row,
                },
            );
        }
    }
    let detail_area = Rect::new(inner.x, inner.bottom() - 4, inner.width, 1);
    if let Some((text, cursor)) = draft.text {
        let prefix = sanitize_terminal_text(&text[..cursor]);
        let cursor_column = display_width(&prefix);
        let left = cursor_column.saturating_sub(usize::from(inner.width.saturating_sub(1)));
        let displayed = sanitize_terminal_text(text);
        frame.render_widget(
            Paragraph::new(displayed.as_ref())
                .scroll((0, left.min(usize::from(u16::MAX)) as u16))
                .style(theme.style(Role::TextPrimary)),
            detail_area,
        );
        frame.set_cursor_position((detail_area.x + (cursor_column - left) as u16, detail_area.y));
    } else {
        frame.render_widget(
            Paragraph::new(locale.settings_detail(draft)).style(theme.style(if draft.invalid {
                Role::StatusError
            } else {
                Role::TextSecondary
            })),
            detail_area,
        );
    }
    if draft.invalid && draft.text.is_some() {
        frame.render_widget(
            Paragraph::new(locale.settings_detail(draft)).style(theme.style(Role::StatusError)),
            Rect::new(inner.x, inner.bottom() - 3, inner.width, 1),
        );
    }
}
