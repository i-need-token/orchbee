use super::super::{ClientAction, NativeDisplayPhase, SessionActionDisposition};
use super::*;

#[cfg(windows)]
#[test]
fn retained_renderer_has_no_cursor_selection_graphics_input_or_ack() {
    use crate::{
        client_view::ClientView,
        localization::Locale,
        terminal_input::{PointerSelectionAdmission, PointerSelectionCell, PointerSelectionIntent},
        terminal_session::{
            PaneGraphicsCanvas, TerminalSessionView,
            pane_graphics::{Layer, Source},
            terminal_selection::Driver,
        },
        tui_pointer::HitMapBuilder,
        tui_theme::{Role, Theme},
    };
    use orchbee_terminal_protocol::{SelectionChanged, SelectionChangedState, SelectionPoint};
    use orchbee_workbench_protocol::{PaneGraphicsLayer, PaneGraphicsPlacement, WorkbenchRevision};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let (mut ui, mailbox, _, _) = super::super::viewport_tests::controlling_ui();
    let old = super::super::viewport_tests::install_projection(&mut ui, Some((0, false, 24)), 0);
    assert!(ui.forward(b"ready"));
    let input = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(
        input.record(),
        super::super::QueuedCommand::Input(_)
    ));
    mailbox.finish_lease(input).unwrap();
    let client = ClientView::new(120);
    let areas =
        crate::tui_layout::project_shell_areas(Rect::new(0, 0, 120, 32), client.sidebar_surface());
    let area =
        crate::tui_shell::active_pane_area(&areas, Some(TerminalSessionView::Native(ui.view())))
            .unwrap();
    let projection = &mut ui.projections[0];
    let mut frame = projection.value.take_frame().unwrap();
    frame.geometry.columns = area.width;
    frame.geometry.rows = area.height;
    projection.value.install_frame_for_test(frame);
    let attachment = *projection.value.attachment();
    let mut selection = Driver::new(attachment);
    selection
        .changed(SelectionChanged {
            attachment_id: attachment,
            state: SelectionChangedState::Active {
                start: SelectionPoint { column: 0, row: 0 },
                end: SelectionPoint { column: 1, row: 0 },
                rectangle: false,
                preview: None,
            },
        })
        .unwrap();
    projection.selection = Some(selection);
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    let theme = Theme::rgb_test_dark();
    let mut receipt = crate::terminal_session::NativeDrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &client,
                Locale::EnUs,
                Some(TerminalSessionView::Native(ui.view())),
                &theme,
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    assert_eq!(
        Some(terminal.backend().buffer()[(area.x, area.y)].bg),
        theme.style(Role::SurfaceSelection).bg
    );
    ui.after_close_draw(receipt);
    ui.after_draw();
    ui.select_tab(ClientAction::SelectTab(2));
    ui.select_tab(ClientAction::SelectTab(1));
    let view = ui.view();
    let panel = view.tab(0).unwrap().panel(0).unwrap();
    let source = Source::capture(panel, view.selected_presentation(panel));
    // Even an available graphics canvas must not accompany a retained text image.
    ui.graphics.install_for_test(
        PaneGraphicsCanvas::new(
            source,
            WorkbenchRevision::new(1),
            vec![
                Layer::new(
                    PaneGraphicsLayer {
                        layer_id: "retained-test".into(),
                        z_index: 0,
                        width: 1,
                        height: 2,
                        placement: PaneGraphicsPlacement {
                            grid_columns: 2,
                            grid_rows: 1,
                            ..Default::default()
                        },
                        resource_revision: WorkbenchRevision::new(1),
                        stream_id: None,
                    },
                    vec![255, 0, 0, 255, 0, 0, 255, 255],
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    assert!(
        ui.view()
            .retained_pane_frame(old.pane().panel(), area.width, area.height)
            .is_some()
    );
    let mut hits = HitMapBuilder::new();
    let mut receipt = crate::terminal_session::NativeDrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &client,
                Locale::EnUs,
                Some(TerminalSessionView::Native(ui.view())),
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(area.x, area.y)].symbol(), "x");
    assert_ne!(
        Some(terminal.backend().buffer()[(area.x, area.y)].bg),
        theme.style(Role::SurfaceSelection).bg
    );
    assert!(!terminal.backend().cursor_visible());
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .all(|cell| cell.symbol() != "▀")
    );
    assert!(receipt.graphics_draws().is_empty());
    assert!(hits.finish().hit_target_at(area.x, area.y).is_none());
    assert!(!ui.forward(b"unready"));
    assert_ne!(
        ui.pointer_selection(
            &old,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: PointerSelectionCell { column: 0, row: 0 }
            }
        ),
        PointerSelectionAdmission::Admitted
    );
    ui.after_close_draw(receipt);
    ui.after_draw();
    assert!(ui.pending_draws.is_empty());
    assert!(ui.state.as_ref().unwrap().ready().is_none());
}

#[test]
fn revisiting_a_tab_paints_its_own_pixels_without_reusing_input_or_feedback() {
    let (mut ui, _mailbox, _clock, _elapsed) = super::super::move_membership_tests::ready_ui();
    let old = super::super::viewport_tests::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.after_draw();
    let panel = old.pane().panel().clone();
    assert_eq!(ui.retained_frames.entries.len(), 1);
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        SessionActionDisposition::DismissNavigator
    );
    let other_panel = ui.view().focus_panel_id().unwrap().clone();
    assert!(
        ui.view()
            .retained_pane_frame(&other_panel, 80, 24)
            .is_none()
    );
    assert!(!ui.forward(b"wrong tab"));
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(1)),
        SessionActionDisposition::DismissNavigator
    );
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Switching);
    assert!(ui.view().pane_projection(&panel).is_none());
    assert_eq!(
        ui.view().retained_pane_frame(&panel, 80, 24).unwrap().rows[0].cells[0]
            .text
            .as_ref(),
        "x"
    );
    assert!(ui.view().retained_pane_frame(&panel, 79, 24).is_none());
    assert!(ui.view().retained_pane_frame(&panel, 80, 23).is_none());
    assert!(!ui.forward(b"not ready"));
    assert!(ui.state.as_ref().unwrap().ready().is_none());
    assert!(ui.pending_draws.is_empty());
    ui.clear_projections();
    assert!(ui.view().retained_pane_frame(&panel, 80, 24).is_some());
    ui.after_draw();
    assert!(
        ui.pending_draws.is_empty(),
        "retained pixels never mint feedback"
    );
    ui.retire(crate::terminal_session::state::CarrierFailure::Protocol);
    assert!(ui.retained_frames.entries.is_empty());
    assert!(ui.view().retained_pane_frame(&panel, 80, 24).is_none());
}

#[test]
fn blocked_projection_drops_cached_pixels_before_another_draw() {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, PayloadEncoding, ProjectionBlockReason,
        ProjectionBlocked, ProjectionDemandControl, encode_projection_demand_control,
        encode_transport_envelope,
    };
    let (mut ui, mailbox, clock, _) = super::super::move_membership_tests::ready_ui();
    super::super::viewport_tests::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.after_draw();
    assert_eq!(ui.retained_frames.entries.len(), 1);
    let projection = &ui.projections[0].value;
    let blocked = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id: *projection.attachment(),
            reason: ProjectionBlockReason::ProjectionTooLarge,
            revision: 2,
        },
    ))
    .unwrap();
    let record = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: blocked.role,
        kind: blocked.kind,
        payload_encoding: PayloadEncoding::ControlJson,
        host_epoch: Some(*projection.epoch()),
        correlation_id: None,
        optional_message: false,
        payload: &blocked.payload,
    })
    .unwrap();
    ui.schedule_poll(&mailbox);
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox
        .publish_poll_result_before(
            lease.poll_key().unwrap().clone(),
            Some(record),
            false,
            clock.now() + std::time::Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(ui.retained_frames.entries.is_empty());
    assert_eq!(ui.projections[0].status, NativeProjectionStatus::Blocked);
}

#[test]
fn membership_replacement_and_context_change_release_retained_pixels() {
    let (mut ui, _mailbox, _clock, _elapsed) = super::super::move_membership_tests::ready_ui();
    let pane = super::super::viewport_tests::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.after_draw();
    let old = Arc::downgrade(&ui.retained_frames.entries[0].frame);
    let mut identity = ui.retained_frames.entries[0].identity.clone();
    identity.presentation = TerminalPresentationId::parse(&format!("{:032x}", 999_001)).unwrap();
    ui.retained_frames.entries[0].identity = identity;
    ui.retained_frames.prune(ui.state.as_ref().unwrap());
    assert!(ui.retained_frames.entries.is_empty());
    ui.after_draw();
    ui.retained_frames.entries[0].identity.context =
        WorkbenchTerminalContextId::parse(&format!("{:032x}", 999_002)).unwrap();
    ui.retained_frames.prune(ui.state.as_ref().unwrap());
    assert!(ui.retained_frames.entries.is_empty());
    ui.clear_pane_projection(&pane);
    assert!(
        old.upgrade().is_none(),
        "last pixel owner releases the frame"
    );
}

#[test]
fn retained_images_have_count_and_byte_bounds_and_never_truncate() {
    let (mut ui, _mailbox, _clock, _elapsed) = super::super::move_membership_tests::ready_ui();
    super::super::viewport_tests::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.after_draw();
    let identity = ui.retained_frames.entries[0].identity.clone();
    let frame = Arc::clone(&ui.retained_frames.entries[0].frame);
    let mut cache = RetainedFrames::default();
    for index in 0..=MAXIMUM_PANES {
        let mut id = identity.clone();
        id.panel = WorkbenchPanelId::parse(&format!("{:032x}", 100_000 + index)).unwrap();
        cache.retain(id, Arc::clone(&frame));
    }
    assert_eq!(cache.entries.len(), MAXIMUM_PANES);
    assert!(
        cache
            .entries
            .iter()
            .all(|pixels| pixels.identity.panel.as_str() != format!("{:032x}", 100_000))
    );
    let mut large = (*frame).clone();
    large.rows[0].cells[0].text = "x".repeat(MAXIMUM_BYTES).into();
    cache.retain(identity, Arc::new(large));
    assert_eq!(cache.entries.len(), MAXIMUM_PANES);
    assert!(
        cache
            .entries
            .iter()
            .map(|pixels| pixels.bytes)
            .sum::<usize>()
            <= MAXIMUM_BYTES
    );
    assert!(
        cache
            .entries
            .iter()
            .all(|pixels| Arc::ptr_eq(&pixels.frame, &frame))
    );
    cache.clear();
    let mut half = (*frame).clone();
    half.rows[0].cells[0].text = "x".repeat(MAXIMUM_BYTES / 2).into();
    let half = Arc::new(half);
    assert!(retained_bytes(&half) < MAXIMUM_BYTES);
    let mut first = ui.retained_frames.entries[0].identity.clone();
    cache.retain(first.clone(), Arc::clone(&half));
    first.panel = WorkbenchPanelId::parse(&format!("{:032x}", 100_100)).unwrap();
    cache.retain(first.clone(), Arc::clone(&half));
    assert_eq!(
        cache.entries.len(),
        1,
        "two individually admissible images exceed the byte bound"
    );
    assert!(cache.entries[0].identity == first);
    assert!(
        Arc::ptr_eq(&cache.entries[0].frame, &half),
        "eviction retains the whole image"
    );
}
