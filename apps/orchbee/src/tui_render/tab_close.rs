use super::{EMPHASIS, SIDEBAR_BACKGROUND, TEXT, WARNING};
use crate::{
    localization::Locale,
    terminal_session::{
        NativeCloseConfirmationToken, NativeCloseConfirmationView, NativeCloseStatusToken,
    },
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

pub(super) struct ModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: Option<NativeCloseConfirmationToken>,
    pub(super) status: Option<NativeCloseStatusToken>,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &NativeCloseConfirmationView<'_>,
) -> ModalDraw {
    let area = centered_overlay(frame.area(), u16::MAX, u16::MAX);
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(2))
        .style(Style::new().bg(SIDEBAR_BACKGROUND).fg(TEXT))
        .border_style(Style::new().fg(EMPHASIS));
    let inner = block.inner(area);
    let status = confirmation.resume_status();
    let mut lines = vec![
        Line::from(locale.close_title(confirmation.kind())),
        Line::from(""),
        Line::from(sanitize_terminal_text(confirmation.target_label()).into_owned()),
        Line::from(""),
        Line::from(locale.close_warning()),
        Line::from(""),
    ];
    if let Some(status) = status.as_ref() {
        lines.push(Line::from(locale.native_close_status(status)));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(locale.close_actions()));
    let text = Text::from(lines);
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        ModalDraw {
            area,
            prompt: Some(confirmation.token()),
            status: status.map(|status| status.token()),
        }
    } else {
        frame.render_widget(
            Paragraph::new(locale.close_resize_hint())
                .style(Style::new().fg(WARNING))
                .wrap(Wrap { trim: false }),
            inner,
        );
        ModalDraw {
            area,
            prompt: None,
            status: None,
        }
    }
}

fn wrapped_text_fits(text: &Text<'_>, area: Rect) -> bool {
    if area.is_empty() {
        return false;
    }
    // The wrapper drops any grapheme wider than the line; line_count alone cannot detect that.
    let graphemes_fit = text.iter().all(|line| {
        line.spans.iter().all(|span| {
            span.styled_graphemes(Style::default())
                .all(|grapheme| Line::raw(grapheme.symbol).width() <= usize::from(area.width))
        })
    });
    graphemes_fit
        && Paragraph::new(text.clone())
            .wrap(Wrap { trim: false })
            .line_count(area.width)
            <= usize::from(area.height)
}

pub(super) fn unwrapped_text_fits(text: &str, area: Rect) -> bool {
    let text = Text::from(text);
    !area.is_empty()
        && text.height() <= usize::from(area.height)
        && text
            .iter()
            .all(|line| line.width() <= usize::from(area.width))
}

#[cfg(test)]
mod tests;
