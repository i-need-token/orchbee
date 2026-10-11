//! Sidebar navigation admits explicit shell prefixes without releasing captured data.
use super::*;

#[test]
fn sidebar_workspace_activation_and_pending_escape_capture_tail_without_opening_picker() {
    for pointer in [false, true] {
        let mut reference = sidebar_view();
        reference.machines.enter_sidebar_navigation();
        let map = rendered_sidebar_map(&reference);
        let cell = (0..40)
            .find_map(|y| {
                (0..30).find_map(|x| {
                    matches!(
                        map.hit_target_at(x, y),
                        Some(HitTarget::MachineWorkspaceRow { .. })
                    )
                    .then_some((x, y))
                })
            })
            .unwrap();
        let input = if pointer {
            format!("\x1b[<0;{};{}M", cell.0 + 1, cell.1 + 1).into_bytes()
        } else {
            b"\r".to_vec()
        };
        // Split inside each encoded report; its completed packet owns the tail.
        for split in 0..input.len() {
            let mut view = sidebar_view();
            let mut port = Port::new(MenuAction::Help);
            let mut dispatcher = InputDispatcher::default();
            dispatcher.set_pointer_capture(&mut view, &mut port, true);
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
            dispatcher.publish_hit_map(rendered_sidebar_map(&view));
            let mut packet = input.clone();
            packet.extend_from_slice(b"tail\x1b[200~literal\x1b[201~");
            dispatch_all(&mut dispatcher, &mut view, &mut port, &packet[..split]);
            dispatch_all(&mut dispatcher, &mut view, &mut port, &packet[split..]);
            assert!(forwarded_bytes(&port.calls).is_empty());
            assert!(!view.machines.open);
            assert!(!view.machines.sidebar_navigation_active());
            assert_eq!(
                view.machines.request.as_ref().unwrap().context,
                Some(sidebar_context().into())
            );
            assert!(view.machines.direct_selection_pending());
            // Leaving sidebar keyboard ownership fences input until the next frame.
            dispatcher.publish_hit_map(rendered_sidebar_map(&view));
            for byte in b"\x1b[27" {
                dispatch_all(&mut dispatcher, &mut view, &mut port, &[*byte]);
            }
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"utail\x03");
            assert!(!view.machines.direct_selection_pending());
            assert!(view.machines.request.is_none());
            assert!(forwarded_bytes(&port.calls).is_empty());
            assert!(!view.machines.open);
        }
    }
}

#[test]
fn sidebar_fixed_creation_and_search_entries_share_keyboard_and_pointer_capture() {
    for (input, create) in [
        (b"N".as_slice(), true),
        (b"\x1b[<0;28;2M".as_slice(), true),
        (b"/".as_slice(), false),
        (b"\x1b[<0;28;3M".as_slice(), false),
    ] {
        for split in 0..=input.len() {
            let (mut view, _) = sidebar_view_with_agent();
            let mut port = Port::new(MenuAction::Help);
            let mut dispatcher = InputDispatcher::default();
            dispatcher.set_pointer_capture(&mut view, &mut port, true);
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
            dispatcher.publish_hit_map(rendered_sidebar_map(&view));
            let mut packet = input.to_vec();
            packet.extend_from_slice(b"tail\x1b[200~literal\x1b[201~");
            dispatch_all(&mut dispatcher, &mut view, &mut port, &packet[..split]);
            dispatch_all(&mut dispatcher, &mut view, &mut port, &packet[split..]);
            assert!(forwarded_bytes(&port.calls).is_empty());
            assert!(!view.machines.sidebar_navigation_active());
            if create {
                assert_eq!(
                    *port.calls.borrow(),
                    [Call::Action(ClientAction::RequestQuickWorkspace)]
                );
                assert!(!view.machines.open);
            } else {
                assert!(view.machines.open);
                assert!(port.calls.borrow().is_empty());
                dispatcher.publish_hit_map(HitMapBuilder::new().finish());
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r");
                assert!(
                    view.machines.open,
                    "Enter finishes query editing without activating a row"
                );
                assert!(view.machines.request.is_none());
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
                assert!(!view.machines.open);
                assert!(forwarded_bytes(&port.calls).is_empty());
            }
        }
    }
}

#[test]
fn sidebar_global_prefix_dispatches_default_creation_and_preserves_return_marker() {
    for width in [87, 120] {
        for (key, action) in [
            (b'N', ClientAction::RequestQuickWorkspace),
            (b'a', ClientAction::RequestWorkspaceCreate),
        ] {
            for split in 0..=2 {
                let mut view = sidebar_view();
                view.resize(width);
                let mut port = Port::new(MenuAction::Help);
                let mut dispatcher = InputDispatcher::default();
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
                assert!(view.machines.sidebar_navigation_active());
                dispatcher.publish_hit_map(sidebar_menu_map());
                let mut input = vec![2, key];
                input.extend_from_slice(b"tail\x1b[200~literal\x1b[201~");
                dispatch_all(&mut dispatcher, &mut view, &mut port, &input[..split]);
                dispatch_all(&mut dispatcher, &mut view, &mut port, &input[split..]);
                assert_eq!(
                    *port.calls.borrow(),
                    [Call::Action(action.clone())],
                    "width={width}, key={key}, split={split}"
                );
                assert!(!view.prefix_active());
                assert!(!view.machines.sidebar_navigation_active());
                assert_eq!(
                    view.overlay(),
                    (width < 96).then_some(Overlay::BusinessSidebar),
                    "the overlay waits for the exact native launch receipt"
                );
                view.settle_project_quick_entry_intent();
                assert!(view.project_quick_entry_pending());
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"before redraw");
                assert!(forwarded_bytes(&port.calls).is_empty());
                assert_eq!(view.complete_project_quick_entry(), width < 96);
                assert!(!view.machines.sidebar_navigation_active());
                assert_eq!(view.overlay(), None);
                assert!(
                    !view.complete_project_quick_entry(),
                    "the launch return is consumed once"
                );
                dispatcher.publish_hit_map(HitMapBuilder::new().finish());
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"terminal");
                assert_eq!(forwarded_bytes(&port.calls), b"terminal");
            }
        }
    }
}

#[test]
fn sidebar_prefix_respects_configured_chords_and_captures_paste_and_key_releases() {
    let settings = orchbee_local_site_runtime::WindowsKeySettings::from_profile(
        "[keys]\nprefix='ctrl+a'\nnew_workspace='prefix+w'\nnew_tab='alt+n'",
    )
    .unwrap();
    let prefix = b"\x1b[97;5u";
    for split in 0..=prefix.len() {
        let mut view = sidebar_view();
        view.resize(87);
        view.configure_keys(&settings);
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x01B");
        dispatcher.publish_hit_map(sidebar_menu_map());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02n\x1bnaqw");
        assert!(!view.prefix_active());
        assert!(
            port.calls.borrow().is_empty(),
            "ordinary sidebar input is captured"
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, &prefix[..split]);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &prefix[split..]);
        assert!(view.prefix_active());
        for bytes in [
            b"\x1b[97;5:3u".as_slice(), // Release cannot consume an armed prefix.
            b"\x1b[200~\x01w\x02N",
            b"\x1b[201~",
            b"\x1b[I\x1b[O\x1b[8;30;87t", // Physical metadata is not a key.
        ] {
            dispatch_all(&mut dispatcher, &mut view, &mut port, bytes);
            assert!(view.prefix_active());
            assert!(view.machines.sidebar_navigation_active());
            assert!(port.calls.borrow().is_empty());
        }
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"wtail");
        assert_eq!(
            *port.calls.borrow(),
            [Call::Action(ClientAction::RequestWorkspaceCreate)]
        );
        assert!(!view.machines.sidebar_navigation_active());
        view.settle_project_quick_entry_intent();
        assert!(view.project_quick_entry_pending());
        assert!(forwarded_bytes(&port.calls).is_empty());
    }
}

#[test]
fn sidebar_prefix_decisions_do_not_also_navigate_and_escape_returns_to_sidebar() {
    let mut view = sidebar_view();
    view.resize(87);
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
    dispatcher.publish_hit_map(sidebar_menu_map());
    let focus = view.machines.sidebar_focused_target().cloned();
    for (input, action) in [
        (b"\x02v".as_slice(), ClientAction::UnknownPrefix),
        (b"\x02\x1b[27u", ClientAction::DismissPrefix),
        (b"\x02c", ClientAction::CreateTab),
    ] {
        dispatch_all(&mut dispatcher, &mut view, &mut port, input);
        assert_eq!(*port.calls.borrow(), [Call::Action(action)]);
        port.calls.borrow_mut().clear();
        assert!(!view.prefix_active());
        assert!(view.machines.sidebar_navigation_active());
        assert_eq!(view.machines.sidebar_focused_target(), focus.as_ref());
        assert!(view.machines.request.is_none());
    }
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x02captured");
    assert_eq!(
        *port.calls.borrow(),
        [
            Call::Action(ClientAction::ForwardLiteralPrefix),
            Call::Forward(vec![2]),
        ],
        "only the explicit literal-prefix action may forward while navigating"
    );
    port.calls.borrow_mut().clear();
    assert!(view.machines.sidebar_navigation_active());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27utail");
    assert!(!view.machines.sidebar_navigation_active());
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"terminal");
    assert_eq!(forwarded_bytes(&port.calls), b"terminal");
}

#[test]
fn sidebar_menu_keeps_exclusive_capture_over_global_prefix_and_closing_packet() {
    let mut view = sidebar_view();
    view.resize(87);
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
    dispatcher.publish_hit_map(sidebar_menu_map());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1bOQ");
    assert!(port.menu.active());
    paint_menu(&mut dispatcher, &mut port);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02N");
    assert!(port.menu.active());
    assert!(!view.prefix_active());
    assert!(port.calls.borrow().is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u\x02N");
    assert!(!port.menu.active());
    assert!(view.machines.sidebar_navigation_active());
    assert!(port.calls.borrow().is_empty());
    dispatcher.publish_hit_map(sidebar_menu_map());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02N");
    assert_eq!(
        *port.calls.borrow(),
        [Call::Action(ClientAction::RequestQuickWorkspace)]
    );
}

#[test]
fn sidebar_creation_return_yields_to_newer_keyboard_navigation() {
    for key in *b"Na" {
        let mut view = sidebar_view();
        view.resize(87);
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
        dispatcher.publish_hit_map(sidebar_menu_map());
        dispatch_all(&mut dispatcher, &mut view, &mut port, &[2, key]);
        view.settle_project_quick_entry_intent();
        assert!(view.project_quick_entry_pending());
        assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
        dispatcher.publish_hit_map(sidebar_menu_map());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
        assert!(view.machines.sidebar_navigation_active());
        view.settle_project_quick_entry_intent();
        assert!(!view.complete_project_quick_entry());
        assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
        assert!(forwarded_bytes(&port.calls).is_empty());
    }
}
