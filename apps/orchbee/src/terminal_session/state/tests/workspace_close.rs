use super::*;
use crate::terminal_session::state::workspace_close::{
    WorkspaceCloseEvidence, WorkspaceCloseResult,
};
use orchbee_workbench_protocol::{
    WorkbenchWorkspaceGroupCloseResult, WorkspaceGroupCloseAction, WorkspaceGroupCloseMember,
    WorkspaceGroupCloseOutcome, WorkspaceGroupCloseStatus,
};

fn plan(
    state: &NativeSessionState,
    token: super::super::workspace_close::WorkspaceCloseToken,
) -> Option<super::super::workspace_close::WorkspaceCloseOperation> {
    state.plan_workspace_close(
        token,
        WorkbenchRequestId::parse(&id(390)).unwrap(),
        2,
        OperationDeadlines::from_sample(Instant::now()).unwrap(),
    )
}
fn receipt(
    operation: &super::super::workspace_close::WorkspaceCloseOperation,
    status: WorkspaceGroupCloseStatus,
) -> WorkbenchWorkspaceGroupCloseResult {
    let WorkspaceGroupCloseAction::Start {
        operation_id,
        project_id,
        workspace_id,
    } = &operation.request.action
    else {
        panic!("Start");
    };
    WorkbenchWorkspaceGroupCloseResult {
        protocol_version: ProtocolVersion,
        request_id: operation.request.request_id.clone(),
        operation_id: operation_id.clone(),
        outcome: WorkspaceGroupCloseOutcome::Available {
            project_id: project_id.clone(),
            workspace_id: workspace_id.clone(),
            attempt: 1,
            status,
            members: vec![WorkspaceGroupCloseMember {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                label: WorkbenchLabel::parse("Workspace").unwrap(),
                close: None,
            }],
        },
    }
}
#[test]
fn workspace_close_requires_the_exact_complete_draw_and_escape_has_no_effect() {
    let mut state = workspace_setup::workspace_state();
    let old = state.cache().clone();
    let token = state.open_workspace_close().unwrap();
    assert!(plan(&state, token).is_none());
    state.acknowledge_workspace_close_draw(Some(token));
    let operation = plan(&state, token).unwrap();
    assert!(matches!(
        operation.request.action,
        WorkspaceGroupCloseAction::Start { .. }
    ));
    state.cancel_workspace_close();
    assert!(plan(&state, token).is_none());
    assert_eq!(state.cache(), &old);
    assert!(state.workspace_close.pending.is_none());
    assert!(state.invariant_holds());
}
#[test]
fn workspace_close_refuses_standalone_and_stale_source_or_draw_without_allocating_a_request() {
    assert!(ready(1).open_workspace_close().is_none());
    let mut state = workspace_setup::workspace_state();
    let token = state.open_workspace_close().unwrap();
    state.cancel_workspace_close();
    let next = state.open_workspace_close().unwrap();
    assert_ne!(next, token);
    state.acknowledge_workspace_close_draw(Some(token));
    assert!(plan(&state, next).is_none());
    state.acknowledge_workspace_close_draw(Some(next));
    let snapshot = workspace_setup::workspace_snapshot(2, 0);
    state.cache = state.cache.project_snapshot(&snapshot).unwrap();
    assert!(plan(&state, next).is_none());
    assert!(state.settle_stale_workspace_close());
    assert!(state.workspace_close_confirmation().is_none());
    assert!(state.invariant_holds());
}
#[test]
fn workspace_close_unknown_and_known_pending_keep_recovery_identity_without_a_second_start() {
    for known in [false, true] {
        let mut state = workspace_setup::workspace_state();
        let token = state.open_workspace_close().unwrap();
        state.acknowledge_workspace_close_draw(Some(token));
        let operation = plan(&state, token).unwrap();
        assert!(state.begin_workspace_close(operation.clone()));
        let result = if known {
            WorkspaceCloseResult::Observed(Box::new(receipt(
                &operation,
                WorkspaceGroupCloseStatus::Pending {},
            )))
        } else {
            WorkspaceCloseResult::Unknown
        };
        let evidence = WorkspaceCloseEvidence {
            operation: operation.clone(),
            result,
        };
        assert!(state.retain_workspace_close_evidence(&evidence));
        assert!(state.finish_workspace_close(&evidence));
        state.pending_refresh = None;
        assert_eq!(
            state.workspace_close_status().unwrap().operation_id(),
            Some(operation.request.action.operation_id())
        );
        assert!(state.open_workspace_close().is_none());
        assert!(state.workspace_close.pending.is_none());
        assert!(state.invariant_holds());
    }
}
#[test]
fn workspace_close_pending_indicator_uses_injected_time_and_retention_precedes_publication() {
    let mut state = workspace_setup::workspace_state();
    let token = state.open_workspace_close().unwrap();
    state.acknowledge_workspace_close_draw(Some(token));
    let operation = plan(&state, token).unwrap();
    let start = operation.deadlines.total() - OPERATION_TOTAL;
    assert!(state.begin_workspace_close(operation));
    assert!(!state.tick_workspace_close_pending(start + Duration::from_millis(199)));
    assert!(state.workspace_close_status().is_none());
    assert!(state.tick_workspace_close_pending(start + Duration::from_millis(200)));
    assert!(state.workspace_close_status().is_some());
    assert!(!state.tick_workspace_close_pending(start + Duration::from_millis(201)));
}
