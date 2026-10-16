//! Local preference commits require an exposed frame and a matching left release.
use super::*;
use crate::tui_layout::{DrawnSidebarWidth, SidebarWidth, project_shell_areas};

#[derive(Default)]
struct Port {
    captured: bool,
}
impl TerminalInputPort for Port {
    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("sidebar width never forwards pointer bytes")
    }
    fn pointer_probe(&mut self) -> PointerProbe {
        PointerProbe {
            input_captured: self.captured,
            ..PointerProbe::default()
        }
    }
}
fn target(view: &ClientView) -> DrawnSidebarWidth {
    DrawnSidebarWidth::new(Rect::new(0, 0, 120, 40), view.sidebar_surface()).unwrap()
}
fn map(target: DrawnSidebarWidth) -> FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Chrome,
        target.edge(),
        HitTarget::SidebarResize(target),
    );
    hits.finish()
}
fn fixture() -> (PointerState, ClientView, Port) {
    let mut view = ClientView::new(120);
    view.machines.available = true;
    view.machines.observe_agent_sidebar(true);
    assert!(view.machines.enter_sidebar_navigation());
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map(target(&view)));
    (pointer, view, Port::default())
}
fn report(
    pointer: &mut PointerState,
    view: &mut ClientView,
    port: &mut Port,
    action: MouseAction,
    column: u16,
) -> PointerOutcome {
    pointer.event(
        view,
        port,
        Instant::now(),
        event(action, MouseButton::Left, column, 4),
    )
}

#[test]
fn sidebar_width_motion_is_reversible_and_only_exact_release_commits_once() {
    let (mut pointer, mut view, mut port) = fixture();
    let original = target(&view);
    let geometry = project_shell_areas(original.viewport, view.sidebar_surface());
    report(&mut pointer, &mut view, &mut port, MouseAction::Press, 29);
    report(&mut pointer, &mut view, &mut port, MouseAction::Motion, 50);
    assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
    assert_eq!(
        project_shell_areas(original.viewport, view.sidebar_surface()),
        geometry
    );
    assert_eq!(pointer.sidebar_width_preview().unwrap().width.columns(), 51);
    let drawn = pointer.sidebar_width_preview().unwrap().projected_target();
    assert_ne!(drawn.sidebar, original.sidebar);
    let effective = pointer.resize_projection().sidebar(view.sidebar_surface());
    assert_ne!(project_shell_areas(original.viewport, effective), geometry);
    pointer.publish(map(drawn));
    assert!(pointer.sidebar_width_preview().is_some());
    report(&mut pointer, &mut view, &mut port, MouseAction::Motion, 40);
    let drawn = pointer.sidebar_width_preview().unwrap().projected_target();
    pointer.publish(map(drawn));
    pointer.tick(&mut view, &mut port, Instant::now(), None);
    assert_eq!(pointer.sidebar_width_preview().unwrap().width.columns(), 41);
    assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
    assert_eq!(
        report(&mut pointer, &mut view, &mut port, MouseAction::Release, 35),
        PointerOutcome::SidebarWidthEnded
    );
    assert_eq!(
        view.machines.sidebar_width().columns(),
        36,
        "release coordinate replaces last motion"
    );
    assert!(pointer.sidebar_width_preview().is_none());
    report(&mut pointer, &mut view, &mut port, MouseAction::Release, 55);
    assert_eq!(view.machines.sidebar_width().columns(), 36);
    assert!(view.machines.sidebar_navigation_active());
    assert!(view.machines.request.is_none());
}

#[test]
fn sidebar_width_admission_requires_plain_left_and_an_exposed_current_frame() {
    for refused in 0..8 {
        let (mut pointer, mut view, mut port) = fixture();
        let mut press = event(MouseAction::Press, MouseButton::Left, 29, 4);
        match refused {
            0 => press.button = MouseButton::Right,
            1 => press.mods = Modifiers::CTRL,
            2 => press.mods = Modifiers::NONE.with_shift(),
            3 => pointer.set_capture(false),
            4 => pointer.invalidate_map(),
            5 => {
                let mut hits = HitMapBuilder::new();
                hits.mark_overlay_active();
                pointer.publish(hits.finish());
            }
            6 => {
                view.machines
                    .set_sidebar_width(SidebarWidth::DEFAULT, SidebarWidth::new(40).unwrap());
            }
            _ => port.captured = true,
        }
        let expected = view.machines.sidebar_width();
        pointer.event(&mut view, &mut port, Instant::now(), press);
        assert!(
            pointer.sidebar_width_preview().is_none(),
            "refused {refused}"
        );
        assert_eq!(view.machines.sidebar_width(), expected);
    }
}

#[test]
fn sidebar_width_cancellation_cannot_replay_a_late_release() {
    for cancellation in 0..10 {
        let (mut pointer, mut view, mut port) = fixture();
        report(&mut pointer, &mut view, &mut port, MouseAction::Press, 29);
        report(&mut pointer, &mut view, &mut port, MouseAction::Motion, 45);
        match cancellation {
            0 => {
                pointer.cancel_sidebar_width();
            }
            1 => pointer.invalidate_map(),
            2 => pointer.set_capture_with_port(&mut port, false),
            3 => {
                let mut shift = event(MouseAction::Motion, MouseButton::Left, 45, 4);
                shift.mods = Modifiers::NONE.with_shift();
                pointer.event(&mut view, &mut port, Instant::now(), shift);
            }
            4 => port.captured = true,
            5 => view.resize(70),
            6 => view.commit(
                view.decide_action(ClientAction::OpenShortcutHelp),
                SessionActionDisposition::KeepNavigator,
            ),
            7 => pointer.publish(HitMapBuilder::new().finish()),
            8 => {
                view.machines
                    .set_sidebar_width(SidebarWidth::DEFAULT, SidebarWidth::new(40).unwrap());
            }
            _ => {
                view.machines.open = true;
            }
        }
        pointer.tick(&mut view, &mut port, Instant::now(), None);
        assert!(
            pointer.sidebar_width_preview().is_none(),
            "cancellation {cancellation}"
        );
        let expected = view.machines.sidebar_width();
        report(&mut pointer, &mut view, &mut port, MouseAction::Release, 45);
        assert_eq!(view.machines.sidebar_width(), expected);
        assert!(view.machines.request.is_none());
    }
}

#[test]
fn compositor_cover_retires_sidebar_width_preview_and_owns_one_late_release() {
    for button in [MouseButton::Left, MouseButton::None] {
        let (mut pointer, mut view, mut port) = fixture();
        report(&mut pointer, &mut view, &mut port, MouseAction::Press, 29);
        report(&mut pointer, &mut view, &mut port, MouseAction::Motion, 45);
        assert!(pointer.sidebar_width_preview().is_some());
        let before = view.machines.sidebar_width();

        assert!(pointer.retire_covered_chrome_gesture());
        assert!(pointer.sidebar_width_preview().is_none());
        let release = event(MouseAction::Release, button, 45, 4);
        assert_eq!(
            pointer.cancelled_ui_event(release),
            PointerOutcome::ChromeGestureEnded,
        );
        assert_eq!(pointer.cancelled_ui_event(release), PointerOutcome::Ignored,);
        assert_eq!(view.machines.sidebar_width(), before);
        assert!(view.machines.request.is_none());
    }
}

// Stable JSON commands: press/move/release/Esc/Shift/invalidate/draw/replace/
// capture/modal/wrong-release. The oracle uses only scalar bounds, not drag code.
fn replay(commands: &[[u16; 2]]) -> Result<(), String> {
    let (mut pointer, mut view, mut port) = fixture();
    let (mut preferred, mut painted, mut capture, mut modal, mut alive) =
        (30, Some(30), true, false, false);
    for (index, &[kind, column]) in commands.iter().enumerate() {
        let was_alive = alive;
        match kind {
            0 => {
                if !alive && capture && !modal && painted == Some(preferred) {
                    alive = true;
                }
                report(
                    &mut pointer,
                    &mut view,
                    &mut port,
                    MouseAction::Press,
                    preferred - 1,
                );
            }
            1 => {
                report(
                    &mut pointer,
                    &mut view,
                    &mut port,
                    MouseAction::Motion,
                    column,
                );
            }
            2 => {
                if alive {
                    preferred = column.saturating_add(1).clamp(24, 60);
                    alive = false;
                }
                report(
                    &mut pointer,
                    &mut view,
                    &mut port,
                    MouseAction::Release,
                    column,
                );
            }
            3 => {
                pointer.cancel_sidebar_width();
                alive = false;
            }
            4 => {
                let mut shift = event(MouseAction::Motion, MouseButton::Left, column, 4);
                shift.mods = Modifiers::NONE.with_shift();
                pointer.event(&mut view, &mut port, Instant::now(), shift);
                alive = false;
            }
            5 => {
                pointer.invalidate_map();
                painted = None;
                alive = false;
            }
            6 => {
                let drawn = pointer
                    .sidebar_width_preview()
                    .map_or_else(|| target(&view), |drag| drag.projected_target());
                pointer.publish(map(drawn));
                painted = Some(preferred);
            }
            7 => {
                let next = if column % 2 == 0 { 24 } else { 60 };
                view.machines.set_sidebar_width(
                    SidebarWidth::new(preferred).unwrap(),
                    SidebarWidth::new(next).unwrap(),
                );
                preferred = next;
                if painted != Some(preferred) {
                    alive = false;
                }
            }
            8 => {
                capture = !capture;
                pointer.set_capture_with_port(&mut port, capture);
                if !capture {
                    alive = false;
                }
            }
            9 => {
                modal = !modal;
                port.captured = modal;
                if modal {
                    alive = false;
                }
            }
            _ => {
                pointer.event(
                    &mut view,
                    &mut port,
                    Instant::now(),
                    event(MouseAction::Release, MouseButton::Right, column, 4),
                );
            }
        }
        if was_alive && !alive {
            painted = None;
        }
        pointer.tick(&mut view, &mut port, Instant::now(), None);
        if view.machines.sidebar_width().columns() != preferred
            || pointer.sidebar_width_preview().is_some() != alive
            || view.machines.request.is_some()
        {
            return Err(format!(
                "step {index}: expected width={preferred} drag={alive}, actual width={} drag={}",
                view.machines.sidebar_width().columns(),
                pointer.sidebar_width_preview().is_some()
            ));
        }
    }
    Ok(())
}

#[test]
fn sidebar_width_seeded_lifetimes_shrink_and_replay_without_late_commits() {
    for seed in [1_u64, 7, 19, 47, 101, 509] {
        let mut state = seed;
        let mut commands = vec![[0, 29], [1, 45], [2, 35], [6, 0]];
        commands.extend((0..11).map(|kind| [kind, 29]));
        for _ in 0..128 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            commands.push([((state >> 32) % 11) as u16, ((state >> 16) % 100) as u16]);
        }
        if replay(&commands).is_err() {
            let mut index = 0;
            while index < commands.len() {
                let mut shorter = commands.clone();
                shorter.remove(index);
                if replay(&shorter).is_err() {
                    commands = shorter;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!(
                "seed {seed}: {:?}; replay={}",
                replay(&commands),
                serde_json::to_string(&commands).unwrap()
            );
        }
    }
}
