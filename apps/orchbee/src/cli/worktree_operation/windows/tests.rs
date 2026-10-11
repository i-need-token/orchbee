use super::*;
use orchbee_workbench_protocol::{
    WorkbenchProjectId, WorkbenchWorktreeOutcome, WorkbenchWorktreeResult,
};

#[test]
fn cancellation_never_erases_unknown_operation_identity() {
    let answer = Answer {
        operation_id: WorkbenchRequestId::parse("00000000000000000000000000000001").unwrap(),
        observation: Observation::Unknown {
            reason: "transport-loss",
        },
        wait: Some(Wait::Cancelled),
        post_open: None,
    };
    assert_eq!(answer.exit_code(), 130);
    let document = answer.document();
    assert_eq!(document["operationId"], "00000000000000000000000000000001");
    assert_eq!(document["observation"]["kind"], "unknown");
}

#[test]
fn later_observation_cannot_replace_known_project_or_attempt_with_unrelated_state() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../schemas/workbench/v1/workbench-worktree-conformance-v1.json"
    ))
    .unwrap();
    let mut value = corpus["valid"][5]["document"].clone();
    value.as_object_mut().unwrap().remove("kind");
    let reply: WorkbenchWorktreeResult = serde_json::from_value(value).unwrap();
    let previous = Observation::Server {
        reply: reply.clone(),
    };
    assert!(same_attempt(&previous, &reply));
    for case in 0..3 {
        let mut changed = reply.clone();
        match case {
            0 => {
                let WorkbenchWorktreeOutcome::Available { attempt, .. } = &mut changed.outcome
                else {
                    unreachable!()
                };
                *attempt += 1;
            }
            1 => {
                let WorkbenchWorktreeOutcome::Available { project_id, .. } = &mut changed.outcome
                else {
                    unreachable!()
                };
                *project_id =
                    Some(WorkbenchProjectId::parse("ffffffffffffffffffffffffffffffff").unwrap());
            }
            _ => changed.outcome = WorkbenchWorktreeOutcome::NotFound {},
        }
        assert!(!same_attempt(&previous, &changed));
    }
}
