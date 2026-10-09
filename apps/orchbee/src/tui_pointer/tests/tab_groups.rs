use super::*;
use orchbee_workbench_protocol::WorkbenchTabGroupId;

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

#[test]
fn group_header_plain_press_toggles_once_without_selection_or_terminal_input() {
    let group = WorkbenchTabGroupId::parse(&format!("{:032x}", 99)).unwrap();
    for (action, button, modifiers, admitted) in [
        (MouseAction::Press, MouseButton::Left, Modifiers::NONE, true),
        (
            MouseAction::Release,
            MouseButton::Left,
            Modifiers::NONE,
            false,
        ),
        (
            MouseAction::Motion,
            MouseButton::Left,
            Modifiers::NONE,
            false,
        ),
        (
            MouseAction::Press,
            MouseButton::Middle,
            Modifiers::NONE,
            false,
        ),
        (
            MouseAction::Press,
            MouseButton::Left,
            Modifiers::CTRL,
            false,
        ),
    ] {
        let mut hits = HitMapBuilder::new();
        hits.record(
            HitLayer::Foreground,
            Rect::new(3, 1, 10, 1),
            HitTarget::TabGroup {
                tab: tab_id(1),
                group: group.clone(),
            },
        );
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        pointer.publish(hits.finish());
        let mut view = ClientView::new(80);
        let mut port = RecordingPort {
            probe: active_probe(0, 0, 2, 1),
            ..RecordingPort::default()
        };
        let old = port.probe.desired.clone();
        let mut input = event(action, button, 4, 1);
        input.mods = modifiers;
        pointer.event(&mut view, &mut port, now(), input);
        assert_eq!(
            port.actions,
            if admitted {
                vec![ClientAction::ToggleTabGroup(group.clone())]
            } else {
                vec![]
            }
        );
        assert_eq!(port.probe.desired, old);
        assert!(port.forwarded.is_empty());
    }
}
