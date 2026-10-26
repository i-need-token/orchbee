use super::*;
use crate::client_view::KeyEvent;
use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};

fn chord(key: &str, ctrl: bool) -> KeyChord {
    KeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    }
}
fn binding(action: &'static str, key: &str, prefix: bool) -> KeyOverride {
    KeyOverride {
        action,
        triggers: vec![KeyTrigger {
            prefix,
            chord: chord(key, false),
        }],
    }
}

fn shortcut_entries(
    locale: crate::localization::Locale,
    view: &ClientView,
) -> Vec<crate::localization::ShortcutHelpEntry> {
    locale.shortcut_help_entries(view, true, crate::client_view::KeyLabelGlyphs::Unicode)
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
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(())
            )
            .unwrap(),
        DispatchOutcome::Continue
    );
}

#[test]
fn configured_actions_displace_defaults_and_preserve_passthrough_order() {
    let keys = KeySettings {
        commands: Vec::new(),
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
fn configured_command_dispatch_and_discovery_share_the_installed_catalog() {
    let keys = KeySettings::from_profile(
        "[[keys.command]]\ncommand='echo hello'\nkey='prefix+c'\ndescription=\"Run \\u001b[31mhello\\u001b[0m\"\n[[keys.command]]\ncommand='tools.refresh'\ntype='plugin_action'\ndescription='Refresh tools'",
    ).unwrap();
    let id = keys.command_catalog()[0].id.clone();
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    run(&mut view, &mut dispatcher, &mut port, b"before\x02ctail");
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before".to_vec()),
            Call::Action(ClientAction::ConfiguredCommand(id.clone())),
            Call::Forward(b"tail".to_vec()),
        ]
    );
    let projected_id = view
        .bindings()
        .entries
        .iter()
        .find_map(|entry| {
            let crate::client_view::PrefixAction::Client(ClientAction::ConfiguredCommand(
                candidate,
            )) = &entry.member.action
            else {
                return None;
            };
            if candidate != &id {
                return None;
            }
            Some(
                crate::action_projection::binding_metadata(&entry.member.action)
                    .expect("configured action metadata")
                    .identity,
            )
        })
        .expect("configured action projection");
    assert_eq!(
        projected_id,
        crate::action_projection::ActionIdentity::ConfiguredCommand(id.as_str())
    );
    let entries = shortcut_entries(crate::localization::Locale::EnUs, &view);
    assert!(entries.iter().any(|entry| entry.title == "Run hello"));
    assert!(!entries.iter().any(|entry| entry.title == "Refresh tools"));
}

#[test]
fn configured_agent_cycle_captures_packet_tail_and_never_acts_inside_picker() {
    let keys =
        KeySettings::from_profile("[keys]\nprefix='ctrl+a'\nprevious_agent='alt+p'\nnext_agent=[]")
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
fn sidebar_pointer_navigation_defers_current_check_without_picker_and_captures_packet_tail() {
    use crate::machine_agents::{
        Observation as Agents,
        tests::{checkpoint, row},
    };
    use crate::machine_picker::{Choice, SidebarPart};
    use crate::tui_pointer::{
        HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget, PrimaryContract,
    };
    for current in [false, true] {
        let agent = row(23);
        let selected = crate::machine_picker::Selection {
            machine: Choice::Local,
            context: Some(crate::workbench_client::ClientStartupTarget::Agent(
                Box::new(agent.target.clone()),
            )),
        };
        let mut view = ClientView::new(120);
        view.machines.available = true;
        view.machines.observe_agent_sidebar(true);
        view.machines
            .observe_local(vec![crate::machine_discovery::Context {
                target: agent.target.context.clone(),
                project_id: None,
                project: None,
                workspace: Some("Workspace".into()),
            }]);
        view.machines.observe_agents(vec![(
            Choice::Local,
            Agents::Ready(checkpoint(vec![agent.clone()])),
        )]);
        if current {
            view.machines.observe_focus(
                Choice::Local,
                Some(agent.target.context.clone()),
                Some(crate::workbench_client::ClientMemberTarget {
                    tab: agent.target.tab.clone(),
                    panel: agent.target.panel.clone(),
                    presentation: agent.target.member.clone(),
                }),
            );
        }
        let owner = view
            .machines
            .sidebar_navigation_rows()
            .iter()
            .find_map(|row| match row {
                crate::machine_picker::SidebarRow::Context { .. } => Some(row.target()),
                _ => None,
            })
            .unwrap();
        if !view.machines.sidebar_context_agents_expanded(&owner) {
            assert!(view.machines.toggle_sidebar_context_agents(&owner));
        }
        let rows = view.machines.sidebar_navigation_rows();
        let agent_target = rows
            .iter()
            .find_map(|row| match row {
                crate::machine_picker::SidebarRow::Agent { .. } => Some(row.target()),
                _ => None,
            })
            .unwrap();
        let window = {
            view.machines.space_window_for_row_gaps(
                &rows,
                &vec![1; rows.len()],
                &vec![0; rows.len()],
                4,
            )
        };
        let source = view.machines.sidebar_source();
        let drawn = source
            .drawn(agent_target, SidebarPart::Primary, window)
            .unwrap();
        let mut hits = HitMapBuilder::new();
        let rect = ratatui::layout::Rect::new(2, 2, 26, 2);
        hits.record_interaction(
            HitLayer::Chrome,
            rect,
            rect,
            InteractionTarget::Sidebar(drawn),
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
        );
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher.publish_hit_map(hits.finish());
        run(
            &mut view,
            &mut dispatcher,
            &mut port,
            b"before\x1b[<0;3;3M\x1b[<0;3;3mtail\x03",
        );
        assert_eq!(*calls.borrow(), [Call::Forward(b"before".to_vec())]);
        assert_eq!(view.machines.request.take(), Some(selected));
        assert!(!view.machines.open);
        assert!(view.machines.direct_selection_pending());
    }
}

#[test]
fn configured_agent_ordinal_uses_binding_digit_captures_tail_and_lists_actual_help() {
    let keys =
        KeySettings::from_profile("[keys]\nprefix='ctrl+a'\nfocus_agent='alt+1..9'").unwrap();
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    view.machines.available = true;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    run(&mut view, &mut dispatcher, &mut port, b"before\x1b3tail");
    assert!(view.machines.wants_agents());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before".to_vec()),
            Call::Action(ClientAction::FocusAgent(2))
        ]
    );
    run(&mut view, &mut dispatcher, &mut port, b"\x1b3");
    assert_eq!(
        calls.borrow().len(),
        2,
        "picker owns input while ordinal waits"
    );
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let entries = shortcut_entries(locale, &view);
        let title = match locale {
            crate::localization::Locale::EnUs => "Select Agent by number",
            crate::localization::Locale::ZhCn => "按编号选择智能体",
        };
        let numbered_agents = entries
            .iter()
            .find(|entry| entry.title == title)
            .expect("numbered Agent binding remains discoverable");
        assert!(numbered_agents.key.contains("Alt+1"));
        assert!(numbered_agents.key.contains("Alt+9"));
    }
    view.machines
        .key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    let shifted = KeySettings::from_profile("[keys]\nfocus_agent='alt+shift+3'").unwrap();
    view.configure_keys(&shifted);
    let event = KeyEvent::press(
        KeyCode::Char('3'),
        Modifiers {
            alt: true,
            shift: true,
            ..Modifiers::NONE
        },
    );
    assert_eq!(
        view.decide(crate::client_view::PrefixInput::Key(event))
            .action(),
        Some(ClientAction::FocusAgent(2))
    );
    run(&mut view, &mut dispatcher, &mut port, b"\x1b#tail");
    assert_eq!(
        calls.borrow().last(),
        Some(&Call::Action(ClientAction::FocusAgent(2)))
    );
    view.machines
        .key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    let literal =
        KeySettings::from_profile("[keys]\nfocus_agent='alt+shift+3'\nhelp='alt+#'").unwrap();
    view.configure_keys(&literal);
    run(&mut view, &mut dispatcher, &mut port, b"\x1b#");
    assert_eq!(
        calls.borrow().last(),
        Some(&Call::Action(ClientAction::OpenShortcutHelp)),
        "exact literal binding wins before shifted-digit compatibility"
    );
    let disabled = KeySettings::from_profile("[keys]\nfocus_agent=[]").unwrap();
    view.configure_keys(&disabled);
    assert!(
        !shortcut_entries(crate::localization::Locale::EnUs, &view)
            .iter()
            .any(|entry| entry.title == "Select Agent by number")
    );
}

#[test]
fn function_prefix_is_atomic_and_literal_forwarding_never_falls_back_to_control_b() {
    let keys = KeySettings {
        commands: Vec::new(),
        prefix: chord("f2", false),
        ..KeySettings::default()
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
        fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
            crate::tui_composition::CaptureClaims::default()
                .with_workspace_close(self.owner == 1)
                .with_agent_board(self.owner == 2)
        }
        fn native_capture_context(
            &self,
            owner: crate::tui_composition::CaptureOwner,
        ) -> Option<crate::workbench_client::CaptureContext> {
            use crate::{
                tui_composition::CaptureOwner,
                tui_presentation::PresentationPhase,
                workbench_client::{
                    AgentBoardEpisodeToken, AgentBoardInputContext, CaptureContext,
                    CaptureReadiness, ConfirmationCaptureContext, WorkspaceCloseToken,
                },
            };

            match (self.owner, owner) {
                (1, CaptureOwner::WorkspaceClose) => {
                    Some(CaptureContext::WorkspaceClose(ConfirmationCaptureContext {
                        token: WorkspaceCloseToken::for_test(1),
                        phase: PresentationPhase::Admitted,
                        readiness: CaptureReadiness::Interactive,
                    }))
                }
                (2, CaptureOwner::AgentBoard) => Some(CaptureContext::AgentBoard(
                    AgentBoardInputContext::for_test(
                        AgentBoardEpisodeToken::for_test(1),
                        PresentationPhase::Admitted,
                        true,
                    ),
                )),
                _ => None,
            }
        }
        fn apply_client_action(&mut self, action: ClientAction) -> Option<ClientActionDisposition> {
            self.actions.push(action);
            Some(ClientActionDisposition::KeepNavigator)
        }
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.extend_from_slice(bytes);
            Ok(())
        }
        fn capture_workspace_confirmation_key_for(
            &mut self,
            _: crate::workbench_client::WorkspaceCloseToken,
            _: ClientKey,
        ) -> bool {
            self.owner == 1
        }
        fn capture_agent_board_key_for(
            &mut self,
            _: crate::workbench_client::AgentBoardEpisodeToken,
            _: crate::client_view::KeyEvent,
        ) -> bool {
            self.owner == 2
        }
        fn history_key(&mut self, _: &crate::client_view::KeyEvent) -> bool {
            self.owner == 3
        }
    }
    let keys = KeySettings {
        commands: Vec::new(),
        overrides: vec![binding("help", "f1", false)],
        ..KeySettings::default()
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
    let mut keys = KeySettings {
        commands: Vec::new(),
        prefix: chord("a", true),
        overrides: vec![
            binding("help", "f1", false),
            KeyOverride {
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
        assert!(!text.hint_short.contains("Ctrl+b"));
        assert!(text.prefix_active.ends_with("Ctrl+a"));
        let entries = shortcut_entries(locale, &view);
        assert!(entries.iter().any(|entry| entry.key == "F1"));
        assert!(!entries.iter().any(|entry| entry.title == "New Tab"));
        assert!(
            !entries
                .iter()
                .any(|entry| entry.key.contains("Ctrl+b") || entry.key.contains("F9"))
        );
    }
}

#[test]
fn indexed_default_conflict_displaces_only_the_occupied_digit() {
    let keys = KeySettings {
        commands: Vec::new(),
        overrides: vec![binding("help", "3", true)],
        ..KeySettings::default()
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
fn configured_indexed_direct_keys_keep_modifiers_and_all_discovery_chords() {
    let keys = KeySettings {
        commands: Vec::new(),
        overrides: vec![KeyOverride {
            action: "switch_tab",
            triggers: (1..=9)
                .map(|digit| KeyTrigger {
                    prefix: false,
                    chord: KeyChord {
                        alt: true,
                        ..chord(&digit.to_string(), false)
                    },
                })
                .collect(),
        }],
        ..KeySettings::default()
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
    let entries = shortcut_entries(crate::localization::Locale::EnUs, &view);
    let indexed_tabs = entries
        .iter()
        .find(|entry| entry.title == "Select Tab by number")
        .expect("direct Tab selection remains discoverable");
    for digit in 1..=9 {
        assert!(indexed_tabs.key.contains(format!("Alt+{digit}").as_str()));
    }
    let keys = KeySettings {
        commands: Vec::new(),
        overrides: vec![binding("new_tab", "n", true)],
        ..KeySettings::default()
    };
    view.configure_keys(&keys);
    let entries = shortcut_entries(crate::localization::Locale::EnUs, &view);
    assert!(!entries.iter().any(|entry| entry.title == "Select next Tab"));
    assert!(
        entries.iter().any(|entry| {
            entry.title == "Select previous Tab" && entry.key.contains("Ctrl+b p")
        })
    );
}

#[test]
fn resize_mode_consumes_keys_prefix_paste_and_complete_sequences_then_escape_restores_passthrough()
{
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    run(
        &mut view,
        &mut dispatcher,
        &mut port,
        b"before\x02P\x1b[C\x1b[1;2Dabc\x02q\x1b[200~\x1b[C\x1b\x02q\x1b[201~\x1b[99~",
    );
    assert!(view.resize_mode());
    assert!(!view.prefix_active());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before".to_vec()),
            Call::Action(ClientAction::EnterResizeMode),
            Call::Action(ClientAction::ResizePane {
                direction: crate::client_view::PanelDirection::Right,
                fine: false
            }),
            Call::Action(ClientAction::ResizePane {
                direction: crate::client_view::PanelDirection::Left,
                fine: true
            })
        ]
    );
    let start = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b",
            &mut port,
            start,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            start + Duration::from_millis(60),
            &mut |_| Ok::<_, io::Error>(()),
        )
        .unwrap();
    assert!(!view.resize_mode());
    run(&mut view, &mut dispatcher, &mut port, b"after\x1b[C");
    assert_eq!(forwarded_bytes(&calls), b"beforeafter\x1b[C".to_vec());
}

#[test]
fn configured_direct_resize_uses_registry_and_never_enters_mode_or_forwards_the_chord() {
    let keys =
        KeySettings::from_profile("[keys]\nresize_mode='prefix+R'\nresize_pane_left='ctrl+left'")
            .unwrap();
    let mut view = ClientView::new(120);
    view.configure_keys(&keys);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    run(
        &mut view,
        &mut dispatcher,
        &mut port,
        b"before\x1b[1;5Dafter",
    );
    assert!(!view.resize_mode());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before".to_vec()),
            Call::Action(ClientAction::ResizePane {
                direction: crate::client_view::PanelDirection::Left,
                fine: false
            }),
            Call::Forward(b"after".to_vec())
        ]
    );
    let entries = shortcut_entries(crate::localization::Locale::EnUs, &view);
    assert!(entries.iter().any(|entry| {
        entry.title == "Resize panel"
            && entry.key.contains("Ctrl+")
            && (entry.key.contains("Left") || entry.key.contains("←"))
    }));
    run(&mut view, &mut dispatcher, &mut port, b"\x02R");
    assert!(view.resize_mode());
}
