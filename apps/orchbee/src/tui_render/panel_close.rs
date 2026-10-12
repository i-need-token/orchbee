use super::tab_close::{modal_area, wrapped_text_fits};
use crate::{
    localization::Locale,
    terminal_session::{
        ModalPresentation, NativePanelCloseConfirmationToken, NativePanelCloseConfirmationView,
    },
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Clear, Padding, Paragraph, Wrap},
};

pub(super) struct PanelModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: ModalPresentation<NativePanelCloseConfirmationToken>,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &NativePanelCloseConfirmationView<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> PanelModalDraw {
    hits.start_modal();
    let text = confirmation_text(
        locale,
        confirmation.panel_position(),
        confirmation.tab_position(),
        confirmation.panel_label(),
        confirmation.tab_label(),
        confirmation.dissolves_tab(),
    );
    let (area, complete) = render_content(
        frame,
        content_area,
        locale.panel_close_resize_hint(),
        text.clone(),
        theme,
    );
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    if let Some(close) = super::controls::close(frame, area, theme) {
        hits.record(
            HitLayer::Overlay,
            close,
            HitTarget::PanelClose {
                token: confirmation.token(),
                confirm: false,
            },
        );
    }
    if complete {
        let inner = super::overlay_surface(theme, true, Padding::horizontal(2)).inner(area);
        if let Some(buttons) = super::controls::pair(
            frame,
            super::controls::text_row(&text, inner, text.lines.len() - 1),
            [
                locale.workspace_close_cancel_button(),
                locale.workspace_close_confirm_button(),
            ],
            theme,
        ) {
            for (rect, confirm) in buttons.into_iter().zip([false, true]) {
                hits.record(
                    HitLayer::Overlay,
                    rect,
                    HitTarget::PanelClose {
                        token: confirmation.token(),
                        confirm,
                    },
                );
            }
        }
    }
    PanelModalDraw {
        area,
        prompt: if complete {
            ModalPresentation::Presented(confirmation.token())
        } else {
            ModalPresentation::NotPresented
        },
    }
}

fn confirmation_text(
    locale: Locale,
    panel_position: usize,
    tab_position: usize,
    panel_label: &str,
    tab_label: &str,
    dissolves_tab: bool,
) -> Text<'static> {
    let [panel, tab] =
        locale.panel_close_targets(panel_position, tab_position, panel_label, tab_label);
    Text::from(vec![
        Line::from(locale.panel_close_title()),
        Line::from(""),
        Line::from(panel),
        Line::from(tab),
        Line::from(""),
        Line::from(locale.panel_close_warning(dissolves_tab)),
        Line::from(""),
        Line::from(format!(
            "{}  {}",
            locale.workspace_close_cancel_button(),
            locale.workspace_close_confirm_button()
        )),
    ])
}

/// A panel close can only be confirmed while its complete warning text fit the
/// draw inside the unified content area: a clipped or unplaceable modal
/// renders the resize hint and yields a `NotPresented` outcome.
fn render_content(
    frame: &mut Frame<'_>,
    content_area: Rect,
    resize_hint: &str,
    text: Text<'_>,
    theme: &Theme,
) -> (Rect, bool) {
    let area = modal_area(content_area, &text);
    let block = super::overlay_surface(theme, true, Padding::horizontal(2));
    let inner = block.inner(area);
    let complete = wrapped_text_fits(&text, inner)
        && text
            .lines
            .last()
            .is_some_and(|line| line.width() <= usize::from(inner.width));
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if complete {
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
    } else {
        frame.render_widget(
            Paragraph::new(resize_hint)
                .style(theme.style(Role::StatusWarning))
                .wrap(Wrap { trim: false }),
            inner,
        );
    }
    (area, complete)
}

#[cfg(test)]
mod tests;
