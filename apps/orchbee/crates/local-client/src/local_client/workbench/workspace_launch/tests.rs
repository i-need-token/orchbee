use super::*;
fn action() -> WorkbenchAction {
    WorkbenchAction::WorkspaceLaunch {
        automatic_label: false,
        placement: orchbee_workbench_protocol::WorkspacePlacement::Project {
            project_id: WorkbenchProjectId::parse(&format!("{:032x}", 1)).unwrap(),
        },
        label: orchbee_workbench_protocol::WorkbenchLabel::parse("test").unwrap(),
        geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
        launch: None,
        profile: None,
        options: Box::new(orchbee_workbench_protocol::LaunchOptions {
            plugin_pane: None,
            plugin_context: None,
            cwd: Some(
                orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(r"C:\repo").unwrap(),
            ),
            environment: Default::default(),
        }),
    }
}
fn outcome() -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::WorkspaceLaunchPending {
        revision: WorkbenchRevision::new(2),
        project_id: Some(WorkbenchProjectId::parse(&format!("{:032x}", 1)).unwrap()),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 2)).unwrap(),
        directory_id: WorkbenchDirectoryId::parse(&format!("{:032x}", 3)).unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 4)).unwrap(),
        tab_id: WorkbenchTabId::parse(&format!("{:032x}", 5)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 6)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 7)).unwrap(),
    }
}
#[test]
fn workspace_launch_only_accepts_correlated_unique_committed_resources() {
    let action = action();
    let revision = WorkbenchRevision::new(1);
    assert!(matches!(
        classify_outcome(&action, revision, outcome()),
        Ok(WorkspaceLaunchSubmission::Committed(_))
    ));
    for field in ["projectId", "workspaceId", "revision"] {
        let mut wire = serde_json::to_value(outcome()).unwrap();
        wire[field] = if field == "revision" {
            serde_json::json!("3")
        } else {
            serde_json::json!(format!("{:032x}", 7))
        };
        assert!(
            classify_outcome(&action, revision, serde_json::from_value(wire).unwrap()).is_err()
        );
    }
    assert!(
        classify_outcome(
            &action,
            revision,
            WorkbenchCommandOutcome::WorkspaceCreated {
                revision: WorkbenchRevision::new(2),
                workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 2)).unwrap(),
                terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 4))
                    .unwrap(),
            }
        )
        .is_err()
    );
    assert!(matches!(
        classify_outcome(
            &action,
            revision,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::DirectoryUnavailable,
            }
        ),
        Ok(WorkspaceLaunchSubmission::DefinitePrecommit(_))
    ));
}
