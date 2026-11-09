use super::{displayed_terminal, first_terminal};
use orchbee_workbench_protocol::{
    CurrentDirectoryPanel, Navigation, PanelNode, PanelOrientation, PanelSlot, Presentation,
    PresentationBinding, ProtocolVersion, SessionId, Snapshot, Tab, TerminalContext, TerminalId,
    TerminalPresentationId, WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchRevision,
    WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
    Workspace,
};

fn id(value: u128) -> String {
    format!("{value:032x}")
}

fn binding(terminal: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&id(terminal)).unwrap(),
        worker_id: WorkerId::parse(&id(1)).unwrap(),
    }
}

fn presentation(presentation: u128, terminal: Option<u128>) -> Presentation {
    Presentation {
        binding: terminal.map(binding),
        custom_label: None,
        label: "pwsh".to_owned(),
        launch_failure: None,
        agent_not_resumed: None,
        launch_program: None,
        presentation_id: TerminalPresentationId::parse(&id(presentation)).unwrap(),
    }
}

fn panel(panel: u128, primary: Presentation, children: Vec<Presentation>) -> PanelNode {
    PanelNode::Panel {
        right_click: Default::default(),
        child_presentations: children,
        panel_id: WorkbenchPanelId::parse(&id(panel)).unwrap(),
        primary_presentation: primary,
    }
}

fn target(panel: u128, presentation: u128) -> CurrentDirectoryPanel {
    CurrentDirectoryPanel {
        panel_id: WorkbenchPanelId::parse(&id(panel)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(presentation)).unwrap(),
    }
}

fn workspace(workspace: u128, context: u128) -> Workspace {
    Workspace {
        automatic_label: false,
        label: "Workspace".to_owned(),
        project_id: None,
        primary_directory_id: None,
        terminal_context_id: WorkbenchTerminalContextId::parse(&id(context)).unwrap(),
        terminal_directory: None,
        workspace_id: WorkbenchWorkspaceId::parse(&id(workspace)).unwrap(),
    }
}

fn context(context: u128, tab: u128, root: PanelNode) -> TerminalContext {
    TerminalContext {
        groups: Vec::new(),
        tabs: vec![Tab {
            automatic_label: false,
            group_id: None,
            label: "1".to_owned(),
            pinned: false,
            root,
            tab_id: WorkbenchTabId::parse(&id(tab)).unwrap(),
        }],
        terminal_context_id: WorkbenchTerminalContextId::parse(&id(context)).unwrap(),
    }
}

/// One Workspace split into Panel 10 (terminal 1) and Panel 20 (terminal 2, showing child
/// terminal 3 as well), beside another Workspace whose Panel 30 holds terminal 4.
fn split_workspace() -> Snapshot {
    let split = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: panel(10, presentation(11, Some(1)), Vec::new()),
                slot_id: WorkbenchPanelSlotId::parse(&id(12)).unwrap(),
            },
            PanelSlot {
                node: panel(
                    20,
                    presentation(21, Some(2)),
                    vec![presentation(22, Some(3)), presentation(23, None)],
                ),
                slot_id: WorkbenchPanelSlotId::parse(&id(24)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        ratio: Default::default(),
        split_id: WorkbenchSplitId::parse(&id(25)).unwrap(),
    };
    Snapshot {
        navigation: Navigation {
            directories: Vec::new(),
            projects: Vec::new(),
            workspaces: vec![workspace(100, 101), workspace(200, 201)],
            workspace_order: Vec::new(),
            project_order: Vec::new(),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(1),
        session_id: SessionId::parse(&id(9)).unwrap(),
        terminal_contexts: vec![
            context(101, 102, split),
            context(201, 202, panel(30, presentation(31, Some(4)), Vec::new())),
        ],
    }
}

/// The Workspace form reads the first terminal; the Panel form reads the terminal the asking
/// Client shows in that Panel, a child included. A Panel or Presentation the Workspace does not
/// hold is not found, and an unbound one leaves the Workspace's own directory to answer.
#[test]
fn a_panel_read_answers_for_the_terminal_that_panel_shows() {
    let snapshot = split_workspace();
    let workspace = &snapshot.navigation.workspaces[0];
    assert_eq!(first_terminal(&snapshot, workspace), Some(&binding(1)));
    assert_eq!(
        displayed_terminal(&snapshot, workspace, &target(20, 21)),
        Ok(Some(&binding(2)))
    );
    assert_eq!(
        displayed_terminal(&snapshot, workspace, &target(20, 22)),
        Ok(Some(&binding(3)))
    );
    assert_eq!(
        displayed_terminal(&snapshot, workspace, &target(20, 23)),
        Ok(None)
    );
    for missing in [target(30, 31), target(20, 11), target(40, 41)] {
        assert_eq!(displayed_terminal(&snapshot, workspace, &missing), Err(()));
    }
}
