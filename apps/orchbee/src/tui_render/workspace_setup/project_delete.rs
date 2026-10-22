//! Complete organization-only Project deletion decision; existing setup owns every effect.
use super::super::modal::paint_modal_frame;
use super::{
    Binding, Frame, HitLayer, HitMapBuilder, HitTarget, KeyCode, Line, Locale, ModalAction,
    ModalControl, ModalPresentation, Modifiers, NativeSetupToken, Paragraph, Rect, Text, Theme,
    Wrap, modal_key_label, modal_layout, paint_modal_action, paint_modal_close,
    sanitize_terminal_text,
};

pub(super) struct Decision<'a> {
    pub token: NativeSetupToken,
    pub project: &'a str,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    decision: Decision<'_>,
    content_area: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<NativeSetupToken>) {
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(
            locale.setup_cancel_label(),
            super::super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut(&escape),
        ModalAction::new(
            locale.sidebar_delete_label(),
            super::super::controls::ActionKind::Primary,
            true,
        )
        .with_tone(crate::action_projection::ActionTone::Destructive)
        .with_shortcut(&enter),
    ];
    let title = locale.project_delete_title();
    let text = Text::from(vec![
        Line::from(""),
        Line::from(sanitize_terminal_text(decision.project).into_owned()),
        Line::from(""),
        Line::from(locale.project_delete_warning()),
        Line::from(""),
        Line::from(""),
    ]);
    let Some(layout) = modal_layout(
        frame.area(),
        content_area,
        title,
        &text,
        &[],
        actions,
        theme,
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    hits.start_modal();
    paint_modal_frame(frame, layout.area, title, theme);
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }),
        layout.inner,
    );
    paint_modal_close(
        frame,
        layout.close,
        HitTarget::Setup {
            token: decision.token,
            control: ModalControl::Cancel,
        },
        theme,
        hits,
    );
    for ((rect, action), control) in layout
        .actions
        .into_iter()
        .zip(actions)
        .zip([ModalControl::Cancel, ModalControl::Submit])
    {
        paint_modal_action(
            frame,
            Rect::new(rect.x, rect.y, layout.inner.right() - rect.x, 1),
            rect,
            action,
            HitTarget::Setup {
                token: decision.token,
                control,
            },
            theme,
            hits,
        );
    }
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    (layout.area, ModalPresentation::Presented(decision.token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn compact(value: &str) -> String {
        value
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    #[test]
    fn project_delete_admits_the_whole_localized_preservation_decision_or_no_confirmation_controls()
    {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                for (width, height) in [(120, 40), (80, 24), (48, 11), (20, 5)] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let token = NativeSetupToken::for_test(17);
                    let mut rendered = None;
                    terminal
                        .draw(|frame| {
                            rendered = Some(render(
                                frame,
                                locale,
                                Decision {
                                    token,
                                    project: "Project 界",
                                },
                                Rect::new(0, 0, width, height - 1),
                                &theme,
                                &mut hits,
                            ));
                        })
                        .unwrap();
                    let (area, presentation) = rendered.unwrap();
                    // Read every interior cell, including wrapped decision text and controls,
                    // without inserting the frame's vertical borders between content lines.
                    let interior = area.inner(ratatui::layout::Margin::new(1, 1));
                    let buffer = terminal.backend().buffer();
                    let text: String = (interior.y..interior.bottom())
                        .map(|row| {
                            let mut line = String::new();
                            let mut column = interior.x;
                            while column < interior.right() {
                                let cell = &buffer[(column, row)];
                                line.push_str(cell.symbol());
                                column += u16::try_from(
                                    crate::terminal_text::display_width(cell.symbol()).max(1),
                                )
                                .unwrap();
                            }
                            line.push('\n');
                            line
                        })
                        .collect();
                    let map = hits.finish();
                    let mut submit = false;
                    let mut cancel = false;
                    for row in 0..height {
                        for column in 0..width {
                            if let Some(HitTarget::Setup {
                                token: hit_token,
                                control,
                            }) = map.hit_target_at(column, row)
                            {
                                assert_eq!(*hit_token, token);
                                match control {
                                    ModalControl::Submit => submit = true,
                                    ModalControl::Cancel => cancel = true,
                                    _ => panic!("decision has no editor or picker authority"),
                                }
                            }
                        }
                    }
                    if matches!(presentation, ModalPresentation::Presented(value) if value == token)
                    {
                        assert!(submit && cancel);
                        // The task heading lives on the frame's top edge, not in the body.
                        let top_edge: String = (area.x..area.right())
                            .map(|column| buffer[(column, area.y)].symbol())
                            .collect();
                        assert!(
                            compact(&top_edge).contains(&compact(locale.project_delete_title())),
                            "{locale:?} {width}x{height}: missing heading in {top_edge:?}"
                        );
                        for complete in [
                            locale.project_delete_warning(),
                            locale.sidebar_delete_label(),
                            locale.setup_cancel_label(),
                            "Project 界",
                            "Esc",
                            "Enter",
                        ] {
                            assert!(
                                compact(&text).contains(&compact(complete)),
                                "{locale:?} {width}x{height}: missing complete {complete:?} in {text:?}"
                            );
                        }
                        assert!(area.bottom() < height, "status row is not dialog content");
                    } else {
                        assert!(matches!(presentation, ModalPresentation::NotPresented));
                        assert!(area.is_empty() && !submit && !cancel);
                    }
                    if width >= 80 {
                        assert!(submit, "standard geometry must admit the complete decision");
                    } else if width == 20 {
                        assert!(!submit, "tiny geometry must refuse");
                    }
                }
            }
        }
    }
}
