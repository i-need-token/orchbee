use super::modal::{
    ModalAction, modal_key_label, modal_layout, paint_modal_action, paint_modal_close,
};
use crate::{
    client_view::{Binding, KeyCode, Modifiers},
    localization::Locale,
    terminal_session::{ModalPresentation, WorkspaceCloseConfirmationView, WorkspaceCloseToken},
    terminal_text::sanitize_terminal_text,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::Theme,
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
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let confirm_key = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.workspace_close_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.workspace_close_confirm_label(),
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(&confirm_key),
    ];
    let text = Text::from(vec![
        Line::from(title),
        Line::from(""),
        Line::from(sanitize_terminal_text(confirmation.label()).into_owned()),
        Line::from(""),
        Line::from(warning),
        Line::from(""),
        Line::from(""),
    ]);
    let Some(layout) = modal_layout(frame.area(), content_area, &text, &[], actions, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };

    hits.start_modal();
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    frame.render_widget(Clear, layout.area);
    frame.render_widget(
        super::overlay_surface(theme, true, Padding::horizontal(2)),
        layout.area,
    );
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }),
        layout.inner,
    );
    paint_modal_close(
        frame,
        layout.close,
        HitTarget::WorkspaceClose {
            token: confirmation.token(),
            confirm: false,
        },
        theme,
        hits,
    );
    for ((rect, action), confirm) in layout.actions.into_iter().zip(actions).zip([false, true]) {
        paint_modal_action(
            frame,
            Rect::new(rect.x, rect.y, layout.inner.right() - rect.x, 1),
            rect,
            action,
            HitTarget::WorkspaceClose {
                token: confirmation.token(),
                confirm,
            },
            theme,
            hits,
        );
    }
    (
        layout.area,
        ModalPresentation::Presented(confirmation.token()),
    )
}
