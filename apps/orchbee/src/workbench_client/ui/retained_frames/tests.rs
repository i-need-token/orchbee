use super::super::{ClientAction, ClientActionDisposition, DisplayPhase, WorkbenchClientUi};
use super::*;
use crate::workbench_client::state::ControllerMode;

#[test]
fn retained_renderer_has_no_cursor_selection_graphics_input_or_ack() {
    use crate::{
        client_view::ClientView,
        localization::Locale,
        terminal_input::{PointerSelectionAdmission, PointerSelectionCell, PointerSelectionIntent},
        tui_pointer::HitMapBuilder,
        tui_theme::{Role, Theme},
        workbench_client::{
            PaneGraphicsCanvas,
            pane_graphics::{Layer, Source},
            terminal_selection::Driver,
        },
    };
    use orchbee_terminal_protocol::{SelectionChanged, SelectionChangedState, SelectionPoint};
    use orchbee_workbench_protocol::{PaneGraphicsLayer, PaneGraphicsPlacement, WorkbenchRevision};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let (mut ui, mailbox, _, _) = super::super::tests::viewport::controlling_ui();
    let old = super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
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
    let area = crate::tui_shell::active_pane_area(&areas, Some(ui.view())).unwrap();
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
    let mut receipt = crate::workbench_client::DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &client,
                Locale::EnUs,
                Some(ui.view()),
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
    let mut receipt = crate::workbench_client::DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &client,
                Locale::EnUs,
                Some(ui.view()),
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
    let (mut ui, _mailbox, _clock, _elapsed) = super::super::tests::move_membership::ready_ui();
    let old = super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.after_draw();
    let panel = old.pane().panel().clone();
    assert_eq!(ui.retained_frames.entries.len(), 1);
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
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
        ClientActionDisposition::DismissNavigator
    );
    assert_eq!(ui.view().phase(), DisplayPhase::Switching);
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
    ui.retire(crate::workbench_client::state::CarrierFailure::Protocol);
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
    let (mut ui, mailbox, clock, _) = super::super::tests::move_membership::ready_ui();
    super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
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
    assert_eq!(ui.projections[0].status, ClientProjectionStatus::Blocked);
}

#[test]
fn membership_replacement_and_context_change_release_retained_pixels() {
    let (mut ui, _mailbox, _clock, _elapsed) = super::super::tests::move_membership::ready_ui();
    let pane = super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
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
    let (mut ui, _mailbox, _clock, _elapsed) = super::super::tests::move_membership::ready_ui();
    super::super::tests::viewport::install_projection(&mut ui, Some((0, false, 24)), 0);
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

    let recently_used = cache.entries[0].identity.clone();
    cache.retain(recently_used.clone(), Arc::clone(&frame));
    let least_recently_used = cache.entries[0].identity.clone();
    let mut replacement = recently_used.clone();
    replacement.panel = WorkbenchPanelId::parse(&format!("{:032x}", 100_500)).unwrap();
    cache.retain(replacement, Arc::clone(&frame));
    assert!(
        cache
            .entries
            .iter()
            .any(|pixels| pixels.identity == recently_used),
        "redrawing an unchanged Arc refreshes its LRU age"
    );
    assert!(
        cache
            .entries
            .iter()
            .all(|pixels| pixels.identity != least_recently_used),
        "the untouched least-recently-used image is evicted first"
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

fn ready_for_retained_handoff(
    cache: crate::workbench_client::workbench_projection::ClientContextTabs,
) -> WorkbenchClientUi {
    use crate::{
        local_client::test_support::server::adjustable_operation_clock,
        workbench_client::mailbox::{CommandMailbox, EventDeadline, WorkerEvent},
    };
    use orchbee_client_server_protocol::Geometry;
    use std::sync::atomic::AtomicU64;

    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(elapsed);
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache,
            },
            EventDeadline::Startup(clock.now() + crate::workbench_client::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    let frame = crate::workbench_client::state::FrameIdentity::new(1);
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&key, frame));
    assert!(state.mark_feedback_accepted_t(&key, frame));
    assert!(state.publish_ready_t(&key, frame, ControllerMode::ReadOnly, None));
    ui
}

fn identity_at(
    cache: &crate::workbench_client::workbench_projection::ClientContextTabs,
    tab_index: usize,
) -> Identity {
    let tab = &cache.tabs()[tab_index];
    let panel = tab.panel(0).unwrap();
    let presentation = panel.primary();
    Identity {
        session: cache.session_id().clone(),
        context: cache.terminal_context_id().unwrap().clone(),
        tab: tab.tab_id().clone(),
        panel: panel.panel_id().clone(),
        presentation: presentation.presentation_id().clone(),
        binding: presentation.binding().unwrap().clone(),
    }
}

fn marked_frame(template: &TerminalTextFrame, marker: &str) -> Arc<TerminalTextFrame> {
    let mut frame = template.clone();
    frame.rows[0].cells[0].text = marker.into();
    Arc::new(frame)
}

#[test]
fn fresh_targeted_surface_keeps_canonical_context_bank_and_newer_pixels_win() {
    use crate::workbench_client::workbench_projection::{
        project_native_context_tabs, project_preferred_workspace,
    };

    let snapshot = super::super::tests::context_selection::snapshot(false);
    let alpha = project_preferred_workspace(&snapshot).unwrap();
    let beta_location = alpha.navigation()[1].location.clone();
    let beta = project_native_context_tabs(&snapshot, &beta_location).unwrap();
    assert_eq!(alpha.tabs().len(), 2);
    assert_eq!(beta.tabs().len(), 1);

    let mut template_ui = ready_for_retained_handoff(alpha.clone());
    super::super::tests::viewport::install_projection(&mut template_ui, Some((0, false, 24)), 0);
    template_ui.after_draw();
    let template = Arc::clone(&template_ui.retained_frames.entries[0].frame);
    let alpha_one = identity_at(&alpha, 0);
    let alpha_two = identity_at(&alpha, 1);
    let beta_one = identity_at(&beta, 0);

    let mut bank = RetainedFrames::default();
    bank.retain(alpha_one.clone(), marked_frame(&template, "alpha-one"));
    bank.retain(alpha_two.clone(), marked_frame(&template, "alpha-two"));
    bank.retain(beta_one.clone(), marked_frame(&template, "beta-one"));

    let mut fresh_beta = ready_for_retained_handoff(beta);
    fresh_beta.inherit_retained_frame_bank(bank.into());
    assert_eq!(fresh_beta.retained_frames.entries.len(), 3);
    assert_eq!(
        fresh_beta
            .view()
            .retained_pane_frame(&beta_one.panel, 80, 24)
            .unwrap()
            .rows[0]
            .cells[0]
            .text
            .as_ref(),
        "beta-one"
    );

    let carried = fresh_beta.retained_frame_bank();
    let mut fresh_alpha = ready_for_retained_handoff(alpha.clone());
    fresh_alpha.inherit_retained_frame_bank(carried.clone());
    assert_eq!(fresh_alpha.retained_frames.entries.len(), 3);
    assert_eq!(
        fresh_alpha
            .view()
            .retained_pane_frame(&alpha_one.panel, 80, 24)
            .unwrap()
            .rows[0]
            .cells[0]
            .text
            .as_ref(),
        "alpha-one"
    );
    assert!(
        fresh_alpha
            .view()
            .retained_pane_frame(&alpha_one.panel, 79, 24)
            .is_none()
    );
    assert_eq!(
        fresh_alpha.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    assert_eq!(
        fresh_alpha
            .view()
            .retained_pane_frame(&alpha_two.panel, 80, 24)
            .unwrap()
            .rows[0]
            .cells[0]
            .text
            .as_ref(),
        "alpha-two"
    );

    let mut candidate_newer = ready_for_retained_handoff(alpha);
    candidate_newer.retained_frames.retain(
        alpha_one.clone(),
        marked_frame(&template, "candidate-newer"),
    );
    candidate_newer.inherit_retained_frame_bank(carried);
    assert_eq!(
        candidate_newer
            .view()
            .retained_pane_frame(&alpha_one.panel, 80, 24)
            .unwrap()
            .rows[0]
            .cells[0]
            .text
            .as_ref(),
        "candidate-newer"
    );
}

#[test]
fn inherited_pixels_reject_wrong_binding_and_known_host_epoch() {
    use crate::workbench_client::workbench_projection::{
        project_native_context_tabs, project_preferred_workspace,
    };
    use orchbee_terminal_protocol::HostEpoch;

    let snapshot = super::super::tests::context_selection::snapshot(false);
    let alpha = project_preferred_workspace(&snapshot).unwrap();
    let beta_location = alpha.navigation()[1].location.clone();
    let beta = project_native_context_tabs(&snapshot, &beta_location).unwrap();
    let alpha_one = identity_at(&alpha, 0);
    let alpha_two = identity_at(&alpha, 1);
    let beta_one = identity_at(&beta, 0);
    assert_ne!(alpha_two.binding, beta_one.binding);

    let mut template_ui = ready_for_retained_handoff(alpha.clone());
    super::super::tests::viewport::install_projection(&mut template_ui, Some((0, false, 24)), 0);
    template_ui.after_draw();
    let template = Arc::clone(&template_ui.retained_frames.entries[0].frame);

    let mut wrong_binding = RetainedFrames::default();
    let mut stale_identity = alpha_two.clone();
    stale_identity.binding = beta_one.binding;
    wrong_binding.retain(stale_identity, marked_frame(&template, "wrong-binding"));
    let mut wrong_binding_ui = ready_for_retained_handoff(alpha.clone());
    wrong_binding_ui.inherit_retained_frame_bank(wrong_binding.into());
    assert!(wrong_binding_ui.retained_frames.entries.is_empty());
    assert_eq!(
        wrong_binding_ui.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    assert!(
        wrong_binding_ui
            .view()
            .retained_pane_frame(&alpha_two.panel, 80, 24)
            .is_none()
    );

    let mut wrong_session = RetainedFrames::default();
    let mut foreign_identity = alpha_one.clone();
    foreign_identity.session = SessionId::parse("88888888888888888888888888888888").unwrap();
    wrong_session.retain(foreign_identity, marked_frame(&template, "wrong-session"));
    let mut wrong_session_ui = ready_for_retained_handoff(alpha.clone());
    wrong_session_ui.inherit_retained_frame_bank(wrong_session.into());
    assert!(wrong_session_ui.retained_frames.entries.is_empty());

    let mut stale_epoch_frame = (*template).clone();
    stale_epoch_frame.route.host_epoch =
        HostEpoch::parse("99999999999999999999999999999999").unwrap();
    let mut stale_epoch = RetainedFrames::default();
    stale_epoch.retain(alpha_one.clone(), Arc::new(stale_epoch_frame));
    let mut stale_epoch_ui = ready_for_retained_handoff(alpha);
    stale_epoch_ui.inherit_retained_frame_bank(stale_epoch.into());
    assert!(
        stale_epoch_ui
            .view()
            .retained_pane_frame(&alpha_one.panel, 80, 24)
            .is_some(),
        "without a live Host epoch, exact canonical pixels remain a safe placeholder"
    );
    super::super::tests::viewport::install_projection(&mut stale_epoch_ui, None, 0);
    stale_epoch_ui.projections[0].status = ClientProjectionStatus::WaitingForFullFrame;
    assert!(
        stale_epoch_ui
            .view()
            .retained_pane_frame(&alpha_one.panel, 80, 24)
            .is_none(),
        "a known different Host epoch fences inherited pixels"
    );
}

#[test]
fn invalid_inherited_lru_entries_cannot_evict_valid_pixels() {
    use crate::workbench_client::workbench_projection::project_preferred_workspace;

    let snapshot = super::super::tests::context_selection::snapshot(false);
    let alpha = project_preferred_workspace(&snapshot).unwrap();
    let alpha_one = identity_at(&alpha, 0);
    let alpha_two = identity_at(&alpha, 1);
    let mut template_ui = ready_for_retained_handoff(alpha.clone());
    super::super::tests::viewport::install_projection(&mut template_ui, Some((0, false, 24)), 0);
    template_ui.after_draw();
    let template = Arc::clone(&template_ui.retained_frames.entries[0].frame);

    let mut older = RetainedFrames::default();
    older.retain(alpha_one.clone(), marked_frame(&template, "alpha-one"));
    for index in 0..(MAXIMUM_PANES - 1) {
        let mut invalid = alpha_one.clone();
        invalid.panel = WorkbenchPanelId::parse(&format!("{:032x}", 800_000 + index)).unwrap();
        older.retain(invalid, Arc::clone(&template));
    }
    assert_eq!(older.entries.len(), MAXIMUM_PANES);

    let mut candidate = ready_for_retained_handoff(alpha);
    candidate.retained_frames.retain(
        alpha_two.clone(),
        marked_frame(&template, "candidate-alpha-two"),
    );
    candidate.inherit_retained_frame_bank(older.into());
    assert_eq!(candidate.retained_frames.entries.len(), 2);
    assert_eq!(
        candidate
            .view()
            .retained_pane_frame(&alpha_one.panel, 80, 24)
            .unwrap()
            .rows[0]
            .cells[0]
            .text
            .as_ref(),
        "alpha-one"
    );
    assert_eq!(
        candidate.select_tab(ClientAction::SelectTab(2)),
        ClientActionDisposition::DismissNavigator
    );
    assert_eq!(
        candidate
            .view()
            .retained_pane_frame(&alpha_two.panel, 80, 24)
            .unwrap()
            .rows[0]
            .cells[0]
            .text
            .as_ref(),
        "candidate-alpha-two"
    );
}
