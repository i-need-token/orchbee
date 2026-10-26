use super::*;
use crate::tui_pointer::{TabDragSource, TabDragTab};
use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchTabId};

#[test]
fn drag_release_and_cancellation_fence_keys_paste_and_fragmented_packet_tails() {
    for cancel in 0..7 {
        let mut port = Port::new(MenuAction::NewTab);
        let context = sidebar_context();
        let drawn = crate::action_menu::DrawnTabCreate {
            source: crate::action_menu::DrawnMenuSource {
                session: context.session_id,
                revision: WorkbenchRevision::new(1),
            },
            location: context.location,
        };
        port.drag = Some(TabDragSource {
            drawn: drawn.clone(),
            tabs: (1..=2)
                .map(|i| TabDragTab {
                    id: WorkbenchTabId::parse(&format!("{i:032x}")).unwrap(),
                    pinned: false,
                    group: None,
                    selected: i == 1,
                    collapsed: false,
                })
                .collect(),
        });
        port.probe.tabs = port
            .drag
            .as_ref()
            .unwrap()
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| crate::terminal_input::PointerTab {
                id: tab.id.clone(),
                position: index + 1,
            })
            .collect();
        let mut hits = HitMapBuilder::new();
        hits.record_action_menu_source(Some(drawn.source));
        for (index, tab) in port.probe.tabs.iter().enumerate() {
            hits.record(
                HitLayer::Foreground,
                Rect::new(u16::try_from(index).unwrap() * 10, 0, 9, 1),
                HitTarget::Tab(tab.id.clone()),
            );
        }
        let map = hits.finish();
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher.publish_hit_map(map.clone());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;1M");
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<32;18;1M");
        assert!(port.arrangements.is_empty());
        match cancel {
            1 => dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u"),
            2 => port.drag.as_mut().unwrap().drawn.source.revision = WorkbenchRevision::new(2),
            3 => dispatcher.invalidate_hit_map(&mut port),
            4..=6 => {
                let cancel_event: &[u8] = match cancel {
                    4 => b"\x1b[O",
                    5 => b"\x1b[<48;18;1M",
                    _ => b"\x1b[<2;18;1M",
                };
                let packet = [cancel_event, b"tail\x1b[200~paste\x1b[201~\x1b["].concat();
                dispatch_all(&mut dispatcher, &mut view, &mut port, &packet);
                assert!(
                    forwarded_bytes(&port.calls).is_empty(),
                    "cancellation packet {cancel} cannot forward its tail"
                );
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
                assert!(forwarded_bytes(&port.calls).is_empty());
            }
            _ => {}
        }
        if cancel == 1 {
            dispatcher.publish_hit_map(map.clone());
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"resume");
            assert_eq!(
                forwarded_bytes(&port.calls),
                b"resume",
                "Escape restores keyboard input without waiting for a lost mouse release"
            );
            port.calls.borrow_mut().clear();
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;18;1mtail\x1b[200~paste\x1b[201~\x1b[",
        );
        assert_eq!(port.arrangements.len(), usize::from(cancel == 0));
        let forwarded = forwarded_bytes(&port.calls);
        assert!(
            forwarded.is_empty(),
            "cancel={cancel} leaked the late-release packet tail: {forwarded:?}",
        );
        dispatcher.publish_hit_map(map);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
        assert!(
            forwarded_bytes(&port.calls).is_empty(),
            "fragment from the release packet remains captured"
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
        assert_eq!(forwarded_bytes(&port.calls), b"fresh");
    }
}

#[test]
fn unfocused_orphan_release_does_not_claim_its_packet_tail() {
    let mut port = Port::new(MenuAction::NewTab);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O");
    port.calls.borrow_mut().clear();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;18;1mtail");

    assert_eq!(forwarded_bytes(&port.calls), b"tail");
}

#[test]
fn asynchronously_opened_menu_retires_tab_drag_but_owns_its_late_release_once() {
    let mut port = Port::new(MenuAction::NewTab);
    let context = sidebar_context();
    let drawn = crate::action_menu::DrawnTabCreate {
        source: crate::action_menu::DrawnMenuSource {
            session: context.session_id,
            revision: WorkbenchRevision::new(1),
        },
        location: context.location,
    };
    port.drag = Some(TabDragSource {
        drawn: drawn.clone(),
        tabs: (1..=2)
            .map(|i| TabDragTab {
                id: WorkbenchTabId::parse(&format!("{i:032x}")).unwrap(),
                pinned: false,
                group: None,
                selected: i == 1,
                collapsed: false,
            })
            .collect(),
    });
    port.probe.tabs = port
        .drag
        .as_ref()
        .unwrap()
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| crate::terminal_input::PointerTab {
            id: tab.id.clone(),
            position: index + 1,
        })
        .collect();
    let mut hits = HitMapBuilder::new();
    hits.record_action_menu_source(Some(drawn.source));
    for (index, tab) in port.probe.tabs.iter().enumerate() {
        hits.record(
            HitLayer::Foreground,
            Rect::new(u16::try_from(index).unwrap() * 10, 0, 9, 1),
            HitTarget::Tab(tab.id.clone()),
        );
    }
    let map = hits.finish();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher.publish_hit_map(map.clone());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;1M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<32;18;1M");
    assert!(dispatcher.tab_drag_preview().is_some());
    assert!(port.arrangements.is_empty());

    assert!(port.open_action_menu(MenuTarget::Global, None, false));
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    assert!(dispatcher.tab_drag_preview().is_none());
    assert!(port.arrangements.is_empty());

    port.menu.cancel();
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    dispatcher.publish_hit_map(map.clone());
    port.calls.borrow_mut().clear();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;18;1mtail\x1b[200~paste\x1b[201~",
    );
    assert!(port.arrangements.is_empty());
    assert!(forwarded_bytes(&port.calls).is_empty());

    dispatcher.publish_hit_map(map);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;18;1mnormal",
    );
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
}
