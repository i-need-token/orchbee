use super::*;
use crate::client_view::{
    ClientAction, ClientActionDisposition, ClientView, KeyLabelGlyphs, Overlay, PrefixDesc,
    PrefixInput, ShellCommand,
};

fn apply(view: &mut ClientView, input: PrefixInput) -> super::super::ClientTransition {
    let transition = view.decide(input);
    view.commit(transition.clone(), ClientActionDisposition::KeepNavigator);
    transition
}

fn enter(view: &mut ClientView) {
    apply(
        view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL)),
    );
    assert_eq!(
        apply(
            view,
            PrefixInput::Key(KeyEvent::press(KeyCode::Char('P'), Modifiers::NONE))
        )
        .action(),
        Some(ClientAction::EnterResizeMode)
    );
    assert!(view.resize_mode());
    assert!(!view.prefix_active());
}

#[test]
fn mode_owns_unknown_keys_and_help_ends_with_exit_and_does_not_nest_prefix() {
    let mut view = ClientView::new(120);
    enter(&mut view);
    for input in [
        PrefixInput::NonKey,
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('x'), Modifiers::NONE)),
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL)),
    ] {
        let before = view.clone();
        let transition = apply(&mut view, input);
        assert_eq!(transition.action(), None);
        assert_eq!(transition.shell(), ShellCommand::None);
        assert_eq!(view, before);
    }
    let action = apply(
        &mut view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Left, Modifiers::NONE)),
    )
    .action();
    assert_eq!(
        action,
        Some(ClientAction::ResizePane {
            direction: PanelDirection::Left,
            fine: false
        })
    );
    apply(
        &mut view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('?'), Modifiers::NONE)),
    );
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    assert_eq!(view.resize_help_rows().last().unwrap().0, "Esc");
    apply(
        &mut view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE)),
    );
    assert!(view.resize_mode());
    assert_eq!(view.overlay(), None);
    assert_eq!(
        apply(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE))
        )
        .action(),
        Some(ClientAction::ExitResizeMode)
    );
    assert!(!view.resize_mode());
    assert_eq!(
        apply(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::Right, Modifiers::NONE))
        )
        .shell(),
        ShellCommand::ForwardTerminal
    );
}

#[test]
fn reloading_or_forced_focus_clears_mode_but_does_not_retract_submission() {
    for reload in [false, true] {
        let mut view = ClientView::new(120);
        enter(&mut view);
        view.resize_pending = true;
        if reload {
            view.configure_keys(&orchbee_local_site_runtime::WindowsKeySettings::default());
        } else {
            view.dismiss_focus_modes();
        }
        assert!(!view.resize_mode());
        assert!(view.resize_pending);
        assert!(!view.prefix_active());
    }
}

#[test]
fn arrows_act_only_with_exact_modifiers_and_press_or_repeat() {
    for (code, direction) in ARROWS {
        for (mods, fine) in [
            (Modifiers::NONE, false),
            (Modifiers::NONE.with_shift(), true),
        ] {
            for kind in [KeyKind::Press, KeyKind::Repeat, KeyKind::Release] {
                assert_eq!(
                    decide(KeyEvent { code, mods, kind }),
                    (kind != KeyKind::Release).then_some(ResizeKey::Adjust { direction, fine })
                );
            }
        }
        for mods in [
            Modifiers::CTRL,
            Modifiers::NONE.with_alt(),
            Modifiers::CTRL.with_shift(),
        ] {
            assert_eq!(decide(KeyEvent::press(code, mods)), None);
        }
    }
}

#[test]
fn exit_help_and_unknown_input_cannot_become_adjustments() {
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Esc, Modifiers::NONE)),
        Some(ResizeKey::Exit)
    );
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Char('?'), Modifiers::NONE)),
        Some(ResizeKey::Help)
    );
    for code in [
        KeyCode::Enter,
        KeyCode::Char('P'),
        KeyCode::Char('b'),
        KeyCode::Char('x'),
    ] {
        assert_eq!(decide(KeyEvent::press(code, Modifiers::NONE)), None);
    }
}

#[test]
fn help_scroll_keys_work_inside_resize_mode_and_are_consumed_with_help_closed() {
    let mut view = ClientView::new(80);
    enter(&mut view);
    assert_eq!(
        apply(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::PageUp, Modifiers::NONE))
        )
        .action(),
        None
    );
    apply(
        &mut view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('?'), Modifiers::NONE)),
    );
    assert_eq!(
        apply(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::PageDown, Modifiers::NONE))
        )
        .action(),
        Some(ClientAction::PageShortcutHelp)
    );
    assert_eq!(view.help_page(), 8);
    assert_eq!(
        apply(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::PageUp, Modifiers::NONE))
        )
        .action(),
        Some(ClientAction::PageShortcutHelp)
    );
    assert_eq!(view.help_page(), 0);
    let rows = view.resize_help_rows_with_glyphs(KeyLabelGlyphs::Ascii);
    for (key, desc) in [
        ("?", PrefixDesc::ShowHelp),
        ("PageUp / PageDown", PrefixDesc::HelpPage),
        ("Esc", PrefixDesc::ResizeExit),
    ] {
        assert!(
            rows.iter()
                .any(|(candidate, candidate_desc)| candidate == key && *candidate_desc == desc),
            "the typed resize registry must expose {key:?} as {desc:?}"
        );
    }
}

#[test]
fn help_scroll_requests_accept_configured_actions_until_a_draw_settles_them() {
    let body = (0..64)
        .map(|i| format!("[[keys.command]]\ncommand='echo {i}'\n"))
        .collect::<String>();
    let keys = orchbee_local_site_runtime::WindowsKeySettings::from_profile(&body).unwrap();
    let mut view = ClientView::new(120);
    view.configure_keys(&keys);
    apply(
        &mut view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL)),
    );
    apply(
        &mut view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('?'), Modifiers::NONE)),
    );
    for _ in 0..200 {
        apply(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL)),
        );
        apply(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::PageDown, Modifiers::NONE)),
        );
    }
    assert_eq!(view.help_page(), 1_600);
}
