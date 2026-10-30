//! Live draft ownership and one-release CAS traces with generated timelines and shrinking.
use super::*;
use crate::tui_layout::{DividerSource, DrawnDivider, divide_panes};
use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, SessionId, WorkbenchPanelSlotId, WorkbenchRevision,
    WorkbenchSplitId, WorkbenchTerminalContextId,
};

struct Port {
    target: DrawnDivider,
    busy: bool,
    failed: bool,
    captured: bool,
    submissions: Vec<(DrawnDivider, ClientSplitRatio)>,
}
impl Port {
    fn pointer_divider_resize(&mut self, target: &DrawnDivider, ratio: ClientSplitRatio) {
        assert!(self.pointer_divider_ready(target).is_ok());
        self.submissions.push((target.clone(), ratio));
        self.busy = true;
    }
}

impl TerminalInputPort for Port {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{LayoutAction, SurfaceAction};
        match action {
            SurfaceAction::Layout(LayoutAction::Divider { target, ratio }) => {
                self.pointer_divider_resize(target, ratio);
                None
            }
            _ => None,
        }
    }

    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("divider gesture must never send program bytes")
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe {
            input_captured: self.captured,
            ..PointerProbe::default()
        }
    }
    fn pointer_divider_ready(
        &mut self,
        target: &DrawnDivider,
    ) -> Result<(), crate::client_view::Notice> {
        if self.failed {
            Err(crate::client_view::Notice::ResizeFailed)
        } else if self.busy {
            Err(crate::client_view::Notice::ResizeBusy)
        } else if &self.target != target {
            Err(crate::client_view::Notice::ResizeFailed)
        } else {
            Ok(())
        }
    }
}
fn target() -> DrawnDivider {
    let orientation = PanelOrientation::Horizontal;
    let ratio = ClientSplitRatio::new(0.5).unwrap();
    let area = Rect::new(7, 9, 101, 30);
    DrawnDivider {
        source: DividerSource {
            session: SessionId::parse(&format!("{:032x}", 1)).unwrap(),
            context: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
            tab: tab_id(3),
            focus: panel_id(4),
            revision: WorkbenchRevision::new(1),
        },
        viewport: Rect::new(0, 0, 120, 40),
        split: WorkbenchSplitId::parse(&format!("{:032x}", 5)).unwrap(),
        first: WorkbenchPanelSlotId::parse(&format!("{:032x}", 6)).unwrap(),
        second: WorkbenchPanelSlotId::parse(&format!("{:032x}", 7)).unwrap(),
        orientation,
        ratio,
        area,
        divider: divide_panes(area, orientation, ratio).unwrap().divider,
    }
}
fn map(target: &DrawnDivider) -> FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.record_divider(target.clone());
    hits.finish()
}
fn fixture() -> (PointerState, ClientView, Port) {
    let target = target();
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map(&target));
    (
        pointer,
        ClientView::new(120),
        Port {
            target,
            busy: false,
            failed: false,
            captured: false,
            submissions: Vec::new(),
        },
    )
}
fn report(
    pointer: &mut PointerState,
    view: &mut ClientView,
    port: &mut Port,
    action: MouseAction,
    x: u16,
    y: u16,
) {
    pointer.event(
        view,
        port,
        Instant::now(),
        event(action, MouseButton::Left, x, y),
    );
}

fn projected(pointer: &PointerState, target: &DrawnDivider) -> ProjectedPointer {
    HitMapBuilder::with_pointer(pointer.projection()).projected_divider_pointer(target)
}

#[test]
fn divider_projection_uses_exact_painted_source_and_clears_on_transport_loss() {
    let (mut pointer, mut view, mut port) = fixture();
    let target = port.target.clone();
    let (x, y) = (target.divider.x, target.divider.y);
    assert_eq!(projected(&pointer, &target), ProjectedPointer::Rest);

    pointer.set_motion_observability(MotionObservability::AnyMotion);
    assert_eq!(
        pointer.event(
            &mut view,
            &mut port,
            Instant::now(),
            event(MouseAction::Motion, MouseButton::None, x, y),
        ),
        PointerOutcome::IdleMotion { changed: true },
    );
    assert_eq!(projected(&pointer, &target), ProjectedPointer::Hovered);

    report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
    assert_eq!(projected(&pointer, &target), ProjectedPointer::Pressed);
    pointer.publish(map(&target));
    assert_eq!(projected(&pointer, &target), ProjectedPointer::Pressed);

    let mut replacement = target.clone();
    replacement.source.revision = WorkbenchRevision::new(2);
    pointer.publish(map(&replacement));
    assert_eq!(projected(&pointer, &target), ProjectedPointer::Rest);
    assert_eq!(
        pointer.event(
            &mut view,
            &mut port,
            Instant::now(),
            event(MouseAction::Release, MouseButton::Left, x, y),
        ),
        PointerOutcome::ChromeGestureEnded,
    );

    pointer.set_host_focus(false);
    assert_eq!(projected(&pointer, &target), ProjectedPointer::Rest);
}

#[test]
fn divider_release_submits_once_without_program_input_or_canonical_layout_changes() {
    let (mut pointer, mut view, mut port) = fixture();
    let source = port.target.clone();
    let (x, y) = (source.divider.x, source.divider.y);
    report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
    report(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Motion,
        x + 20,
        y,
    );
    assert_eq!(pointer.divider_preview().unwrap().ratio.value(), 0.7);
    assert!(port.submissions.is_empty());
    assert_eq!(port.target, source);
    report(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        x + 20,
        y,
    );
    assert_eq!(
        port.submissions,
        [(source, ClientSplitRatio::new(0.7).unwrap())]
    );
    assert!(pointer.divider_preview().is_none());
    report(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        0,
        0,
    );
    report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
    assert_eq!(port.submissions.len(), 1);
    assert!(pointer.divider_preview().is_none());
    assert_eq!(view.notice(), Some(crate::client_view::Notice::ResizeBusy));
    port.busy = false;
    pointer.tick(&mut view, &mut port, Instant::now(), None);
    assert_eq!(port.submissions.len(), 1, "busy press is never replayed");
}

#[test]
fn divider_two_held_moves_survive_own_frames_but_not_external_geometry() {
    for orientation in [PanelOrientation::Horizontal, PanelOrientation::Vertical] {
        for external in [false, true] {
            let (mut pointer, mut view, mut port) = fixture();
            port.target.orientation = orientation;
            port.target.divider = divide_panes(port.target.area, orientation, port.target.ratio)
                .unwrap()
                .divider;
            pointer.publish(map(&port.target));
            let origin = port.target.clone();
            let (x, y) = (origin.divider.x, origin.divider.y);
            report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
            for delta in [4, 8] {
                let (column, row) = match orientation {
                    PanelOrientation::Horizontal => (x + delta, y),
                    PanelOrientation::Vertical => (x, y + delta),
                };
                report(
                    &mut pointer,
                    &mut view,
                    &mut port,
                    MouseAction::Motion,
                    column,
                    row,
                );
                let drawn = pointer.divider_preview().unwrap().projected_target();
                assert_ne!(drawn.divider, origin.divider);
                pointer.publish(map(&drawn));
                pointer.tick(&mut view, &mut port, Instant::now(), None);
                let drag = pointer
                    .divider_preview()
                    .expect("our completed frame retains this drag");
                assert_eq!(drag.target, origin, "the canonical origin never moves");
                assert_eq!(drag.painted, drawn);
                assert_eq!(projected(&pointer, &drawn), ProjectedPointer::Pressed);
                assert!(port.submissions.is_empty(), "held frames submit no CAS");
            }
            let final_ratio = pointer.divider_preview().unwrap().ratio;
            if external {
                let mut replaced = pointer.divider_preview().unwrap().projected_target();
                replaced.area.width += 1;
                pointer.publish(map(&replaced));
                assert!(pointer.divider_preview().is_none());
            }
            let (column, row) = match orientation {
                PanelOrientation::Horizontal => (x + 8, y),
                PanelOrientation::Vertical => (x, y + 8),
            };
            report(
                &mut pointer,
                &mut view,
                &mut port,
                MouseAction::Release,
                column,
                row,
            );
            assert_eq!(port.submissions.len(), usize::from(!external));
            if !external {
                assert_eq!(port.submissions[0], (origin, final_ratio));
            }
        }
    }
}

#[test]
fn divider_idle_lane_failure_and_busy_are_visible_without_waiting_for_another_mouse_event() {
    for failed in [false, true] {
        let (mut pointer, mut view, mut port) = fixture();
        let (x, y) = (port.target.divider.x, port.target.divider.y);
        report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
        assert!(pointer.divider_preview().is_some());
        port.failed = failed;
        port.busy = !failed;
        assert!(pointer.tick(&mut view, &mut port, Instant::now(), None));
        assert!(pointer.divider_preview().is_none());
        assert_eq!(
            view.notice(),
            Some(if failed {
                crate::client_view::Notice::ResizeFailed
            } else {
                crate::client_view::Notice::ResizeBusy
            })
        );
        report(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            x + 10,
            y,
        );
        assert!(port.submissions.is_empty());
    }
}

#[test]
fn divider_admitted_owner_survives_ctrl_alt_until_release_but_shift_cancels() {
    for mods in [Modifiers::CTRL, Modifiers::NONE.with_alt()] {
        let (mut pointer, mut view, mut port) = fixture();
        let (x, y) = (port.target.divider.x, port.target.divider.y);
        report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
        let mut moved = event(MouseAction::Motion, MouseButton::Left, x + 10, y);
        moved.mods = mods;
        pointer.event(&mut view, &mut port, Instant::now(), moved);
        assert_eq!(pointer.divider_preview().unwrap().ratio.value(), 0.6);
        moved.action = MouseAction::Release;
        pointer.event(&mut view, &mut port, Instant::now(), moved);
        assert_eq!(port.submissions.len(), 1);
    }
}

#[test]
fn divider_plain_click_return_to_origin_and_perpendicular_motion_have_no_effect() {
    for moved in [false, true] {
        let (mut pointer, mut view, mut port) = fixture();
        let (x, y) = (port.target.divider.x, port.target.divider.y);
        report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
        if moved {
            report(
                &mut pointer,
                &mut view,
                &mut port,
                MouseAction::Motion,
                u16::MAX,
                u16::MAX,
            );
        }
        pointer.event(
            &mut view,
            &mut port,
            Instant::now(),
            event(MouseAction::Release, MouseButton::None, x, 0),
        );
        assert!(port.submissions.is_empty());
        assert!(pointer.divider_preview().is_none());
    }
}

#[test]
fn divider_cancellation_never_commits_or_revives_on_late_release() {
    for cancellation in 0..8 {
        let (mut pointer, mut view, mut port) = fixture();
        let (x, y) = (port.target.divider.x, port.target.divider.y);
        report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
        report(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Motion,
            x + 10,
            y,
        );
        match cancellation {
            0 => {
                assert!(pointer.cancel_divider());
            }
            1 => pointer.invalidate_map_with_port(&mut port),
            2 => pointer.set_capture_with_port(&mut port, false),
            3 => {
                let mut shift = event(MouseAction::Motion, MouseButton::Left, x + 10, y);
                shift.mods = Modifiers::NONE.with_shift();
                pointer.event(&mut view, &mut port, Instant::now(), shift);
            }
            4 => {
                port.captured = true;
                pointer.tick(&mut view, &mut port, Instant::now(), None);
            }
            5 => {
                port.target.viewport.width += 1;
                pointer.tick(&mut view, &mut port, Instant::now(), None);
            }
            6 => {
                view.dismiss_overlay();
                commit_overlay(&mut view);
                pointer.tick(&mut view, &mut port, Instant::now(), None);
            }
            _ => {
                port.target.source.revision = WorkbenchRevision::new(2);
                pointer.publish(map(&port.target));
            }
        }
        assert!(
            pointer.divider_preview().is_none(),
            "cancellation {cancellation}"
        );
        report(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Release,
            x + 10,
            y,
        );
        assert!(port.submissions.is_empty());
    }
}

#[test]
fn compositor_cover_retires_divider_preview_and_owns_one_late_release() {
    for button in [MouseButton::Left, MouseButton::None] {
        let (mut pointer, mut view, mut port) = fixture();
        let (x, y) = (port.target.divider.x, port.target.divider.y);
        report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
        report(
            &mut pointer,
            &mut view,
            &mut port,
            MouseAction::Motion,
            x + 10,
            y,
        );
        assert!(pointer.divider_preview().is_some());

        assert!(pointer.retire_covered_chrome_gesture());
        assert!(pointer.divider_preview().is_none());
        let release = event(MouseAction::Release, button, x + 10, y);
        assert_eq!(
            pointer.cancelled_ui_event(release),
            PointerOutcome::ChromeGestureEnded,
        );
        assert_eq!(pointer.cancelled_ui_event(release), PointerOutcome::Ignored,);
        assert!(port.submissions.is_empty());
    }
}
fn commit_overlay(view: &mut ClientView) {
    let transition = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('b'),
        Modifiers::CTRL,
    )));
    view.commit(transition, ClientActionDisposition::KeepNavigator);
    let transition = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('?'),
        Modifiers::NONE,
    )));
    view.commit(transition, ClientActionDisposition::KeepNavigator);
}

#[test]
fn divider_repaint_preserves_exact_source_and_wrong_buttons_cannot_replace_owner() {
    let (mut pointer, mut view, mut port) = fixture();
    let (x, y) = (port.target.divider.x, port.target.divider.y);
    report(&mut pointer, &mut view, &mut port, MouseAction::Press, x, y);
    pointer.publish(map(&port.target));
    for button in [
        MouseButton::Right,
        MouseButton::Middle,
        MouseButton::WheelDown,
    ] {
        pointer.event(
            &mut view,
            &mut port,
            Instant::now(),
            event(MouseAction::Release, button, 0, 0),
        );
        assert!(pointer.divider_preview().is_some());
    }
    report(
        &mut pointer,
        &mut view,
        &mut port,
        MouseAction::Release,
        u16::MAX,
        u16::MAX,
    );
    assert_eq!(port.submissions[0].1.value(), 0.9);
}

// Compact actions: press, motion, release, Esc, Shift, invalidate, draw, replace,
// capture toggle, modal toggle, busy, settle, perpendicular motion, wrong release.
fn replay(actions: &[[u16; 3]]) -> Result<u16, String> {
    let (mut pointer, mut view, mut port) = fixture();
    let mut alive = false;
    let mut capture = true;
    let mut painted = true;
    let mut last = port.target.divider.x;
    let mut expected = 0;
    let mut coverage = 0;
    for &[kind, x, y] in actions {
        let (press_x, press_y) = (port.target.divider.x, port.target.divider.y);
        coverage |= 1 << kind;
        match kind {
            0 => {
                if !alive && capture && painted && !port.captured && !port.busy {
                    alive = true;
                    last = press_x;
                }
                report(
                    &mut pointer,
                    &mut view,
                    &mut port,
                    MouseAction::Press,
                    press_x,
                    press_y,
                );
            }
            1 | 12 => {
                let next = if kind == 12 { press_x } else { x };
                if alive {
                    last = next;
                }
                report(
                    &mut pointer,
                    &mut view,
                    &mut port,
                    MouseAction::Motion,
                    next,
                    y,
                );
            }
            2 => {
                if alive {
                    expected += usize::from(x != press_x);
                    alive = false;
                }
                report(
                    &mut pointer,
                    &mut view,
                    &mut port,
                    MouseAction::Release,
                    x,
                    y,
                );
            }
            3 => {
                pointer.cancel_divider();
                alive = false;
            }
            4 => {
                let mut shift = event(MouseAction::Motion, MouseButton::Left, x, y);
                shift.mods = Modifiers::NONE.with_shift();
                pointer.event(&mut view, &mut port, Instant::now(), shift);
                alive = false;
            }
            5 => {
                pointer.invalidate_map();
                alive = false;
                painted = false;
            }
            6 => {
                let drawn = pointer
                    .divider_preview()
                    .map_or_else(|| port.target.clone(), |drag| drag.projected_target());
                painted = drawn == port.target;
                pointer.publish(map(&drawn));
            }
            7 => {
                port.target.source.revision =
                    WorkbenchRevision::new(port.target.source.revision.value() + 1);
                alive = false;
                painted = false;
            }
            8 => {
                capture = !capture;
                pointer.set_capture_with_port(&mut port, capture);
                if !capture {
                    alive = false;
                }
            }
            9 => {
                port.captured = !port.captured;
                if port.captured {
                    alive = false;
                }
            }
            10 => {
                port.busy = true;
                alive = false;
            }
            11 => port.busy = false,
            13 => {
                pointer.event(
                    &mut view,
                    &mut port,
                    Instant::now(),
                    event(MouseAction::Release, MouseButton::Right, x, y),
                );
            }
            _ => return Err("unknown action".to_owned()),
        }
        pointer.tick(&mut view, &mut port, Instant::now(), None);
        if pointer.divider_preview().is_some() != alive || port.submissions.len() != expected {
            return Err(format!(
                "step {kind}: alive={alive}, expected={expected}, actual={}",
                port.submissions.len()
            ));
        }
        if let Some(drag) = pointer.divider_preview() {
            let expected_ratio =
                (0.5 + (f64::from(last) - f64::from(press_x)) / 100.0).clamp(0.1, 0.9);
            if drag.target != port.target || drag.ratio.value() != expected_ratio {
                return Err("preview source or ratio diverged".to_owned());
            }
        }
    }
    Ok(coverage)
}
fn shrink(mut trace: Vec<[u16; 3]>) -> Vec<[u16; 3]> {
    let mut index = 0;
    while index < trace.len() {
        let mut candidate = trace.clone();
        candidate.remove(index);
        if replay(&candidate).is_err() {
            trace = candidate;
            index = 0;
        } else {
            index += 1;
        }
    }
    trace
}
#[test]
fn divider_generated_timelines_have_replay_shrinking_and_all_event_classes() {
    if let Ok(serialized) = std::env::var("ORCHBEE_DIVIDER_TRACE") {
        let actions: Vec<[u16; 3]> = serde_json::from_str(&serialized).unwrap();
        replay(&actions).unwrap();
        return;
    }
    let mut coverage = 0;
    for seed in [1_u64, 2, 7, 19, 101, 997, 65537, 987654321] {
        let mut state = seed;
        let mut trace: Vec<[u16; 3]> = (0..14).map(|kind| [kind, 67, 17]).collect();
        for _ in 0..192 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            trace.push([
                u16::try_from((state >> 32) % 14).unwrap(),
                u16::try_from((state >> 16) % 180).unwrap(),
                u16::try_from(state % 60).unwrap(),
            ]);
        }
        let encoded = serde_json::to_string(&trace).unwrap();
        let decoded = serde_json::from_str::<Vec<[u16; 3]>>(&encoded).unwrap();
        match replay(&decoded) {
            Ok(seen) => coverage |= seen,
            Err(reason) => panic!(
                "seed={seed}: {reason}; ORCHBEE_DIVIDER_TRACE={}",
                serde_json::to_string(&shrink(decoded)).unwrap()
            ),
        }
    }
    assert_eq!(coverage, (1 << 14) - 1);
}
