//! Board unit tests: the key map, selection/scroll bounds, the
//! painted-verdict input gate, jump refusal and expiry paths, the pure
//! staged-jump planner, and the presentation index join — plus read
//! request and answer settlement. Nothing here needs a live OBAP endpoint
//! or a lane; requests are accepted or refused and answers injected as data.

use super::*;
mod mouse;
mod rows;
use crate::client_view::{KeyCode, KeyEvent, Modifiers};
use crate::local_client::test_support::server::adjustable_operation_clock;
use crate::workbench_projection::ClientContextLocation;
use orchbee_agent_observation_protocol::{
    AgentAttachmentGeneration, AgentExecutionLocation, AgentExecutionLocationId, AgentObservation,
    AgentObservationFidelity, AgentObservationSource, AgentPresentationId, AgentSafeInteger,
};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, SessionId, Snapshot,
    Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchPanelId, WorkbenchProjectId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

fn clock() -> crate::local_client::LocalOperationClock {
    adjustable_operation_clock(Arc::new(AtomicU64::new(0)))
}

/// A read lane that accepts every request.
fn accepted(_: AgentBoardReadRequest) -> Result<(), AgentBoardReadRefusal> {
    Ok(())
}

/// No read lane can serve any request.
fn no_lane(_: AgentBoardReadRequest) -> Result<(), AgentBoardReadRefusal> {
    Err(AgentBoardReadRefusal::LaneUnavailable)
}

fn no_preference(_: &AgentPresentationAttachment) -> bool {
    false
}

#[test]
fn board_index_keeps_workspace_presentations_after_regroup_and_dissolve() {
    use orchbee_workbench_protocol::{Project, Workspace};
    let mut snapshot = presentation_snapshot();
    let mut terminal = snapshot.terminal_contexts[0].clone();
    snapshot.terminal_contexts[0].tabs.clear();
    terminal.terminal_context_id = context_id(80);
    snapshot.navigation.workspaces.push(Workspace {
        terminal_directory: None,
        automatic_label: false,
        workspace_id: WorkbenchWorkspaceId::parse(&hex(81)).unwrap(),
        project_id: None,
        label: "Feature".into(),
        primary_directory_id: None,
        terminal_context_id: terminal.terminal_context_id.clone(),
    });
    snapshot.navigation.workspace_order = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    snapshot.terminal_contexts.push(terminal);
    let index = PresentationIndex::rebuild(&snapshot);
    let before = index.placement_by_presentation(&hex(42)).unwrap().clone();
    assert_eq!(before.workspace_label.as_deref(), Some("Feature"));
    assert_eq!(before.project_label, None);
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&hex(82)).unwrap(),
        label: "Group".into(),
    });
    snapshot.navigation.workspaces[1].project_id =
        Some(snapshot.navigation.projects[0].project_id.clone());
    let index = PresentationIndex::rebuild(&snapshot);
    let grouped = index.placement_by_presentation(&hex(42)).unwrap();
    assert_eq!(grouped.location, before.location);
    assert_eq!(grouped.tab_id, before.tab_id);
    assert_eq!(grouped.panel_id, before.panel_id);
    assert_eq!(grouped.project_label.as_deref(), Some("Group"));
    snapshot.navigation.projects.clear();
    snapshot.navigation.workspaces[1].project_id = None;
    let index = PresentationIndex::rebuild(&snapshot);
    let dissolved = index.placement_by_presentation(&hex(42)).unwrap();
    assert_eq!(dissolved.location, before.location);
    assert_eq!(dissolved.tab_id, before.tab_id);
    assert_eq!(dissolved.panel_id, before.panel_id);
    assert_eq!(dissolved.project_label, None);
}

fn hex(seed: u64) -> String {
    format!("{seed:032x}")
}

fn session_id(text: &str) -> AgentSessionId {
    AgentSessionId::parse(text).expect("valid session id")
}

fn attachment(presentation: &str, navigable: bool) -> AgentPresentationAttachment {
    AgentPresentationAttachment {
        generation: AgentAttachmentGeneration::parse("gen-1").unwrap(),
        navigable,
        presentation_id: AgentPresentationId::parse(presentation).unwrap(),
        visibility: AgentPresentationVisibility::Visible,
    }
}

fn summary(session: &str, attachments: Vec<AgentPresentationAttachment>) -> AgentSessionSummary {
    AgentSessionSummary {
        execution_location: AgentExecutionLocation {
            execution_location_id: AgentExecutionLocationId::parse("local").unwrap(),
        },
        lifecycle: AgentLifecycle::Live,
        observation: AgentObservation {
            fidelity: AgentObservationFidelity::Rich,
            freshness: AgentObservationFreshness::Fresh,
            generation: AgentSafeInteger::new(7),
            sequence: AgentSafeInteger::new(3),
            source: AgentObservationSource::Native,
        },
        parent_session_id: None,
        pending_request_count: AgentSafeInteger::new(0),
        presentation_attachments: attachments,
        provider_id: AgentProviderId::parse("devin").unwrap(),
        session_id: session_id(session),
        status: AgentSemanticStatus::Idle {},
    }
}

fn board_with(sessions: &[&str]) -> AgentBoard {
    let mut board = AgentBoard::new(true);
    board.rows = sessions
        .iter()
        .map(|id| AgentBoardRow::from_summary(summary(id, vec![])))
        .collect();
    board
}

#[test]
fn one_started_completion_requests_exactly_one_refresh() {
    use crate::agent_start::{DeliveryDisposition, Report, ReservationState, Verdict};
    use orchbee_agent_observation_protocol::{AgentName, ObapRequestId};

    let mut board = AgentBoard::new(true);
    let mut jobs = start::jobs::Jobs::default();
    let origin = crate::local_client::test_support::server::disconnected_origin();
    jobs.set_origin(origin.clone());
    board.set_origin_locality(origin.is_local());
    board.open();
    board.start.install_catalog(&["codex"]);
    board.start.set_name("alpha");
    board.start.accept_without_task();
    let target = panel_id(1);
    assert!(jobs.start(&mut board.start, board.epoch, target, clock()));
    let report = Report {
        name: AgentName::parse("alpha").unwrap(),
        provider: AgentProviderId::parse("codex").unwrap(),
        reservation: Some(ObapRequestId::parse(&"1".repeat(32)).unwrap()),
        last_observed: None,
        reservation_state: ReservationState::Completed,
        submission: None,
        delivery: DeliveryDisposition::Applied,
        elapsed_ms: 1,
        verdict: Verdict::Started {
            session: AgentSessionId::parse("session-1").unwrap(),
        },
    };
    assert!(jobs.queue_completion_t(&board.start, report));
    board.intent = None;
    assert!(board.drain_start(Instant::now(), |start, admission| {
        jobs.drain(start, admission)
    }));
    assert!(matches!(board.intent, Some(BoardIntent::Refresh)));
    board.intent = None;
    assert!(!board.drain_start(Instant::now(), |start, admission| {
        jobs.drain(start, admission)
    }));
    assert!(board.intent.is_none());
}

fn presented(board: &AgentBoard, visible_rows: usize) -> AgentBoardDraw {
    AgentBoardDraw::Presented(
        board
            .actual_draw(visible_rows)
            .expect("the open Board has a non-empty viewport"),
    )
}

fn tab_id(seed: u64) -> WorkbenchTabId {
    WorkbenchTabId::parse(&hex(seed)).unwrap()
}

fn panel_id(seed: u64) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&hex(seed)).unwrap()
}

fn context_id(seed: u64) -> WorkbenchTerminalContextId {
    WorkbenchTerminalContextId::parse(&hex(seed)).unwrap()
}

fn workspace_location(seed: u64) -> ClientContextLocation {
    ClientContextLocation::Workspace {
        workspace_id: WorkbenchWorkspaceId::parse(&hex(seed + 200)).unwrap(),
        terminal_context_id: context_id(seed),
    }
}

fn placement(location: ClientContextLocation, tab: u64, panel: u64) -> PresentationPlacement {
    PresentationPlacement {
        location,
        tab_id: tab_id(tab),
        panel_id: panel_id(panel),
        tab_position: 1,
        tab_label: "tab".into(),
        panel_name: NameFacts::default(),
        panel_position: 1,
        presentation_name: NameFacts::default(),
        automatic_tab_name: None,
        project_label: None,
        workspace_label: None,
    }
}

#[test]
fn key_map_decodes_the_board_contract() {
    let press = KeyEvent::press;
    assert_eq!(
        board_key_action(press(KeyCode::Up, Modifiers::NONE)),
        Some(AgentBoardKeyAction::SelectUp)
    );
    assert_eq!(
        board_key_action(press(KeyCode::Char('k'), Modifiers::NONE)),
        Some(AgentBoardKeyAction::SelectUp)
    );
    assert_eq!(
        board_key_action(press(KeyCode::Down, Modifiers::NONE)),
        Some(AgentBoardKeyAction::SelectDown)
    );
    assert_eq!(
        board_key_action(press(KeyCode::Char('j'), Modifiers::NONE)),
        Some(AgentBoardKeyAction::SelectDown)
    );
    assert_eq!(
        board_key_action(press(KeyCode::Enter, Modifiers::NONE)),
        Some(AgentBoardKeyAction::Jump)
    );
    assert_eq!(
        board_key_action(press(KeyCode::Char('r'), Modifiers::NONE)),
        Some(AgentBoardKeyAction::Refresh)
    );
    assert_eq!(
        board_key_action(press(KeyCode::Esc, Modifiers::NONE)),
        Some(AgentBoardKeyAction::Close)
    );
    // Everything else — including the prefix chord's second key and any
    // modified key — is swallowed by capture, never an action.
    assert_eq!(
        board_key_action(press(KeyCode::Char('g'), Modifiers::NONE)),
        None
    );
    assert_eq!(board_key_action(press(KeyCode::Up, Modifiers::CTRL)), None);
    assert_eq!(
        board_key_action(press(KeyCode::Char('x'), Modifiers::NONE)),
        None
    );
}

#[test]
fn open_queues_refresh_and_close_invalidates_everything() {
    let mut board = board_with(&["s-1"]);
    board.open();
    assert!(board.is_open());
    assert!(matches!(board.intent, Some(BoardIntent::Refresh)));
    let epoch = board.epoch;
    board.jump = Some(BoardJump {
        serial: 1,
        stage: JumpStage::Checking,
        deadline: Instant::now(),
    });
    board.notice = Some(JumpFailure::Gone);
    board.close();
    assert!(!board.is_open());
    assert!(!board.owns_input());
    assert_eq!(board.epoch, epoch + 1);
    assert!(board.intent.is_none());
    assert!(board.inflight.is_none());
    assert!(board.jump.is_none());
    assert!(board.notice.is_none());
}

#[test]
fn selection_and_scroll_clamp_to_the_row_window() {
    let mut board = board_with(&["s-1", "s-2", "s-3", "s-4", "s-5"]);
    board.viewport_rows = 2;
    board.move_selection(10);
    assert_eq!(board.selection, 4);
    assert_eq!(board.scroll, 3);
    board.move_selection(-10);
    assert_eq!(board.selection, 0);
    assert_eq!(board.scroll, 0);
    board.scroll_by(100);
    assert_eq!(board.scroll, 3);
    assert_eq!(board.selection, 0, "wheel reading does not move selection");
    board.scroll_by(-100);
    assert_eq!(board.scroll, 0);
    assert_eq!(board.selection, 0);
}

#[test]
fn pointer_select_requires_the_exact_visible_draw() {
    let mut board = board_with(&["s-1", "s-2", "s-3"]);
    board.open();
    let draw = board.actual_draw(3).unwrap();
    let target = draw.row_target(session_id("s-2"));
    board.settle_draw(Some(AgentBoardDraw::Presented(draw)));
    assert!(board.select_drawn_row(&target));
    assert_eq!(board.selection, 1);
    // Selection changed the exact draw source, so the same painted target
    // cannot act twice or migrate onto the new projection.
    assert!(!board.select_drawn_row(&target));
    assert_eq!(board.selection, 1);
}

#[test]
fn local_navigation_keeps_the_surface_lease_but_revokes_activation_proof() {
    let mut board = board_with(&["s-1", "s-2", "s-3"]);
    board.open();
    let draw = presented(&board, 3);
    board.settle_draw(Some(draw));
    assert!(board.input_context().unwrap().interactive());
    assert!(board.selected_is_shown());

    board.move_selection(1);
    assert_eq!(board.selection, 1);
    assert!(
        board.input_context().unwrap().interactive(),
        "the still-visible owner accepts the next navigation key"
    );
    assert!(board.current_draw().is_none());
    assert!(
        !board.selected_is_shown(),
        "activation waits for a frame proving the new cursor"
    );

    board.move_selection(1);
    assert_eq!(board.selection, 2);
}

#[test]
fn draw_lifecycle_retains_an_admitted_board_across_cover_and_geometry_block() {
    let mut board = board_with(&["s-1"]);
    board.open();
    let episode = board.episode();
    assert!(board.owns_input());
    assert_eq!(board.presentation, PresentationPhase::Pending);
    assert!(!board.interactive());

    let first = presented(&board, 4);
    board.settle_draw(Some(first));
    assert_eq!(board.presentation, PresentationPhase::Admitted);
    assert!(board.interactive());
    assert!(board.owns_input());
    assert_eq!(board.viewport_rows, 4);

    assert_eq!(board.settle_draw(None), None);
    assert!(board.is_open());
    assert_eq!(board.episode(), episode);
    assert_eq!(board.presentation, PresentationPhase::Admitted);
    assert!(!board.interactive(), "covered draw proof is revoked");

    let uncovered = presented(&board, 4);
    board.settle_draw(Some(uncovered));
    assert!(board.interactive());
    assert_eq!(
        board.settle_draw(Some(AgentBoardDraw::NotPresented(episode))),
        Some(GeometrySettlement::Suspended)
    );
    assert!(board.is_open());
    assert_eq!(board.episode(), episode);
    assert_eq!(board.presentation, PresentationPhase::Suspended);
    assert!(!board.interactive());
    assert_eq!(board.viewport_rows, 4);

    let resumed = presented(&board, 2);
    board.settle_draw(Some(resumed));
    assert_eq!(board.presentation, PresentationPhase::Admitted);
    assert!(board.interactive());
    assert_eq!(board.viewport_rows, 2);
}

#[test]
fn first_geometry_block_refuses_and_a_stale_episode_cannot_reopen() {
    let mut board = board_with(&["s-1"]);
    board.open();
    let refused = board.episode();
    let stale_presented = presented(&board, 1);
    assert_eq!(
        board.settle_draw(Some(AgentBoardDraw::NotPresented(refused))),
        Some(GeometrySettlement::Refused)
    );
    assert!(!board.is_open());
    assert!(!board.owns_input());

    board.open();
    let replacement = board.episode();
    assert_ne!(replacement, refused);
    assert_eq!(
        board.settle_draw(Some(stale_presented)),
        None,
        "late paint from the refused episode is ignored"
    );
    assert_eq!(board.episode(), replacement);
    assert_eq!(board.presentation, PresentationPhase::Pending);
    assert!(!board.interactive());
}

#[test]
fn covered_board_rejects_painted_row_and_control_activation() {
    let mut board = AgentBoard::new(true);
    board.rows = vec![AgentBoardRow::from_summary(summary(
        "s-1",
        vec![attachment(&hex(9), true)],
    ))];
    board.open();
    board.intent = None;
    let draw = board.actual_draw(1).unwrap();
    let row = draw.row_target(session_id("s-1"));
    let open = draw.control_target(AgentBoardControl::Open);
    board.settle_draw(Some(AgentBoardDraw::Presented(draw)));
    board.settle_draw(None);

    assert!(!board.select_drawn_row(&row));
    assert!(!board.control_drawn(&open, Instant::now()));
    assert!(board.intent.is_none(), "hidden Open cannot queue a jump");
    assert_eq!(board.selection, 0);
}

#[test]
fn request_jump_refuses_a_row_without_navigable_attachment() {
    let mut board = AgentBoard::new(true);
    board.rows = vec![AgentBoardRow::from_summary(summary(
        "s-1",
        vec![attachment(&hex(9), false)],
    ))];
    board.open();
    board.request_jump();
    assert_eq!(board.notice, Some(JumpFailure::NoPlacement));
    assert!(board.jump.is_none());
    assert!(board.inflight.is_none());
    assert!(!matches!(board.intent, Some(BoardIntent::Jump { .. })));
}

#[test]
fn request_jump_on_a_navigable_row_queues_the_intent() {
    let mut board = AgentBoard::new(true);
    board.rows = vec![AgentBoardRow::from_summary(summary(
        "s-1",
        vec![attachment(&hex(9), true)],
    ))];
    board.open();
    let draw = presented(&board, 1);
    board.settle_draw(Some(draw));
    board.request_jump();
    assert!(board.notice.is_none());
    assert!(matches!(board.intent, Some(BoardIntent::Jump { .. })));
    assert!(board.input_context().unwrap().interactive());
    assert!(
        board.current_draw().is_none(),
        "starting a jump consumes the exact activation proof without hiding the Board"
    );
}

#[test]
fn expire_jump_reports_budget_exhaustion() {
    let mut board = AgentBoard::new(true);
    board.open();
    let deadline = Instant::now();
    board.jump = Some(BoardJump {
        serial: 1,
        stage: JumpStage::Checking,
        deadline,
    });
    assert!(!board.expire_jump(deadline - Duration::from_secs(1)));
    assert!(board.expire_jump(deadline + Duration::from_secs(1)));
    assert_eq!(board.notice, Some(JumpFailure::Expired));
    assert!(board.jump.is_none());
}

#[test]
fn jump_plan_walks_context_then_tab_then_panel() {
    let initial_workspace = {
        let fixture_context_id = context_id(1);
        ClientContextLocation::Workspace {
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap(),
            terminal_context_id: fixture_context_id,
        }
    };
    let workspace = workspace_location(2);
    let target = placement(workspace.clone(), 5, 6);
    let contexts = vec![initial_workspace.clone(), workspace.clone()];
    // Wrong context: select the owning 0-based position.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&initial_workspace),
            &contexts,
            Some(0),
            None,
            None,
            true,
            false,
            false
        ),
        JumpStep::SelectContext(1)
    );
    // A context selection already in flight waits.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&initial_workspace),
            &contexts,
            Some(0),
            None,
            None,
            true,
            true,
            false
        ),
        JumpStep::Wait
    );
    // The owning context vanished from navigation: candidate advances.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&initial_workspace),
            std::slice::from_ref(&initial_workspace),
            Some(0),
            None,
            None,
            true,
            false,
            false
        ),
        JumpStep::Advance
    );
    // Right context, but the tab no longer resolves in the cache.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            None,
            None,
            None,
            true,
            false,
            false
        ),
        JumpStep::Advance
    );
    // Right context, other tab active: select the tab's 1-based position.
    let other_tab = tab_id(7);
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            Some(1),
            Some(&other_tab),
            None,
            true,
            false,
            false
        ),
        JumpStep::SelectTab(2)
    );
    // A tab switch already in flight waits rather than double-issuing.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            Some(1),
            Some(&other_tab),
            None,
            true,
            false,
            true
        ),
        JumpStep::Wait
    );
    // Right context and tab, wrong panel focus on a live target: focus it.
    let other_panel = panel_id(8);
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            Some(1),
            Some(&target.tab_id),
            Some(&other_panel),
            true,
            false,
            false
        ),
        JumpStep::FocusPanel(target.panel_id.clone())
    );
    // Right context, tab, and focus on a live viewer: the jump landed.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            Some(1),
            Some(&target.tab_id),
            Some(&target.panel_id),
            true,
            false,
            false
        ),
        JumpStep::Landed
    );
}

#[test]
fn jump_plan_reattaches_a_dead_viewer_and_never_lands_on_it() {
    let workspace = workspace_location(2);
    let target = placement(workspace.clone(), 5, 6);
    let contexts = vec![workspace.clone()];
    // The resolved placement is already the active context+tab with the
    // target focused — but its viewer is dead. A `Landed` verdict here
    // would strand the user on a corpse pane; the planner demands the
    // same-tab reattach instead.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            Some(1),
            Some(&target.tab_id),
            Some(&target.panel_id),
            false,
            false,
            false
        ),
        JumpStep::Reattach {
            position: 2,
            panel_id: target.panel_id.clone()
        }
    );
    // A different focused panel with a dead target reattaches too —
    // focusing a dead record is not a legal step either.
    let other_panel = panel_id(8);
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            Some(1),
            Some(&target.tab_id),
            Some(&other_panel),
            false,
            false,
            false
        ),
        JumpStep::Reattach {
            position: 2,
            panel_id: target.panel_id.clone()
        }
    );
    // A switch already in flight waits — the reattach never double-mints.
    assert_eq!(
        plan_jump_step(
            &target,
            Some(&workspace),
            &contexts,
            Some(1),
            Some(&target.tab_id),
            Some(&target.panel_id),
            false,
            false,
            true
        ),
        JumpStep::Wait
    );
}

#[test]
fn a_new_jump_supersedes_at_request_time_while_the_lane_credit_stays_held() {
    let mut board = AgentBoard::new(true);
    board.rows = vec![
        AgentBoardRow::from_summary(summary("s-1", vec![attachment(&hex(9), true)])),
        AgentBoardRow::from_summary(summary("s-2", vec![attachment(&hex(10), true)])),
    ];
    board.open();
    // The prior jump is issued: serial 7's Get is in flight on the lane.
    board.jump = Some(BoardJump {
        serial: 7,
        stage: JumpStage::Checking,
        deadline: Instant::now() + Duration::from_secs(8),
    });
    board.inflight = Some(BoardInflight::JumpGet { serial: 7 });
    board.selection = 1;
    board.request_jump();
    // Superseded immediately — the old stage dies at the request, not
    // when its outcome happens to arrive.
    assert!(board.jump.is_none());
    assert!(matches!(
        board.intent,
        Some(BoardIntent::Jump { ref target }) if *target == session_id("s-2")
    ));
    // The physical credit stays held: the lane's serial-7 Get still owes
    // an outcome, and the queued jump may not overlap it.
    assert!(matches!(
        board.inflight,
        Some(BoardInflight::JumpGet { serial: 7 })
    ));
    assert!(!board.drive_intent(Instant::now(), accepted));
    assert!(matches!(board.intent, Some(BoardIntent::Jump { .. })));
}

#[test]
fn a_refused_enter_preserves_the_prior_jump() {
    let mut board = AgentBoard::new(true);
    board.rows = vec![AgentBoardRow::from_summary(summary(
        "s-1",
        vec![attachment(&hex(9), false)],
    ))];
    board.open();
    board.jump = Some(BoardJump {
        serial: 7,
        stage: JumpStage::Checking,
        deadline: Instant::now() + Duration::from_secs(8),
    });
    board.request_jump();
    assert_eq!(board.notice, Some(JumpFailure::NoPlacement));
    // The rejected request never touched the live jump or its intent.
    assert!(board.jump.as_ref().is_some_and(|jump| jump.serial == 7));
    assert!(!matches!(board.intent, Some(BoardIntent::Jump { .. })));
}

#[test]
fn fail_jump_drops_the_stage_but_never_the_lane_credit() {
    let mut board = AgentBoard::new(true);
    board.open();
    board.jump = Some(BoardJump {
        serial: 7,
        stage: JumpStage::Checking,
        deadline: Instant::now() + Duration::from_secs(8),
    });
    board.inflight = Some(BoardInflight::JumpGet { serial: 7 });
    board.fail_jump(JumpFailure::Expired);
    assert_eq!(board.notice, Some(JumpFailure::Expired));
    assert!(board.jump.is_none());
    // The credit is physical — only the matching outcome or the lane
    // thread's exit releases it, never a logical failure.
    assert!(matches!(
        board.inflight,
        Some(BoardInflight::JumpGet { serial: 7 })
    ));
}

#[test]
fn reopen_preserves_last_good_age_and_rides_out_the_residual_cooldown() {
    let mut board = AgentBoard::new(true);
    board.open();
    board.rows = vec![AgentBoardRow::from_summary(summary("s-1", vec![]))];
    board.rows_from_complete = true;
    let installed = Instant::now();
    board.rows_at = Some(installed);
    board.last_read_end = Some(installed);
    board.last_summary = Some(AgentBoardSummary {
        sessions: 1,
        complete: true,
        at: installed,
    });
    board.close();
    // Reopen inside the cooldown window: last-good rows, their install
    // age, and the sidebar summary all survive the close.
    board.open();
    assert_eq!(board.rows_at, Some(installed));
    let summary = board.summary().expect("summary survives the reopen");
    assert_eq!(
        (summary.sessions, summary.complete, summary.at),
        (1, true, installed)
    );
    // The open-time refresh waits out the residual cooldown instead of
    // issuing early or being dropped.
    assert!(!board.drive_intent(installed + Duration::from_secs(1), no_lane));
    assert!(matches!(board.intent, Some(BoardIntent::Refresh)));
    // Past the cooldown the same queued intent issues for real — refused
    // for want of a lane, the attempt reports the honest lane failure.
    assert!(board.drive_intent(installed + Duration::from_secs(3), no_lane));
    assert_eq!(
        board.read,
        AgentBoardRead::Failed(AgentBoardReadFailure::LaneUnavailable)
    );
    assert!(board.intent.is_none());
}

fn presentation_snapshot() -> Snapshot {
    let context = context_id(2);
    let presentation = TerminalPresentationId::parse(&hex(42)).unwrap();
    Snapshot {
        navigation: {
            let fixture_context_id = context.clone();
            let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap();
            Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                    terminal_directory: None,
                    automatic_label: false,
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(Vec::new())
                .collect(),
                workspace_order: Vec::new(),
                projects: Vec::new(),
            }
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(1),
        session_id: SessionId::parse(&hex(1)).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            terminal_context_id: context,
            tabs: vec![Tab {
                automatic_label: false,
                group_id: None,
                pinned: false,
                label: "Tab".into(),
                tab_id: tab_id(5),
                root: PanelNode::Panel {
                    right_click: Default::default(),
                    child_presentations: Vec::new(),
                    panel_id: panel_id(6),
                    primary_presentation: Presentation {
                        custom_label: None,
                        label: "Terminal".to_owned(),
                        presentation_id: presentation.clone(),
                        binding: Some(PresentationBinding {
                            terminal_id: TerminalId::parse(&hex(7)).unwrap(),
                            worker_id: WorkerId::parse(&hex(8)).unwrap(),
                        }),
                        launch_failure: None,
                        agent_not_resumed: None,
                        launch_program: None,
                    },
                },
            }],
        }],
    }
}

#[test]
fn presentation_index_resolves_only_live_panel_identities() {
    let snapshot = presentation_snapshot();
    let index = PresentationIndex::rebuild(&snapshot);
    assert_eq!(index.len(), 1);
    // The attachment's stable presentation identity lands on the exact
    // context/tab/panel the canonical snapshot projects.
    let hit = index
        .placement(&attachment(&hex(42), true))
        .expect("registered presentation resolves");
    assert_eq!(hit.tab_id, tab_id(5));
    assert_eq!(hit.panel_id, panel_id(6));
    assert_eq!(hit.location, {
        let fixture_context_id = context_id(2);
        ClientContextLocation::Workspace {
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap(),
            terminal_context_id: fixture_context_id,
        }
    });
    // An identity the snapshot never painted resolves to nothing — the
    // jump's Gone path, never a guess.
    assert!(index.placement(&attachment(&hex(43), true)).is_none());
    let mut with_child = snapshot;
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut with_child.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    child_presentations.push(Presentation {
        presentation_id: TerminalPresentationId::parse(&hex(43)).unwrap(),
        label: "child".into(),
        custom_label: None,
        binding: Some(PresentationBinding {
            terminal_id: TerminalId::parse(&hex(44)).unwrap(),
            worker_id: WorkerId::parse(&hex(8)).unwrap(),
        }),
        launch_failure: None,
        agent_not_resumed: None,
        launch_program: None,
    });
    let index = PresentationIndex::rebuild(&with_child);
    assert_eq!(index.len(), 2);
    let child = index.placement(&attachment(&hex(43), true)).unwrap();
    assert_eq!(child.panel_id, panel_id(6));
}

#[test]
fn request_refresh_preserves_a_queued_jump_intent() {
    let now = Instant::now();
    let mut board = AgentBoard::new(true);
    board.rows = vec![AgentBoardRow::from_summary(summary("s-1", vec![]))];
    board.open();
    // Enter queued a jump behind a busy round — `r` is consumed, never
    // a replacement.
    board.intent = Some(BoardIntent::Jump {
        target: session_id("s-1"),
    });
    board.request_refresh(now);
    assert!(matches!(board.intent, Some(BoardIntent::Jump { .. })));
    // A completed round then a fully elapsed cooldown — the queued jump
    // still holds the single intent slot.
    board.last_read_end = Some(now);
    let later = now + Duration::from_secs(10);
    board.request_refresh(later);
    assert!(matches!(board.intent, Some(BoardIntent::Jump { .. })));
    // With no jump queued the same request still queues a refresh.
    board.intent = None;
    board.request_refresh(later);
    assert!(matches!(board.intent, Some(BoardIntent::Refresh)));
}

mod read_answers {
    use super::*;
    use orchbee_agent_observation_protocol::{
        AgentCapability, AgentSessionDetail, AgentSessionSnapshot, AgentTurnState,
    };

    fn settle_list(
        board: &mut AgentBoard,
        items: Vec<AgentSessionSummary>,
        complete: bool,
        now: Instant,
    ) {
        let episode = board.episode();
        board.settle_read(
            AgentBoardReadEvent::Answer(AgentBoardReadAnswer::List {
                episode,
                result: Ok(AgentListRound { items, complete }),
            }),
            now,
            &mut no_preference,
        );
    }

    fn settle_get(
        board: &mut AgentBoard,
        serial: u64,
        result: Result<AgentGetOutcome, AgentReadFailure>,
    ) {
        let episode = board.episode();
        board.settle_read(
            AgentBoardReadEvent::Answer(AgentBoardReadAnswer::Get {
                episode,
                serial,
                result,
            }),
            Instant::now(),
            &mut no_preference,
        );
    }

    #[test]
    fn client_path_catalog_waits_for_the_same_open_episodes_complete_empty_read() {
        let mut board = AgentBoard::new(true);
        let mut jobs = start::jobs::Jobs::default();
        board.set_origin_locality(false);
        board.open_with_start_catalog(Some(std::ffi::OsString::from("C:\\client-bin")));
        assert!(!board.drain_start(Instant::now(), |start, admission| {
            jobs.drain(start, admission)
        }));
        assert_eq!(board.start.catalog_status(), start::CatalogStatus::Loading);

        board.inflight = Some(BoardInflight::ListRound);
        settle_list(&mut board, vec![], false, Instant::now());
        assert!(!board.drain_start(Instant::now(), |start, admission| {
            jobs.drain(start, admission)
        }));
        board.inflight = Some(BoardInflight::ListRound);
        settle_list(
            &mut board,
            vec![summary("s-1", vec![])],
            true,
            Instant::now(),
        );
        assert!(!board.drain_start(Instant::now(), |start, admission| {
            jobs.drain(start, admission)
        }));

        board.inflight = Some(BoardInflight::ListRound);
        settle_list(&mut board, vec![], true, Instant::now());
        assert!(board.drain_start(Instant::now(), |start, admission| {
            jobs.drain(start, admission)
        }));
        assert_eq!(
            board.start.catalog_status(),
            start::CatalogStatus::Unavailable
        );
        board.close();
        assert!(
            !board.drain_start(Instant::now(), |start, admission| jobs
                .drain(start, admission)),
            "close fences deferred probe admission"
        );
        board.open_with_start_catalog(Some(std::ffi::OsString::from("C:\\client-bin")));
        assert!(matches!(
            board.read,
            AgentBoardRead::Ready { complete: true, .. }
        ));
        assert!(
            !board.drain_start(Instant::now(), |start, admission| jobs
                .drain(start, admission)),
            "a previous episode's complete-empty result cannot admit a probe"
        );
        board.close();
    }

    #[test]
    fn list_outcome_replaces_rows_and_marks_incomplete() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.inflight = Some(BoardInflight::ListRound);
        settle_list(
            &mut board,
            vec![summary("s-1", vec![]), summary("s-2", vec![])],
            false,
            Instant::now(),
        );
        assert_eq!(board.rows.len(), 2);
        assert!(matches!(
            board.read,
            AgentBoardRead::Ready {
                complete: false,
                ..
            }
        ));
        assert!(board.inflight.is_none());
    }

    #[test]
    fn stale_epoch_outcome_is_dropped_without_touching_rows() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.inflight = Some(BoardInflight::ListRound);
        board.settle_read(
            AgentBoardReadEvent::Answer(AgentBoardReadAnswer::List {
                episode: AgentBoardEpisodeToken(board.epoch + 1),
                result: Ok(AgentListRound {
                    items: vec![summary("s-1", vec![])],
                    complete: true,
                }),
            }),
            Instant::now(),
            &mut no_preference,
        );
        assert!(board.rows.is_empty());
        // The in-flight marker stays: the current epoch's request is
        // still owed an outcome.
        assert!(matches!(board.inflight, Some(BoardInflight::ListRound)));
    }

    #[test]
    fn an_incomplete_round_keeps_the_last_good_rows_install_age() {
        // The bug lived on the real path: issue_list_round's InFlight
        // transition hid the installed rows' timestamp from
        // apply_list_outcome. This drives the issue → InFlight transition
        // for real instead of fabricating a Ready read state.
        let mut board = AgentBoard::new(true);
        board.open();
        let t0 = Instant::now();
        assert!(board.drive_intent(t0, accepted));
        assert!(matches!(board.read, AgentBoardRead::InFlight));
        assert!(matches!(board.inflight, Some(BoardInflight::ListRound)));
        settle_list(
            &mut board,
            vec![summary("s-1", vec![]), summary("s-2", vec![])],
            true,
            t0,
        );
        assert!(matches!(
            board.read,
            AgentBoardRead::Ready {
                complete: true,
                at
            } if at == t0
        ));
        // Past the cooldown, `r` queues a second real round.
        let t1 = t0 + Duration::from_secs(5);
        board.request_refresh(t1);
        assert!(board.drive_intent(t1, accepted));
        assert!(matches!(board.read, AgentBoardRead::InFlight));
        settle_list(&mut board, vec![summary("s-1", vec![])], false, t1);
        // The kept last_good rows carry their install age — not the
        // partial's landing time — through the read state and the
        // sidebar summary alike.
        assert!(matches!(
            board.read,
            AgentBoardRead::Ready {
                complete: false,
                at
            } if at == t0
        ));
        let summary = board.summary().expect("a completed round leaves a summary");
        assert_eq!(
            (summary.sessions, summary.complete, summary.at),
            (2, false, t0)
        );
    }

    #[test]
    fn a_refresh_repairs_selection_by_identity_not_the_old_index() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.inflight = Some(BoardInflight::ListRound);
        settle_list(
            &mut board,
            vec![
                summary("s-1", vec![]),
                summary("s-2", vec![]),
                summary("s-3", vec![]),
            ],
            true,
            Instant::now(),
        );
        assert!(board.select_session(&session_id("s-3")));
        assert_eq!(board.selection, 2);
        // The refresh drops s-3 and fills the list so a *different*
        // session occupies index 2 — an index-following repair would
        // land on it.
        board.inflight = Some(BoardInflight::ListRound);
        settle_list(
            &mut board,
            vec![
                summary("s-1", vec![]),
                summary("s-4", vec![]),
                summary("s-5", vec![]),
            ],
            true,
            Instant::now(),
        );
        assert_eq!(board.selection, 0);
        assert_eq!(board.rows[board.selection].session_id(), &session_id("s-1"));
    }

    #[test]
    fn a_refresh_preserves_the_manual_top_row_independently_of_selection() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.viewport_rows = 2;
        board.inflight = Some(BoardInflight::ListRound);
        settle_list(
            &mut board,
            vec![
                summary("s-1", vec![]),
                summary("s-2", vec![]),
                summary("s-3", vec![]),
                summary("s-4", vec![]),
            ],
            true,
            Instant::now(),
        );
        board.selection = 0;
        board.scroll = 2;

        board.inflight = Some(BoardInflight::ListRound);
        settle_list(
            &mut board,
            vec![
                summary("s-new", vec![]),
                summary("s-1", vec![]),
                summary("s-2", vec![]),
                summary("s-3", vec![]),
                summary("s-4", vec![]),
            ],
            true,
            Instant::now(),
        );

        assert_eq!(board.rows[board.selection].session_id(), &session_id("s-1"));
        assert_eq!(
            board.rows[board.scroll].session_id(),
            &session_id("s-3"),
            "the refresh follows the stable top-row id instead of snapping to selection"
        );
    }

    fn detail(
        session: &str,
        lifecycle: AgentLifecycle,
        attachments: Vec<AgentPresentationAttachment>,
    ) -> AgentSessionDetail {
        AgentSessionDetail {
            snapshot: AgentSessionSnapshot {
                activity_hint: None,
                capabilities: vec![AgentCapability::SessionLifecycle],
                execution_location: AgentExecutionLocation {
                    execution_location_id: AgentExecutionLocationId::parse("local").unwrap(),
                },
                last_outcome: None,
                lifecycle,
                observation: AgentObservation {
                    fidelity: AgentObservationFidelity::Basic,
                    freshness: AgentObservationFreshness::Stale,
                    generation: AgentSafeInteger::new(2),
                    sequence: AgentSafeInteger::new(9),
                    source: AgentObservationSource::Hook,
                },
                parent_session_id: None,
                pending_requests: Vec::new(),
                presentation_attachments: attachments,
                provider_id: AgentProviderId::parse("devin").unwrap(),
                session_id: session_id(session),
                turn: AgentTurnState::Idle {},
            },
            status: AgentSemanticStatus::Idle {},
        }
    }

    #[test]
    fn get_outcome_orders_candidates_and_starts_navigating() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.jump = Some(BoardJump {
            serial: 7,
            stage: JumpStage::Checking,
            deadline: Instant::now() + Duration::from_secs(5),
        });
        board.inflight = Some(BoardInflight::JumpGet { serial: 7 });
        settle_get(
            &mut board,
            7,
            Ok(AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(detail(
                    "s-1",
                    AgentLifecycle::Live,
                    vec![attachment(&hex(9), true), attachment(&hex(10), true)],
                )),
            }),
        );
        let Some(jump) = board.jump.as_ref() else {
            panic!("jump survives the verified get");
        };
        let JumpStage::Navigating { candidates, index } = &jump.stage else {
            panic!("found outcome enters navigation");
        };
        assert_eq!(*index, 0);
        assert_eq!(candidates.len(), 2);
        // Deterministic order: ascending stable presentation identity
        // when nothing prefers the active tab.
        assert_eq!(candidates[0].presentation_id.as_str(), hex(9).as_str());
        assert_eq!(candidates[1].presentation_id.as_str(), hex(10).as_str());
    }

    #[test]
    fn get_outcome_candidate_exhaustion_fails_gone() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.jump = Some(BoardJump {
            serial: 7,
            stage: JumpStage::Navigating {
                candidates: vec![attachment(&hex(9), true)].into_boxed_slice(),
                index: 0,
            },
            deadline: Instant::now() + Duration::from_secs(5),
        });
        assert!(board.advance_jump_candidate());
        assert_eq!(board.notice, Some(JumpFailure::Gone));
        assert!(board.jump.is_none());
    }

    #[test]
    fn get_outcome_ended_lifecycle_refuses_before_navigation() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.jump = Some(BoardJump {
            serial: 7,
            stage: JumpStage::Checking,
            deadline: Instant::now() + Duration::from_secs(5),
        });
        board.inflight = Some(BoardInflight::JumpGet { serial: 7 });
        settle_get(
            &mut board,
            7,
            Ok(AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(detail("s-1", AgentLifecycle::Ended, vec![])),
            }),
        );
        assert_eq!(board.notice, Some(JumpFailure::Ended));
        assert!(board.jump.is_none());
    }

    #[test]
    fn get_outcome_for_a_superseded_serial_is_dropped() {
        let mut board = AgentBoard::new(true);
        board.open();
        board.jump = Some(BoardJump {
            serial: 8,
            stage: JumpStage::Checking,
            deadline: Instant::now() + Duration::from_secs(5),
        });
        board.inflight = Some(BoardInflight::JumpGet { serial: 8 });
        // Serial 7's outcome arrives after serial 8 took over — dropped.
        settle_get(
            &mut board,
            7,
            Ok(AgentGetOutcome::Found {
                revision: AgentSafeInteger::new(1),
                session: Box::new(detail("s-1", AgentLifecycle::Live, vec![])),
            }),
        );
        assert!(matches!(
            board.inflight,
            Some(BoardInflight::JumpGet { serial: 8 })
        ));
        assert!(matches!(
            board.jump.as_ref().map(|jump| &jump.stage),
            Some(JumpStage::Checking)
        ));
    }

    #[test]
    fn a_dead_lanes_last_matching_answer_keeps_its_verdict_over_the_end() {
        // The lane's final answer was still queued when its thread ended,
        // so it settles before the end: the serial match releases the
        // credit with the answer's own verdict (Gone, not the end's
        // Unavailable), and the end then has nothing left to fail.
        let mut board = AgentBoard::new(true);
        board.open();
        board.jump = Some(BoardJump {
            serial: 5,
            stage: JumpStage::Checking,
            deadline: Instant::now() + Duration::from_secs(8),
        });
        board.inflight = Some(BoardInflight::JumpGet { serial: 5 });
        settle_get(
            &mut board,
            5,
            Ok(AgentGetOutcome::Rejected {
                reason: orchbee_agent_observation_protocol::AgentSessionRejectionReason::NotFound,
            }),
        );
        board.settle_read(
            AgentBoardReadEvent::LaneEnded,
            Instant::now(),
            &mut no_preference,
        );
        assert_eq!(board.notice, Some(JumpFailure::Gone));
        assert!(board.jump.is_none());
        assert!(board.inflight.is_none());
    }

    #[test]
    fn a_dead_lane_releases_residual_credit_as_unavailable() {
        // An issued Get whose answer will never arrive: the command
        // landed on a channel whose thread was already gone. The lane's
        // end is the only witness, and it releases the credit into the
        // honest Unavailable verdict.
        let mut board = AgentBoard::new(true);
        board.open();
        board.jump = Some(BoardJump {
            serial: 5,
            stage: JumpStage::Checking,
            deadline: Instant::now() + Duration::from_secs(8),
        });
        board.inflight = Some(BoardInflight::JumpGet { serial: 5 });
        board.settle_read(
            AgentBoardReadEvent::LaneEnded,
            Instant::now(),
            &mut no_preference,
        );
        assert_eq!(board.notice, Some(JumpFailure::Unavailable));
        assert!(board.jump.is_none());
        assert!(board.inflight.is_none());
    }

    #[test]
    fn a_dead_lane_drops_stale_answers_before_releasing_the_held_credit() {
        // A superseded serial's answer settles first and is dropped by its
        // own correlation — it must not release the live credit nor fail
        // the live jump. The lane's end then releases the still-held
        // credit into Unavailable.
        let mut board = AgentBoard::new(true);
        board.open();
        board.jump = Some(BoardJump {
            serial: 5,
            stage: JumpStage::Checking,
            deadline: Instant::now() + Duration::from_secs(8),
        });
        board.inflight = Some(BoardInflight::JumpGet { serial: 5 });
        settle_get(
            &mut board,
            4,
            Ok(AgentGetOutcome::Unavailable {
                reason:
                    orchbee_agent_observation_protocol::AgentUnavailableReason::RuntimeUnavailable,
            }),
        );
        assert!(board.notice.is_none(), "the stale answer changes nothing");
        board.settle_read(
            AgentBoardReadEvent::LaneEnded,
            Instant::now(),
            &mut no_preference,
        );
        assert_eq!(board.notice, Some(JumpFailure::Unavailable));
        assert!(board.jump.is_none());
        assert!(board.inflight.is_none());
    }

    #[test]
    fn a_dead_lane_releases_a_held_list_credit_as_unavailable() {
        // The same residual-credit witness for the read side: the
        // outstanding ListRound fails Unavailable — not Transport and
        // never silently InFlight — once the end is witnessed, and the
        // cooldown starts from the witnessed failure.
        let mut board = AgentBoard::new(true);
        board.open();
        board.inflight = Some(BoardInflight::ListRound);
        board.read = AgentBoardRead::InFlight;
        let now = Instant::now();
        board.settle_read(AgentBoardReadEvent::LaneEnded, now, &mut no_preference);
        assert_eq!(
            board.read,
            AgentBoardRead::Failed(AgentBoardReadFailure::Unavailable)
        );
        assert!(board.inflight.is_none());
        assert_eq!(board.last_read_end, Some(now));
    }
}
