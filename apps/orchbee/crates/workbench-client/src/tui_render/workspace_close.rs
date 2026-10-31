//! Paints the admitted Workspace close confirmation and records only controls tied to its
//! captured token. It does not decide membership or submit Workspace closure.

use super::modal::{
    ModalAction, modal_key_label, modal_layout, paint_modal_action, paint_modal_close,
    paint_modal_frame,
};
use crate::{
    client_view::{Binding, KeyCode, Modifiers},
    localization::Locale,
    terminal_text::sanitize_terminal_text,
    tui_hit_map::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::Theme,
    workbench_client::{ModalPresentation, WorkspaceCloseConfirmationView, WorkspaceCloseToken},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::Paragraph,
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
            if confirmation.destructive() {
                locale.sidebar_delete_label()
            } else {
                locale.workspace_close_confirm_label()
            },
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(&confirm_key)
        .with_tone(if confirmation.destructive() {
            crate::action_projection::ActionTone::Destructive
        } else {
            crate::action_projection::ActionTone::Ordinary
        }),
    ];
    let text = Text::from(vec![
        Line::from(""),
        Line::from(sanitize_terminal_text(confirmation.label()).into_owned()),
        Line::from(""),
        Line::from(warning),
        Line::from(""),
        Line::from(""),
    ]);
    let Some(layout) = modal_layout(
        frame.area(),
        content_area,
        title,
        &text,
        &[],
        actions,
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };

    hits.start_modal();
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    paint_modal_frame(frame, layout.area, title, theme);
    frame.render_widget(
        Paragraph::new(crate::terminal_text::wrap_text(&text, layout.inner.width)),
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
