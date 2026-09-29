use super::*;
use crate::client_view::KeyEvent;
use orchbee_local_site_runtime::{
    WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger,
};

fn chord(key: &str, ctrl: bool) -> WindowsKeyChord {
    WindowsKeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    }
}
fn binding(action: &'static str, key: &str, prefix: bool) -> WindowsKeyOverride {
    WindowsKeyOverride {
        action,
        triggers: vec![WindowsKeyTrigger {
            prefix,
            chord: chord(key, false),
        }],
    }
}
fn run(
    view: &mut ClientView,
    dispatcher: &mut InputDispatcher,
    port: &mut impl TerminalInputPort,
    bytes: &[u8],
) {
    assert_eq!(
        dispatcher
            .dispatch(
                view,
                bytes,
                port,
                Instant::now(),
                |_, _| Ok::<_, io::Error>(()),
                &mut |_| Ok(())
            )
            .unwrap(),
        DispatchOutcome::Continue
    );
}

#[test]
fn configured_actions_displace_defaults_and_preserve_passthrough_order() {
    let keys = WindowsKeySettings {
        prefix: chord("a", true),
        overrides: vec![binding("new_tab", "n", true), binding("help", "f1", false)],
    };
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    run(
        &mut view,
        &mut dispatcher,
        &mut port,
        b"before\x02\x01nmid\x01\x01after\x1bOP",
    );
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before\x02".to_vec()),
            Call::Action(ClientAction::CreateTab),
            Call::Forward(b"mid".to_vec()),
            Call::Action(ClientAction::ForwardLiteralPrefix),
            Call::Forward(vec![1]),
            Call::Forward(b"after".to_vec()),
            Call::Action(ClientAction::OpenShortcutHelp)
        ]
    );
    run(&mut view, &mut dispatcher, &mut port, b"\x01c");
    assert_eq!(
        calls.borrow().last(),
        Some(&Call::Action(ClientAction::UnknownPrefix))
    );
}

#[test]
fn configured_agent_cycle_captures_packet_tail_and_never_acts_inside_picker() {
    let keys = WindowsKeySettings::from_profile(
        "[keys]\nprefix='ctrl+a'\nprevious_agent='alt+p'\nnext_agent=[]",
    )
    .unwrap();
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    view.machines.available = true;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    run(&mut view, &mut dispatcher, &mut port, b"before\x1bptail");
    assert!(view.machines.wants_agents());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before".to_vec()),
            Call::Action(ClientAction::PreviousAgent)
        ]
    );
    run(&mut view, &mut dispatcher, &mut port, b"\x1bp");
    assert_eq!(
        calls.borrow().len(),
        2,
        "picker captures configured direct keys"
    );
    view.machines
        .key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    run(&mut view, &mut dispatcher, &mut port, b"\x01}");
    assert_eq!(
        calls.borrow().last(),
        Some(&Call::Action(ClientAction::UnknownPrefix))
    );
    assert!(!view.machines.open);
}

#[test]
fn function_prefix_is_atomic_and_literal_forwarding_never_falls_back_to_control_b() {
    let keys = WindowsKeySettings {
        prefix: chord("f2", false),
        ..WindowsKeySettings::default()
    };
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    run(&mut view, &mut dispatcher, &mut port, b"\x1bO");
    run(&mut view, &mut dispatcher, &mut port, b"Q\x1bOQtail");
    assert_eq!(
        *calls.borrow(),
        [
            Call::Action(ClientAction::ForwardLiteralPrefix),
            Call::Forward(b"\x1bOQ".to_vec()),
            Call::Forward(b"tail".to_vec())
        ]
    );
    assert!(!view.prefix_active());
}

#[test]
fn custom_direct_keys_respect_confirmation_board_history_paste_and_releases() {
    struct Port {
        owner: u8,
        actions: Vec<ClientAction>,
        forwarded: Vec<u8>,
    }
    impl TerminalInputPort for Port {
        fn apply_client_action(
            &mut self,
            action: ClientAction,
        ) -> Option<SessionActionDisposition> {
            self.actions.push(action);
            Some(SessionActionDisposition::KeepNavigator)
        }
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.extend_from_slice(bytes);
            Ok(())
        }
        fn capture_confirmation_key(&mut self, _: ClientKey) -> bool {
            self.owner == 1
        }
        fn agent_board_input(&mut self) -> bool {
            self.owner == 2
        }
        fn capture_agent_board_key(&mut self, _: crate::client_view::KeyEvent) -> bool {
            true
        }
        fn history_key(&mut self, _: &crate::client_view::KeyEvent) -> bool {
            self.owner == 3
        }
    }
    let keys = WindowsKeySettings {
        overrides: vec![binding("help", "f1", false)],
        ..WindowsKeySettings::default()
    };
    for owner in 1..=3 {
        let mut view = ClientView::new(100);
        view.configure_keys(&keys);
        let mut port = Port {
            owner,
            actions: Vec::new(),
            forwarded: Vec::new(),
        };
        run(
            &mut view,
            &mut InputDispatcher::default(),
            &mut port,
            b"\x1bOP",
        );
        assert!(port.actions.is_empty(), "owner {owner}");
        assert!(port.forwarded.is_empty());
    }
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    let mut port = Port {
        owner: 0,
        actions: Vec::new(),
        forwarded: Vec::new(),
    };
    let mut dispatcher = InputDispatcher::default();
    let paste = b"\x1b[200~\x1bOP\x02?\x1b[201~";
    run(&mut view, &mut dispatcher, &mut port, paste);
    run(&mut view, &mut dispatcher, &mut port, b"\x1b[57364;1:3u");
    assert_eq!(port.forwarded, paste);
    assert!(port.actions.is_empty());
}

#[test]
fn configured_discovery_and_snapshot_follow_the_dispatch_registry() {
    let mut keys = WindowsKeySettings {
        prefix: chord("a", true),
        overrides: vec![
            binding("help", "f1", false),
            WindowsKeyOverride {
                action: "new_tab",
                triggers: vec![],
            },
        ],
    };
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    keys.prefix = chord("f9", false);
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let bound = locale.shell().bind(&view);
        let text = bound.text();
        assert!(text.hint_short.contains("F1"));
        assert!(!text.hint_short.contains("Ctrl+B"));
        assert!(text.prefix_active.ends_with("Ctrl+A"));
        let rows = text.configured_shortcut_rows(&view);
        assert!(rows.iter().any(|(key, _)| key.trim() == "F1"));
        assert!(
            rows.iter()
                .any(|(key, _)| key.trim() == text.binding_unbound)
        );
        assert!(
            !rows
                .iter()
                .any(|(key, _)| key.contains("Ctrl+B") || key.contains("F9"))
        );
    }
}

#[test]
fn indexed_default_conflict_displaces_only_the_occupied_digit() {
    let keys = WindowsKeySettings {
        overrides: vec![binding("help", "3", true)],
        ..WindowsKeySettings::default()
    };
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    run(
        &mut view,
        &mut InputDispatcher::default(),
        &mut port,
        b"\x022\x023\x024",
    );
    assert_eq!(
        *calls.borrow(),
        [
            Call::Action(ClientAction::SelectTab(2)),
            Call::Action(ClientAction::OpenShortcutHelp),
            Call::Action(ClientAction::SelectTab(4))
        ]
    );
}

#[test]
fn configured_indexed_direct_keys_keep_modifiers_and_compact_discovery() {
    let keys = WindowsKeySettings {
        overrides: vec![WindowsKeyOverride {
            action: "switch_tab",
            triggers: (1..=9)
                .map(|digit| WindowsKeyTrigger {
                    prefix: false,
                    chord: WindowsKeyChord {
                        alt: true,
                        ..chord(&digit.to_string(), false)
                    },
                })
                .collect(),
        }],
        ..WindowsKeySettings::default()
    };
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    run(
        &mut view,
        &mut InputDispatcher::default(),
        &mut port,
        b"\x1b[53;3u\x1b[53;4u",
    );
    assert_eq!(
        *calls.borrow(),
        [
            Call::Action(ClientAction::SelectTab(5)),
            Call::Forward(b"\x1b[53;4u".to_vec())
        ]
    );
    assert!(
        crate::localization::Locale::EnUs
            .shell()
            .configured_shortcut_rows(&view)
            .iter()
            .any(|(key, _)| key.trim() == "Alt+1..9")
    );
    let keys = WindowsKeySettings {
        overrides: vec![binding("new_tab", "n", true)],
        ..WindowsKeySettings::default()
    };
    view.configure_keys(&keys);
    let row = view
        .bindings()
        .group_label(crate::client_view::PrefixDesc::SelectTabWrap, "Unbound")
        .unwrap();
    assert_eq!(row, "Ctrl+B Unbound / p");
}
