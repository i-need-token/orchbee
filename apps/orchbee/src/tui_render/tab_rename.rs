use super::modal::{
    ModalAction, modal_key_label, modal_layout, paint_modal_action, paint_modal_close,
    set_modal_cursor,
};
use crate::{
    client_view::{Binding, KeyCode, Modifiers},
    localization::Locale,
    terminal_session::{ModalPresentation, NativeRenameDraftToken, NativeRenameDraftView},
    terminal_text::sanitize_terminal_text,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Clear, Padding, Paragraph, Wrap},
};

/// The draft's editable line inside the modal's line vector.
const FIELD: usize = 5;

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeRenameDraftView<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<NativeRenameDraftToken>) {
    let mut lines = vec![
        Line::from(locale.rename_title()),
        Line::from(""),
        Line::from(sanitize_terminal_text(&draft.target_label()).into_owned()),
        Line::from(""),
        Line::from(locale.rename_label()),
        // The field's one visible row is sized like the rest: the windowed
        // projection never takes more than the inner width it is given.
        Line::from(""),
        Line::from(""),
        Line::from(locale.rename_warning()),
        Line::from(""),
    ];
    if draft.input_fault() {
        lines.push(Line::from(locale.rename_fault()));
        lines.push(Line::from(""));
    } else if !draft.label_valid() {
        lines.push(Line::from(locale.rename_invalid()));
        lines.push(Line::from(""));
    }
    lines.extend([Line::from(""), Line::from("")]);
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let backspace = modal_key_label(theme, &Binding::new(KeyCode::Backspace, Modifiers::NONE));
    let clear = modal_key_label(theme, &Binding::new(KeyCode::Char('u'), Modifiers::CTRL));
    let hints = [
        super::controls::KeyHint::new(&escape, locale.rename_cancel_hint()),
        super::controls::KeyHint::new(&enter, locale.rename_submit_hint()),
        super::controls::KeyHint::new(&backspace, locale.rename_delete_hint()),
        super::controls::KeyHint::new(&clear, locale.rename_clear_hint()),
    ];
    let submit_enabled = draft.label_valid() && !draft.input_fault();
    let actions = [
        ModalAction::new(
            locale.rename_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        ),
        ModalAction::new(
            locale.rename_submit_label(),
            super::controls::ActionKind::Primary,
            submit_enabled,
        ),
    ];
    let text = Text::from(lines);
    let Some(candidate) = modal_layout(frame.area(), content_area, &text, &hints, actions, theme)
    else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    // The raw draft stays exact; what the modal draws is its safe projection
    // inside the inner width, with the raw cursor mapped through the same
    // projection — never a raw byte offset applied to sanitized text.
    let Some(field_width) = candidate.inner.width.checked_sub(4) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(window) = draft.field_window(usize::from(field_width)) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let mut lines = text.lines;
    let mut displayed = window.display().to_owned();
    displayed.push_str(&" ".repeat(
        usize::from(field_width).saturating_sub(crate::terminal_text::display_width(&displayed)),
    ));
    lines[FIELD] = Line::from(vec![
        Span::styled("[ ", theme.style(crate::tui_theme::Role::AccentFocus)),
        Span::styled(displayed, theme.style(crate::tui_theme::Role::TextPrimary)),
        Span::styled(" ]", theme.style(crate::tui_theme::Role::AccentFocus)),
    ]);
    let text = Text::from(lines);
    let Some(layout) = modal_layout(frame.area(), content_area, &text, &hints, actions, theme)
    else {
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
        Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
        layout.inner,
    );
    paint_modal_close(
        frame,
        layout.close,
        HitTarget::Rename {
            token: draft.token(),
            submit: false,
        },
        theme,
        hits,
    );
    let rendered_hints = super::controls::render_key_hints(frame, layout.hints, &hints, theme)
        .expect("admitted modal hints must paint");
    debug_assert!(layout.hints.intersection(rendered_hints) == rendered_hints);
    for ((rect, action), submit) in layout.actions.into_iter().zip(actions).zip([false, true]) {
        paint_modal_action(
            frame,
            Rect::new(rect.x, rect.y, layout.inner.right() - rect.x, 1),
            rect,
            action,
            HitTarget::Rename {
                token: draft.token(),
                submit,
            },
            theme,
            hits,
        );
    }
    set_modal_cursor(
        frame,
        &text,
        layout.inner,
        FIELD,
        window.cursor_column() + 2,
    );
    (layout.area, ModalPresentation::Presented(draft.token()))
}
