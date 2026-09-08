use super::{EMPHASIS, SIDEBAR_BACKGROUND, TEXT, WARNING, tab_close::wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{NativeRenameDraftToken, NativeRenameDraftView},
    terminal_text::sanitize_terminal_text,
    tui_layout::centered_overlay,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &NativeRenameDraftView<'_>,
) -> (Rect, Option<NativeRenameDraftToken>) {
    let area = centered_overlay(frame.area(), u16::MAX, u16::MAX);
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(2))
        .style(Style::new().bg(SIDEBAR_BACKGROUND).fg(TEXT))
        .border_style(Style::new().fg(EMPHASIS));
    let inner = block.inner(area);
    let mut lines = vec![
        Line::from(locale.rename_title()),
        Line::from(""),
        Line::from(sanitize_terminal_text(draft.target_label()).into_owned()),
        Line::from(""),
        Line::from(locale.rename_label()),
        Line::from(sanitize_terminal_text(draft.text()).into_owned()),
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
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        (area, Some(draft.token()))
    } else {
        frame.render_widget(
            Paragraph::new(locale.rename_resize_hint())
                .style(Style::new().fg(WARNING))
                .wrap(Wrap { trim: false }),
            inner,
        );
        (area, None)
    }
}
