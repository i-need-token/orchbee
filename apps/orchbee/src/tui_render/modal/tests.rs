use super::{ModalAction, modal_layout, unwrapped_text_fits, wrapped_text_fits};
use crate::{terminal_text::sanitize_terminal_text, tui_theme::Theme};
use ratatui::{
    layout::Rect,
    text::{Line, Text},
};

#[test]
fn complete_wrapped_unicode_uses_final_inner_dimensions() {
    let label = "界".repeat(512);
    let text = Text::from(sanitize_terminal_text(&label).into_owned());
    assert!(wrapped_text_fits(&text, Rect::new(0, 0, 64, 16)));
    assert!(!wrapped_text_fits(&text, Rect::new(0, 0, 64, 15)));
    assert!(!wrapped_text_fits(&text, Rect::new(0, 0, 1, 1024)));
    assert!(!wrapped_text_fits(&text, Rect::default()));
}

#[test]
fn combining_text_is_measured_after_terminal_sanitization() {
    let unsafe_label = "e\u{301}\x1b[31m\n界";
    let safe = sanitize_terminal_text(unsafe_label);
    assert!(!safe.contains('\x1b'));
    assert!(!safe.contains('\n'));
    let text = Text::from(safe.as_ref());
    let width = u16::try_from(text.width()).unwrap();
    assert!(wrapped_text_fits(&text, Rect::new(0, 0, width, 1)));
    assert!(unwrapped_text_fits(&safe, Rect::new(0, 0, width, 1)));
    assert!(!unwrapped_text_fits(&safe, Rect::new(0, 0, width - 1, 1)));
}

#[test]
fn modal_layout_requires_complete_frame_actions_and_close_geometry() {
    let theme = Theme::rgb_test_dark();
    let hints = [
        super::super::controls::KeyHint::new("Esc", "cancel"),
        super::super::controls::KeyHint::new("Y", "close"),
    ];
    let actions = [
        ModalAction::new(
            "Cancel",
            super::super::controls::ActionKind::Secondary,
            true,
        ),
        ModalAction::new("Close", super::super::controls::ActionKind::Primary, true),
    ];
    let text = Text::from(vec![
        Line::from("Close"),
        Line::from(""),
        Line::from("Warning"),
        Line::from(""),
        Line::from(""),
    ]);
    let bounds = Rect::new(0, 0, 80, 24);
    let admitted = modal_layout(bounds, bounds, &text, &hints, actions, &theme).unwrap();
    assert_eq!(bounds.intersection(admitted.area), admitted.area);
    assert_eq!(admitted.close.width, 3);
    assert_eq!(admitted.close.height, 1);
    assert!(
        admitted
            .actions
            .iter()
            .all(|rect| admitted.inner.intersection(*rect) == *rect)
    );

    assert!(
        modal_layout(
            bounds,
            Rect::new(0, 0, 10, 3),
            &text,
            &hints,
            actions,
            &theme,
        )
        .is_none()
    );
    assert!(
        modal_layout(
            bounds,
            Rect::new(76, 2, 20, 10),
            &text,
            &hints,
            actions,
            &theme,
        )
        .is_none()
    );

    let long_label = "Confirm permanently".repeat(8);
    let clipped_actions = [
        actions[0],
        ModalAction::new(
            &long_label,
            super::super::controls::ActionKind::Primary,
            true,
        ),
    ];
    assert!(modal_layout(bounds, bounds, &text, &hints, clipped_actions, &theme).is_none());

    let oversized = Text::from("界".repeat(4096));
    assert!(modal_layout(bounds, bounds, &oversized, &hints, actions, &theme).is_none());
    let empty = Text::from(Vec::<Line<'static>>::new());
    assert!(modal_layout(bounds, bounds, &empty, &hints, actions, &theme).is_none());
}

#[test]
fn optional_help_never_stretches_the_reading_width_or_displaces_complete_decision_buttons() {
    use super::super::controls::{ActionKind, KeyHint};
    use crate::localization::Locale;

    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::one_light()] {
            let actions = [
                ModalAction::new(locale.setup_cancel_label(), ActionKind::Secondary, true)
                    .with_shortcut("Esc"),
                ModalAction::new(locale.setup_submit_label(), ActionKind::Primary, true)
                    .with_shortcut("Enter"),
            ];
            let text = Text::from(vec![
                super::title_line(locale.rename_title(), &theme),
                Line::from(""),
                super::description_line("wide 界 context ".repeat(16), &theme),
                Line::from(""),
                Line::from(""),
                Line::from(""),
            ]);
            let long_help = "supplemental help ".repeat(40);
            let hints = [
                KeyHint::new("Ctrl+U", locale.setup_clear_hint()),
                KeyHint::new("F1", &long_help),
            ];
            for (width, expected_width) in [(120, 78), (80, 76), (60, 56), (48, 44)] {
                let bounds = Rect::new(0, 0, width, 40);
                let layout = modal_layout(bounds, bounds, &text, &hints, actions, &theme).unwrap();
                let without_optional =
                    modal_layout(bounds, bounds, &text, &hints[..1], actions, &theme).unwrap();
                assert_eq!(layout.area.width, expected_width);
                assert_eq!(layout.area, without_optional.area);
                assert!(layout.actions[0].right() + 2 <= layout.actions[1].x);
                assert_eq!(layout.actions[0].y, layout.actions[1].y);
                assert!(layout.inner.intersection(layout.actions[1]) == layout.actions[1]);
            }
        }
    }
}

#[test]
fn single_action_can_own_its_exit_binding_without_a_duplicate_hint_row() {
    use super::super::controls::ActionKind;

    let theme = Theme::terminal_native();
    let text = Text::from(vec![
        Line::from("Task"),
        Line::from(""),
        Line::from(""),
        Line::from(""),
    ]);
    let bounds = Rect::new(0, 0, 60, 12);
    let action = ModalAction::new("Cancel", ActionKind::Secondary, true).with_shortcut("Esc");
    let layout = super::single_modal_layout(bounds, bounds, &text, &[], action, &theme).unwrap();
    assert_eq!(layout.inner.intersection(layout.action), layout.action);
    assert!(
        super::single_modal_layout(
            Rect::new(0, 0, 20, 5),
            Rect::new(0, 0, 20, 5),
            &text,
            &[],
            action,
            &theme
        )
        .is_none()
    );
}
