use super::tab_close::{modal_area, wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{ModalPresentation, WorkspaceCloseConfirmationView, WorkspaceCloseToken},
    terminal_text::{display_width, sanitize_terminal_text},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
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
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorkspaceCloseToken>) {
    hits.start_modal();
    let (title, warning) = match confirmation.project_member_count() {
        Some(count) => (
            locale.project_close_all_title(),
            locale.project_close_all_warning(count),
        ),
        None => (
            locale.workspace_close_title(),
            locale.workspace_close_warning().to_owned(),
        ),
    };
    let cancel = locale.workspace_close_cancel_button();
    let confirm = locale.workspace_close_confirm_button();
    let actions = format!("{cancel}  {confirm}");
    let text = Text::from(vec![
        Line::from(title),
        Line::from(""),
        Line::from(sanitize_terminal_text(confirmation.label()).into_owned()),
        Line::from(""),
        Line::from(warning),
        Line::from(""),
        Line::from(actions.as_str()),
    ]);
    let area = modal_area(content_area, &text);
    let block = super::overlay_surface(theme, true, Padding::horizontal(2));
    let inner = block.inner(area);
    // Decision labels stay on one row, so every recorded cell is actually painted.
    let complete =
        wrapped_text_fits(&text, inner) && display_width(&actions) <= usize::from(inner.width);
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        let action_row = Paragraph::new(Text::from(text.lines[..text.lines.len() - 1].to_vec()))
            .wrap(Wrap { trim: false })
            .line_count(inner.width);
        let cancel_width = display_width(cancel) as u16;
        let y = inner.y + action_row as u16;
        for (x, width, confirm) in [
            (inner.x, cancel_width, false),
            (
                inner.x + cancel_width + 2,
                display_width(confirm) as u16,
                true,
            ),
        ] {
            hits.record(
                HitLayer::Overlay,
                Rect::new(x, y, width, 1),
                HitTarget::WorkspaceClose {
                    token: confirmation.token(),
                    confirm,
                },
            );
        }
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
