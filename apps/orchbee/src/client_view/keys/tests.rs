use super::*;

const fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::press(code, Modifiers::NONE)
}

#[test]
fn c0_chord_and_named_alias_share_one_canonical_form() {
    let enter_event = KeyEvent::press(KeyCode::Enter, Modifiers::NONE);
    let ctrl_m = Binding::new(KeyCode::Char('m'), Modifiers::CTRL);
    assert!(ctrl_m.matches(enter_event));

    let ctrl_m_event = KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL);
    let enter_alias = Binding::new(KeyCode::Enter, Modifiers::NONE);
    assert!(enter_alias.matches(ctrl_m_event));
    assert_eq!(enter_alias, ctrl_m);
}

#[test]
fn modifier_supersets_never_match_a_binding() {
    let ctrl_x = Binding::new(KeyCode::Char('x'), Modifiers::CTRL);
    for mods in [
        Modifiers::CTRL.with_shift(),
        Modifiers::CTRL.with_alt(),
        Modifiers::CTRL.with_shift().with_alt(),
        Modifiers::CTRL.with_super_(),
    ] {
        assert!(!ctrl_x.matches(KeyEvent::press(KeyCode::Char('x'), mods)));
    }
    assert!(!ctrl_x.matches(key(KeyCode::Char('x'))));
}

#[test]
fn uppercase_events_fold_to_lowercase_plus_shift() {
    let shift_r = Binding::new(KeyCode::Char('r'), Modifiers::NONE.with_shift());
    assert!(shift_r.matches(key(KeyCode::Char('R'))));
    assert!(!shift_r.matches(key(KeyCode::Char('r'))));
    let plain_r = Binding::new(KeyCode::Char('r'), Modifiers::NONE);
    assert!(plain_r.matches(key(KeyCode::Char('r'))));
    assert!(!plain_r.matches(key(KeyCode::Char('R'))));
}

#[test]
fn modifier_subsets_do_not_match_either() {
    let alt_enter = Binding::new(KeyCode::Enter, Modifiers::NONE.with_alt());
    assert!(!alt_enter.matches(KeyEvent::press(KeyCode::Enter, Modifiers::NONE)));
    assert!(alt_enter.matches(KeyEvent::press(KeyCode::Enter, Modifiers::NONE.with_alt())));
}

#[test]
fn labels_keep_alias_display_names() {
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Enter, Modifiers::NONE),
            LabelStyle::Other
        ),
        "Enter"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Tab, Modifiers::NONE),
            LabelStyle::Other
        ),
        "Tab"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Space, Modifiers::NONE),
            LabelStyle::Other
        ),
        "Space"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Enter, Modifiers::NONE.with_shift()),
            LabelStyle::Other
        ),
        "Shift+Enter"
    );
}

#[test]
fn labels_render_modifiers_and_case_per_host_style() {
    assert_eq!(
        join_key_labels(
            &[
                "Shift+H".into(),
                "Shift+J".into(),
                "Shift+K".into(),
                "Shift+L".into()
            ],
            LabelStyle::Other,
            "/"
        ),
        "Shift+H/J/K/L"
    );
    assert_eq!(
        join_key_labels(&["⇧H".into(), "⇧J".into()], LabelStyle::Apple, " / "),
        "⇧H / J"
    );
    assert_eq!(
        join_key_labels(&["Shift+H".into(), "j".into()], LabelStyle::Other, "/"),
        "Shift+H/j"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Char('b'), Modifiers::CTRL),
            LabelStyle::Other
        ),
        "Ctrl+B"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Char('b'), Modifiers::CTRL),
            LabelStyle::Apple
        ),
        "⌃B"
    );
    // Letter case alone must not hide a required modifier from discovery.
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Char('x'), Modifiers::NONE.with_shift()),
            LabelStyle::Other
        ),
        "Shift+X"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Char('x'), Modifiers::CTRL.with_shift()),
            LabelStyle::Other
        ),
        "Ctrl+Shift+X"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Char('r'), Modifiers::NONE),
            LabelStyle::Other
        ),
        "r"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Char('r'), Modifiers::NONE.with_shift()),
            LabelStyle::Other
        ),
        "Shift+R"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Char('r'), Modifiers::NONE.with_shift()),
            LabelStyle::Apple
        ),
        "⇧R"
    );
    assert_eq!(
        format_binding(
            &Binding::new(KeyCode::Up, Modifiers::NONE),
            LabelStyle::Other
        ),
        "↑"
    );
}
