use super::modal::{
    ModalAction, description_line, modal_key_label, modal_layout, paint_modal_action,
    paint_modal_close, paint_modal_frame, set_modal_cursor, status_line,
};
use crate::{
    client_view::{Binding, KeyCode, Modifiers},
    localization::Locale,
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
    workbench_client::{ModalPresentation, NativeRenameDraftToken, NativeRenameDraftView},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Wrap},
};

/// The draft's editable line inside the modal's line vector.
const FIELD: usize = 4;

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeRenameDraftView<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<NativeRenameDraftToken>) {
    let placeholder_visible = draft.automatic_label() && draft.text().is_empty();
    let title = locale.rename_title();
    let mut lines = vec![
        Line::from(""),
        description_line(
            sanitize_terminal_text(&draft.target_label(locale)).into_owned(),
            theme,
        ),
        Line::from(""),
        Line::from(locale.rename_label()),
        // The field's one visible row is sized like the rest: the windowed
        // projection never takes more than the inner width it is given.
        Line::from(""),
        description_line(locale.rename_warning(), theme),
        Line::from(""),
    ];
    if draft.input_fault() {
        lines.push(status_line(locale.rename_fault(), Role::StatusError, theme));
        lines.push(Line::from(""));
    } else if !draft.label_valid() && !placeholder_visible {
        lines.push(status_line(
            locale.rename_invalid(),
            Role::StatusError,
            theme,
        ));
        lines.push(Line::from(""));
    }
    lines.extend([Line::from(""), Line::from("")]);
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let backspace = modal_key_label(theme, &Binding::new(KeyCode::Backspace, Modifiers::NONE));
    let clear = modal_key_label(theme, &Binding::new(KeyCode::Char('u'), Modifiers::CTRL));
    let hints = [
        super::controls::KeyHint::new(&backspace, locale.rename_delete_hint()),
        super::controls::KeyHint::new(&clear, locale.rename_clear_hint()),
    ];
    let submit_enabled = draft.label_valid() && !draft.input_fault();
    let actions = [
        ModalAction::new(
            locale.rename_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.rename_submit_label(),
            super::controls::ActionKind::Primary,
            submit_enabled,
        )
        .with_shortcut(&enter),
    ];
    let text = Text::from(lines);
    let Some(candidate) = modal_layout(
        frame.area(),
        content_area,
        title,
        &text,
        &hints,
        actions,
        theme,
    ) else {
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
    let (display, role) = if placeholder_visible {
        let target_label = draft.target_label(locale);
        let label = sanitize_terminal_text(&target_label);
        (
            truncate_display(&label, usize::from(field_width), theme.glyphs.ellipsis),
            Role::TextSecondary,
        )
    } else {
        (window.display().to_owned(), Role::TextPrimary)
    };
    let mut displayed = display;
    displayed.push_str(&" ".repeat(
        usize::from(field_width).saturating_sub(crate::terminal_text::display_width(&displayed)),
    ));
    lines[FIELD] = Line::from(vec![
        Span::styled("[ ", theme.style(crate::tui_theme::Role::AccentFocus)),
        Span::styled(displayed, theme.style(role)),
        Span::styled(" ]", theme.style(crate::tui_theme::Role::AccentFocus)),
    ]);
    let text = Text::from(lines);
    let Some(layout) = modal_layout(
        frame.area(),
        content_area,
        title,
        &text,
        &hints,
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
    let rendered_hints = super::controls::render_overlay_hints(frame, layout.hints, &hints, theme)
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

/// Same draft, input owner, and shown token as the full editor, anchored to its tab.
#[cfg(windows)]
pub(super) fn render_inline(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeRenameDraftView<'_>,
    area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<NativeRenameDraftToken>) {
    let width = usize::from(area.width.saturating_sub(4));
    let Some(window) = draft
        .field_window(width)
        .filter(|_| width >= 4 && frame.area().intersection(area) == area)
    else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let text = if draft.automatic_label() && draft.text().is_empty() {
        truncate_display(
            &sanitize_terminal_text(&draft.target_label(locale)),
            width,
            theme.glyphs.ellipsis,
        )
    } else {
        window.display().to_owned()
    };
    let role = if draft.input_fault() || !draft.label_valid() && !draft.text().is_empty() {
        Role::StatusError
    } else {
        Role::AccentFocus
    };
    hits.start_modal();
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(format!("[ {text} ]")).style(theme.style(role)),
        area,
    );
    frame.set_cursor_position((
        area.x + 2 + u16::try_from(window.cursor_column()).unwrap_or(0),
        area.y,
    ));
    (area, ModalPresentation::Presented(draft.token()))
}
