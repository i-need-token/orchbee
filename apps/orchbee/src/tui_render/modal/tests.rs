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
