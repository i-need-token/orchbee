//! Navigator entry uses the existing pointer cleanup without exporting or replaying.
use super::*;
use crate::terminal_input::InputDispatcher;

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut RecordingPort,
    bytes: &[u8],
) {
    dispatcher
        .dispatch(
            view,
            bytes,
            port,
            now(),
            |_, _| Ok::<_, std::io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn entry_cancels_an_admitted_selection_once_without_copy_or_export() {
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    let mut port = RecordingPort {
        probe: active_probe(0, 0, 1, 1),
        selection_admitted: true,
        ..RecordingPort::default()
    };
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Foreground,
        Rect::new(4, 3, 6, 4),
        HitTarget::Pane {
            pane: pane_ref(1, 1, 1, 0),
            content: Rect::new(4, 3, 6, 4),
        },
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;6;5M");
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x02w\x1b[<0;6;5m");
    assert!(view.navigator_active());
    assert_eq!(port.selections.len(), 2);
    assert!(matches!(
        port.selections[0].1,
        PointerSelectionIntent::Start { .. }
    ));
    assert_eq!(port.selections[1].1, PointerSelectionIntent::Cancel);
    assert!(port.bytes().is_empty());
}

#[test]
fn entry_discards_pending_pane_mouse_bytes_before_that_pane_becomes_ready() {
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    let mut probe = active_probe(0, 0, 1, 2);
    set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(true), false));
    probe.frame = None;
    let mut port = RecordingPort {
        probe,
        ..RecordingPort::default()
    };
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Foreground,
        Rect::new(0, 0, 20, 10),
        HitTarget::Pane {
            pane: pane_ref(1, 1, 2, 0),
            content: Rect::new(0, 0, 20, 10),
        },
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    assert!(
        port.actions
            .contains(&ClientAction::FocusPanel(panel_id(2)))
    );
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x02w");
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 2, 0, true, Some(true), true),
    );
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, true));
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert!(port.bytes().is_empty());
    assert!(port.selections.is_empty());
}
