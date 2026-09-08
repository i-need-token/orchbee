use super::*;
use std::collections::BTreeSet;

// Canonical trace tuple: action (move/replay/conflict/stale/denied/retire), target, direction, history.
// The bounded navigation model's Move, Replay, Conflict, Retire and refusal observations map to
// actual command results here; this oracle separately uses remove/insert over stable identities.
type Step = [u8; 4];

#[test]
fn generated_move_histories_have_truthful_outcomes_and_stable_shrinkable_replay() {
    let mut coverage = BTreeSet::new();
    for seed in 1_u64..=64 {
        let mut random = seed;
        let mut trace = vec![
            [0, 0, 0, 0],
            [0, 0, 1, 0],
            [0, 3, 2, 0],
            [1, 0, 0, 1],
            [2, 0, 0, 1],
            [3, 0, 0, 0],
            [4, 0, 0, 0],
        ];
        for _ in 0..64 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let bytes = random.to_le_bytes();
            trace.push([bytes[0] % 5, bytes[1] % 4, bytes[2] % 4, bytes[3]]);
        }
        trace.extend([[5, 0, 0, 0], [1, 0, 0, 0], [0, 0, 1, 0], [4, 0, 0, 0]]);
        let encoded = serde_json::to_string(&trace).unwrap();
        let decoded: Vec<Step> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, trace);
        match run(&decoded) {
            Ok(reached) => coverage.extend(reached),
            Err(failure) => {
                let reduced = shrink(decoded);
                panic!(
                    "seed={seed}; failure={failure}; replay={}; reduced_failure={:?}",
                    serde_json::to_string(&reduced).unwrap(),
                    run(&reduced)
                );
            }
        }
    }
    assert_eq!(
        coverage,
        BTreeSet::from([
            "moved",
            "unchanged",
            "missing",
            "replay",
            "conflict",
            "stale",
            "denied",
            "retired"
        ])
    );
}

fn run(trace: &[Step]) -> Result<BTreeSet<&'static str>, String> {
    let (mut authority, grant) = authority(3);
    let baseline = authority.projection.snapshot().clone();
    let mut order = vec![0, 1, 2];
    let mut revision = baseline.revision;
    let mut retired = false;
    let mut history: Vec<(WorkbenchCommand, WorkbenchCommandResult)> = Vec::new();
    let mut coverage = BTreeSet::new();
    for (index, [action, target, direction, previous]) in trace.iter().copied().enumerate() {
        if action == 5 {
            authority.retire_standalone_composition();
            retired = true;
            continue;
        }
        let direction = [
            WorkbenchTabMoveDirection::Previous,
            WorkbenchTabMoveDirection::Next,
            WorkbenchTabMoveDirection::First,
            WorkbenchTabMoveDirection::Last,
        ][usize::from(direction)];
        let mut request = command(
            &authority,
            100 + index as u128,
            usize::from(target),
            direction,
        );
        let retained = if history.is_empty() {
            None
        } else {
            history.get(usize::from(previous) % history.len())
        };
        if (action == 1 || action == 2)
            && let Some((original, _)) = retained
        {
            request = original.clone();
            if action == 2 {
                request.expected_revision = WorkbenchRevision::new(u64::MAX);
            }
        }
        if action == 3 {
            request.expected_revision = WorkbenchRevision::new(0);
        }
        let mut retain = false;
        let expected_outcome = if action == 4 {
            coverage.insert("denied");
            WorkbenchCommandOutcome::Denied {}
        } else if retired {
            coverage.insert("retired");
            unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
        } else if action == 1
            && let Some((_, result)) = retained
        {
            coverage.insert("replay");
            result.outcome.clone()
        } else if action == 2 && retained.is_some() {
            coverage.insert("conflict");
            WorkbenchCommandOutcome::Rejected {
                revision,
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
            }
        } else if action == 3 {
            coverage.insert("stale");
            retain = true;
            WorkbenchCommandOutcome::Rejected {
                revision,
                reason: WorkbenchRejectionReason::RevisionMismatch,
            }
        } else {
            retain = true;
            if let Some(source) = order
                .iter()
                .position(|identity| *identity == usize::from(target))
            {
                let destination = match direction {
                    WorkbenchTabMoveDirection::Previous => {
                        if source == 0 {
                            source
                        } else {
                            source - 1
                        }
                    }
                    WorkbenchTabMoveDirection::Next => {
                        if source == order.len() - 1 {
                            source
                        } else {
                            source + 1
                        }
                    }
                    WorkbenchTabMoveDirection::First => 0,
                    WorkbenchTabMoveDirection::Last => order.len() - 1,
                };
                if source == destination {
                    coverage.insert("unchanged");
                    WorkbenchCommandOutcome::Unchanged { revision }
                } else {
                    coverage.insert("moved");
                    let identity = order.remove(source);
                    order.insert(destination, identity);
                    revision = WorkbenchRevision::new(revision.value() + 1);
                    WorkbenchCommandOutcome::TabMoved {
                        revision,
                        terminal_context_id: baseline
                            .navigation
                            .standalone_terminal_context_id
                            .clone(),
                        tab_id: baseline.terminal_contexts[0].tabs[usize::from(target)]
                            .tab_id
                            .clone(),
                    }
                }
            } else {
                coverage.insert("missing");
                WorkbenchCommandOutcome::Rejected {
                    revision,
                    reason: WorkbenchRejectionReason::TabUnavailable,
                }
            }
        };
        let actual = if action == 4 {
            let WorkbenchTabMoveBegin::Ready(result) =
                authority.begin_terminal_tab_move(None, request.clone())
            else {
                return Err("denied request was admitted".to_owned());
            };
            result
        } else {
            submit(&mut authority, &grant, request.clone())
        };
        let expected = result(&request, expected_outcome);
        if actual != expected {
            return Err(format!(
                "step={index}; actual={}; expected={}",
                serde_json::to_string(&actual).unwrap(),
                serde_json::to_string(&expected).unwrap()
            ));
        }
        if retain {
            history.push((request, expected));
        }
        let mut expected_snapshot = baseline.clone();
        expected_snapshot.revision = revision;
        expected_snapshot.terminal_contexts[0].tabs = order
            .iter()
            .map(|index| baseline.terminal_contexts[0].tabs[*index].clone())
            .collect();
        if authority.projection.snapshot() != &expected_snapshot
            || authority.last_identity != 10
            || authority.ledger.len() != history.len()
            || authority.retired != retired
        {
            return Err(format!(
                "step={index}; projection, cursor, ledger or retirement changed outside its result"
            ));
        }
    }
    Ok(coverage)
}

fn shrink(mut trace: Vec<Step>) -> Vec<Step> {
    let mut index = 0;
    while index < trace.len() {
        let mut candidate = trace.clone();
        candidate.remove(index);
        if run(&candidate).is_err() {
            trace = candidate;
            index = 0;
        } else {
            index += 1;
        }
    }
    trace
}
