use super::*;
use crate::terminal_session::WorkspaceDeleteRequest;

#[test]
fn sidebar_delete_has_a_distinct_conditional_request_and_needs_a_new_draw_to_escalate() {
    let mut state = background_state(true);
    let original = (
        state.cache.clone(),
        state.desired.clone(),
        state.focus_pane_ref(),
    );
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let request =
        WorkspaceDeleteRequest::capture(&state, &workspace_id(1), deadlines.total()).unwrap();
    let operation = state
        .plan_sidebar_workspace_delete(
            &request,
            WorkbenchRequestId::parse(&id(900)).unwrap(),
            2,
            deadlines,
        )
        .unwrap();
    assert_eq!(
        operation.workspace_command().unwrap().action,
        WorkbenchAction::WorkspaceCloseIfIdle {
            workspace_id: workspace_id(1)
        }
    );
    assert!(state.workspace_close_confirmation().is_none());
    assert!(state.begin_workspace_close(operation.clone()));
    let evidence = WorkspaceCloseEvidence {
        operation: operation.clone(),
        result: WorkspaceCloseResult::ConfirmationRequired,
    };
    assert!(!retains_close_result(
        &evidence.result,
        &WorkspaceCloseResult::Unknown
    ));
    assert!(!retains_close_result(
        &evidence.result,
        &WorkspaceCloseResult::WorkspaceAccepted(accepted(&operation))
    ));
    assert!(retains_close_result(&evidence.result, &evidence.result));
    assert!(state.finish_workspace_close(&evidence));
    assert!(
        !state.review_sidebar_workspace_delete(&evidence),
        "pending refresh still owns settlement"
    );
    state.apply_prepared_refresh(
        PreparedRefresh::DefiniteFailure {
            generation: 2,
            baseline_revision: operation.baseline.revision(),
        },
        Instant::now(),
    );
    assert!(state.review_sidebar_workspace_delete(&evidence));
    let prompt = state.workspace_close_confirmation().unwrap();
    assert!(prompt.destructive());
    assert!(!prompt.interactive());
    let token = prompt.token();
    let next = WorkbenchRequestId::parse(&id(901)).unwrap();
    assert!(
        state
            .plan_workspace_close(token, next.clone(), 3, deadlines)
            .is_none()
    );
    state.acknowledge_workspace_close_draw(Some(token));
    let confirmed = state
        .plan_workspace_close(token, next.clone(), 3, deadlines)
        .unwrap();
    assert_eq!(
        confirmed.workspace_command().unwrap().action,
        WorkbenchAction::WorkspaceClose {
            workspace_id: workspace_id(1)
        }
    );
    assert_eq!(confirmed.request_id, next);
    assert_ne!(confirmed.request_id, operation.request_id);
    assert_eq!(
        (
            state.cache.clone(),
            state.desired.clone(),
            state.focus_pane_ref()
        ),
        original
    );
}

#[test]
fn conditional_delete_refusal_unknown_and_stale_source_never_mint_confirmation_or_replay() {
    for result in [
        WorkspaceCloseResult::NotSubmitted,
        WorkspaceCloseResult::Refused,
        WorkspaceCloseResult::Unknown,
    ] {
        let mut state = background_state(true);
        let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
        let request =
            WorkspaceDeleteRequest::capture(&state, &workspace_id(0), deadlines.total()).unwrap();
        let operation = state
            .plan_sidebar_workspace_delete(
                &request,
                WorkbenchRequestId::parse(&id(900)).unwrap(),
                2,
                deadlines,
            )
            .unwrap();
        let evidence = WorkspaceCloseEvidence { operation, result };
        assert!(!state.review_sidebar_workspace_delete(&evidence));
        assert!(state.workspace_close_confirmation().is_none());
        assert!(state.workspace_close.pending.is_none());
    }
    let original = background_state(true);
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let request =
        WorkspaceDeleteRequest::capture(&original, &workspace_id(0), deadlines.total()).unwrap();
    let changed = close_state(1);
    assert!(
        changed
            .plan_sidebar_workspace_delete(
                &request,
                WorkbenchRequestId::parse(&id(900)).unwrap(),
                2,
                deadlines
            )
            .is_none()
    );
    assert!(changed.workspace_close_confirmation().is_none());
}
