//! Paints the admitted Panel close confirmation and returns only controls and draw proof tied
//! to its exact confirmation token. It does not submit closure or infer physical disposal from
//! the displayed state.

use super::modal::{
    ModalAction, description_line, modal_key_label, modal_layout, paint_modal_action,
    paint_modal_close, paint_modal_frame,
};
use crate::{
    client_view::{Binding, KeyCode, Modifiers},
    localization::Locale,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::Theme,
    workbench_client::{
        ModalPresentation, PanelCloseConfirmationToken, PanelCloseConfirmationView,
    },
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Paragraph, Wrap},
};

pub(super) struct PanelModalDraw {
    pub(super) area: Rect,
    pub(super) prompt: ModalPresentation<PanelCloseConfirmationToken>,
}

struct PreparedContent<'a> {
    title: &'a str,
    text: Text<'a>,
    hints: &'a [super::controls::KeyHint<'a>],
    actions: [ModalAction<'a>; 2],
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    confirmation: &PanelCloseConfirmationView<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> PanelModalDraw {
    let text = confirmation_text(
        locale,
        confirmation.panel_position(),
        confirmation.tab_position(),
        confirmation.panel_label(),
        confirmation.tab_label(),
        confirmation.dissolves_tab(),
        theme,
    );
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let confirm = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.panel_close_cancel_label(),
            super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.panel_close_confirm_label(),
            super::controls::ActionKind::Primary,
            true,
        )
        .with_shortcut(&confirm),
    ];
    render_prepared(
        frame,
        confirmation.token(),
        content_area,
        PreparedContent {
            title: locale.panel_close_title(),
            text,
            hints: &[],
            actions,
        },
        theme,
        hits,
    )
}

fn render_prepared(
    frame: &mut Frame<'_>,
    token: PanelCloseConfirmationToken,
    content_area: Rect,
    content: PreparedContent<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> PanelModalDraw {
    let PreparedContent {
        title,
        text,
        hints,
        actions,
    } = content;
    let Some(layout) = modal_layout(
        frame.area(),
        content_area,
        title,
        &text,
        hints,
        actions,
        theme,
    ) else {
        return PanelModalDraw {
            area: Rect::default(),
            prompt: ModalPresentation::NotPresented,
        };
    };

    hits.start_modal();
    paint_modal_frame(frame, layout.area, title, theme);
    frame.render_widget(
        Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
        layout.inner,
    );
    paint_modal_close(
        frame,
        layout.close,
        HitTarget::PanelClose {
            token,
            confirm: false,
        },
        theme,
        hits,
    );
    if !hints.is_empty() {
        let rendered_hints =
            super::controls::render_overlay_hints(frame, layout.hints, hints, theme)
                .expect("admitted modal hints must paint");
        debug_assert!(layout.hints.intersection(rendered_hints) == rendered_hints);
    }
    for ((rect, action), confirm) in layout.actions.into_iter().zip(actions).zip([false, true]) {
        paint_modal_action(
            frame,
            Rect::new(rect.x, rect.y, layout.inner.right() - rect.x, 1),
            rect,
            action,
            HitTarget::PanelClose { token, confirm },
            theme,
            hits,
        );
    }
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    PanelModalDraw {
        area: layout.area,
        prompt: ModalPresentation::Presented(token),
    }
}

fn confirmation_text(
    locale: Locale,
    panel_position: usize,
    tab_position: usize,
    panel_label: &str,
    tab_label: &str,
    dissolves_tab: bool,
    theme: &Theme,
) -> Text<'static> {
    let [panel, tab] =
        locale.panel_close_targets(panel_position, tab_position, panel_label, tab_label);
    Text::from(vec![
        Line::from(""),
        Line::from(panel),
        description_line(tab, theme),
        Line::from(""),
        Line::from(locale.panel_close_warning(dissolves_tab)),
        Line::from(""),
        Line::from(""),
    ])
}

/// Test seam for the same atomic geometry admission and paint used by the
/// production panel-close compositor.
#[cfg(test)]
fn render_content(
    frame: &mut Frame<'_>,
    content_area: Rect,
    text: Text<'_>,
    hints: &[super::controls::KeyHint<'_>],
    actions: [ModalAction<'_>; 2],
    theme: &Theme,
) -> (Rect, bool) {
    let mut hits = HitMapBuilder::new();
    let draw = render_prepared(
        frame,
        PanelCloseConfirmationToken::for_test(0),
        content_area,
        PreparedContent {
            title: Locale::EnUs.panel_close_title(),
            text,
            hints,
            actions,
        },
        theme,
        &mut hits,
    );
    (
        draw.area,
        matches!(draw.prompt, ModalPresentation::Presented(_)),
    )
}

#[cfg(test)]
mod tests;
