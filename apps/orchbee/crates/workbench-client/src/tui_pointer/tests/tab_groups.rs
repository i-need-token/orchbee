use super::*;
use orchbee_workbench_protocol::WorkbenchTabGroupId;

#[test]
fn tab_group_selection_pointer_modifiers_mark_exact_tab_and_never_forward_to_terminal() {
    for (modifiers, action, expected) in [
        (
            Modifiers::CTRL,
            MouseAction::Press,
            Some(crate::action_menu::MenuAction::SelectTab),
        ),
        (Modifiers::NONE.with_shift(), MouseAction::Press, None),
        (Modifiers::CTRL.with_shift(), MouseAction::Press, None),
        (Modifiers::NONE.with_alt(), MouseAction::Press, None),
        (Modifiers::CTRL, MouseAction::Release, None),
        (Modifiers::NONE.with_shift(), MouseAction::Motion, None),
    ] {
        let mut hits = HitMapBuilder::new();
        hits.record(
            HitLayer::Foreground,
            Rect::new(3, 1, 10, 1),
            HitTarget::Tab(tab_id(2)),
        );
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        pointer.publish(hits.finish());
        let mut port = RecordingPort {
            probe: active_probe(0, 0, 2, 1),
            ..RecordingPort::default()
        };
        let original = port.probe.desired.clone();
        let mut input = event(action, MouseButton::Left, 4, 1);
        input.mods = modifiers;
        pointer.event(&mut ClientView::new(80), &mut port, now(), input);
        assert_eq!(
            port.actions,
            expected
                .map(|action| ClientAction::TabMenu {
                    action,
                    tab_id: Some(tab_id(2))
                })
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(port.probe.desired, original);
        assert!(port.forwarded.is_empty());
    }
}

#[test]
fn group_header_right_press_opens_group_menu_with_exact_painted_source() {
    use crate::action_menu::{DrawnMenuSource, MenuTarget};
    let source = DrawnMenuSource {
        session: orchbee_workbench_protocol::SessionId::parse(&format!("{:032x}", 1)).unwrap(),
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(7),
    };
    let mut hits = HitMapBuilder::new();
    hits.record_action_menu_source(Some(source.clone()));
    hits.record(
        HitLayer::Foreground,
        Rect::new(3, 1, 10, 1),
        HitTarget::TabGroup {
            tab: tab_id(2),
            group: WorkbenchTabGroupId::parse(&format!("{:032x}", 99)).unwrap(),
        },
    );
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(hits.finish());
    let mut port = RecordingPort {
        probe: active_probe(0, 0, 2, 1),
        ..RecordingPort::default()
    };
    let result = pointer.event(
        &mut ClientView::new(80),
        &mut port,
        now(),
        event(MouseAction::Press, MouseButton::Right, 4, 1),
    );
    assert_eq!(
        result,
        PointerOutcome::MenuRequested {
            target: MenuTarget::Drawn {
                source,
                target: Box::new(MenuTarget::TabGroups {
                    tab_id: tab_id(2),
                    colors: false
                })
            },
            anchor: (4, 1),
            button: MouseButton::Right
        }
    );
    assert!(port.actions.is_empty() && port.forwarded.is_empty());
}

#[test]
fn panel_held_hit_map_keeps_tab_close_clickable_and_holds_panel_input() {
    use crate::action_menu::{DrawnMenuSource, DrawnTabCreate};
    use crate::terminal_input::{PointerProbe, TerminalInputUnavailable};
    use crate::tui_hit_map::{
        InteractionAvailability, InteractionSpec, InteractionTarget, PrimaryContract,
    };
    use crate::workbench_projection::ClientContextLocation;
    use orchbee_workbench_protocol::{SessionId, WorkbenchRevision, WorkbenchTerminalContextId};

    struct HeldPanelPort {
        probe: PointerProbe,
        forwarded: Vec<Vec<u8>>,
    }
    impl TerminalInputPort for HeldPanelPort {
        fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.push(input.to_vec());
            Ok(())
        }
        fn pointer_probe(&mut self) -> PointerProbe {
            self.probe.clone()
        }
        fn navigation_pending(&self) -> bool {
            true
        }
    }

    let location = ClientContextLocation::Workspace {
        workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!(
            "{:032x}",
            5
        ))
        .unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 6)).unwrap(),
    };
    let drawn = DrawnTabCreate {
        source: DrawnMenuSource {
            session: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
            revision: WorkbenchRevision::new(4),
        },
        location,
    };
    let mut hits = HitMapBuilder::new();
    hits.record_interaction(
        HitLayer::Foreground,
        Rect::new(16, 0, 3, 1),
        Rect::new(16, 0, 3, 1),
        InteractionTarget::TabCloseRequest {
            source: drawn.clone(),
            tab: tab_id(3),
        },
        InteractionSpec::new(
            PrimaryContract::ActivateOnRelease,
            InteractionAvailability::Enabled,
        ),
    );
    let pane_area = Rect::new(0, 1, 80, 20);
    hits.record(
        HitLayer::Foreground,
        pane_area,
        HitTarget::Pane {
            pane: pane_ref(1, 2, 10, 0),
            content: pane_area,
        },
    );

    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(hits.finish_panel_held());
    let mut port = HeldPanelPort {
        probe: active_probe(0, 0, 3, 1),
        forwarded: Vec::new(),
    };
    let mut view = ClientView::new(80);

    // Pressing the held panel interior holds input without reaching the hidden pane.
    let held_press = pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Press, MouseButton::Left, 10, 5),
    );
    assert_eq!(held_press, PointerOutcome::Handled);
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 10, 5),
    );
    assert_eq!(
        view.notice(),
        Some(crate::localization::Notice::NavigationInputHeld)
    );
    assert!(port.forwarded.is_empty());

    // Pressing and releasing [x] on the freshly painted strip still emits TabCloseRequest.
    let mut strip_view = ClientView::new(80);
    pointer.event(
        &mut strip_view,
        &mut port,
        now(),
        event(MouseAction::Press, MouseButton::Left, 17, 0),
    );
    let release = pointer.event(
        &mut strip_view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 17, 0),
    );
    assert_eq!(
        release,
        PointerOutcome::Interaction {
            target: InteractionTarget::TabCloseRequest {
                source: drawn,
                tab: tab_id(3),
            },
            contract: PrimaryContract::ActivateOnRelease,
        }
    );
    assert_eq!(strip_view.notice(), None);

    // Subsequent NavigationHold::Frame ticks preserve a published panel-held hit map so Strip
    // [x] targets remain clickable, whereas ordinary (non-panel-held) maps still invalidate.
    pointer.invalidate_map_preserving_panel_held(&mut port);
    pointer.event(
        &mut strip_view,
        &mut port,
        now(),
        event(MouseAction::Press, MouseButton::Left, 17, 0),
    );
    let retained_release = pointer.event(
        &mut strip_view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 17, 0),
    );
    assert!(matches!(
        retained_release,
        PointerOutcome::Interaction {
            target: InteractionTarget::TabCloseRequest { .. },
            ..
        }
    ));

    let ordinary = HitMapBuilder::new();
    pointer.publish(ordinary.finish());
    pointer.invalidate_map_preserving_panel_held(&mut port);
    assert_eq!(
        pointer.event(
            &mut strip_view,
            &mut port,
            now(),
            event(MouseAction::Press, MouseButton::Left, 17, 0),
        ),
        PointerOutcome::Ignored
    );
}
