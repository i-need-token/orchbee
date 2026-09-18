use super::tab_close::{modal_area, wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{
        ModalPresentation, NativePanelCloseConfirmationToken, NativePanelCloseConfirmationView,
    },
    terminal_text::sanitize_terminal_text,
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
    pub(super) prompt: ModalPresentation<NativePanelCloseConfirmationToken>,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &NativePanelCloseConfirmationView<'_>,
    content_area: Rect,
    theme: &Theme,
) -> PanelModalDraw {
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
    let (area, complete) =
        render_content(frame, content_area, locale.panel_close_resize_hint(), text, theme);
    PanelModalDraw {
        area,
        prompt: if complete {
            ModalPresentation::Presented(confirmation.token())
        } else {
            ModalPresentation::NotPresented
        },
    }
}

/// A panel close can only be confirmed while its complete warning text fit the
/// draw inside the unified content area: a clipped or unplaceable modal
/// renders the resize hint and yields a `NotPresented` outcome.
fn render_content(
    frame: &mut Frame<'_>,
    content_area: Rect,
    resize_hint: &str,
    text: Text<'_>,
    theme: &Theme,
) -> (Rect, bool) {
    let area = modal_area(content_area, &text);
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
    (area, complete)
}

#[cfg(test)]
mod tests;
