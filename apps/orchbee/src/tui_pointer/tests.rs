//! Hit-map, gesture, pending-focus, and wheel-normalization tests for the
//! pointer ownership state. A recording port supplies the probe facts;
//! each test drives `PointerState::event`/`tick` like the dispatcher does.
use super::*;

use orchbee_terminal_protocol::MouseFormat;
use orchbee_workbench_protocol::WorkbenchPanelId;

use crate::terminal_input::{PointerPane, PointerTab};
use crate::terminal_session::state::{AttachmentKey, PaneKey};

use crate::client_view::{
    KeyCode, KeyEvent, Modifiers, Overlay, PrefixInput, SessionActionDisposition,
};

fn tab_id(value: u128) -> WorkbenchTabId {
    WorkbenchTabId::parse(&format!("{value:032x}")).unwrap()
}

fn panel_id(value: u128) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&format!("{value:032x}")).unwrap()
}

fn pane_key(anchor: u64, tab: u128, panel: u128) -> PaneKey {
    PaneKey::new(AttachmentKey::new(anchor, tab_id(tab)), panel_id(panel))
}

/// The exact incarnation a painted pane leaf records: anchor generation,
/// stable tab/panel identities, and the record's incarnation counter.
fn pane_ref(anchor: u64, tab: u128, panel: u128, incarnation: u32) -> PaneRef {
    PaneRef::new(pane_key(anchor, tab, panel), incarnation)
}

fn context_location(value: u128) -> NativeContextLocation {
    NativeContextLocation::Standalone(
        orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&format!("{value:032x}"))
            .unwrap(),
    )
}

fn event(action: MouseAction, button: MouseButton, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        action,
        button,
        mods: crate::client_view::Modifiers::NONE,
        column,
        row,
    }
}

fn frame_facts(
    tracking: MouseTracking,
    format: MouseFormat,
    alternate_screen: bool,
) -> PointerFrameFacts {
    PointerFrameFacts {
        tracking,
        format,
        cell_pixel: (0, 0),
        alternate_screen,
        application_cursor: false,
    }
}

#[derive(Default)]
struct RecordingPort {
    forwarded: Vec<Vec<u8>>,
    actions: Vec<ClientAction>,
    probe: PointerProbe,
}

impl RecordingPort {
    fn bytes(&self) -> Vec<u8> {
        self.forwarded.concat()
    }
}

impl TerminalInputPort for RecordingPort {
    fn forward(
        &mut self,
        input: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        self.forwarded.push(input.to_vec());
        Ok(())
    }

    fn pointer_probe(&mut self) -> PointerProbe {
        self.probe.clone()
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        if let ClientAction::FocusPanel(panel) = &action {
            // Mirror the session's local focus transition: the desired
            // pane becomes the clicked panel's record and, when that
            // record is already attached, focus lands on its exact
            // incarnation synchronously — no attach round trip.
            if let Some(leaf) = self
                .probe
                .panes
                .iter()
                .find(|pane| pane.record.pane().panel() == panel && pane.attached)
                .cloned()
            {
                self.probe.desired = Some(leaf.record.pane().clone());
                self.probe.focus = Some(leaf.record);
            }
        }
        self.actions.push(action);
        Some(SessionActionDisposition::KeepNavigator)
    }
}

fn press(column: u16, row: u16) -> MouseEvent {
    event(MouseAction::Press, MouseButton::Left, column, row)
}

fn now() -> Instant {
    Instant::now()
}

/// One pane record's probe facts. `attached`/`controlling`/
/// `frame_available` are the live readings the pending settler requires.
fn pane_leaf(
    anchor: u64,
    tab: u128,
    panel: u128,
    incarnation: u32,
    attached: bool,
    controlling: Option<bool>,
    frame_available: bool,
) -> PointerPane {
    PointerPane {
        record: pane_ref(anchor, tab, panel, incarnation),
        attached,
        controlling,
        frame_available,
    }
}

/// A probe whose focus rests on the pane at `(tab_index, panel_index)`:
/// anchor generation 1, incarnation 0, all leaves attached under a
/// Controlling mode with a usable frame.
fn active_probe(tab_index: usize, panel_index: usize, tabs: usize, panels: usize) -> PointerProbe {
    let focus = pane_ref(
        1,
        tab_index as u128 + 1,
        (tab_index * 10 + panel_index) as u128 + 1,
        0,
    );
    PointerProbe {
        input_ready: true,
        focus: Some(focus.clone()),
        desired: Some(focus.pane().clone()),
        panes: (0..tabs)
            .flat_map(|tab| {
                (0..panels).map(move |panel| {
                    pane_leaf(
                        1,
                        tab as u128 + 1,
                        (tab * 10 + panel) as u128 + 1,
                        0,
                        true,
                        Some(true),
                        true,
                    )
                })
            })
            .collect(),
        tabs: (0..tabs)
            .map(|index| PointerTab {
                id: tab_id(index as u128 + 1),
                position: index + 1,
            })
            .collect(),
        frame: Some(frame_facts(MouseTracking::None, MouseFormat::Sgr, false)),
        ..PointerProbe::default()
    }
}

/// Replaces the probe's leaf facts for one logical pane — how a test
/// re-mints an incarnation, flips attachment, or changes the mode.
fn set_leaf(probe: &mut PointerProbe, leaf: PointerPane) {
    probe
        .panes
        .retain(|pane| pane.record.pane() != leaf.record.pane());
    probe.panes.push(leaf);
}

#[test]
fn hit_regions_resolve_layer_order_and_dead_space() {
    let mut builder = HitMapBuilder::new();
    builder.record(
        HitLayer::Foreground,
        Rect::new(0, 0, 10, 1),
        HitTarget::Tab(tab_id(1)),
    );
    builder.record(
        HitLayer::Foreground,
        Rect::new(0, 2, 10, 4),
        HitTarget::Pane {
            pane: pane_ref(1, 1, 1, 0),
            content: Rect::new(0, 3, 10, 3),
        },
    );
    let map = builder.finish();
    assert!(matches!(
        map.hit(5, 0),
        HitResolution::Region(HitTarget::Tab(_))
    ));
    assert!(matches!(
        map.hit(5, 3),
        HitResolution::Region(HitTarget::Pane { .. })
    ));
    assert!(matches!(map.hit(30, 30), HitResolution::Dead));
}

#[test]
fn overlay_swallows_inside_click_and_backdrop_dismisses() {
    let mut builder = HitMapBuilder::new();
    builder.mark_overlay_active();
    builder.record(
        HitLayer::OverlaySurface,
        Rect::new(4, 2, 20, 6),
        HitTarget::OverlaySurface,
    );
    builder.record(
        HitLayer::Overlay,
        Rect::new(4, 3, 20, 1),
        HitTarget::Context(context_location(7)),
    );
    builder.record(
        HitLayer::Foreground,
        Rect::new(0, 0, 40, 1),
        HitTarget::Tab(tab_id(1)),
    );
    let map = builder.finish();
    assert!(matches!(
        map.hit(10, 4),
        HitResolution::Region(HitTarget::OverlaySurface)
    ));
    assert!(matches!(map.hit(0, 20), HitResolution::OverlayBackdrop));
    // The tab strip the overlay does not cover is still backdrop: an
    // outside press dismisses, the foreground region never resolves.
    assert!(matches!(map.hit(10, 0), HitResolution::OverlayBackdrop));
    assert!(matches!(
        map.hit(10, 3),
        HitResolution::Region(HitTarget::Context(_))
    ));
    // Without an active overlay the same cell is ordinary foreground.
    let mut plain = HitMapBuilder::new();
    plain.record(
        HitLayer::Foreground,
        Rect::new(0, 0, 40, 1),
        HitTarget::Tab(tab_id(1)),
    );
    let plain = plain.finish();
    assert!(matches!(
        plain.hit(10, 0),
        HitResolution::Region(HitTarget::Tab(_))
    ));
}

/// Opens the help overlay through the real prefix decision path.
fn open_help_overlay(view: &mut ClientView) {
    let armed = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('b'),
        Modifiers::CTRL,
    )));
    view.commit(armed, SessionActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
    let opened = view.decide(PrefixInput::Key(KeyEvent::press(
        KeyCode::Char('?'),
        Modifiers::NONE,
    )));
    view.commit(opened, SessionActionDisposition::KeepNavigator);
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
}

#[test]
fn overlay_backdrop_press_dismisses_without_selecting_or_forwarding() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    open_help_overlay(&mut view);
    let mut port = RecordingPort {
        probe: active_probe(0, 0, 1, 2),
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.mark_overlay_active();
        builder.record(
            HitLayer::OverlaySurface,
            Rect::new(4, 2, 20, 6),
            HitTarget::OverlaySurface,
        );
        // Foreground chrome the overlay does not cover: the tab strip and
        // both panes' content.
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 40, 1),
            HitTarget::Tab(tab_id(1)),
        );
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 8, 40, 10),
            HitTarget::Pane {
                pane: pane_ref(1, 1, 1, 0),
                content: Rect::new(0, 9, 40, 9),
            },
        );
        builder.finish()
    });

    // A press on the uncovered tab strip is the dismiss-only backdrop:
    // the overlay closes, no SelectTab applies, nothing reaches the pane.
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(10, 0))
            .handled()
    );
    assert_eq!(view.overlay(), None);
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());

    // Same for the uncovered pane region: dismiss, never a focus switch or
    // a forwarded report.
    open_help_overlay(&mut view);
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(10, 10))
            .handled()
    );
    assert_eq!(view.overlay(), None);
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
}

#[test]
fn pointer_released_capture_gates_every_event() {
    let mut pointer = PointerState::default();
    let mut view = ClientView::new(120);
    let mut port = RecordingPort::default();
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 1),
            HitTarget::Tab(tab_id(1)),
        );
        builder.finish()
    });
    // Released: even a strip press does nothing.
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 0))
            .handled()
    );
    assert!(port.actions.is_empty());
    pointer.set_capture(true);
    port.probe = active_probe(0, 0, 1, 1);
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(2, 0))
            .handled()
    );
    assert_eq!(port.actions, vec![ClientAction::SelectTab(1)]);
}

/// Publishes a map whose pane region is the pane at `(tab 1, panel 2)`,
/// anchor generation 1, incarnation 0.
fn publish_two_panel_map(pointer: &mut PointerState) {
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 6),
            HitTarget::Pane {
                pane: pane_ref(1, 1, 2, 0),
                content: Rect::new(0, 1, 10, 5),
            },
        );
        builder.finish()
    });
}

#[test]
fn pane_press_focuses_sibling_locally_by_stable_panel_identity() {
    // A press on an attached sibling pane issues `FocusPanel` carrying the
    // *stable panel identity* — never a positional index — and the local
    // transition settles the press in place: no Pending, no attach.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, true)),
            ..active_probe(0, 0, 1, 2)
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    // Press inside pane (tab 1, panel 2)'s content while pane (1, 1) is
    // focused.
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert_eq!(port.actions, vec![ClientAction::FocusPanel(panel_id(2))]);
    // The local transition installed focus on the recorded incarnation;
    // a current frame settles the press immediately — never a Pending.
    assert!(matches!(pointer.gesture, Gesture::Pane(_)));
    assert_eq!(port.bytes(), b"\x1b[<0;3;2M".to_vec());
    // The release forwards through the same gesture.
    assert!(
        pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Release, MouseButton::Left, 2, 2)
            )
            .handled()
    );
    assert!(port.bytes().ends_with(b"\x1b[<0;3;2m"));
}

#[test]
fn pane_press_on_reminted_incarnation_drops_without_focus() {
    // The region recorded incarnation 0; the record re-minted to
    // incarnation 1 before the press. The click belongs to the dead
    // incarnation: no focus action, no forwarding, no migration.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 1, true, Some(true), true));
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
    assert!(matches!(pointer.gesture, Gesture::Idle));
}

#[test]
fn pane_press_on_readonly_pane_never_steals_focus() {
    // A settled ReadOnly verdict on the target pane rejects the press
    // outright: no FocusPanel action leaves the anchor where it was and
    // no input reaches the pane.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(false), true));
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
    assert!(matches!(pointer.gesture, Gesture::Idle));
}

#[test]
fn pane_press_on_unattached_pane_never_steals_focus() {
    // A pane whose record has no live attachment cannot take a local
    // focus — the planner would refuse it; the pointer rejects the press
    // without issuing the action at all.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, false, Some(true), false));
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
}

#[test]
fn pane_press_waits_for_first_frame_then_flushes_queued_input() {
    // The local focus installs on the exact recorded incarnation but its
    // projection has no usable frame yet: the press queues until the
    // frame lands — frame availability alone is sufficient to settle.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(true), false));
            probe.frame = None;
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert_eq!(port.actions, vec![ClientAction::FocusPanel(panel_id(2))]);
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    assert!(port.forwarded.is_empty());
    // The incarnation's first frame admits the queue — the press flushes
    // as an SGR report at pane-local (3, 2).
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 2, 0, true, Some(true), true),
    );
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, true));
    assert!(pointer.tick(&mut view, &mut port, now()));
    assert_eq!(port.bytes(), b"\x1b[<0;3;2M".to_vec());
    assert!(matches!(pointer.gesture, Gesture::Pane(_)));
}

#[test]
fn pending_focus_orphans_when_incarnation_is_reminted() {
    // A pending press must never migrate onto a fresh incarnation of the
    // same (anchor, panel): once the record re-mints, the queued press is
    // orphaned and dropped — nothing reaches the replacement.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(true), false));
            probe.frame = None;
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    // The record re-mints: same logical pane, incarnation 1, focused.
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 2, 1, true, Some(true), true),
    );
    port.probe.focus = Some(pane_ref(1, 1, 2, 1));
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, true));
    pointer.tick(&mut view, &mut port, now());
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.forwarded.is_empty());
}

#[test]
fn pending_focus_orphans_when_pane_record_vanishes() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(true), false));
            probe.frame = None;
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    // The record is gone entirely — the queue orphans.
    port.probe
        .panes
        .retain(|pane| pane.record.pane().panel() != &panel_id(2));
    pointer.tick(&mut view, &mut port, now());
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.forwarded.is_empty());
}

#[test]
fn pending_focus_orphans_when_focus_settles_elsewhere() {
    // The focus moved to a different pane while the target stayed
    // attached — the press never chases a new anchor.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(true), false));
            probe.frame = None;
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    // The focus was redirected back to pane (1, 1) instead.
    port.probe.focus = Some(pane_ref(1, 1, 1, 0));
    port.probe.desired = Some(pane_key(1, 1, 1));
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 2, 0, true, Some(true), true),
    );
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, true));
    pointer.tick(&mut view, &mut port, now());
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.forwarded.is_empty());
}

#[test]
fn pending_focus_waits_for_an_unarrived_controller_mode() {
    // The settled pane reports `controlling` `None` until its mode
    // arrives — the queue waits inside the deadline; only a settled
    // `Some(false)` is ReadOnly.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(true), false));
            probe.frame = None;
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    // Mode not yet arrived, `forward`'s gate closed — keep waiting.
    set_leaf(&mut port.probe, pane_leaf(1, 1, 2, 0, true, None, true));
    port.probe.input_ready = false;
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, true));
    assert!(!pointer.tick(&mut view, &mut port, now()));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    assert!(port.forwarded.is_empty());
    assert_eq!(view.notice(), None);
    // The mode's arrival admits the queue on the next pass.
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 2, 0, true, Some(true), true),
    );
    port.probe.input_ready = true;
    assert!(pointer.tick(&mut view, &mut port, now()));
    assert_eq!(port.bytes(), b"\x1b[<0;3;2M".to_vec());
}

#[test]
fn pending_focus_on_a_settled_read_only_verdict_drops_with_notice() {
    // The mode arrives during the pending window as ReadOnly: the queue
    // drops immediately with the pointer notice instead of waiting out
    // the deadline.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, None, false));
            probe.frame = None;
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 2, 0, true, Some(false), true),
    );
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, true));
    assert!(pointer.tick(&mut view, &mut port, now()));
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.forwarded.is_empty());
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::PointerInputDropped)
    );
}

#[test]
fn pane_gesture_dies_when_incarnation_is_replaced_mid_drag() {
    // A live gesture owns the pointer only while its recorded incarnation
    // is still the focus pane and still attached. A re-mint mid-gesture
    // ends it — the release never reaches the replacement.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false)),
            ..active_probe(0, 0, 1, 1)
        },
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 6),
            HitTarget::Pane {
                pane: pane_ref(1, 1, 1, 0),
                content: Rect::new(0, 1, 10, 5),
            },
        );
        builder.finish()
    });
    pointer.event(&mut view, &mut port, now(), press(2, 3));
    assert!(matches!(pointer.gesture, Gesture::Pane(_)));
    // Re-mint: the focus moves to incarnation 1 of the same pane.
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 1, 1, true, Some(true), true),
    );
    port.probe.focus = Some(pane_ref(1, 1, 1, 1));
    assert!(
        !pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Release, MouseButton::Left, 2, 3)
            )
            .handled()
    );
    assert!(matches!(pointer.gesture, Gesture::Idle));
    // Only the original press byte exists — no release reached pane 1's
    // new incarnation.
    assert_eq!(port.bytes(), b"\x1b[<0;3;3M".to_vec());
}

#[test]
fn pane_press_on_reporting_pane_encodes_local_coordinates() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false)),
            ..active_probe(0, 0, 1, 1)
        },
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 6),
            HitTarget::Pane {
                pane: pane_ref(1, 1, 1, 0),
                content: Rect::new(0, 1, 10, 5),
            },
        );
        builder.finish()
    });
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(2, 3))
            .handled()
    );
    // No focus action — the pane was already the anchor.
    assert!(port.actions.is_empty());
    // Local cell (2, 2) → wire 1-based (3, 3).
    assert_eq!(port.bytes(), b"\x1b[<0;3;3M".to_vec());
}

#[test]
fn alternate_screen_wheel_becomes_bounded_cursor_arrows() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::None, MouseFormat::Sgr, true)),
            ..active_probe(0, 0, 1, 1)
        },
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 4, 6),
            HitTarget::Pane {
                pane: pane_ref(1, 1, 1, 0),
                content: Rect::new(0, 1, 4, 5),
            },
        );
        builder.finish()
    });
    for _ in 0..4 {
        pointer.event(
            &mut view,
            &mut port,
            now(),
            event(MouseAction::Press, MouseButton::WheelDown, 1, 2),
        );
    }
    // 4 notches = 12 down-arrows, but the flush caps at one viewport (5
    // rows) — the bounded remainder applies on later passes.
    assert_eq!(port.bytes().len(), 5 * 3);
    assert_eq!(port.bytes(), b"\x1b[B".repeat(5));
    pointer.tick(&mut view, &mut port, now());
    pointer.tick(&mut view, &mut port, now());
    pointer.tick(&mut view, &mut port, now());
    assert_eq!(port.bytes().len(), 12 * 3);
    // Direction reversal clears the pending remainder.
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Press, MouseButton::WheelUp, 1, 2),
    );
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Press, MouseButton::WheelDown, 1, 2),
    );
    assert_eq!(port.bytes().len(), 12 * 3 + 3 * 3);
}

#[test]
fn wheel_on_unfocused_pane_is_a_dead_notch() {
    // Wheel never focuses: a notch over a sibling pane drops even while
    // that pane reports the pointer.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false)),
            ..active_probe(0, 0, 1, 2)
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    assert!(
        !pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Press, MouseButton::WheelDown, 2, 2)
            )
            .handled()
    );
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
}

#[test]
fn pending_focus_overflow_and_timeout_report() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, Some(true), false));
            probe.frame = None;
            probe
        },
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    // Fill the queue past the bound — the gesture cancels with a notice.
    for _ in 0..PENDING_FOCUS_LIMIT {
        pointer.event(&mut view, &mut port, now(), press(3, 3));
    }
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::PointerInputDropped)
    );

    // The first press already installed local focus on the clicked pane —
    // a second click on it takes the focused-pane path. Move focus back
    // to the sibling so the next press is a fresh focus wait again.
    let sibling = pane_ref(1, 1, 1, 0);
    port.probe.focus = Some(sibling.clone());
    port.probe.desired = Some(sibling.pane().clone());
    // A fresh pending that never settles expires with the same notice.
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    let later = now() + PENDING_FOCUS_TIMEOUT + Duration::from_millis(1);
    assert!(pointer.tick(&mut view, &mut port, later));
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::PointerInputDropped)
    );
}

#[test]
fn map_invalidation_drops_events_until_next_draw() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: active_probe(0, 0, 1, 1),
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 1),
            HitTarget::Tab(tab_id(1)),
        );
        builder.finish()
    });
    pointer.invalidate_map();
    // Between the invalidation and the next publish, presses drop rather
    // than misrouting to a stale region.
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 0))
            .handled()
    );
    assert!(port.actions.is_empty());
}

#[test]
fn context_row_click_selects_by_stable_location() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let location = context_location(9);
    let mut port = RecordingPort {
        probe: PointerProbe {
            contexts: vec![context_location(8), location.clone()],
            ..PointerProbe::default()
        },
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.mark_overlay_active();
        builder.record(
            HitLayer::OverlaySurface,
            Rect::new(4, 2, 30, 8),
            HitTarget::OverlaySurface,
        );
        builder.record(
            HitLayer::Overlay,
            Rect::new(4, 3, 30, 1),
            HitTarget::Context(location),
        );
        builder.finish()
    });
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(10, 3))
            .handled()
    );
    // The recorded location resolved to navigation index 1.
    assert_eq!(port.actions, vec![ClientAction::SelectContextAt(1)]);
}

#[test]
fn captured_input_swallows_pointer_without_gestures() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            input_captured: true,
            ..PointerProbe::default()
        },
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 1),
            HitTarget::Tab(tab_id(1)),
        );
        builder.finish()
    });
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 0))
            .handled()
    );
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
}

#[test]
fn pixel_format_reports_bounded_notice_instead_of_coordinates() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(
                MouseTracking::Button,
                MouseFormat::SgrPixels,
                false,
            )),
            ..active_probe(0, 0, 1, 1)
        },
        ..RecordingPort::default()
    };
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 6),
            HitTarget::Pane {
                pane: pane_ref(1, 1, 1, 0),
                content: Rect::new(0, 1, 10, 5),
            },
        );
        builder.finish()
    });
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    pointer.event(&mut view, &mut port, now(), press(4, 3));
    // Nothing forwarded; the notice fired once per gesture.
    assert!(port.forwarded.is_empty());
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::PointerPixelsUnsupported)
    );
}
