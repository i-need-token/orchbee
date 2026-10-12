//! Scrollable common settings; only rows from the last successful frame authorize clicks.
use crate::{
    localization::Locale,
    native_settings::{Control, DraftView, Page, Row, Token, View},
    terminal_session::ModalPresentation,
    terminal_text::{
        display_width, project_terminal_text, sanitize_terminal_text, truncate_display,
    },
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
        if let Some(rect) = super::controls::close(frame, content, theme) {
            hits.record(
                HitLayer::Overlay,
                rect,
                HitTarget::SettingsControl {
                    token: view.token(),
                    control: Control::Close,
                },
            );
        }
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
    if let Some(rect) = super::controls::close(frame, area, theme) {
        hits.record(
            HitLayer::Overlay,
            rect,
            HitTarget::SettingsControl {
                token: view.token(),
                control: Control::Close,
            },
        );
    }
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
    let row = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let child =
        matches!(view, View::Draft(draft) if draft.text.is_some() || draft.confirmation.is_some());
    if child {
        if let Some(buttons) = super::controls::pair(
            frame,
            row,
            [locale.settings_cancel(), locale.settings_accept_button()],
            theme,
        ) {
            for (rect, control) in buttons.into_iter().zip([Control::Back, Control::Accept]) {
                hits.record(
                    HitLayer::Overlay,
                    rect,
                    HitTarget::SettingsControl {
                        token: view.token(),
                        control,
                    },
                );
            }
        }
    } else {
        let back = matches!(view, View::Draft(draft) if draft.page == Page::Profiles);
        let label = if back {
            locale.settings_back_button()
        } else {
            locale.settings_close_button()
        };
        if let Some(rect) = super::controls::button(frame, row, label, theme, false) {
            hits.record(
                HitLayer::Overlay,
                rect,
                HitTarget::SettingsControl {
                    token: view.token(),
                    control: if back { Control::Back } else { Control::Close },
                },
            );
        }
    }
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
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
    if draft.text.is_none() && draft.confirmation.is_none() {
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(inner.x, inner.y + 1, inner.width, visible as u16),
            HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Settings(draft.token)),
        );
    }
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
        let adjust = matches!(row, Row::Profile | Row::Argument)
            && draft.enabled(row)
            && draft.text.is_none()
            && draft.confirmation.is_none();
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
                truncate_display(
                    &label,
                    usize::from(inner.width.saturating_sub(if adjust { 10 } else { 2 })),
                    "…"
                )
            ))
            .style(style),
            row_area,
        );
        if draft.text.is_none() && draft.confirmation.is_none() && draft.enabled(row) {
            hits.record(
                HitLayer::Overlay,
                Rect::new(
                    row_area.x,
                    row_area.y,
                    row_area.width.saturating_sub(if adjust { 8 } else { 0 }),
                    1,
                ),
                HitTarget::SettingsRow {
                    token: draft.token,
                    row,
                },
            );
            if adjust && row_area.width >= 10 {
                for (x, label, forward) in [
                    (row_area.right() - 7, "[<]", false),
                    (row_area.right() - 3, "[>]", true),
                ] {
                    if let Some(rect) = super::controls::button(
                        frame,
                        Rect::new(x, row_area.y, 3, 1),
                        label,
                        theme,
                        false,
                    ) {
                        hits.record(
                            HitLayer::Overlay,
                            rect,
                            HitTarget::SettingsControl {
                                token: draft.token,
                                control: Control::Adjust { row, forward },
                            },
                        );
                    }
                }
            }
        }
    }
    let detail_area = Rect::new(inner.x, inner.bottom() - 4, inner.width, 1);
    if let Some((text, cursor)) = draft.text {
        let projection = project_terminal_text(text);
        let displayed = projection.display();
        let cursor_column = display_width(&displayed[..projection.cursor_boundary(cursor)]);
        let left = cursor_column.saturating_sub(usize::from(inner.width.saturating_sub(1)));
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
