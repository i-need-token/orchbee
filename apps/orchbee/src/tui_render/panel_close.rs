use super::tab_close::wrapped_text_fits;
use crate::{
    localization::Locale,
    terminal_session::{NativePanelCloseConfirmationToken, NativePanelCloseConfirmationView},
    terminal_text::sanitize_terminal_text,
    tui_layout::place_overlay,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

pub(super) struct PanelModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: Option<NativePanelCloseConfirmationToken>,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &NativePanelCloseConfirmationView<'_>,
    area: Rect,
    theme: &Theme,
) -> Option<PanelModalDraw> {
    let target = format!(
        "{} · {}",
        sanitize_terminal_text(confirmation.panel_label()),
        sanitize_terminal_text(confirmation.tab_label())
    );
    let text = Text::from(vec![
        Line::from(locale.panel_close_title()),
        Line::from(""),
        Line::from(target),
        Line::from(""),
        Line::from(locale.panel_close_warning(confirmation.dissolves_tab())),
        Line::from(""),
        Line::from(locale.panel_close_actions()),
    ]);
    render_content(frame, locale.panel_close_resize_hint(), text, area, theme).map(
        |(area, complete)| PanelModalDraw {
            area,
            prompt: complete.then(|| confirmation.token()),
        },
    )
}

/// A panel close can only be confirmed while its complete warning text fit the
/// draw: a clipped modal renders the resize hint and yields no prompt token.
fn render_content(
    frame: &mut Frame<'_>,
    resize_hint: &str,
    text: Text<'_>,
    area: Rect,
    theme: &Theme,
) -> Option<(Rect, bool)> {
    let area = place_overlay(area, u16::MAX, u16::MAX)?;
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(2))
        .style(
            theme
                .style(Role::SurfaceOverlay)
                .fg(theme.color(Role::TextPrimary)),
        )
        .border_style(theme.style(Role::AccentFocus));
    let inner = block.inner(area);
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
    } else {
        frame.render_widget(
            Paragraph::new(resize_hint)
                .style(theme.style(Role::StatusWarning))
                .wrap(Wrap { trim: false }),
            inner,
        );
    }
    Some((area, complete))
}

#[cfg(test)]
mod tests;
