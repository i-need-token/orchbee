use crate::{
    Directory, MAXIMUM_DIRECTORIES, Navigation, Project, ProtocolVersion, SiteId, Snapshot,
    TerminalContext, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchProjectId,
    WorkbenchRevision, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId, Workspace,
    validation::validate_snapshot,
};

fn snapshot() -> Snapshot {
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 1)).unwrap();
    Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: {
            let fixture_context_id = context.clone();
            let fixture_workspace_id =
                crate::WorkbenchWorkspaceId::parse(fixture_context_id.as_str()).unwrap();
            Navigation {
                projects: vec![],
                directories: vec![],
                workspaces: std::iter::once(crate::Workspace {
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(vec![])
                .collect(),
                workspace_order: Vec::new(),
                project_order: Vec::new(),
            }
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: context,
            groups: vec![],
            tabs: vec![],
        }],
    }
}

#[test]
fn global_directory_capacity_preserves_the_legacy_aggregate_independently_of_wire_bytes() {
    let mut value = snapshot();
    value.navigation.directories = (1..=MAXIMUM_DIRECTORIES)
        .map(|index| Directory {
            directory_id: WorkbenchDirectoryId::parse(&format!("{index:032x}")).unwrap(),
            worker_id: WorkerId::parse(&format!("{:032x}", 1)).unwrap(),
            path: WorkbenchDirectoryPath::parse("/").unwrap(),
        })
        .collect();
    assert_eq!(MAXIMUM_DIRECTORIES, 65_536);
    assert!(validate_snapshot(&value).is_ok());
    let mut extra = value.navigation.directories[0].clone();
    extra.directory_id =
        WorkbenchDirectoryId::parse(&format!("{:032x}", MAXIMUM_DIRECTORIES + 1)).unwrap();
    value.navigation.directories.push(extra);
    assert!(validate_snapshot(&value).is_err());
}

#[test]
fn membership_limits_and_order_are_independent_of_identity_and_resources() {
    let mut value = snapshot();
    value.navigation.workspaces.clear();
    value.navigation.workspace_order.clear();
    value.terminal_contexts.clear();
    let project_id = WorkbenchProjectId::parse(&format!("{:032x}", 1)).unwrap();
    value.navigation.projects.push(Project {
        project_id: project_id.clone(),
        label: "Group".into(),
    });
    for index in 1..=1025 {
        let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", index + 1)).unwrap();
        value.navigation.workspaces.push(Workspace {
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{index:032x}")).unwrap(),
            project_id: (index <= 256).then(|| project_id.clone()),
            primary_directory_id: None,
            terminal_context_id: context.clone(),
            label: "Workspace".into(),
        });
        value.terminal_contexts.push(TerminalContext {
            terminal_context_id: context,
            groups: vec![],
            tabs: vec![],
        });
        if index == 256 || index == 1024 {
            assert!(validate_snapshot(&value).is_ok());
        }
        if index == 257 {
            value.navigation.workspaces[index - 1].project_id = Some(project_id.clone());
            assert!(validate_snapshot(&value).is_err());
            value.navigation.workspaces[index - 1].project_id = None;
        }
    }
    assert!(validate_snapshot(&value).is_err());
    value.navigation.workspaces.pop();
    value.terminal_contexts.pop();
    value.navigation.workspace_order = value
        .navigation
        .workspaces
        .iter()
        .rev()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    assert!(validate_snapshot(&value).is_ok());
    let resources = value.terminal_contexts.clone();
    for workspace in &mut value.navigation.workspaces {
        workspace.project_id = None;
    }
    value.navigation.projects.clear();
    assert!(validate_snapshot(&value).is_ok());
    assert_eq!(value.terminal_contexts, resources);
    let ordered = value.navigation.ordered_workspaces().unwrap();
    assert!(ordered.iter().all(|(project, _)| project.is_none()));
    assert_eq!(
        ordered[0].1.workspace_id,
        value.navigation.workspace_order[0]
    );
    value.navigation.workspace_order.pop();
    assert!(validate_snapshot(&value).is_err());
}
