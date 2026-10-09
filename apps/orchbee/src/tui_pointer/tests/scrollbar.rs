//! Full pointer traces distinguish scrollbar chrome from pane-owned input.
use super::*;
use crate::terminal_scrollbar::ScrollbarSnapshot;
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use std::num::NonZeroU64;

fn fixture() -> (ClientView, PointerState, RecordingPort, FrameHitMap) {
    let view = ClientView::new(120);
    let probe = active_probe(0, 0, 1, 2);
    let source = ScrollbarSnapshot::new(
        AttachmentId::from_bytes([1; 16]).unwrap(),
        HostEpoch::from_bytes([2; 16]).unwrap(),
        NonZeroU64::new(1).unwrap(),
        (80, 10),
        10,
        100,
        90,
    )
    .unwrap();
    let pane = probe.focus.clone().unwrap();
    let content = Rect::new(2, 3, 80, 10);
    let mut builder = HitMapBuilder::new();
    builder.record(
        HitLayer::Foreground,
        content,
        HitTarget::Pane {
            pane: pane.clone(),
            content,
        },
    );
    builder.record(
        HitLayer::Foreground,
        Rect::new(82, 3, 1, 10),
        HitTarget::Scrollbar {
            pane,
            content,
            source,
        },
    );
    let map = builder.finish();
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map.clone());
    let port = RecordingPort {
        probe,
        ..RecordingPort::default()
    };
    (view, pointer, port, map)
}

#[test]
fn scrollbar_thumb_drag_clamps_outside_track_and_returns_to_bottom_without_terminal_input() {
    let (mut view, mut pointer, mut port, _) = fixture();
    pointer.event(&mut view, &mut port, now(), press(82, 12));
    assert!(port.scrolls.is_empty(), "grabbing the thumb does not jump");
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Motion, MouseButton::Left, 0, 0),
    );
    assert_eq!(port.scrolls.last().unwrap().1, 90);
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Motion, MouseButton::Left, 119, 8),
    );
    assert_eq!(port.scrolls.last().unwrap().1, 40);
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 119, 39),
    );
    assert_eq!(port.scrolls.last().unwrap().1, 0);
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.bytes().is_empty() && port.selections.is_empty());
    assert!(
        port.scrolls
            .iter()
            .all(|(pane, _)| Some(pane) == port.probe.focus.as_ref())
    );
}

#[test]
fn scrollbar_press_focuses_the_painted_pane_before_scrolling() {
    let (mut view, mut pointer, mut port, _) = fixture();
    let painted = port.probe.focus.clone().unwrap();
    port.probe = active_probe(0, 1, 1, 2);
    pointer.event(&mut view, &mut port, now(), press(82, 3));
    assert!(
        matches!(port.actions.as_slice(), [ClientAction::FocusPanel(panel)]
        if panel == painted.pane().panel())
    );
    assert_eq!(port.probe.focus.as_ref(), Some(&painted));
    assert_eq!(port.scrolls, [(painted, 90)]);
    assert!(port.bytes().is_empty() && port.selections.is_empty());
}

#[test]
fn scrollbar_track_click_and_every_region_edge_have_one_owner() {
    for (x, y, admitted) in [
        (82, 3, true),
        (82, 11, true),
        (82, 2, false),
        (83, 3, false),
        (82, 13, false),
    ] {
        let (mut view, mut pointer, mut port, _) = fixture();
        pointer.event(&mut view, &mut port, now(), press(x, y));
        assert_eq!(!port.scrolls.is_empty(), admitted, "{x},{y}");
        assert!(port.bytes().is_empty());
    }
    for mods in [Modifiers::CTRL, Modifiers::NONE.with_shift()] {
        let (mut view, mut pointer, mut port, _) = fixture();
        let mut input = press(82, 3);
        input.mods = mods;
        pointer.event(&mut view, &mut port, now(), input);
        assert!(port.scrolls.is_empty());
    }
}

#[test]
fn scrollbar_drag_cancels_on_capture_resize_switch_close_overlay_and_reminted_identity() {
    for cause in 0..8 {
        let (mut view, mut pointer, mut port, map) = fixture();
        pointer.event(&mut view, &mut port, now(), press(82, 12));
        match cause {
            0 => {
                pointer.set_capture(false);
                pointer.set_capture(true);
            }
            1 => {
                pointer.invalidate_map();
                pointer.publish(map);
            }
            2 => {
                port.probe = active_probe(0, 1, 1, 2);
            }
            3 => {
                port.probe.panes.clear();
            }
            4 => {
                port.probe.input_captured = true;
            }
            5 => {
                port.probe.focus = Some(pane_ref(1, 1, 1, 2));
            }
            6 => {
                pointer.publish(HitMapBuilder::new().finish());
            }
            7 => {
                port.probe.selection_ready = false;
            }
            _ => unreachable!(),
        }
        pointer.tick(&mut view, &mut port, now());
        pointer.event(
            &mut view,
            &mut port,
            now(),
            event(MouseAction::Motion, MouseButton::Left, 82, 3),
        );
        pointer.event(
            &mut view,
            &mut port,
            now(),
            event(MouseAction::Release, MouseButton::Left, 82, 3),
        );
        assert!(port.scrolls.is_empty(), "cause {cause}");
        assert!(port.bytes().is_empty() && port.selections.is_empty());
    }
}

#[test]
fn scrollbar_pending_window_keeps_capture_but_changed_generation_cancels() {
    let (mut view, mut pointer, mut port, mut map) = fixture();
    pointer.event(&mut view, &mut port, now(), press(82, 12));
    let original = map.clone();
    map.regions
        .retain(|region| !matches!(region.target, HitTarget::Scrollbar { .. }));
    pointer.publish(map);
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Motion, MouseButton::Left, 82, 8),
    );
    assert_eq!(port.scrolls.len(), 1);
    let mut changed = original;
    for region in &mut changed.regions {
        if let HitTarget::Scrollbar { source, .. } = &mut region.target {
            source.generation = NonZeroU64::new(2).unwrap();
        }
    }
    pointer.publish(changed);
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 82, 3),
    );
    assert_eq!(port.scrolls.len(), 1);
}

#[test]
fn scrollbar_chrome_never_forwards_any_mouse_tracking_mode_and_range_growth_cancels_grab() {
    for tracking in [
        MouseTracking::None,
        MouseTracking::X10,
        MouseTracking::Normal,
        MouseTracking::Button,
        MouseTracking::Any,
    ] {
        let (mut view, mut pointer, mut port, mut map) = fixture();
        port.probe.frame = Some(frame_facts(tracking, MouseFormat::Sgr, false));
        pointer.event(&mut view, &mut port, now(), press(82, 3));
        assert_eq!(port.scrolls.len(), 1);
        assert!(port.bytes().is_empty() && port.selections.is_empty());
        for region in &mut map.regions {
            if let HitTarget::Scrollbar { source, .. } = &mut region.target {
                *source = ScrollbarSnapshot::new(
                    source.attachment,
                    source.epoch,
                    source.generation,
                    (80, 10),
                    10,
                    150,
                    0,
                )
                .unwrap();
            }
        }
        pointer.publish(map);
        pointer.event(
            &mut view,
            &mut port,
            now(),
            event(MouseAction::Motion, MouseButton::Left, 82, 8),
        );
        pointer.event(
            &mut view,
            &mut port,
            now(),
            event(MouseAction::Release, MouseButton::Left, 82, 8),
        );
        assert_eq!(
            port.scrolls.len(),
            1,
            "{tracking:?}: changed total cancels the drag"
        );
        assert!(port.bytes().is_empty() && port.selections.is_empty());
    }
}
