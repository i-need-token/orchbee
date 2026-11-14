//! Picker owner and presentation index unit tests: the draw lifecycle's admission, suspension
//! and refusal, the painted-verdict gate on controls, and the presentation index join.

use super::*;
use crate::workbench_projection::ClientContextLocation;
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, SessionId, Snapshot,
    Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchPanelId, WorkbenchProjectId,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
};

fn opened() -> AgentBoard {
    let mut board = AgentBoard::new(true);
    board.open_picker(None, false);
    board.install_start_catalog_t(&["codex"]);
    board
}

fn presented(board: &AgentBoard, visible_rows: usize) -> AgentBoardDraw {
    AgentBoardDraw::Presented(
        board
            .actual_draw(visible_rows)
            .expect("the open picker has a non-empty viewport"),
    )
}

fn shown_rows(board: &AgentBoard) -> Option<usize> {
    board.shown.as_ref().map(AgentBoardActualDraw::visible_rows)
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

fn tab_id(seed: u64) -> WorkbenchTabId {
    WorkbenchTabId::parse(&hex(seed)).unwrap()
}

fn panel_id(seed: u64) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&hex(seed)).unwrap()
}

fn context_id(seed: u64) -> WorkbenchTerminalContextId {
    WorkbenchTerminalContextId::parse(&hex(seed)).unwrap()
}

#[test]
fn draw_lifecycle_retains_an_admitted_picker_across_cover_and_geometry_block() {
    let mut board = opened();
    let episode = board.episode();
    assert!(board.owns_input());
    assert_eq!(board.presentation, PresentationPhase::Pending);
    assert!(!board.interactive());

    let first = presented(&board, 4);
    board.settle_draw(Some(first));
    assert_eq!(board.presentation, PresentationPhase::Admitted);
    assert!(board.interactive());
    assert!(board.owns_input());
    assert_eq!(shown_rows(&board), Some(4));

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

    let resumed = presented(&board, 2);
    board.settle_draw(Some(resumed));
    assert_eq!(board.presentation, PresentationPhase::Admitted);
    assert!(board.interactive());
    assert_eq!(shown_rows(&board), Some(2));
}

#[test]
fn first_geometry_block_refuses_and_a_stale_episode_cannot_reopen() {
    let mut board = opened();
    let refused = board.episode();
    let stale_presented = presented(&board, 1);
    assert_eq!(
        board.settle_draw(Some(AgentBoardDraw::NotPresented(refused))),
        Some(GeometrySettlement::Refused)
    );
    assert!(!board.is_open());
    assert!(!board.owns_input());

    board.open_picker(None, false);
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
fn covered_picker_rejects_painted_control_activation() {
    let mut board = opened();
    let draw = board.actual_draw(4).unwrap();
    let place = draw.control_target(AgentBoardControl::Place);
    let before = board.picker().unwrap().place().clone();
    board.settle_draw(Some(AgentBoardDraw::Presented(draw)));
    board.settle_draw(None);

    assert!(!board.drawn(&place));
    assert_eq!(board.picker_control(&place), picker::PickerOutcome::Handled);
    assert_eq!(
        board.picker().unwrap().place(),
        &before,
        "a hidden control acts on nothing"
    );
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
                        agent_start_failure: None,
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
    // The stable presentation identity lands on the exact
    // context/tab/panel the canonical snapshot projects.
    let hit = index
        .placement_by_presentation(&hex(42))
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
    // An identity the snapshot never painted resolves to nothing, never a guess.
    assert!(index.placement_by_presentation(&hex(43)).is_none());
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
        agent_start_failure: None,
        launch_program: None,
    });
    let index = PresentationIndex::rebuild(&with_child);
    assert_eq!(index.len(), 2);
    let child = index.placement_by_presentation(&hex(43)).unwrap();
    assert_eq!(child.panel_id, panel_id(6));
}
