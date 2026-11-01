use super::*;
use orchbee_workbench_protocol::{Project, WorkbenchProjectId, WorkbenchWorkspaceId, Workspace};

fn workspace_snapshot(revision: u64) -> Snapshot {
    let mut snapshot = snapshot(2, revision);
    let work_context = snapshot.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    snapshot.navigation.workspaces[0].terminal_context_id = context_id(99);
    snapshot.terminal_contexts.push(terminal_context(99, 0));
    snapshot.navigation.workspaces.extend(vec![Workspace {
        terminal_directory: None,
        automatic_label: false,
        project_id: Some((WorkbenchProjectId::parse(&id(90)).unwrap()).clone()),
        workspace_id: WorkbenchWorkspaceId::parse(&id(91)).unwrap(),
        label: "work".into(),
        primary_directory_id: None,
        terminal_context_id: work_context,
    }]);
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&id(90)).unwrap(),
        label: "project".into(),
    });
    snapshot
}
fn projected(snapshot: &Snapshot) -> ClientContextTabs {
    let workspace = &snapshot.navigation.workspaces[1];
    project_native_context_tabs(
        snapshot,
        &ClientContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        },
    )
    .unwrap()
}

#[test]
fn workspace_replacement_requires_fresh_presentation_and_terminal_and_retains_worker_and_label() {
    let before = workspace_snapshot(10);
    let current = projected(&before);
    for case in 0..8 {
        let mut next = workspace_snapshot(if case == 7 { 10 } else { 11 });
        let other = current.tabs()[1].panel(0).unwrap().clone();
        let PanelNode::Panel {
            primary_presentation: presentation,
            ..
        } = &mut next.terminal_contexts[0].tabs[0].root
        else {
            panic!("panel")
        };
        presentation.presentation_id = presentation_id(9000);
        presentation.binding.as_mut().unwrap().terminal_id = terminal_id(9001);
        match case {
            1 => {
                presentation.presentation_id = current.tabs()[0]
                    .panel(0)
                    .unwrap()
                    .presentation_id()
                    .clone()
            }
            2 => {
                presentation.binding.as_mut().unwrap().terminal_id = current.tabs()[0]
                    .panel(0)
                    .unwrap()
                    .binding()
                    .expect("bound fixture")
                    .terminal_id
                    .clone()
            }
            3 => presentation.binding.as_mut().unwrap().worker_id = worker_id(9002),
            4 => presentation.label = "changed identity label".into(),
            5 => presentation.presentation_id = other.presentation_id().clone(),
            6 => {
                presentation.binding.as_mut().unwrap().terminal_id =
                    other.binding().expect("bound fixture").terminal_id.clone()
            }
            _ => {}
        }
        // The projector itself can reject duplicate identities before canonical comparison.
        let candidate = project_native_context_tabs(&next, current.location());
        let accepted = candidate
            .is_ok_and(|candidate| validate_canonical_refresh(&current, candidate).is_ok());
        assert_eq!(accepted, case == 0, "case {case}");
    }
    let mut initial_workspace = snapshot(2, 11);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut initial_workspace.terminal_contexts[0].tabs[0].root
    else {
        panic!("panel")
    };
    primary_presentation.presentation_id = presentation_id(9000);
    primary_presentation.binding.as_mut().unwrap().terminal_id = terminal_id(9001);
    // The first Workspace follows the same replacement contract as every other one.
    assert!(
        validate_canonical_refresh(
            &project_preferred_workspace(&snapshot(2, 10)).unwrap(),
            project_preferred_workspace(&initial_workspace).unwrap()
        )
        .is_ok()
    );
}
