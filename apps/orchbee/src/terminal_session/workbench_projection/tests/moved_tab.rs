use super::*;
use orchbee_workbench_protocol::WorkbenchTabMoveDirection;

#[test]
fn explicit_three_tab_move_table_and_revision_overflow_pin_the_commit_contract() {
    use WorkbenchTabMoveDirection::{First, Last, Next, Previous};
    let baseline = projected(3, 10);
    let cases = [
        (0, Previous, None),
        (0, Next, Some([1, 0, 2])),
        (0, First, None),
        (0, Last, Some([1, 2, 0])),
        (1, Previous, Some([1, 0, 2])),
        (1, Next, Some([0, 2, 1])),
        (1, First, Some([1, 0, 2])),
        (1, Last, Some([0, 2, 1])),
        (2, Previous, Some([0, 2, 1])),
        (2, Next, None),
        (2, First, Some([2, 0, 1])),
        (2, Last, None),
    ];
    for (source, direction, order) in cases {
        let expected = MovedTabExpectation::new(
            WorkbenchRevision::new(11),
            baseline.terminal_context_id().clone(),
            baseline.tabs[source].tab_id.clone(),
            direction,
        );
        let mut candidate = projected(3, 11);
        candidate.tabs = order
            .unwrap_or([0, 1, 2])
            .into_iter()
            .map(|index| baseline.tabs[index].clone())
            .collect();
        assert_eq!(
            validate_moved_candidate(&baseline, &expected, candidate.clone()),
            order.map_or(Err(NativeProjectionDivergence), |_| Ok(candidate))
        );
    }
    let baseline = projected(3, u64::MAX);
    let expected = MovedTabExpectation::new(
        WorkbenchRevision::new(u64::MAX),
        baseline.terminal_context_id().clone(),
        baseline.tabs[0].tab_id.clone(),
        Last,
    );
    assert_eq!(
        validate_moved_candidate(&baseline, &expected, baseline.clone()),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn moved_projection_requires_exact_commit_permutation_and_allows_later_history() {
    for count in [
        1,
        2,
        3,
        MAXIMUM_TABS_PER_CONTEXT - 1,
        MAXIMUM_TABS_PER_CONTEXT,
    ] {
        let current = projected(count, 10);
        for from in [0, count / 2, count - 1] {
            for direction in [
                WorkbenchTabMoveDirection::Previous,
                WorkbenchTabMoveDirection::Next,
                WorkbenchTabMoveDirection::First,
                WorkbenchTabMoveDirection::Last,
            ] {
                let to = match direction {
                    WorkbenchTabMoveDirection::Previous => from.saturating_sub(1),
                    WorkbenchTabMoveDirection::Next => (from + 1).min(count - 1),
                    WorkbenchTabMoveDirection::First => 0,
                    WorkbenchTabMoveDirection::Last => count - 1,
                };
                let expected = MovedTabExpectation::new(
                    WorkbenchRevision::new(11),
                    current.terminal_context_id().clone(),
                    current.tabs[from].tab_id.clone(),
                    direction,
                );
                let mut candidate = current.clone();
                candidate.revision = WorkbenchRevision::new(11);
                let moved = candidate.tabs.remove(from);
                candidate.tabs.insert(to, moved);
                let accepted = validate_moved_candidate(&current, &expected, candidate.clone());
                if from == to {
                    assert_eq!(accepted, Err(NativeProjectionDivergence));
                    continue;
                }
                assert_eq!(accepted, Ok(candidate.clone()));
                candidate.tabs[to].tab_label = label("impossible rename at commit");
                assert_eq!(
                    validate_moved_candidate(&current, &expected, candidate.clone()),
                    Err(NativeProjectionDivergence)
                );
                candidate.revision = WorkbenchRevision::new(12);
                candidate.tabs.reverse();
                assert_eq!(
                    validate_moved_candidate(&current, &expected, candidate.clone()),
                    Ok(candidate)
                );
            }
        }
    }
}

#[test]
fn moved_projection_rejects_wrong_commit_tuple_revision_and_retained_member_changes() {
    let current = projected(3, 10);
    let expected = MovedTabExpectation::new(
        WorkbenchRevision::new(11),
        current.terminal_context_id().clone(),
        current.tabs[0].tab_id.clone(),
        WorkbenchTabMoveDirection::Last,
    );
    let mut candidate = projected(3, 11);
    candidate.tabs.rotate_left(1);
    let mut failures = Vec::new();
    let mut wrong = expected.clone();
    wrong.committed_revision = WorkbenchRevision::new(12);
    failures.push((wrong, candidate.clone()));
    let mut wrong = expected.clone();
    wrong.terminal_context_id = context_id(900);
    failures.push((wrong, candidate.clone()));
    let mut wrong = expected.clone();
    wrong.tab_id = tab_id(901);
    failures.push((wrong, candidate.clone()));
    failures.push((expected.clone(), current.clone()));
    failures.push((expected.clone(), projected(3, 11)));
    candidate.revision = WorkbenchRevision::new(12);
    candidate.tabs[0].binding.as_mut().unwrap().terminal_id = terminal_id(902);
    failures.push((expected.clone(), candidate));
    for (expected, candidate) in failures {
        assert_eq!(
            validate_moved_candidate(&current, &expected, candidate),
            Err(NativeProjectionDivergence)
        );
    }
    let mut later = projected(4, 12);
    later.tabs.reverse();
    assert_eq!(
        validate_moved_candidate(&current, &expected, later.clone()),
        Ok(later)
    );
}

fn validate_moved_candidate(
    baseline: &NativeContextTabs,
    expected: &MovedTabExpectation,
    candidate: NativeContextTabs,
) -> Result<NativeContextTabs, NativeProjectionDivergence> {
    validate_moved_tab(baseline, expected, candidate).map(|validated| validated.candidate)
}
