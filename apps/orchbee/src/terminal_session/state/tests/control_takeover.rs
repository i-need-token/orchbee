use super::*;
use crate::terminal_session::state::control_takeover::{
    TakeoverEvidence, TakeoverResult, TakeoverStatus,
};
use orchbee_client_server_protocol::RequestId;

#[test]
fn display_geometry_is_strict_for_each_first_frame_then_tracks_host_snapshots_through_recovery() {
    let mut state =
        ready_with_cache(project_native_standalone_tabs(&split_panel_baseline()).unwrap());
    let focus = state.focus_pane_ref().unwrap();
    let peer = attach_nonfirst_leaf(&mut state);
    // The global session is Ready, but the background attachment still owes its own first frame.
    assert_eq!(state.expected_frame_geometry(&focus), None);
    assert_eq!(state.expected_frame_geometry(&peer), Some(geometry(80)));
    assert!(state.stage_drawn_frame(&peer, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&peer, FrameIdentity::new(2)));
    assert_eq!(state.expected_frame_geometry(&peer), None);
    // A blocked presentation does not return an established observer to initial setup.
    assert!(state.invalidate_projection_frame(&peer));
    assert_eq!(state.expected_frame_geometry(&peer), None);
    assert_eq!(state.last_accepted_geometry(&peer), Some(geometry(80)));
    assert!(state.settle_pane_fault(&peer, PaneFault::ViewerLost));
    assert!(state.invariant_holds());
}

#[test]
fn explicit_takeover_keeps_incarnation_and_uses_read_only_leaf_geometry() {
    let mut state = ready(2);
    let key = state.focus_pane_ref().unwrap();
    assert!(state.settle_pane_fault(&key, PaneFault::ControlLost));
    state.sync_pane_set(
        &[(key.pane().panel().clone(), 0, geometry(53))],
        Instant::now(),
    );
    let operation = state
        .plan_takeover(
            RequestId::parse(&id(600)).unwrap(),
            2,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert_eq!(operation.geometry, geometry(53));
    assert!(state.begin_takeover(operation.clone()));
    assert!(state.mutation_blocks_structure());
    assert!(
        state
            .plan_takeover(RequestId::parse(&id(601)).unwrap(), 3, operation.deadlines)
            .is_none()
    );
    let evidence = TakeoverEvidence {
        operation,
        result: TakeoverResult::Ready(ControllerId::parse(&id(602)).unwrap()),
    };
    assert!(state.finish_takeover(&evidence, Instant::now()));
    assert_eq!(state.focus_pane_ref(), Some(key.clone()));
    assert!(state.pane_attached(&key));
    assert!(
        state
            .live_pane_record(&key)
            .unwrap()
            .accepted_frame
            .is_none()
    );
    assert!(!state.takeover_input_ready());
    assert_eq!(
        state.takeover_status().unwrap().0,
        TakeoverStatus::WaitingForFrame
    );
    assert_eq!(state.expected_frame_geometry(&key), Some(geometry(53)));
    // A refused later gesture cannot bypass the first grant's pending frame admission.
    let retry = state
        .plan_takeover(
            RequestId::parse(&id(603)).unwrap(),
            3,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    assert!(state.begin_takeover(retry.clone()));
    assert!(state.finish_takeover(
        &TakeoverEvidence {
            operation: retry,
            result: TakeoverResult::Refused,
        },
        Instant::now(),
    ));
    assert!(!state.takeover_input_ready());
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(2)));
    assert!(state.takeover_input_ready());
    assert_eq!(state.expected_frame_geometry(&key), None);
    // Once the handoff's first frame is accepted, ordinary frame updates must not drop typing.
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(3)));
    assert!(state.takeover_input_ready());
    assert_eq!(
        state.live_pane_record(&key).unwrap().last_accepted_geometry,
        Some(geometry(53))
    );
    assert!(state.invariant_holds());
}

#[test]
fn takeover_reports_waiting_until_feedback_even_after_transient_status_expiry() {
    let mut state = ready(1);
    let key = state.focus_pane_ref().unwrap();
    let sampled = Instant::now();
    let operation = state
        .plan_takeover(
            RequestId::parse(&id(620)).unwrap(),
            2,
            OperationDeadlines::from_sample(sampled).unwrap(),
        )
        .unwrap();
    assert!(state.begin_takeover(operation.clone()));
    assert!(state.finish_takeover(
        &TakeoverEvidence {
            operation,
            result: TakeoverResult::Ready(ControllerId::parse(&id(621)).unwrap()),
        },
        sampled,
    ));
    state.tick_takeover(sampled + Duration::from_secs(6));
    assert_eq!(
        state.takeover_status().unwrap().0,
        TakeoverStatus::WaitingForFrame
    );
    assert!(!state.takeover_input_ready());
    assert!(state.stage_drawn_frame(&key, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&key, FrameIdentity::new(2)));
    assert!(state.takeover_input_ready());
    assert!(state.takeover_status().is_none());
    assert!(state.invariant_holds());
}
#[test]
fn takeover_retains_grant_on_exit_and_rejects_replay_after_unknown() {
    for known in [false, true] {
        let mut state = ready(1);
        let sampled = Instant::now();
        let operation = state
            .plan_takeover(
                RequestId::parse(&id(610)).unwrap(),
                2,
                OperationDeadlines::from_sample(sampled).unwrap(),
            )
            .unwrap();
        assert!(state.begin_takeover(operation.clone()));
        assert!(!state.tick_takeover(sampled + Duration::from_millis(199)));
        assert!(state.tick_takeover(sampled + Duration::from_millis(200)));
        let result = if known {
            TakeoverResult::Granted(ControllerId::parse(&id(611)).unwrap())
        } else {
            TakeoverResult::Unknown
        };
        let evidence = TakeoverEvidence {
            operation: operation.clone(),
            result: result.clone(),
        };
        assert!(state.retain_takeover(&evidence));
        state.retire(CarrierFailure::Transport);
        assert!(state.finish_takeover(&evidence, sampled));
        assert!(state.finish_takeover(
            &TakeoverEvidence {
                operation: operation.clone(),
                result: TakeoverResult::Unknown
            },
            sampled
        ));
        assert_eq!(state.takeover.evidence.as_ref().unwrap().result, result);
        assert!(
            state
                .plan_takeover(RequestId::parse(&id(612)).unwrap(), 3, operation.deadlines)
                .is_none()
        );
        assert!(state.invariant_holds());
    }
}
