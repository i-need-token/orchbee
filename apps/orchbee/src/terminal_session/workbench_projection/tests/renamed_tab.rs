use super::super::renamed::{RenamedTabExpectation, validate_renamed_tab};
use super::*;

#[test]
fn exact_rename_preserves_requested_text_and_every_other_member_field() {
    let baseline = projected(3, 10);
    for requested in [
        "  标签 🚀  ".to_owned(),
        " \t\r\n\u{1b} ".to_owned(),
        "雪".repeat(MAXIMUM_LABEL_LENGTH),
    ] {
        for target in 0..3 {
            let expected = expectation(&baseline, target, 11, &requested);
            let mut candidate = baseline.clone();
            candidate.revision = WorkbenchRevision::new(11);
            candidate.tabs[target].tab_label = label(&requested);
            let validated = validate_renamed_tab(&baseline, &expected, candidate.clone()).unwrap();
            assert_eq!(validated.candidate, candidate);
            assert_eq!(validated.renamed_index, Some(target));
        }
    }
    assert_eq!(baseline, projected(3, 10));
}

#[test]
fn exact_commit_rejects_every_change_beyond_target_label() {
    let baseline = projected(3, 10);
    let expected = expectation(&baseline, 1, 11, " renamed ");
    let mut correct = projected(3, 11);
    correct.tabs[1].tab_label = label(" renamed ");
    let mut failures = Vec::new();
    let mut wrong = correct.clone();
    wrong.tabs[1].tab_label = label("renamed");
    failures.push(wrong);
    let mut wrong = correct.clone();
    wrong.tabs[0].tab_label = label("another rename");
    failures.push(wrong);
    let mut wrong = correct.clone();
    wrong.tabs.reverse();
    failures.push(wrong);
    let mut wrong = correct.clone();
    wrong.tabs.push(projected(4, 11).tabs.remove(3));
    failures.push(wrong);
    for removed in [0, 1] {
        let mut wrong = correct.clone();
        wrong.tabs.remove(removed);
        failures.push(wrong);
    }
    let mut wrong = correct.clone();
    wrong.tabs[1].presentation_label = label("presentation is immutable");
    failures.push(wrong);
    let mut wrong = correct.clone();
    wrong.tabs[1].binding.worker_id = worker_id(999);
    failures.push(wrong);
    for wrong in failures {
        assert_eq!(
            validate_renamed_tab(&baseline, &expected, wrong),
            Err(NativeProjectionDivergence)
        );
    }
    assert_eq!(
        validate_renamed_tab(&baseline, &expected, correct.clone())
            .unwrap()
            .candidate,
        correct
    );
}

#[test]
fn known_rename_requires_existing_target_changed_label_and_exact_successor() {
    let baseline = projected(3, 10);
    let mut candidate = projected(3, 12);
    candidate.tabs[1].tab_label = label("later text");
    for expected in [
        expectation(&baseline, 1, 10, "changed"),
        expectation(&baseline, 1, 12, "changed"),
        expectation(&baseline, 1, 11, baseline.tabs[1].tab_label.as_str()),
        RenamedTabExpectation::new(
            WorkbenchRevision::new(11),
            context_id(999),
            baseline.tabs[1].tab_id.clone(),
            label("changed"),
        ),
        RenamedTabExpectation::new(
            WorkbenchRevision::new(11),
            baseline.terminal_context_id.clone(),
            tab_id(999),
            label("changed"),
        ),
    ] {
        assert_eq!(
            validate_renamed_tab(&baseline, &expected, candidate.clone()),
            Err(NativeProjectionDivergence)
        );
    }
    for revision in [9, 10] {
        assert_eq!(
            validate_renamed_tab(
                &baseline,
                &expectation(&baseline, 1, 11, "changed"),
                projected(3, revision),
            ),
            Err(NativeProjectionDivergence)
        );
    }
    let maximum = projected(1, u64::MAX);
    assert_eq!(
        validate_renamed_tab(
            &maximum,
            &expectation(&maximum, 0, u64::MAX, "changed"),
            maximum.clone(),
        ),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn later_history_can_overwrite_move_remove_or_empty_without_erasing_commit() {
    let baseline = projected(3, 10);
    let expected = expectation(&baseline, 1, 11, "committed text");
    let retained = expected.clone();
    let mut later = projected(4, 12);
    later.tabs[1].tab_label = label("subsequent rename");
    later.tabs.rotate_left(1);
    let validated = validate_renamed_tab(&baseline, &expected, later.clone()).unwrap();
    assert_eq!(validated.renamed_index, Some(0));
    assert_eq!(validated.candidate, later);
    later.tabs.remove(0);
    let validated = validate_renamed_tab(&baseline, &expected, later.clone()).unwrap();
    assert_eq!(validated.renamed_index, None);
    assert_eq!(validated.candidate, later);
    later.tabs.clear();
    let validated = validate_renamed_tab(&baseline, &expected, later.clone()).unwrap();
    assert_eq!(validated.renamed_index, None);
    assert_eq!(validated.candidate, later);
    assert_eq!(expected, retained);
    assert_eq!(baseline, projected(3, 10));
}

#[test]
fn later_history_still_enforces_canonical_identity_and_membership_bounds() {
    let baseline = projected(3, 10);
    let expected = expectation(&baseline, 1, 11, "changed");
    let mut failures = Vec::new();
    let mut wrong = projected(3, 12);
    wrong.site_id = site_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.terminal_context_id = context_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs[1].tab_id = tab_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs[1].panel_id = panel_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs[1].presentation_id = presentation_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs[1].binding.terminal_id = terminal_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs.push(wrong.tabs[0].clone());
    failures.push(wrong);
    let mut wrong = projected(MAXIMUM_TABS_PER_CONTEXT, 12);
    let extra = project_tab(&tab(MAXIMUM_TABS_PER_CONTEXT)).unwrap();
    wrong.tabs.push(extra);
    failures.push(wrong);
    for wrong in failures {
        assert_eq!(
            validate_renamed_tab(&baseline, &expected, wrong),
            Err(NativeProjectionDivergence)
        );
    }
}

fn expectation(
    baseline: &NativeStandaloneTabs,
    target: usize,
    revision: u64,
    requested_label: &str,
) -> RenamedTabExpectation {
    RenamedTabExpectation::new(
        WorkbenchRevision::new(revision),
        baseline.terminal_context_id.clone(),
        baseline.tabs[target].tab_id.clone(),
        label(requested_label),
    )
}
