//! Creation gestures preserve the painted source and never forward terminal input.
use super::*;
use crate::action_menu::{DrawnMenuSource, DrawnTabCreate};
use crate::tui_hit_map::HitRegion;
use crate::workbench_projection::ClientContextLocation;
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

/// The two blank surfaces must share the create owner without turning a lone click,
/// a drag, a slow pair or clicks across changed drawing authority into creation.
#[test]
fn blank_tab_creation_requires_two_current_unmodified_clicks() {
    for change in 0..12 {
        let area = Rect::new(10, 0, 50, 10);
        let blank_map = |source| {
            let mut hits = HitMapBuilder::new();
            hits.record_interaction(
                HitLayer::Foreground,
                area,
                area,
                InteractionTarget::TabCreate(source),
                InteractionSpec::enabled(PrimaryContract::ActivateOnDoubleClick),
            );
            hits.finish()
        };
        let mut pointer = PointerState::default();
        let mut view = ClientView::new(120);
        let mut port = RecordingPort::default();
        pointer.set_capture(true);
        pointer.publish(blank_map(target()));
        let now = Instant::now();
        let press = event(MouseAction::Press, MouseButton::Left, 20, 2);
        let release = event(MouseAction::Release, MouseButton::Left, 20, 2);
        pointer.event(&mut view, &mut port, now, press);
        assert!(!requested(
            &pointer.event(&mut view, &mut port, now, release),
            &target()
        ));
        let mut second_press = press;
        let mut second_release = release;
        let mut later = now + Duration::from_millis(100);
        match change {
            0 => {}
            1 => later = now + Duration::from_millis(351),
            2 => {
                second_press.column += 1;
                second_release.column += 1;
            }
            3 => {
                second_press.mods = Modifiers::CTRL;
                second_release.mods = Modifiers::CTRL;
            }
            4 => {
                pointer.invalidate_map();
                pointer.publish(blank_map(target()));
            }
            5 => {
                pointer.set_capture(false);
                pointer.set_capture(true);
            }
            6 => {
                let mut changed = target();
                changed.source.revision = WorkbenchRevision::new(2);
                pointer.publish(blank_map(changed));
            }
            7 => {
                pointer.event(
                    &mut view,
                    &mut port,
                    now,
                    event(MouseAction::Press, MouseButton::Right, 20, 2),
                );
            }
            8 => port.probe.input_captured = true,
            9 => pointer.publish(blank_map(target())), // an unchanged repaint preserves the pair
            10 => second_release.column += 1,
            11 => {
                let mut hits = HitMapBuilder::new();
                hits.mark_overlay_active();
                pointer.publish(hits.finish());
            }
            _ => unreachable!(),
        }
        pointer.event(&mut view, &mut port, later, second_press);
        let outcome = pointer.event(&mut view, &mut port, later, second_release);
        assert_eq!(
            requested(&outcome, &target()),
            matches!(change, 0 | 9),
            "case {change}"
        );
        assert!(!requested(
            &pointer.event(&mut view, &mut port, later, release),
            &target()
        ));
        assert!(port.actions.is_empty() && port.forwarded.is_empty());
    }
}
