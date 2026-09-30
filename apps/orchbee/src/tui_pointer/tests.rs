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

#[cfg(windows)]
fn sidebar_fixture(count: u64) -> (ClientView, FrameHitMap) {
    let mut view = ClientView::new(120);
    view.machines.available = true;
    view.machines.observe_agent_sidebar(true);
    view.machines.observe_agents(vec![(
        crate::machine_picker::Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::tests::checkpoint(
            (23..23 + count)
                .map(crate::machine_agents::tests::row)
                .collect(),
        )),
    )]);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &view,
                crate::localization::Locale::EnUs,
                None,
                &crate::tui_theme::Theme::terminal_native(),
                &mut hits,
            );
        })
        .unwrap();
    (view, hits.finish())
}

#[cfg(windows)]
#[test]
fn sidebar_gestures_require_exact_plain_left_press_and_obey_capture_and_modals() {
    for (action, button, mods) in [
        (MouseAction::Press, MouseButton::Left, Modifiers::NONE),
        (MouseAction::Press, MouseButton::Right, Modifiers::NONE),
        (MouseAction::Press, MouseButton::Middle, Modifiers::NONE),
        (MouseAction::Release, MouseButton::Left, Modifiers::NONE),
        (MouseAction::Motion, MouseButton::Left, Modifiers::NONE),
        (
            MouseAction::Press,
            MouseButton::Left,
            Modifiers::NONE.with_shift(),
        ),
        (MouseAction::Press, MouseButton::Left, Modifiers::CTRL),
    ] {
        let (mut view, map) = sidebar_fixture(2);
        let region = map
            .regions
            .iter()
            .find(|r| matches!(r.target, HitTarget::MachineAgentRow { .. }))
            .unwrap();
        let selected = match &region.target {
            HitTarget::MachineAgentRow { selected, .. } => selected.clone(),
            _ => unreachable!(),
        };
        let mut input = event(action, button, region.rect.x, region.rect.y);
        input.mods = mods;
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        pointer.publish(map.clone());
        let mut port = RecordingPort::default();
        let outcome = pointer.event(&mut view, &mut port, Instant::now(), input);
        if action == MouseAction::Press && button == MouseButton::Left && mods == Modifiers::NONE {
            assert_eq!(outcome, PointerOutcome::MachineNavigation);
            assert_eq!(view.machines.request.take(), Some(selected));
        } else {
            assert_eq!(outcome, PointerOutcome::Ignored);
            assert!(!view.machines.open);
        }
        assert!(port.actions.is_empty() && port.forwarded.is_empty());
        view.machines.open = false;
        pointer.set_capture(false);
        assert_eq!(
            pointer.event(&mut view, &mut port, Instant::now(), input),
            PointerOutcome::Ignored
        );
        pointer.set_capture(true);
        port.probe.input_captured = true;
        assert_eq!(
            pointer.event(&mut view, &mut port, Instant::now(), input),
            PointerOutcome::Ignored
        );
        assert!(!view.machines.open);
    }
}

#[cfg(windows)]
#[test]
fn sidebar_scroll_is_cadence_bounded_and_never_forwards_mouse_to_a_pane() {
    let (mut view, map) = sidebar_fixture(40);
    let region = map
        .regions
        .iter()
        .find(|r| matches!(r.target, HitTarget::MachineAgentRow { .. }))
        .unwrap();
    let window = match region.target {
        HitTarget::MachineAgentRow { window, .. } => window,
        _ => unreachable!(),
    };
    let down = event(
        MouseAction::Press,
        MouseButton::WheelDown,
        region.rect.x,
        region.rect.y,
    );
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map.clone());
    let mut port = RecordingPort::default();
    for _ in 0..10 {
        assert_eq!(
            pointer.event(&mut view, &mut port, Instant::now(), down),
            PointerOutcome::Handled
        );
    }
    assert_eq!(view.machines.sidebar_window(window.capacity).top, 0);
    assert!(pointer.tick(&mut view, &mut port, Instant::now()));
    assert_eq!(
        view.machines.sidebar_window(window.capacity).top,
        window.capacity
    );
    assert!(pointer.map.is_none());
    assert!(view.machines.request.is_none() && port.bytes().is_empty());
    pointer.set_capture(false);
    assert!(!pointer.tick(&mut view, &mut port, Instant::now()));
    let after = view.machines.sidebar_window(window.capacity);
    pointer.set_capture(true);
    assert!(!pointer.tick(&mut view, &mut port, Instant::now()));
    assert_eq!(view.machines.sidebar_window(window.capacity), after);
}

#[cfg(windows)]
#[test]
fn sidebar_chrome_occludes_panes_but_a_capturing_overlay_occludes_sidebar() {
    let (mut view, map) = sidebar_fixture(1);
    let region = map
        .regions
        .iter()
        .find(|r| matches!(r.target, HitTarget::MachineAgentRow { .. }))
        .unwrap()
        .clone();
    let mut hits = HitMapBuilder::new();
    hits.record(HitLayer::Foreground, region.rect, HitTarget::Tab(tab_id(1)));
    hits.record(HitLayer::Chrome, region.rect, region.target.clone());
    assert!(matches!(
        hits.map.hit_target_at(region.rect.x, region.rect.y),
        Some(HitTarget::MachineAgentRow { .. })
    ));
    hits.mark_overlay_active();
    assert!(
        hits.map
            .hit_target_at(region.rect.x, region.rect.y)
            .is_none()
    );
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(hits.finish());
    let mut port = RecordingPort::default();
    assert_eq!(
        pointer.event(
            &mut view,
            &mut port,
            Instant::now(),
            event(
                MouseAction::Press,
                MouseButton::Left,
                region.rect.x,
                region.rect.y
            )
        ),
        PointerOutcome::RegionsChanged
    );
    assert!(view.machines.request.is_none() && !view.machines.open && port.actions.is_empty());
}

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
        alternate_scroll: true,
        application_cursor: false,
    }
}

#[derive(Default)]
struct RecordingPort {
    forwarded: Vec<Vec<u8>>,
    actions: Vec<ClientAction>,
    probe: PointerProbe,
    wheels: Vec<(PaneRef, bool)>,
    /// Logical panes holding a non-Live history core — the FocusPanel arm
    /// recomputes the probe's `viewport_non_live` for the settled focus,
    /// the way the session's probe reads the focused core's mode.
    non_live: Vec<PaneKey>,
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

    fn history_wheel(&mut self, key: &PaneRef, up: bool) -> bool {
        self.wheels.push((key.clone(), up));
        true
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
                self.probe.viewport_non_live =
                    self.non_live.iter().any(|key| *key == *leaf.record.pane());
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
        right_click: Default::default(),
        right_click_reporting: false,
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
    // no input reaches the pane. A content press is consumed with the
    // pointer drop notice — the same negative adjudication the pending
    // ReadOnly branch publishes — while a title-row press stays Client
    // territory and drops quietly.
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
        pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::PointerInputDropped)
    );
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
    assert!(matches!(pointer.gesture, Gesture::Idle));

    // The title row is never pane input: a read-only verdict drops it
    // the same silent way an unfocused press on a live pane's title does.
    let mut view = ClientView::new(120);
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 0))
            .handled()
    );
    assert_eq!(view.notice(), None);
    assert!(port.actions.is_empty());
    assert!(matches!(pointer.gesture, Gesture::Idle));
}

#[test]
fn pane_press_on_unsettled_mode_pane_reports_drop_without_focus() {
    // An attached sibling whose controller mode has not arrived yet
    // (`None`, between Attached and SetupConfigured) cannot admit input
    // either: a content press reports the same drop notice without a
    // focus request the client could never turn into input, and the
    // title row still drops quietly. `None` is not a permission verdict
    // — the notice states a factual input drop only.
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: {
            let mut probe = active_probe(0, 0, 1, 2);
            set_leaf(&mut probe, pane_leaf(1, 1, 2, 0, true, None, true));
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
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::PointerInputDropped)
    );
    assert!(port.actions.is_empty());
    assert!(port.forwarded.is_empty());
    assert!(matches!(pointer.gesture, Gesture::Idle));

    let mut view = ClientView::new(120);
    assert!(
        !pointer
            .event(&mut view, &mut port, now(), press(2, 0))
            .handled()
    );
    assert_eq!(view.notice(), None);
    assert!(port.actions.is_empty());
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
    // A controlling pane whose frame is not ready yet queues a pending
    // focus. The mode then settles as ReadOnly during the window: the
    // queue drops immediately with the pointer notice instead of waiting
    // out the deadline.
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
fn pending_wheel_arrows_stop_when_the_latest_frame_revokes_alternate_scroll() {
    for revoked in 0..4 {
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
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 4, 6),
            HitTarget::Pane {
                pane: pane_ref(1, 1, 1, 0),
                content: Rect::new(0, 1, 4, 5),
            },
        );
        pointer.publish(builder.finish());
        for _ in 0..4 {
            pointer.event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Press, MouseButton::WheelDown, 1, 2),
            );
        }
        assert_eq!(port.bytes(), b"\x1b[B".repeat(5));
        let frame = port.probe.frame.as_mut().unwrap();
        match revoked {
            0 => frame.alternate_scroll = false,
            1 => frame.alternate_screen = false,
            2 => frame.tracking = MouseTracking::Normal,
            _ => port.probe.viewport_non_live = true,
        }
        pointer.tick(&mut view, &mut port, now());
        assert_eq!(port.bytes(), b"\x1b[B".repeat(5));
        assert_eq!(pointer.wheel.pending, 0);
        if revoked == 0 {
            pointer.event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Press, MouseButton::WheelUp, 1, 2),
            );
            assert_eq!(port.bytes(), b"\x1b[B".repeat(5));
        }
    }
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
fn pixel_format_keeps_pane_local_cell_press_drag_and_release() {
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
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Motion, MouseButton::Left, 4, 3),
    );
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 4, 3),
    );
    assert_eq!(port.bytes(), b"\x1b[<0;3;2M\x1b[<32;5;3M\x1b[<0;5;3m");
    assert_eq!(view.notice(), None);
}

/// A one-pane hit map whose pane is the focused incarnation.
fn publish_focused_map(pointer: &mut PointerState, pane: PaneRef) {
    pointer.publish({
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(0, 0, 10, 6),
            HitTarget::Pane {
                pane,
                content: Rect::new(0, 1, 10, 5),
            },
        );
        builder.finish()
    });
}

// A non-Live focus owns every non-wheel pointer event locally — even on a
// pane whose frame reports the pointer, a content press mints no gesture
// and encodes no bytes.
#[test]
fn non_live_focus_consumes_content_press_without_pane_bytes() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false)),
            viewport_non_live: true,
            ..active_probe(0, 1, 1, 2)
        },
        ..RecordingPort::default()
    };
    publish_focused_map(&mut pointer, pane_ref(1, 1, 2, 0));
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.forwarded.is_empty());
    assert!(port.actions.is_empty());
    // Motion and release while the press would have been held are dead too.
    assert!(
        pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Motion, MouseButton::Left, 4, 3)
            )
            .handled()
    );
    assert!(
        pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Release, MouseButton::Left, 4, 3)
            )
            .handled()
    );
    assert!(port.forwarded.is_empty());
}

// A non-Live focus owns the wheel before any frame contract: the notch
// reaches `history_wheel` for local scrolling and never encodes.
#[test]
fn non_live_focus_wheel_scrolls_locally() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let focused = pane_ref(1, 1, 2, 0);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false)),
            viewport_non_live: true,
            ..active_probe(0, 1, 1, 2)
        },
        ..RecordingPort::default()
    };
    publish_focused_map(&mut pointer, focused.clone());
    assert!(
        pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Press, MouseButton::WheelUp, 2, 2)
            )
            .handled()
    );
    assert!(
        pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Press, MouseButton::WheelDown, 2, 2)
            )
            .handled()
    );
    assert_eq!(
        port.wheels,
        vec![(focused.clone(), true), (focused, false)],
        "each notch scrolls the retained window locally"
    );
    assert!(port.forwarded.is_empty());
}

// A press that moves focus onto a non-Live pane still issues `FocusPanel`
// — the title row stays interactive — but the content press itself mints
// no pane gesture and no bytes.
#[test]
fn non_live_focus_move_press_consumes_without_gesture() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false)),
            ..active_probe(0, 0, 1, 2)
        },
        non_live: vec![pane_key(1, 1, 2)],
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert_eq!(port.actions, vec![ClientAction::FocusPanel(panel_id(2))]);
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.forwarded.is_empty());
}

// A gesture that began on a Live pane is consumed locally the moment the
// core goes non-Live — drags encode nothing more, and the release ends
// the gesture without a byte.
#[test]
fn non_live_transition_mid_gesture_consumes_until_release() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: PointerProbe {
            frame: Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false)),
            ..active_probe(0, 1, 1, 2)
        },
        ..RecordingPort::default()
    };
    publish_focused_map(&mut pointer, pane_ref(1, 1, 2, 0));
    assert!(
        pointer
            .event(&mut view, &mut port, now(), press(2, 2))
            .handled()
    );
    assert!(matches!(pointer.gesture, Gesture::Pane(_)));
    let live_bytes = port.bytes();
    assert_eq!(live_bytes, b"\x1b[<0;3;2M".to_vec());
    // The pane enters history mid-drag: the core flips non-Live.
    port.probe.viewport_non_live = true;
    assert!(
        pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Motion, MouseButton::Left, 4, 3)
            )
            .handled()
    );
    assert_eq!(
        port.bytes(),
        live_bytes,
        "no drag bytes after the transition"
    );
    assert!(
        pointer
            .event(
                &mut view,
                &mut port,
                now(),
                event(MouseAction::Release, MouseButton::Left, 4, 3)
            )
            .handled()
    );
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert_eq!(port.bytes(), live_bytes, "the release encodes nothing");
}

// A focus-move press that queued for its first frame settles onto a
// non-Live core as a dead gesture — the queued press is consumed, never
// replayed as pane bytes.
#[test]
fn non_live_pending_settlement_consumes_queue() {
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
        non_live: vec![pane_key(1, 1, 2)],
        ..RecordingPort::default()
    };
    publish_two_panel_map(&mut pointer);
    pointer.event(&mut view, &mut port, now(), press(2, 2));
    assert!(matches!(pointer.gesture, Gesture::Pending(_)));
    // The first frame lands while the pane has already gone non-Live.
    set_leaf(
        &mut port.probe,
        pane_leaf(1, 1, 2, 0, true, Some(true), true),
    );
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false));
    pointer.tick(&mut view, &mut port, now());
    assert!(matches!(pointer.gesture, Gesture::Idle));
    assert!(port.forwarded.is_empty(), "the queued press never replays");
}

#[test]
fn right_click_policy_admits_only_unmodified_reporting_content_and_keeps_captured_release() {
    use orchbee_workbench_protocol::PanelRightClickTarget;
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: active_probe(0, 0, 1, 1),
        ..RecordingPort::default()
    };
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false));
    publish_focused_map(&mut pointer, pane_ref(1, 1, 1, 0));
    let right = event(MouseAction::Press, MouseButton::Right, 3, 4);
    pointer.event(&mut view, &mut port, now(), right);
    assert!(port.forwarded.is_empty() && port.actions.is_empty());
    port.probe.panes[0].right_click = PanelRightClickTarget::Panel;
    pointer.event(&mut view, &mut port, now(), right);
    assert!(port.forwarded.is_empty());
    port.probe.panes[0].right_click_reporting = true;
    for mods in [
        Modifiers::NONE.with_shift(),
        Modifiers::CTRL,
        Modifiers::NONE.with_alt(),
    ] {
        pointer.event(&mut view, &mut port, now(), MouseEvent { mods, ..right });
        assert!(port.forwarded.is_empty() && port.actions.is_empty());
    }
    pointer.event(&mut view, &mut port, now(), right);
    assert_eq!(port.forwarded.len(), 1);
    assert!(matches!(pointer.gesture, Gesture::Pane(_)));
    port.probe.panes[0].right_click = PanelRightClickTarget::Orchbee;
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 3, 4),
    );
    assert_eq!(port.forwarded.len(), 1);
    assert!(matches!(pointer.gesture, Gesture::Pane(_)));
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::None, 3, 4),
    );
    assert_eq!(port.forwarded.len(), 2);
    assert!(port.forwarded[0].starts_with(b"\x1b[<2;"));
    assert!(port.forwarded[1].starts_with(b"\x1b[<2;") && port.forwarded[1].ends_with(b"m"));
    assert!(matches!(pointer.gesture, Gesture::Idle));
    pointer.event(&mut view, &mut port, now(), right);
    assert_eq!(port.forwarded.len(), 2);
}

#[test]
fn another_button_cannot_bypass_right_click_policy_or_release_a_left_drag() {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    let mut view = ClientView::new(120);
    let mut port = RecordingPort {
        probe: active_probe(0, 0, 1, 1),
        ..RecordingPort::default()
    };
    port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false));
    publish_focused_map(&mut pointer, pane_ref(1, 1, 1, 0));
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Press, MouseButton::Left, 3, 4),
    );
    for action in [
        MouseAction::Press,
        MouseAction::Motion,
        MouseAction::Release,
    ] {
        pointer.event(
            &mut view,
            &mut port,
            now(),
            event(action, MouseButton::Right, 3, 4),
        );
    }
    assert_eq!(port.forwarded.len(), 1);
    assert!(matches!(pointer.gesture, Gesture::Pane(_)));
    pointer.event(
        &mut view,
        &mut port,
        now(),
        event(MouseAction::Release, MouseButton::Left, 3, 4),
    );
    assert_eq!(port.forwarded.len(), 2);
    assert!(matches!(pointer.gesture, Gesture::Idle));
}

#[test]
fn queued_right_press_rechecks_policy_before_any_bytes_and_stops_at_its_release() {
    use orchbee_workbench_protocol::PanelRightClickTarget;
    for policy in [PanelRightClickTarget::Orchbee, PanelRightClickTarget::Panel] {
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        let mut view = ClientView::new(120);
        let mut port = RecordingPort {
            probe: active_probe(0, 0, 1, 1),
            ..RecordingPort::default()
        };
        port.probe.panes[0].right_click = policy;
        port.probe.panes[0].right_click_reporting = true;
        port.probe.frame = Some(frame_facts(MouseTracking::Button, MouseFormat::Sgr, false));
        pointer.gesture = Gesture::Pending(PendingFocus {
            button: MouseButton::Right,
            expected: pane_ref(1, 1, 1, 0),
            viewport: Rect::new(0, 0, 20, 10),
            deadline: now() + PENDING_FOCUS_TIMEOUT,
            queued: VecDeque::from([
                event(MouseAction::Press, MouseButton::Right, 3, 4),
                event(MouseAction::Release, MouseButton::Left, 3, 4),
                event(MouseAction::Release, MouseButton::Right, 3, 4),
                event(MouseAction::Press, MouseButton::Right, 3, 4),
            ]),
        });
        pointer.tick(&mut view, &mut port, now());
        assert_eq!(
            port.forwarded.len(),
            if policy == PanelRightClickTarget::Panel {
                2
            } else {
                0
            }
        );
        assert!(matches!(pointer.gesture, Gesture::Idle));
    }
}

#[test]
fn primary_wheel_enters_history_only_over_focused_content_without_shift() {
    for (history, shift, row, expected) in [
        (false, false, 2, true),
        (true, false, 2, true),
        (false, true, 2, false),
        (true, true, 2, false),
        (false, false, 0, false),
        (true, false, 0, false),
    ] {
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        let mut view = ClientView::new(120);
        let focused = pane_ref(1, 1, 2, 0);
        let mut port = RecordingPort {
            probe: PointerProbe {
                frame: Some(frame_facts(
                    MouseTracking::None,
                    MouseFormat::Default,
                    false,
                )),
                viewport_non_live: history,
                ..active_probe(0, 1, 1, 2)
            },
            ..RecordingPort::default()
        };
        publish_focused_map(&mut pointer, focused.clone());
        let mut wheel = event(MouseAction::Press, MouseButton::WheelUp, 2, row);
        if shift {
            wheel.mods = Modifiers::NONE.with_shift();
        }
        pointer.event(&mut view, &mut port, now(), wheel);
        assert_eq!(
            port.wheels,
            if expected {
                vec![(focused, true)]
            } else {
                vec![]
            }
        );
        assert!(port.forwarded.is_empty());
        assert!(port.actions.is_empty(), "wheel never changes focus");
        port.wheels.clear();
        pointer.event(&mut view, &mut port, now(), press(2, 2));
        assert!(
            port.wheels.is_empty(),
            "ordinary press never enters history"
        );
    }
}

#[test]
fn presentation_click_is_exact_unmodified_left_only_and_never_terminal_input() {
    let presentation =
        orchbee_workbench_protocol::TerminalPresentationId::parse(&"9".repeat(32)).unwrap();
    let expected = pane_ref(1, 1, 1, 0);
    for scenario in 0..12 {
        let mut builder = HitMapBuilder::new();
        builder.record(
            HitLayer::Foreground,
            Rect::new(4, 2, 3, 1),
            HitTarget::Presentation {
                pane: expected.clone(),
                presentation: presentation.clone(),
            },
        );
        if scenario == 8 {
            builder.mark_overlay_active();
        }
        let mut pointer = PointerState::default();
        pointer.set_capture(scenario != 9);
        pointer.publish(builder.finish());
        let mut port = RecordingPort {
            probe: active_probe(0, 0, 2, 1),
            ..Default::default()
        };
        let mut input = press(5, 2);
        match scenario {
            1 => input.action = MouseAction::Release,
            2 => input.button = MouseButton::Right,
            3 => input.button = MouseButton::WheelUp,
            4 => input.mods.shift = true,
            5 => port.probe.panes[0].record = pane_ref(1, 1, 1, 1),
            6 => port.probe.desired = Some(pane_key(2, 1, 1)),
            7 => port.probe.input_captured = true,
            10 => port.probe.desired = Some(pane_key(1, 2, 11)),
            11 => port.probe.panes.clear(),
            _ => {}
        }
        let mut view = ClientView::new(120);
        pointer.event(&mut view, &mut port, now(), input);
        if scenario == 0 {
            assert_eq!(
                port.actions,
                vec![ClientAction::SelectPresentation {
                    panel: panel_id(1),
                    presentation: presentation.clone()
                }]
            );
            pointer.stale_map();
            assert_eq!(
                pointer.event(&mut view, &mut port, now(), input),
                PointerOutcome::Ignored
            );
            assert_eq!(port.actions.len(), 1);
        } else {
            assert!(port.actions.is_empty(), "scenario {scenario}");
        }
        assert!(port.forwarded.is_empty() && port.wheels.is_empty());
    }
}

#[test]
fn unbound_clicks_require_exact_empty_member_and_never_forward_input() {
    let id = orchbee_workbench_protocol::TerminalPresentationId::parse(&"8".repeat(32)).unwrap();
    let destination =
        orchbee_workbench_protocol::TerminalPresentationId::parse(&"9".repeat(32)).unwrap();
    for member_arrow in [false, true] {
        for scenario in 0..7 {
            let selected = crate::terminal_input::PointerUnboundPane {
                key: pane_key(1, 1, 1),
                presentation: id.clone(),
            };
            let mut builder = HitMapBuilder::new();
            builder.record(
                HitLayer::Foreground,
                Rect::new(0, 0, 4, 4),
                HitTarget::Unbound {
                    selected: selected.clone(),
                    presentation: member_arrow.then(|| destination.clone()),
                },
            );
            let mut pointer = PointerState::default();
            pointer.set_capture(true);
            pointer.publish(builder.finish());
            let mut port = RecordingPort {
                probe: PointerProbe {
                    desired: Some(selected.key.clone()),
                    unbound: vec![selected],
                    ..PointerProbe::default()
                },
                ..RecordingPort::default()
            };
            let mut input = press(1, 1);
            match scenario {
                1 => port.probe.unbound.clear(),
                2 => port.probe.unbound[0].presentation = destination.clone(),
                3 => port.probe.desired = Some(pane_key(2, 1, 1)),
                4 => input.button = MouseButton::Right,
                5 => input.mods = Modifiers::NONE.with_shift(),
                6 => input.action = MouseAction::Release,
                _ => {}
            }
            pointer.event(&mut ClientView::new(120), &mut port, now(), input);
            assert_eq!(port.actions.len(), usize::from(scenario == 0));
            if scenario == 0 {
                assert_eq!(
                    port.actions[0],
                    if member_arrow {
                        ClientAction::SelectPresentation {
                            panel: panel_id(1),
                            presentation: destination.clone(),
                        }
                    } else {
                        ClientAction::FocusPanel(panel_id(1))
                    }
                );
            }
            assert!(port.forwarded.is_empty() && port.wheels.is_empty());
        }
    }
}
