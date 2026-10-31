//! The Agent board's UI-seam tests: the presentation index is fed only
//! by snapshots the session actually accepted — through bootstrap and
//! every snapshot-bearing applier — and a jump resolving against an
//! index no canonical snapshot ever fed waits inside its own deadline
//! instead of reporting `Gone`.

use super::super::*;
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_client::mailbox::EventDeadline;
use orchbee_agent_observation_protocol::{
    AgentAttachmentGeneration, AgentPresentationAttachment, AgentPresentationId,
    AgentPresentationVisibility,
};
use orchbee_workbench_protocol::*;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

mod captures;

fn key(code: crate::client_view::KeyCode) -> crate::client_view::KeyEvent {
    crate::client_view::KeyEvent::press(code, crate::client_view::Modifiers::NONE)
}

fn attachment(presentation: u64) -> AgentPresentationAttachment {
    AgentPresentationAttachment {
        generation: AgentAttachmentGeneration::parse("gen-1").unwrap(),
        navigable: true,
        presentation_id: AgentPresentationId::parse(&format!("{presentation:032x}")).unwrap(),
        visibility: AgentPresentationVisibility::Visible,
    }
}

fn fixture() -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    crate::local_client::LocalOperationClock,
) {
    let clock = adjustable_operation_clock(Arc::new(AtomicU64::new(0)));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    (ui, mailbox, clock)
}

fn bootstrap(ui: &mut WorkbenchClientUi, mailbox: &CommandMailbox, clock: &LocalOperationClock) {
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: crate::workbench_projection::project_preferred_workspace(
                    &super::super::tests::membership::snapshot(1, 1),
                )
                .unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::workbench_state::OPERATION_TOTAL),
            clock,
        )
        .unwrap();
    assert!(ui.tick());
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::BootstrapApplied(_))
    ));
}

/// Drives the bootstrap's focus pane through the test-path setup chain to
/// `Ready` — the phase `open_navigator`'s refresh admission requires.
fn ready(ui: &mut WorkbenchClientUi) {
    let state = ui.state.as_mut().unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(&key, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
}

#[test]
fn bootstrap_feeds_the_index_only_through_the_accepted_cache() {
    let (mut ui, mailbox, clock) = fixture();
    // Before any canonical feed the index is unready — an empty map a
    // jump could only misread as "nothing exists".
    assert!(!ui.agent_index.ready());
    bootstrap(&mut ui, &mailbox, &clock);
    assert!(ui.agent_index.ready());
    // `snapshot(1, ·)` paints one tab: presentation 0x3ea (1002) on tab
    // 0x3e8, panel 0x3e9 — the index resolves the stable identity to the
    // exact canonical placement.
    let placement = ui
        .agent_index
        .placement(&attachment(0x3ea))
        .expect("the bootstrap snapshot's presentation resolves");
    assert_eq!(
        placement.tab_id,
        WorkbenchTabId::parse(&format!("{:032x}", 0x3e8)).unwrap()
    );
    assert_eq!(
        placement.panel_id,
        WorkbenchPanelId::parse(&format!("{:032x}", 0x3e9)).unwrap()
    );
    // An identity the snapshot never painted resolves to nothing.
    assert!(ui.agent_index.placement(&attachment(0x9ff)).is_none());
}

#[test]
fn an_accepted_refresh_rebuilds_but_an_unaccepted_snapshot_never_feeds() {
    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    // An accepted refresh snapshot rebuilds the index: `snapshot(2, ·)`
    // adds the second tab's presentation 0x3f4 (1012).
    let (lease, wait) = super::super::tests::membership::place_refresh(
        &mut ui,
        &mailbox,
        &clock,
        super::super::tests::membership::snapshot(2, 2),
    );
    assert!(ui.tick());
    mailbox.take_deposited_reply(&wait).unwrap();
    assert!(
        ui.agent_index.placement(&attachment(0x3f4)).is_some(),
        "the accepted snapshot's new presentation resolves"
    );
    mailbox.finish_lease(lease).unwrap();
    // A snapshot-bearing reply the session does not accept for the
    // flight — here a generation that is not the pending refresh's —
    // is refused at admission as a protocol fault: it never becomes an
    // event at all, so it can never reach the index.
    ui.open_navigator();
    let lease = mailbox.try_recv().unwrap().expect("Refresh queued");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("Refresh only");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    let published = mailbox.publish_refresh_result_before(
        marker.generation() + 7,
        WorkbenchSnapshotOutcome::Available {
            snapshot: super::super::tests::membership::snapshot(3, 9),
        },
        marker.deadline(),
        &clock,
    );
    assert_eq!(
        published.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol),
        "a generation the flight never staged is refused at admission"
    );
    // `snapshot(3, ·)` would add presentation 0x3fe (1022); the refused
    // reply leaves the accepted placements intact and adds nothing.
    assert!(
        ui.agent_index.placement(&attachment(0x3f4)).is_some(),
        "accepted placements survive the refused reply"
    );
    assert!(ui.agent_index.placement(&attachment(0x3fe)).is_none());
}

#[test]
fn a_jump_against_an_unfed_index_waits_instead_of_reporting_gone() {
    let (mut ui, _mailbox, _clock) = fixture();
    ui.board.open();
    ui.board.force_jump_t(crate::agent_board::BoardJump {
        serial: 1,
        stage: crate::agent_board::JumpStage::Navigating {
            candidates: vec![attachment(0x3ea)].into_boxed_slice(),
            index: 0,
        },
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
    });
    let actual = ui.agent_board_view().unwrap().actual_draw(4).unwrap();
    ui.after_agent_board_draw(Some(AgentBoardDraw::Presented(actual)));
    // No canonical snapshot has ever fed the index: resolving now could
    // only guess, and guessing reports `Gone` for sessions that exist.
    // The drive must keep the candidate exactly where it is — waiting
    // inside the jump's own deadline, with no failure notice.
    let _changed = ui.drive_agent_board(true);
    let Some(jump) = ui.board.jump() else {
        panic!("the jump survives the unfed-index wait");
    };
    assert!(matches!(
        jump.stage,
        crate::agent_board::JumpStage::Navigating { index: 0, .. }
    ));
    assert!(ui.board.notice().is_none());
    assert!(ui.board.jumping());
}

#[test]
fn exact_board_capture_is_fail_closed_during_pending_and_suspended_recovery() {
    let (mut ui, _mailbox, _clock) = fixture();
    ui.board.open();
    let stale = ui.board.episode();
    ui.board.close();
    ui.board.open();
    let episode = ui.board.episode();
    assert_ne!(episode, stale);

    assert!(!ui.capture_agent_board_key(stale, &key(crate::client_view::KeyCode::Esc)));
    assert!(
        ui.board.is_open(),
        "a stale owner cannot close this episode"
    );
    assert!(ui.capture_agent_board_key(episode, &key(crate::client_view::KeyCode::Enter)));
    assert!(ui.board.is_open(), "Pending ordinary input is swallowed");

    let actual = ui.agent_board_view().unwrap().actual_draw(4).unwrap();
    ui.after_agent_board_draw(Some(AgentBoardDraw::Presented(actual)));
    assert!(ui.board.input_context().unwrap().interactive());
    assert_eq!(
        ui.board
            .settle_draw(Some(AgentBoardDraw::NotPresented(episode))),
        Some(crate::tui_presentation::GeometrySettlement::Suspended)
    );
    assert!(!ui.board.input_context().unwrap().interactive());
    assert!(ui.capture_agent_board_key(episode, &key(crate::client_view::KeyCode::Enter)));
    assert!(ui.board.is_open(), "Suspended activation remains inert");
    assert!(ui.capture_agent_board_key(episode, &key(crate::client_view::KeyCode::Esc)));
    assert!(
        !ui.board.is_open(),
        "the declared local recovery exit remains live"
    );
}

#[test]
fn a_covered_board_keeps_an_accepted_jump_without_moving_workbench_focus() {
    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    ui.board.open();
    ui.board.force_jump_t(crate::agent_board::BoardJump {
        serial: 1,
        stage: crate::agent_board::JumpStage::Navigating {
            candidates: vec![attachment(0x3ea)].into_boxed_slice(),
            index: 0,
        },
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
    });
    let actual = ui.agent_board_view().unwrap().actual_draw(4).unwrap();
    ui.after_agent_board_draw(Some(AgentBoardDraw::Presented(actual)));
    assert!(ui.board.input_context().unwrap().interactive());

    let focused = ui.state.as_ref().unwrap().focus_panel_id().cloned();
    let _changed = ui.drive_agent_board(false);
    assert_eq!(
        ui.state.as_ref().unwrap().focus_panel_id(),
        focused.as_ref(),
        "a Client-level cover blocks navigation before its draw receipt"
    );
    assert!(matches!(
        ui.board.jump().map(|jump| &jump.stage),
        Some(crate::agent_board::JumpStage::Navigating { index: 0, .. })
    ));

    ui.after_agent_board_draw(None);
    assert!(!ui.board.input_context().unwrap().interactive());
    let _changed = ui.drive_agent_board(true);

    assert_eq!(
        ui.state.as_ref().unwrap().focus_panel_id(),
        focused.as_ref(),
        "a covered completion cannot move focus behind its child"
    );
    assert!(matches!(
        ui.board.jump().map(|jump| &jump.stage),
        Some(crate::agent_board::JumpStage::Navigating { index: 0, .. })
    ));
}

#[test]
fn a_new_higher_capture_blocks_a_jump_before_the_next_cover_receipt() {
    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    ui.board.open();
    ui.board.force_jump_t(crate::agent_board::BoardJump {
        serial: 1,
        stage: crate::agent_board::JumpStage::Navigating {
            candidates: vec![attachment(0x3ea)].into_boxed_slice(),
            index: 0,
        },
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
    });
    let actual = ui.agent_board_view().unwrap().actual_draw(4).unwrap();
    ui.after_agent_board_draw(Some(AgentBoardDraw::Presented(actual)));
    assert!(ui.board.input_context().unwrap().interactive());

    assert!(matches!(
        ui.request_panel_close_confirmation(),
        ClientActionDisposition::DismissNavigator
    ));
    assert!(ui.view().panel_close_confirmation().is_some());
    assert!(
        ui.board.input_context().unwrap().interactive(),
        "the previous Board frame has not received its cover receipt yet"
    );
    let focused = ui.state.as_ref().unwrap().focus_panel_id().cloned();
    let _changed = ui.drive_agent_board(true);

    assert_eq!(
        ui.state.as_ref().unwrap().focus_panel_id(),
        focused.as_ref(),
        "a synchronously opened higher capture blocks Board focus effects"
    );
    assert!(matches!(
        ui.board.jump().map(|jump| &jump.stage),
        Some(crate::agent_board::JumpStage::Navigating { index: 0, .. })
    ));
}
