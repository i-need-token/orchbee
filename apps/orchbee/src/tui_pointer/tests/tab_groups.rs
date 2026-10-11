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
        site: orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", 1)).unwrap(),
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
