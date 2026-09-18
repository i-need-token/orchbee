use crate::{
    localization::Locale,
    terminal_session::{
        ModalPresentation, NativeCloseConfirmationToken, NativeCloseConfirmationView,
        NativeCloseStatusToken,
    },
    terminal_text::sanitize_terminal_text,
    tui_layout::centered_overlay,
    tui_theme::{Role, Theme},
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
    pub(super) prompt: ModalPresentation<NativeCloseConfirmationToken>,
    pub(super) status: Option<NativeCloseStatusToken>,
}

/// A modal surface asks for exactly the finite size its complete content needs
/// — borders plus horizontal padding around the natural text bounds — inside
/// the unified content area, which `centered_overlay` insets by a fixed margin.
/// When the offered area is narrower than the natural bounds the preferred
/// height grows to the wrapped line count at the resulting inner width; a
/// content area that still cannot present the complete text yields a
/// `NotPresented` outcome rather than a clipped confirmation.
pub(super) fn modal_area(content_area: Rect, text: &Text<'_>) -> Rect {
    let natural_width = u16::try_from(text.width().saturating_add(6)).unwrap_or(u16::MAX);
    let width = natural_width.min(content_area.width.saturating_sub(4));
    let inner_width = width.saturating_sub(6).max(1);
    let wrapped_height = Paragraph::new(text.clone())
        .wrap(Wrap { trim: false })
        .line_count(inner_width)
        .saturating_add(2);
    let height = u16::try_from(wrapped_height)
        .unwrap_or(u16::MAX)
        .min(content_area.height.saturating_sub(2));
    centered_overlay(content_area, width, height)
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &NativeCloseConfirmationView<'_>,
    content_area: Rect,
    theme: &Theme,
) -> ModalDraw {
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
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        ModalDraw {
            area,
            prompt: ModalPresentation::Presented(confirmation.token()),
            status: status.map(|status| status.token()),
        }
    } else {
        frame.render_widget(
            Paragraph::new(locale.close_resize_hint())
                .style(theme.style(Role::StatusWarning))
                .wrap(Wrap { trim: false }),
            inner,
        );
        ModalDraw {
            area,
            prompt: ModalPresentation::NotPresented,
            status: None,
        }
    }
}

pub(super) fn wrapped_text_fits(text: &Text<'_>, area: Rect) -> bool {
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
