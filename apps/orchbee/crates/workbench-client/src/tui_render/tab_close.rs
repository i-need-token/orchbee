//! Paints the admitted Tab close modal and returns token-bound control geometry and
//! presentation evidence. It does not accept a close decision or perform terminal lifecycle
//! effects.

use super::modal::{
    ModalAction, description_line, modal_key_label, modal_layout, paint_modal_action,
    paint_modal_close, paint_modal_frame,
};
use crate::{
    action_projection::ActionTone,
    client_view::{Binding, KeyCode, Modifiers},
    localization::Locale,
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::hit_map::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::Theme,
    workbench_client::{
        CloseConfirmationToken, CloseConfirmationView, CloseStatusToken, ModalPresentation,
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::Paragraph,
};

pub(super) struct ModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: ModalPresentation<CloseConfirmationToken>,
    pub(super) status: Option<CloseStatusToken>,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &CloseConfirmationView<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalDraw {
    let status = confirmation.resume_status();
    let title = confirmation.set_count().map_or_else(
        || locale.close_title(confirmation.kind()).to_owned(),
        |count| locale.close_set_title(count),
    );
    let mut lines = if let Some(count) = confirmation.set_count() {
        // Names are previews, not target identities. Bound them to one line, reserving the
        // modal margin, border and padding; the full warning/count must still fit to arm it.
        let preview_width = usize::from(content_area.width.saturating_sub(10)).min(64);
        let mut lines = vec![
            Line::from(""),
            description_line(
                truncate_display(
                    &locale.close_set_context(confirmation.set_context_label().unwrap_or_default()),
                    preview_width,
                    theme.glyphs.ellipsis,
                ),
                theme,
            ),
        ];
        for (position, label) in confirmation
            .set_labels(locale)
            .unwrap_or_default()
            .iter()
            .take(3)
        {
            let prefix = format!("#{position} · ");
            lines.push(Line::from(format!(
                "{prefix}{}",
                truncate_display(
                    &sanitize_terminal_text(label),
                    preview_width.saturating_sub(display_width(&prefix)),
                    theme.glyphs.ellipsis,
                )
            )));
        }
        if count > 3 {
            lines.push(description_line(locale.close_set_more(count - 3), theme));
        }
        lines.extend([
            Line::from(""),
            Line::from(locale.close_set_warning(confirmation.impact())),
            Line::from(""),
        ]);
        lines
    } else {
        vec![
            Line::from(""),
            Line::from(sanitize_terminal_text(&confirmation.target_label(locale)).into_owned()),
            Line::from(""),
            Line::from(locale.close_warning(confirmation.impact())),
            Line::from(""),
        ]
    };
    if let Some(status) = status.as_ref() {
        let label = status.target_label(locale);
        lines.push(description_line(
            locale.native_close_status(status.text(&label)),
            theme,
        ));
        lines.push(Line::from(""));
        if let Some(progress) = confirmation.resume_set_status() {
            lines.push(description_line(locale.close_set_status(progress), theme));
            lines.push(Line::from(""));
        }
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let confirm = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.tab_close_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.tab_close_confirm_label(),
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(&confirm)
        .with_tone(ActionTone::Destructive),
    ];
    lines.push(Line::from(""));
    let text = Text::from(lines);
    let Some(layout) = modal_layout(
        frame.area(),
        content_area,
        &title,
        &text,
        &[],
        actions,
        theme,
    ) else {
        return ModalDraw {
            area: Rect::default(),
            prompt: ModalPresentation::NotPresented,
            status: None,
        };
    };

    hits.start_modal();
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    paint_modal_frame(frame, layout.area, &title, theme);
    frame.render_widget(
        Paragraph::new(crate::terminal_text::wrap_text(&text, layout.inner.width)),
        layout.inner,
    );
    paint_modal_close(
        frame,
        layout.close,
        HitTarget::TabClose {
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
            HitTarget::TabClose {
                token: confirmation.token(),
                confirm,
            },
            theme,
            hits,
        );
    }
    ModalDraw {
        area: layout.area,
        prompt: ModalPresentation::Presented(confirmation.token()),
        status: status.map(|status| status.token()),
    }
}
