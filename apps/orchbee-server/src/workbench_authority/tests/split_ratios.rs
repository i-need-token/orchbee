use super::*;
use orchbee_workbench_protocol::{ClientSplitIdentity, WorkbenchSplitRatio};

mod generated;

fn fixture(revision: u64) -> (WorkbenchAuthority, WorkbenchGrant) {
    let (mut owner, grant) = authority();
    let mut value = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../../schemas/workbench/v1/workbench-conformance-v1.json"
    ))
    .unwrap()["documents"]["snapshot-full-r2"]
        .clone();
    value["siteId"] = serde_json::json!(owner.site_id().as_str());
    value["revision"] = serde_json::json!(revision.to_string());
    owner.projection =
        WorkbenchProjectionStore::load(&serde_json::to_vec(&value).unwrap()).unwrap();
    owner.last_identity = u128::MAX;
    (owner, grant)
}
fn set(snapshot: &Snapshot, value: f64) -> WorkbenchAction {
    let context = &snapshot.terminal_contexts[1];
    let tab = &context.tabs[0];
    let PanelNode::Split {
        children,
        split_id,
        ratio,
        ..
    } = &tab.root
    else {
        panic!("split");
    };
    WorkbenchAction::SplitRatioSet {
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: tab.tab_id.clone(),
        split: ClientSplitIdentity {
            split_id: split_id.clone(),
            first_slot_id: children[0].slot_id.clone(),
            second_slot_id: children[1].slot_id.clone(),
        },
        previous_ratio: *ratio,
        ratio: WorkbenchSplitRatio::new(value).unwrap(),
    }
}
fn ratio_mut(snapshot: &mut Snapshot) -> &mut WorkbenchSplitRatio {
    let PanelNode::Split { ratio, .. } = &mut snapshot.terminal_contexts[1].tabs[0].root else {
        panic!("split");
    };
    ratio
}

#[test]
fn ratio_changes_only_one_canonical_field_and_replay_is_effect_free() {
    let (mut owner, grant) = fixture(0);
    let before = owner.snapshot().clone();
    let request = command(1, 0, set(&before, 0.7));
    let result = submit(&mut owner, &grant, request.clone());
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::SplitRatioConfigured {
            revision: WorkbenchRevision::new(1),
        }
    );
    let mut expected = before.clone();
    expected.revision = WorkbenchRevision::new(1);
    *ratio_mut(&mut expected) = WorkbenchSplitRatio::new(0.7).unwrap();
    assert_eq!(owner.snapshot(), &expected);
    assert_eq!(owner.last_identity, u128::MAX);
    assert_eq!(submit(&mut owner, &grant, request), result);
    assert_eq!(owner.snapshot(), &expected);
    assert!(matches!(
        submit(&mut owner, &grant, command(1, 1, set(&expected, 0.8))).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    assert_eq!(
        submit(&mut owner, &grant, command(2, 1, set(&expected, 0.7))).outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(1)
        }
    );
    assert_eq!(owner.snapshot(), &expected);
}

#[test]
fn concurrent_old_revision_cannot_overwrite_a_committed_ratio() {
    let (mut owner, grant) = fixture(0);
    let before = owner.snapshot().clone();
    let first = command(1, 0, set(&before, 0.3));
    let second = command(2, 0, set(&before, 0.7));
    assert!(matches!(
        submit(&mut owner, &grant, first).outcome,
        WorkbenchCommandOutcome::SplitRatioConfigured { .. }
    ));
    let accepted = owner.snapshot().clone();
    assert!(matches!(
        submit(&mut owner, &grant, second).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &accepted);
}

#[test]
fn replaced_ordered_slots_and_forged_previous_ratio_reject_atomically() {
    let (mut owner, grant) = fixture(0);
    let before = owner.snapshot().clone();
    for (id, swap) in [(1, true), (2, false)] {
        let mut action = set(&before, 0.7);
        let WorkbenchAction::SplitRatioSet {
            split,
            previous_ratio,
            ..
        } = &mut action
        else {
            panic!("set");
        };
        let reason = if swap {
            std::mem::swap(&mut split.first_slot_id, &mut split.second_slot_id);
            WorkbenchRejectionReason::SplitUnavailable
        } else {
            *previous_ratio = WorkbenchSplitRatio::new(0.6).unwrap();
            WorkbenchRejectionReason::RevisionMismatch
        };
        let request = command(id, 0, action);
        let rejected = submit(&mut owner, &grant, request.clone());
        assert!(
            matches!(rejected.outcome, WorkbenchCommandOutcome::Rejected { reason: actual, .. } if actual == reason)
        );
        assert_eq!(submit(&mut owner, &grant, request), rejected);
        assert_eq!(owner.snapshot(), &before);
    }
}

#[test]
fn equal_ratio_at_revision_exhaustion_is_still_an_unchanged_operation() {
    let (mut owner, grant) = fixture(u64::MAX);
    let before = owner.snapshot().clone();
    assert_eq!(
        submit(&mut owner, &grant, command(1, u64::MAX, set(&before, 0.5))).outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(u64::MAX)
        }
    );
    assert!(matches!(
        submit(&mut owner, &grant, command(2, u64::MAX, set(&before, 0.7))).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionExhausted,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &before);
}
