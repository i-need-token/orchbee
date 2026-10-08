//! The explicit-save draft is projected with frame-bound row identities.
use crate::{
    localization::Locale,
    native_settings::{Row, Token, View},
    terminal_session::ModalPresentation,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};
#[cfg(test)]
mod tests;

pub(super) fn render(
    frame: &mut Frame<'_>,
    content: Rect,
    view: View,
    pending: bool,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<Token> {
    hits.start_settings();
    if content.width < 36 || content.height < 10 {
        return ModalPresentation::NotPresented;
    }
    let width = content.width.min(58);
    let height = 10;
    let area = Rect::new(
        content.x + (content.width - width) / 2,
        content.y + (content.height - height) / 2,
        width,
        height,
    );
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .title(locale.settings_title())
        .style(
            theme
                .style(Role::SurfaceOverlay)
                .fg(theme.color(Role::TextPrimary)),
        )
        .border_style(theme.style(Role::AccentFocus));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    if let View::Draft {
        token,
        values,
        selected,
        dirty,
    } = view
    {
        for row in Row::ALL {
            let value = match row {
                Row::Theme => locale.settings_theme(values.theme),
                Row::Reminders => locale.settings_toggle(values.reminders),
                Row::Sound => locale.settings_toggle(values.sound),
                Row::Save => "",
            };
            let label = if row == Row::Save {
                locale.settings_save(dirty).to_owned()
            } else {
                format!("{}: {value}", locale.settings_row(row))
            };
            let style = if row == Row::Save && !dirty {
                theme.style(Role::TextDisabled)
            } else if row == selected {
                theme
                    .style(Role::SurfaceSelection)
                    .fg(theme.color(Role::TextPrimary))
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.style(Role::TextPrimary)
            };
            let row_area = Rect::new(inner.x, inner.y + row.index() as u16, inner.width, 1);
            frame.render_widget(
                Paragraph::new(format!(
                    "{} {label}",
                    if row == selected { ">" } else { " " }
                ))
                .style(style),
                row_area,
            );
            hits.record(
                HitLayer::Overlay,
                row_area,
                HitTarget::SettingsRow { token, row },
            );
        }
    } else if pending {
        frame.render_widget(
            Paragraph::new(locale.settings_pending(matches!(view, View::Saving { .. })))
                .style(theme.style(Role::StatusPending)),
            inner,
        );
    }
    let saving = matches!(view, View::Saving { .. });
    frame.render_widget(
        Paragraph::new(locale.settings_footer(view)).style(theme.style(Role::HintKey)),
        Rect::new(inner.x, inner.bottom() - 2, inner.width, 1),
    );
    if !saving {
        frame.render_widget(
            Paragraph::new(locale.settings_cancel()).style(theme.style(Role::HintKey)),
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
        );
    }
    ModalPresentation::Presented(view.token())
}
