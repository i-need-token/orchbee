use super::modal::{
    ModalAction, modal_key_label, modal_layout, paint_modal_action, paint_modal_close,
};
use crate::{
    client_view::{Binding, KeyCode, Modifiers},
    localization::Locale,
    terminal_session::{
        ModalPresentation, NativeCloseConfirmationToken, NativeCloseConfirmationView,
        NativeCloseStatusToken,
    },
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Clear, Padding, Paragraph, Wrap},
};

pub(super) struct ModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: ModalPresentation<NativeCloseConfirmationToken>,
    pub(super) status: Option<NativeCloseStatusToken>,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &NativeCloseConfirmationView<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalDraw {
    let status = confirmation.resume_status();
    let mut lines = if let Some(count) = confirmation.set_count() {
        // Names are previews, not target identities. Bound them to one line, reserving the
        // modal margin, border and padding; the full warning/count must still fit to arm it.
        let preview_width = usize::from(content_area.width.saturating_sub(10)).min(64);
        let mut lines = vec![
            Line::from(locale.close_set_title(count)),
            Line::from(""),
            Line::from(truncate_display(
                &locale.close_set_context(confirmation.set_context_label().unwrap_or_default()),
                preview_width,
                theme.glyphs.ellipsis,
            )),
        ];
        for (position, label) in confirmation.set_labels().unwrap_or_default().iter().take(3) {
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
            lines.push(Line::from(locale.close_set_more(count - 3)));
        }
        lines.extend([
            Line::from(""),
            Line::from(locale.close_set_warning()),
            Line::from(""),
        ]);
        lines
    } else {
        vec![
            Line::from(locale.close_title(confirmation.kind())),
            Line::from(""),
            Line::from(sanitize_terminal_text(confirmation.target_label()).into_owned()),
            Line::from(""),
            Line::from(locale.close_warning()),
            Line::from(""),
        ]
    };
    if let Some(status) = status.as_ref() {
        lines.push(Line::from(locale.native_close_status(status)));
        lines.push(Line::from(""));
        if let Some(progress) = confirmation.resume_set_status() {
            lines.push(Line::from(locale.close_set_status(progress)));
            lines.push(Line::from(""));
        }
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let confirm = modal_key_label(theme, &Binding::new(KeyCode::Char('y'), Modifiers::NONE));
    let hints = [
        super::controls::KeyHint::new(&escape, locale.tab_close_cancel_hint()),
        super::controls::KeyHint::new(&confirm, locale.tab_close_confirm_hint()),
    ];
    let actions = [
        ModalAction::new(
            locale.tab_close_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        ),
        ModalAction::new(
            locale.tab_close_confirm_label(),
            super::controls::ActionKind::Primary,
            true,
        ),
    ];
    lines.extend([Line::from(""), Line::from("")]);
    let text = Text::from(lines);
    let Some(layout) = modal_layout(frame.area(), content_area, &text, &hints, actions, theme)
    else {
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
    frame.render_widget(Clear, layout.area);
    frame.render_widget(
        super::overlay_surface(theme, true, Padding::horizontal(2)),
        layout.area,
    );
    frame.render_widget(
        Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
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
    let rendered_hints = super::controls::render_key_hints(frame, layout.hints, &hints, theme)
        .expect("admitted modal hints must paint");
    debug_assert!(layout.hints.intersection(rendered_hints) == rendered_hints);
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
