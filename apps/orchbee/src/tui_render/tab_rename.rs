use super::tab_close::{modal_area, set_modal_cursor, wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{ModalPresentation, NativeRenameDraftToken, NativeRenameDraftView},
    terminal_text::sanitize_terminal_text,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

/// The draft's editable line inside the modal's line vector.
const FIELD: usize = 5;

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeRenameDraftView<'_>,
    content_area: Rect,
    theme: &Theme,
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
    lines.push(Line::from(locale.rename_actions()));
    let text = Text::from(lines);
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
    // The raw draft stays exact; what the modal draws is its safe projection
    // inside the inner width, with the raw cursor mapped through the same
    // projection — never a raw byte offset applied to sanitized text.
    let window = draft.field_window(usize::from(inner.width));
    let mut lines = text.lines;
    if let Some(window) = &window {
        lines[FIELD] = Line::from(window.display().to_owned());
    }
    let text = Text::from(lines);
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(
            Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
            inner,
        );
        if let Some(window) = window {
            set_modal_cursor(frame, &text, inner, FIELD, window.cursor_column());
        }
        (area, ModalPresentation::Presented(draft.token()))
    } else {
        frame.render_widget(
            Paragraph::new(locale.rename_resize_hint())
                .style(theme.style(Role::StatusWarning))
                .wrap(Wrap { trim: false }),
            inner,
        );
        (area, ModalPresentation::NotPresented)
    }
}
