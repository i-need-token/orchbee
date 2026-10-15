use super::*;
use crate::{
    Directory, Project, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchTerminalContextId,
    WorkerId, Workspace,
};

fn id(value: usize) -> String {
    format!("{value:032x}")
}
fn project(value: usize) -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&id(value)).unwrap()
}
fn workspace(value: usize) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&id(20 + value)).unwrap()
}
fn navigation(groups: &[Option<usize>]) -> Navigation {
    Navigation {
        directories: vec![Directory {
            directory_id: WorkbenchDirectoryId::parse(&id(1000)).unwrap(),
            path: WorkbenchDirectoryPath::parse("C:/shared-checkout").unwrap(),
            worker_id: WorkerId::parse(&id(8888)).unwrap(),
        }],
        projects: (1..=2)
            .map(|value| Project {
                project_id: project(value),
                label: format!("Project {value}"),
            })
            .collect(),
        workspaces: groups
            .iter()
            .enumerate()
            .map(|(index, group)| Workspace {
                workspace_id: workspace(index),
                project_id: group.map(project),
                label: format!("Workspace {index}"),
                primary_directory_id: Some(WorkbenchDirectoryId::parse(&id(1000)).unwrap()),
                terminal_context_id: WorkbenchTerminalContextId::parse(&id(2000 + index)).unwrap(),
            })
            .collect(),
        standalone_terminal_context_id: WorkbenchTerminalContextId::parse(&id(9000)).unwrap(),
        workspace_order: vec![],
        project_order: vec![],
    }
}
fn members(
    navigation: &Navigation,
    group: Option<&WorkbenchProjectId>,
) -> Vec<WorkbenchWorkspaceId> {
    navigation
        .ordered_workspaces()
        .unwrap()
        .into_iter()
        .filter(|(_, workspace)| workspace.project_id.as_ref() == group)
        .map(|(_, workspace)| workspace.workspace_id.clone())
        .collect()
}

#[test]
fn bounded_group_and_order_traces_preserve_membership_resources_and_noops() {
    for assignment in 0..27 {
        let groups = (0..3)
            .map(|index| match assignment / 3_usize.pow(index) % 3 {
                0 => None,
                value => Some(value),
            })
            .collect::<Vec<_>>();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut before = navigation(&groups);
            before.workspace_order = order.map(workspace).to_vec();
            for (source, source_group) in groups.iter().enumerate() {
                for destination in [None, Some(project(1)), Some(project(2))] {
                    let anchors = std::iter::once(None)
                        .chain(members(&before, destination.as_ref()).into_iter().map(Some))
                        .collect::<Vec<_>>();
                    for anchor in anchors {
                        let operation = NavigationConfiguration::MoveWorkspace {
                            workspace_id: workspace(source),
                            project_id: destination.clone(),
                            before_workspace_id: anchor.clone(),
                        };
                        let result = configure_navigation(&before, &operation).unwrap();
                        let source_id = workspace(source);
                        let old_group = members(&before, destination.as_ref());
                        let expected_noop = source_group.map(project) == destination
                            && (anchor.as_ref() == Some(&source_id)
                                || match &anchor {
                                    Some(anchor) => old_group
                                        .windows(2)
                                        .any(|pair| pair == [source_id.clone(), anchor.clone()]),
                                    None => old_group.last() == Some(&source_id),
                                });
                        assert_eq!(
                            result.is_none(),
                            expected_noop,
                            "assignment={assignment} order={order:?} source={source} destination={destination:?} anchor={anchor:?}"
                        );
                        let after = result.as_ref().unwrap_or(&before);
                        crate::validation::validate_navigation_structure(after).unwrap();
                        assert_eq!(after.projects, before.projects);
                        assert_eq!(after.project_order, before.project_order);
                        assert_eq!(after.directories, before.directories);
                        assert_eq!(
                            after.standalone_terminal_context_id,
                            before.standalone_terminal_context_id
                        );
                        for (old, new) in before.workspaces.iter().zip(&after.workspaces) {
                            let mut expected = old.clone();
                            if old.workspace_id == source_id {
                                expected.project_id = destination.clone();
                            }
                            assert_eq!(*new, expected);
                        }
                        let actual = members(after, destination.as_ref());
                        match anchor.as_ref() {
                            Some(anchor) if anchor != &source_id => assert!(
                                actual
                                    .windows(2)
                                    .any(|pair| pair == [source_id.clone(), anchor.clone()])
                            ),
                            None => assert_eq!(actual.last(), Some(&source_id)),
                            _ => {}
                        }
                        for group in [None, Some(project(1)), Some(project(2))] {
                            let without_source = |value: &Navigation| {
                                members(value, group.as_ref())
                                    .into_iter()
                                    .filter(|id| id != &source_id)
                                    .collect::<Vec<_>>()
                            };
                            assert_eq!(without_source(after), without_source(&before));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn project_order_is_separate_from_identity_records_and_semantic_noops() {
    let before = navigation(&[Some(1), None, Some(2)]);
    let operation = NavigationConfiguration::MoveProject {
        project_id: project(2),
        before_project_id: Some(project(1)),
    };
    let after = configure_navigation(&before, &operation).unwrap().unwrap();
    assert_eq!(
        after
            .ordered_projects()
            .unwrap()
            .iter()
            .map(|project| project.project_id.clone())
            .collect::<Vec<_>>(),
        [project(2), project(1)]
    );
    assert_eq!(after.projects, before.projects);
    assert_eq!(after.workspaces, before.workspaces);
    assert_eq!(after.workspace_order, before.workspace_order);
    assert!(configure_navigation(&after, &operation).unwrap().is_none());
    assert!(
        configure_navigation(
            &before,
            &NavigationConfiguration::MoveProject {
                project_id: project(1),
                before_project_id: Some(project(1))
            }
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn cross_group_anchor_missing_identity_and_capacity_refuse_the_entire_candidate() {
    let before = navigation(&[Some(1), Some(2), None]);
    for (operation, expected) in [
        (
            NavigationConfiguration::MoveWorkspace {
                workspace_id: workspace(0),
                project_id: None,
                before_workspace_id: Some(workspace(1)),
            },
            WorkbenchRejectionReason::InvalidNavigationConfiguration,
        ),
        (
            NavigationConfiguration::MoveWorkspace {
                workspace_id: workspace(9),
                project_id: None,
                before_workspace_id: None,
            },
            WorkbenchRejectionReason::WorkspaceUnavailable,
        ),
        (
            NavigationConfiguration::MoveProject {
                project_id: project(9),
                before_project_id: None,
            },
            WorkbenchRejectionReason::ProjectUnavailable,
        ),
    ] {
        assert_eq!(configure_navigation(&before, &operation), Err(expected));
    }
    let mut groups = vec![Some(1); MAXIMUM_WORKSPACES_PER_PROJECT];
    groups.push(None);
    let full = navigation(&groups);
    assert_eq!(
        configure_navigation(
            &full,
            &NavigationConfiguration::MoveWorkspace {
                workspace_id: workspace(MAXIMUM_WORKSPACES_PER_PROJECT),
                project_id: Some(project(1)),
                before_workspace_id: None
            }
        ),
        Err(WorkbenchRejectionReason::ResourceLimit)
    );
}

#[test]
fn explicit_project_order_is_validated_and_nullable_destinations_are_required() {
    let mut source = navigation(&[None]);
    for order in [
        vec![project(1)],
        vec![project(1), project(1)],
        vec![project(1), project(9)],
    ] {
        source.project_order = order;
        assert!(source.ordered_projects().is_err());
        assert_eq!(
            configure_navigation(
                &source,
                &NavigationConfiguration::MoveProject {
                    project_id: project(1),
                    before_project_id: None
                }
            ),
            Err(WorkbenchRejectionReason::InvalidNavigationConfiguration)
        );
    }
    for value in [
        serde_json::json!({"kind":"move-project","projectId":project(1)}),
        serde_json::json!({"kind":"move-workspace","workspaceId":workspace(0),"beforeWorkspaceId":null}),
        serde_json::json!({"kind":"move-workspace","workspaceId":workspace(0),"projectId":null}),
    ] {
        assert!(serde_json::from_value::<NavigationConfiguration>(value).is_err());
    }
}
