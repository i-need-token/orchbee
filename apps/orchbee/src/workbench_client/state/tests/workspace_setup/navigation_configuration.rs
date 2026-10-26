use super::*;
use orchbee_workbench_protocol::{
    NavigationConfiguration, Project, TerminalContext, WorkbenchProjectId,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, Workspace,
};

fn project(value: u128) -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&id(value)).unwrap()
}
fn workspace(value: u128) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&id(value)).unwrap()
}
fn baseline() -> orchbee_workbench_protocol::Snapshot {
    let mut snapshot = snapshot(3, 1, None);
    snapshot.navigation.projects = [900, 901]
        .into_iter()
        .map(|value| Project {
            project_id: project(value),
            label: format!("Project {value}"),
        })
        .collect();
    snapshot.navigation.project_order = vec![project(901), project(900)];
    for (index, group) in [Some(900), Some(901), None].into_iter().enumerate() {
        let index = u128::try_from(index).unwrap();
        let terminal_context_id = WorkbenchTerminalContextId::parse(&id(920 + index)).unwrap();
        snapshot.navigation.workspaces.push(Workspace {
            workspace_id: workspace(910 + index),
            project_id: group.map(project),
            primary_directory_id: None,
            label: format!("Workspace {index}"),
            terminal_context_id: terminal_context_id.clone(),
        });
        snapshot.terminal_contexts.push(TerminalContext {
            terminal_context_id,
            groups: vec![],
            tabs: vec![],
        });
    }
    snapshot
}
#[test]
fn navigation_receipt_requires_membership_and_order_together_and_preserves_contexts() {
    let baseline = baseline();
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let operation = state
        .plan_navigation_configuration(
            NavigationConfiguration::MoveWorkspace {
                workspace_id: workspace(910),
                project_id: Some(project(901)),
                before_workspace_id: None,
            },
            WorkbenchRequestId::parse(&id(999)).unwrap(),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(
        operation
            .known_commit(WorkbenchSetupCommit::NavigationConfigured {
                revision: WorkbenchRevision::new(1),
                unchanged: true
            })
            .is_none()
    );
    assert!(
        operation
            .known_commit(WorkbenchSetupCommit::NavigationConfigured {
                revision: WorkbenchRevision::new(3),
                unchanged: false
            })
            .is_none()
    );
    let known = operation
        .known_commit(WorkbenchSetupCommit::NavigationConfigured {
            revision: WorkbenchRevision::new(2),
            unchanged: false,
        })
        .unwrap();
    state.begin_setup(operation);
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    let mut exact = baseline.clone();
    exact.revision = WorkbenchRevision::new(2);
    exact.navigation.workspaces[1].project_id = Some(project(901));
    exact.navigation.workspace_order = vec![workspace(911), workspace(910), workspace(912)];
    assert!(matches!(
        state.prepare_setup_refresh(1, &exact),
        PreparedRefresh::Membership { .. }
    ));
    for fault in 0..4 {
        let mut hostile = exact.clone();
        match fault {
            0 => hostile.navigation.workspace_order = baseline.navigation.workspace_order.clone(),
            1 => hostile.navigation.workspaces[1].project_id = Some(project(900)),
            2 => hostile.navigation.project_order.reverse(),
            _ => hostile.terminal_contexts[0].tabs[0].label = "Unrelated edit".into(),
        }
        assert!(
            matches!(
                state.prepare_setup_refresh(1, &hostile),
                PreparedRefresh::Divergent { .. }
            ),
            "fault {fault}"
        );
        assert_eq!(state.cache().canonical_snapshot(), &baseline);
    }
}

#[test]
fn sidebar_overlay_is_owned_by_the_admitted_operation_and_never_changes_the_canonical_cache() {
    for uncertain in [false, true] {
        let baseline = baseline();
        let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
        let operation = state
            .plan_navigation_configuration(
                NavigationConfiguration::MoveWorkspace {
                    workspace_id: workspace(910),
                    project_id: None,
                    before_workspace_id: None,
                },
                WorkbenchRequestId::parse(&id(999)).unwrap(),
                1,
                OperationDeadlines::from_sample(Instant::now()).unwrap(),
            )
            .unwrap();
        assert!(state.view().sidebar_navigation().pending.is_none());
        state.begin_setup(operation.clone());
        assert!(state.view().sidebar_navigation().pending.is_some());
        assert_eq!(state.cache().canonical_snapshot(), &baseline);
        let mut stale = operation.clone();
        stale.generation += 1;
        assert!(!state.settle_setup_not_applied(&stale));
        assert!(state.view().sidebar_navigation().pending.is_some());
        assert!(state.retain_setup_worker(if uncertain {
            SetupWorkerEvidence::Unknown(operation)
        } else {
            SetupWorkerEvidence::NotApplied(operation)
        }));
        assert!(state.view().sidebar_navigation().pending.is_none());
        assert_eq!(state.cache().canonical_snapshot(), &baseline);
        assert!(state.invariant_holds());
    }
}
