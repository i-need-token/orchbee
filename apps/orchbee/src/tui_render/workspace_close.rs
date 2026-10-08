use super::tab_close::{modal_area, wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{ModalPresentation, WorkspaceCloseConfirmationView, WorkspaceCloseToken},
    terminal_text::sanitize_terminal_text,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Clear, Padding, Paragraph, Wrap},
};
pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: WorkspaceCloseConfirmationView<'_>,
    content_area: Rect,
    theme: &Theme,
) -> (Rect, ModalPresentation<WorkspaceCloseToken>) {
    let text = Text::from(vec![
        Line::from(locale.workspace_close_title()),
        Line::from(""),
        Line::from(sanitize_terminal_text(confirmation.label()).into_owned()),
        Line::from(""),
        Line::from(locale.workspace_close_warning()),
        Line::from(""),
        Line::from(locale.workspace_close_actions()),
    ]);
    let area = modal_area(content_area, &text);
    let block = super::overlay_surface(theme, true, Padding::horizontal(2));
    let inner = block.inner(area);
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
    } else {
        frame.render_widget(
            Paragraph::new(locale.close_resize_hint())
                .style(theme.style(Role::StatusWarning))
                .wrap(Wrap { trim: false }),
            inner,
        );
    }
    (
        area,
        if complete {
            ModalPresentation::Presented(confirmation.token())
        } else {
            ModalPresentation::NotPresented
        },
    )
}
