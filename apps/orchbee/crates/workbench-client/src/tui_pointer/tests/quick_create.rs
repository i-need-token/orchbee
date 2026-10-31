//! Creation gestures preserve the painted source and never forward terminal input.
use super::*;
use crate::action_menu::{DrawnMenuSource, DrawnTabCreate};
use crate::tui_hit_map::HitRegion;
use crate::workbench_client::ClientContextLocation;
use orchbee_workbench_protocol::{SessionId, WorkbenchRevision, WorkbenchTerminalContextId};

fn target() -> DrawnTabCreate {
    DrawnTabCreate {
        source: DrawnMenuSource {
            session: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
            revision: WorkbenchRevision::new(1),
        },
        location: {
            let fixture_context_id =
                WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
            ClientContextLocation::Workspace {
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                    fixture_context_id.as_str(),
                )
                .unwrap(),
                terminal_context_id: fixture_context_id,
            }
        },
    }
}
fn map(target: DrawnTabCreate, area: Rect) -> FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.record_interaction(
        HitLayer::Foreground,
        area,
        area,
        InteractionTarget::TabCreate(target),
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
    );
    hits.finish()
}

fn requested(outcome: &PointerOutcome, expected: &DrawnTabCreate) -> bool {
    matches!(
        outcome,
        PointerOutcome::Interaction {
            target: InteractionTarget::TabCreate(actual),
            contract: PrimaryContract::ActivateOnRelease,
        } if actual == expected
    )
}

#[test]
fn tab_create_requires_a_complete_plain_click_inside_the_same_drawn_control() {
    let area = Rect::new(10, 0, 3, 1);
    for x in [9, 10, 11, 12, 13] {
        for mods in [
            Modifiers::NONE,
            Modifiers::CTRL,
            Modifiers::NONE.with_shift(),
        ] {
            let mut pointer = PointerState::default();
            let mut view = ClientView::new(120);
            let mut port = RecordingPort::default();
            pointer.set_capture(true);
            pointer.publish(map(target(), area));
            let mut press = event(MouseAction::Press, MouseButton::Left, x, 0);
            press.mods = mods;
            assert_eq!(
                pointer.event(&mut view, &mut port, Instant::now(), press),
                if (10..13).contains(&x) && mods == Modifiers::NONE {
                    PointerOutcome::Handled
                } else {
                    PointerOutcome::Ignored
                },
            );
            let release = event(MouseAction::Release, MouseButton::Left, x, 0);
            let outcome = pointer.event(&mut view, &mut port, Instant::now(), release);
            assert_eq!(
                requested(&outcome, &target()),
                (10..13).contains(&x) && mods == Modifiers::NONE
            );
            assert!(!requested(
                &pointer.event(&mut view, &mut port, Instant::now(), release),
                &target(),
            ));
            assert!(port.actions.is_empty() && port.forwarded.is_empty());
        }
    }
}

#[test]
fn tab_create_cancels_on_changed_identity_geometry_capture_or_outside_release() {
    for change in 0..7 {
        let area = Rect::new(10, 0, 3, 1);
        let mut pointer = PointerState::default();
        let mut view = ClientView::new(120);
        let mut port = RecordingPort::default();
        pointer.set_capture(true);
        pointer.publish(map(target(), area));
        pointer.event(
            &mut view,
            &mut port,
            Instant::now(),
            event(MouseAction::Press, MouseButton::Left, 11, 0),
        );
        let mut release = event(MouseAction::Release, MouseButton::Left, 11, 0);
        match change {
            0 => {
                let mut changed = target();
                changed.source.revision = WorkbenchRevision::new(2);
                pointer.publish(map(changed, area));
            }
            1 => pointer.publish(map(target(), Rect::new(11, 0, 3, 1))),
            2 => pointer.invalidate_map(),
            3 => {
                pointer.set_capture(false);
                pointer.set_capture(true);
            }
            4 => port.probe.input_captured = true,
            5 => release.column = area.right(),
            6 => {
                let mut hits = HitMapBuilder::new();
                hits.mark_overlay_active();
                hits.record(
                    HitLayer::OverlaySurface,
                    Rect::new(0, 0, 120, 30),
                    HitTarget::OverlaySurface,
                );
                pointer.publish(hits.finish());
            }
            _ => unreachable!(),
        }
        assert_eq!(
            pointer.event(&mut view, &mut port, Instant::now(), release),
            PointerOutcome::InteractionEnded,
            "change {change}",
        );
        assert!(
            matches!(
                pointer.event(&mut view, &mut port, Instant::now(), release),
                PointerOutcome::Ignored | PointerOutcome::Handled
            ),
            "change {change} must settle cleanup once without another end or semantic effect",
        );
        assert!(port.actions.is_empty() && port.forwarded.is_empty());
    }
}

#[test]
fn compositor_cover_cancels_generic_tab_create_and_owns_one_late_release() {
    for button in [MouseButton::Left, MouseButton::None] {
        let area = Rect::new(10, 0, 3, 1);
        let mut pointer = PointerState::default();
        let mut view = ClientView::new(120);
        let mut port = RecordingPort::default();
        pointer.set_capture(true);
        pointer.publish(map(target(), area));
        assert_eq!(
            pointer.event(
                &mut view,
                &mut port,
                Instant::now(),
                event(MouseAction::Press, MouseButton::Left, 11, 0),
            ),
            PointerOutcome::Handled,
        );
        assert!(matches!(pointer.gesture, Gesture::Idle));
        let mut covered = HitMapBuilder::new();
        covered.mark_overlay_active();
        covered.record(
            HitLayer::OverlaySurface,
            Rect::new(0, 0, 120, 30),
            HitTarget::OverlaySurface,
        );
        pointer.publish(covered.finish());
        assert!(!pointer.retire_covered_chrome_gesture());
        assert!(matches!(pointer.gesture, Gesture::Idle));
        let release = event(MouseAction::Release, button, 11, 0);
        assert_eq!(
            pointer.cancelled_ui_event(release),
            PointerOutcome::InteractionEnded,
        );
        assert_eq!(pointer.cancelled_ui_event(release), PointerOutcome::Ignored,);
        assert!(port.actions.is_empty() && port.forwarded.is_empty());
    }
}

#[test]
fn project_create_entry_uses_the_existing_exact_machine_and_session_menu_admission() {
    let (mut view, mut drawn) = workspace_fixture(2);
    let project = view
        .machines
        .sidebar_navigation_rows()
        .into_iter()
        .find_map(|row| {
            let target = row.target();
            matches!(target, crate::machine_picker::SidebarTarget::Project { .. }).then_some(target)
        })
        .unwrap();
    let source = DrawnMenuSource {
        session: project.session_id().clone(),
        revision: WorkbenchRevision::new(1),
    };
    drawn.menu_source = Some(source.clone());
    // A dedicated painted Project control, with no overlapping header hit.
    drawn.regions.clear();
    drawn.regions.push(HitRegion {
        layer: HitLayer::Chrome,
        rect: Rect::new(20, 1, 3, 1),
        target: HitTarget::CreateMenu(Some(project.clone())),
        interaction: None,
    });
    for stale in [false, true] {
        let mut pointer = PointerState::default();
        let mut port = RecordingPort::default();
        pointer.set_capture(true);
        let mut map = drawn.clone();
        if stale {
            map.menu_source.as_mut().unwrap().session =
                SessionId::parse(&format!("{:032x}", 999)).unwrap();
        }
        pointer.publish(map);
        let outcome = pointer.event(
            &mut view,
            &mut port,
            Instant::now(),
            event(MouseAction::Press, MouseButton::Left, 21, 1),
        );
        assert_eq!(
            matches!(outcome, PointerOutcome::MenuRequested { target: crate::action_menu::MenuTarget::Drawn { target, .. }, .. } if matches!(*target, crate::action_menu::MenuTarget::Create(Some(_)))),
            !stale
        );
        assert!(port.actions.is_empty() && port.forwarded.is_empty());
    }
}
