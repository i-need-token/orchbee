//! Client-local per-Tab zoom witnesses (`Ctrl+B z`): the volatile flag's
//! guards and no-op cases, the focus-follow drawn set that never detaches
//! viewers, the latest-known-membership GC at both install points, and the
//! observed-geometry split that keeps hidden leaves alive but unresized.
use super::context_selection::{contexts, finish_selection};
use super::*;
use crate::client_view::{ClientView, PanelDirection};
use crate::localization::Locale;
use crate::terminal_device::WindowDimensions;
use crate::terminal_projection::TerminalProjection;
use crate::terminal_session::ui::KeyedProjection;
use crate::terminal_session::{NativeSessionView, TerminalSessionView};
use crate::tui_pointer::{HitMapBuilder, HitTarget};
use crate::tui_shell::pane_geometries;
use orchbee_terminal_protocol::{
    Envelope, encode_transport_envelope, inspect_terminal_frame_route,
};
use ratatui::{Terminal, backend::TestBackend};

/// The shared ghostty surface fixture: a real encoded terminal frame the
/// projections can receive so the zoomed paint has live cells to draw.
const SNAPSHOT: &[u8] = include_bytes!(
    "../../../../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
);

/// A projection carrying the fixture frame — the live-paint stand-in the
/// renderer's single-pane path requires.
fn framed_projection() -> TerminalProjection {
    let route = inspect_terminal_frame_route(SNAPSHOT).unwrap();
    let record = encode_transport_envelope(&Envelope {
        protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
        role: orchbee_terminal_protocol::MessageRole::Event,
        kind: 41,
        payload_encoding: orchbee_terminal_protocol::PayloadEncoding::TerminalFrame,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: SNAPSHOT,
    })
    .unwrap();
    let mut projection = TerminalProjection::new(route.attachment_id, route.host_epoch);
    assert!(matches!(projection.receive(&record), Ok(Some(_))));
    projection
}

fn zoom_ready() -> NativeSessionState {
    ready_with_cache(project_native_standalone_tabs(&split_panel_baseline()).unwrap())
}

fn split_two_tab_ready() -> NativeSessionState {
    let mut baseline = split_panel_baseline();
    baseline.terminal_contexts[0].tabs.push(tab(1));
    ready_with_cache(project_native_standalone_tabs(&baseline).unwrap())
}

fn deadlines() -> OperationDeadlines {
    OperationDeadlines::from_sample(Instant::now()).unwrap()
}

/// A three-context snapshot whose Workspace context (id 72) holds one
/// two-leaf Tab: the departure/retention fixture for zoom flags riding a
/// retained context snapshot.
fn split_contexts() -> Snapshot {
    let mut snapshot = contexts(1, 1);
    let original = snapshot.terminal_contexts[1].tabs[0].root.clone();
    snapshot.terminal_contexts[1].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: original,
                slot_id: WorkbenchPanelSlotId::parse(&id(6_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: panel_id(6_010),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: terminal_id(6_012),
                            worker_id: worker_id(6_011),
                        }),
                        label: "peer".into(),
                        presentation_id: presentation_id(6_013),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&id(6_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&id(6_020)).unwrap(),
    };
    snapshot
}

/// Drive the fixture into context 72 (the Workspace holding the split
/// Tab), one `start_context_selection`/`finish_selection` round away from
/// the standalone context the cache opens on.
fn into_split_context(state: &mut NativeSessionState, snapshot: &Snapshot) {
    assert!(state.start_context_selection(1, true, deadlines()));
    finish_selection(state, snapshot.clone(), 1);
    assert_eq!(state.cache().terminal_context_id(), &context_id(72));
    assert_eq!(state.tab(0).unwrap().panel_count(), 2);
}

#[test]
fn zoom_toggle_round_trips_on_the_active_multi_leaf_tab() {
    let mut state = zoom_ready();
    let tab_id = state.tab(0).unwrap().tab_id().clone();
    assert!(state.toggle_zoom());
    assert!(state.zoom_active());
    assert!(state.zoomed_tabs.contains(&tab_id));
    assert!(state.toggle_zoom());
    assert!(!state.zoom_active());
    assert!(state.zoomed_tabs.is_empty());
    assert!(state.invariant_holds());
}

#[test]
fn a_single_leaf_active_tab_is_an_honest_noop() {
    let mut state = ready(1);
    assert!(!state.toggle_zoom());
    assert!(!state.zoom_active());
    assert!(state.zoomed_tabs.is_empty());
    assert!(state.invariant_holds());
}

#[test]
fn busy_phases_reject_the_toggle_without_mutating_state() {
    // A pending refresh is one shared focus guard.
    let mut state = zoom_ready();
    assert!(state.start_pending_refresh(7));
    assert!(!state.toggle_zoom());
    assert!(state.zoomed_tabs.is_empty());
    assert!(state.invariant_holds());

    // A structural phase (Create in flight) is another: the flag must not
    // exist to collect later, so the rejected toggle mutates nothing.
    let mut state = zoom_ready();
    let operation = create_operation(&state, 2, 20, "2");
    assert!(state.begin_create(operation));
    assert!(!state.toggle_zoom());
    assert!(state.zoomed_tabs.is_empty());
    assert!(state.invariant_holds());
}

#[test]
fn zoom_follows_the_focus_move_without_detaching_viewers() {
    let mut state = zoom_ready();
    let peer = attach_nonfirst_leaf(&mut state);
    let focus = state.focus_pane_ref().unwrap();
    assert!(state.toggle_zoom());
    let pane = state.plan_panel_focus_local(PanelDirection::Right).unwrap();
    assert_eq!(&pane, peer.pane());
    assert!(state.move_panel_focus_local(pane));
    assert_eq!(state.focus_pane_ref(), Some(peer.clone()));
    // Both incarnations stay live and attached; the flag simply re-points
    // the drawn leaf.
    assert!(state.pane_attached(&focus));
    assert!(state.pane_attached(&peer));
    assert!(state.zoom_active());
    assert!(state.invariant_holds());
}

#[test]
fn hidden_leaf_geometry_is_never_admitted_for_resize() {
    let mut state = zoom_ready();
    let peer = attach_nonfirst_leaf(&mut state);
    let focus = state.focus_pane_ref().unwrap();
    let focus_panel = state.focus_panel_id().unwrap().clone();
    assert!(state.toggle_zoom());
    // The observation still reports every leaf, but resize admission is
    // focus-scoped: the hidden leaf's fresh report admits nothing while
    // the focused leaf's whole-area report takes the ordinary path.
    let resizes = state.admit_observed_geometries(&[
        (focus_panel, 0, geometry(96)),
        (peer.pane().panel().clone(), 1, geometry(96)),
    ]);
    assert_eq!(resizes, vec![(focus, geometry(96))]);
    assert!(state.invariant_holds());
}

#[test]
fn the_flag_survives_a_same_context_tab_switch() {
    let mut state = split_two_tab_ready();
    let zoomed_id = state.tab(0).unwrap().tab_id().clone();
    assert!(state.toggle_zoom());

    let key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state
        .target_for_switch(1, key.clone(), deadlines())
        .unwrap();
    assert!(state.begin_switch(target));
    install_switch_candidate(&mut state, &key);
    assert_eq!(state.active().unwrap().index(), 1);
    assert!(!state.zoom_active());
    assert!(state.zoomed_tabs.contains(&zoomed_id));

    // The installed switch re-mints the anchor one generation up, so the
    // return leg must clear it.
    let key = AttachmentKey::new(4, zoomed_id);
    let target = state
        .target_for_switch(0, key.clone(), deadlines())
        .unwrap();
    assert!(state.begin_switch(target));
    install_switch_candidate(&mut state, &key);
    assert_eq!(state.active().unwrap().index(), 0);
    assert!(state.zoom_active());
    assert!(state.invariant_holds());
}

#[test]
fn shrinking_to_a_single_leaf_collects_the_flag() {
    let mut state = zoom_ready();
    assert!(state.toggle_zoom());
    // The peer leaf vanishes from canonical truth at the next refresh.
    assert!(state.start_pending_refresh(7));
    let prepared = state.prepare_refresh(
        7,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(1, 2, None),
        },
    );
    assert!(matches!(prepared, PreparedRefresh::Append { .. }));
    assert_eq!(
        state.apply_prepared_refresh(prepared, Instant::now()),
        RefreshDecision::Continue
    );
    assert_eq!(state.tab(0).unwrap().panel_count(), 1);
    assert!(state.zoomed_tabs.is_empty());
    assert!(!state.zoom_active());
    assert!(state.invariant_holds());
}

#[test]
fn deleting_a_zoomed_background_tab_collects_the_flag() {
    let mut state = split_two_tab_ready();
    let zoomed_id = state.tab(0).unwrap().tab_id().clone();
    assert!(state.toggle_zoom());
    // Move to the sibling Tab, leaving the flag on a background Tab — an
    // active-Tab deletion retires the whole session instead of running GC.
    let key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state
        .target_for_switch(1, key.clone(), deadlines())
        .unwrap();
    assert!(state.begin_switch(target));
    let fresh = install_switch_candidate(&mut state, &key);
    // The refresh path needs a Ready phase: complete the switched
    // attachment's projection/configuration/frame handshake.
    assert!(state.install_projection_t(&fresh));
    assert!(state.record_setup_configuration_t(&fresh, controlling(), Some(geometry(80))));
    assert!(state.stage_drawn_frame_t(&fresh, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&fresh, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(
        &fresh,
        FrameIdentity::new(1),
        controlling(),
        Some(geometry(80))
    ));
    assert_eq!(state.phase_kind(), SessionPhaseKind::Ready);
    assert_eq!(state.active().unwrap().index(), 1);
    assert!(state.zoomed_tabs.contains(&zoomed_id));

    // The next refresh drops the zoomed background Tab while the active
    // Tab's identity survives — the flag is collected at install.
    assert!(state.start_pending_refresh(7));
    let mut candidate = snapshot(1, 2, None);
    candidate.terminal_contexts[0].tabs = vec![tab(1)];
    let prepared = state.prepare_refresh(
        7,
        WorkbenchSnapshotOutcome::Available {
            snapshot: candidate,
        },
    );
    assert_eq!(
        state.apply_prepared_refresh(prepared, Instant::now()),
        RefreshDecision::Continue
    );
    assert_eq!(state.tab(0).unwrap().tab_id(), &tab_id(1_010));
    assert!(state.zoomed_tabs.is_empty());
    assert!(!state.zoom_active());
    assert!(state.invariant_holds());
}

#[test]
fn a_context_round_trip_preserves_the_flag_through_retained_truth() {
    let snapshot = split_contexts();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    into_split_context(&mut state, &snapshot);
    let zoomed_id = state.tab(0).unwrap().tab_id().clone();
    assert!(state.toggle_zoom());
    assert!(state.zoom_active());

    // Departure keeps the flag on the retained snapshot's last-known
    // membership — it cannot paint on another context's cache.
    assert!(state.start_context_selection(2, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 2);
    assert_eq!(state.cache().terminal_context_id(), &context_id(74));
    assert!(!state.zoom_active());
    assert!(state.zoomed_tabs.contains(&zoomed_id));

    // The return install restores the same truth: the flag paints again.
    assert!(state.start_context_selection(3, false, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 3);
    assert_eq!(state.cache().terminal_context_id(), &context_id(72));
    assert!(state.zoom_active());
    assert!(state.invariant_holds());
}

#[test]
fn a_stale_retained_flag_drops_at_the_next_install() {
    let snapshot = split_contexts();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    into_split_context(&mut state, &snapshot);
    let zoomed_id = state.tab(0).unwrap().tab_id().clone();
    assert!(state.toggle_zoom());
    assert!(state.start_context_selection(2, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 2);
    // The flag survives on the departed context's retained truth.
    assert!(state.zoomed_tabs.contains(&zoomed_id));

    // The retained snapshot still says two leaves, but the return
    // candidate's fresh truth shrank the Tab to one — the flag is
    // collected at install, never lazily at read time.
    let shrunk = contexts(1, 2);
    assert!(matches!(
        shrunk.terminal_contexts[1].tabs[0].root,
        PanelNode::Panel { .. }
    ));
    assert!(state.start_context_selection(3, false, deadlines()));
    finish_selection(&mut state, shrunk, 3);
    assert_eq!(state.cache().terminal_context_id(), &context_id(72));
    assert!(state.zoomed_tabs.is_empty());
    assert!(!state.zoom_active());
    assert!(state.invariant_holds());
}

#[test]
fn navigation_removal_evicts_the_retained_flag() {
    let snapshot = split_contexts();
    let mut state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    into_split_context(&mut state, &snapshot);
    assert!(state.toggle_zoom());
    assert!(state.start_context_selection(2, true, deadlines()));
    finish_selection(&mut state, snapshot.clone(), 2);

    // The next departure's candidate drops the zoomed context from
    // canonical navigation: its retained snapshot is recycled at the same
    // install and the flag's last-known membership becomes zero leaves.
    // The revision must advance — same-revision different-content is
    // divergent publication, not an install.
    let mut shrunk = contexts(1, 2);
    shrunk.navigation.projects[0]
        .workspaces
        .retain(|workspace| workspace.terminal_context_id != context_id(72));
    shrunk
        .terminal_contexts
        .retain(|context| context.terminal_context_id != context_id(72));
    assert!(state.start_context_selection(3, true, deadlines()));
    finish_selection(&mut state, shrunk, 3);
    assert_eq!(state.cache().terminal_context_id(), &context_id(2));
    assert!(state.zoomed_tabs.is_empty());
    assert!(!state.zoom_active());
    assert!(state.invariant_holds());
}

/// The drawn-set witness: under zoom the focused leaf paints across the
/// whole content area and records the only pane hit region; every leaf —
/// including the hidden one — still reports an observed geometry.
#[test]
fn zoomed_paint_narrows_the_drawn_set_but_not_the_observed_set() {
    let mut state = zoom_ready();
    let peer = attach_nonfirst_leaf(&mut state);
    let focus = state.focus_pane_ref().unwrap();
    let focus_panel = state.focus_panel_id().unwrap().clone();
    let unzoomed_panes = [
        KeyedProjection::for_test(
            focus.clone(),
            framed_projection(),
            crate::terminal_session::NativeProjectionStatus::FrameAvailable,
        ),
        KeyedProjection::for_test(
            peer.clone(),
            framed_projection(),
            crate::terminal_session::NativeProjectionStatus::FrameAvailable,
        ),
    ];
    let unzoomed_view = NativeSessionView::test_view(
        &state,
        &unzoomed_panes,
        crate::terminal_session::NativeProjectionStatus::FrameAvailable,
    );
    let client_view = ClientView::new(120);
    let dimensions = WindowDimensions {
        columns: 120,
        rows: 34,
        pixel_width: 1200,
        pixel_height: 1020,
    };
    let unzoomed = pane_geometries(dimensions, &client_view, &unzoomed_view).unwrap();
    assert_eq!(unzoomed.len(), 2);
    let (focused_report, hidden_report) = (
        unzoomed
            .iter()
            .find(|(panel, _, _)| panel == &focus_panel)
            .unwrap()
            .2,
        unzoomed
            .iter()
            .find(|(panel, _, _)| panel != &focus_panel)
            .unwrap()
            .2,
    );

    assert!(state.toggle_zoom());
    let zoomed_panes = [
        KeyedProjection::for_test(
            focus.clone(),
            framed_projection(),
            crate::terminal_session::NativeProjectionStatus::FrameAvailable,
        ),
        KeyedProjection::for_test(
            peer.clone(),
            framed_projection(),
            crate::terminal_session::NativeProjectionStatus::FrameAvailable,
        ),
    ];
    let zoomed_view = NativeSessionView::test_view(
        &state,
        &zoomed_panes,
        crate::terminal_session::NativeProjectionStatus::FrameAvailable,
    );
    let zoomed = pane_geometries(dimensions, &client_view, &zoomed_view).unwrap();
    // The observed set is unchanged: both leaves still report — the focused
    // leaf on the whole content area, the hidden leaf on its canonical rect.
    assert_eq!(zoomed.len(), 2);
    let zoomed_focused = zoomed
        .iter()
        .find(|(panel, _, _)| panel == &focus_panel)
        .unwrap()
        .2;
    let zoomed_hidden = zoomed
        .iter()
        .find(|(panel, _, _)| panel != &focus_panel)
        .unwrap()
        .2;
    assert!(zoomed_focused.columns() > focused_report.columns());
    assert_eq!(zoomed_hidden, hidden_report);

    // Paint the zoomed frame: the only `Pane` hit region anywhere on the
    // surface belongs to the focused incarnation; the hidden leaf's pane
    // never appears, so a press cannot reach it.
    let mut terminal = Terminal::new(TestBackend::new(120, 34)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &client_view,
                Locale::EnUs,
                Some(TerminalSessionView::Native(zoomed_view)),
                &crate::tui_theme::Theme::rgb_test_dark(),
                &mut hits,
            );
        })
        .unwrap();
    let map = hits.finish();
    let mut focused_regions = 0usize;
    for row in 0..34u16 {
        for column in 0..120u16 {
            if let Some(HitTarget::Pane { pane, .. }) = map.hit_target_at(column, row) {
                assert_eq!(pane, &focus);
                focused_regions += 1;
            }
        }
    }
    assert!(focused_regions > 0, "zoomed paint recorded no pane region");
}
