use super::super::renamed::{RenamedTabExpectation, validate_renamed_tab};
use super::*;

#[test]
fn automatic_equal_text_rename_clears_mode_and_cannot_be_undone_by_a_refresh() {
    let mut baseline = projected(3, 10);
    baseline.tabs[1].automatic_label = true;
    let requested = baseline.tabs[1].tab_label.clone();
    let expected = expectation(&baseline, 1, 11, requested.as_str());
    let mut candidate = baseline.clone();
    candidate.revision = WorkbenchRevision::new(11);
    assert!(validate_renamed_tab(&baseline, &expected, candidate.clone()).is_err());
    candidate.tabs[1].automatic_label = false;
    assert!(validate_renamed_tab(&baseline, &expected, candidate.clone()).is_ok());
    let mut reversed = candidate.clone();
    reversed.revision = WorkbenchRevision::new(12);
    reversed.tabs[1].automatic_label = true;
    assert!(validate_canonical_refresh(&candidate, reversed).is_err());
    let mut invented = baseline.clone();
    invented.revision = WorkbenchRevision::new(11);
    invented.tabs[1].tab_label = label("different raw text without a rename");
    assert!(validate_canonical_refresh(&baseline, invented).is_err());
}

#[test]
fn automatic_survivors_derive_positions_after_reorder_and_removal_without_metadata_changes() {
    let mut before = projected(3, 10);
    before.tabs[0].automatic_label = true;
    before.tabs[2].automatic_label = true;
    before.tabs[1].tab_label = label("2");
    let mut moved = before.clone();
    moved.tabs.rotate_right(1);
    moved.revision = WorkbenchRevision::new(11);
    let moved = validate_canonical_refresh(&before, moved).unwrap();
    assert_eq!(moved.tabs[0], before.tabs[2]);
    assert_eq!(moved.tabs[1], before.tabs[0]);
    assert_eq!(moved.tabs[2], before.tabs[1]);
    assert_eq!(
        moved
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| t.display_label(i).into_owned())
            .collect::<Vec<_>>(),
        ["1", "2", "2"]
    );
    let mut removed = moved.clone();
    removed.tabs.remove(0);
    removed.revision = WorkbenchRevision::new(12);
    let removed = validate_canonical_refresh(&moved, removed).unwrap();
    assert_eq!(removed.tabs[0], before.tabs[0]);
    assert_eq!(removed.tabs[0].display_label(0), "1");
    assert_eq!(removed.tabs[1].display_label(1), "2");
}

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
    wrong.tabs[1].binding.as_mut().unwrap().worker_id = worker_id(999);
    failures.push(wrong);
    let mut wrong = correct.clone();
    wrong.tabs[1].panels[0].primary.launch_program = None;
    failures.push(wrong);
    for wrong in failures {
        assert_eq!(
            validate_renamed_tab(&baseline, &expected, wrong),
            Err(ClientProjectionDivergence)
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
            baseline.terminal_context_id().unwrap().clone(),
            tab_id(999),
            label("changed"),
        ),
    ] {
        assert_eq!(
            validate_renamed_tab(&baseline, &expected, candidate.clone()),
            Err(ClientProjectionDivergence)
        );
    }
    for revision in [9, 10] {
        assert_eq!(
            validate_renamed_tab(
                &baseline,
                &expectation(&baseline, 1, 11, "changed"),
                projected(3, revision),
            ),
            Err(ClientProjectionDivergence)
        );
    }
    let maximum = projected(1, u64::MAX);
    assert_eq!(
        validate_renamed_tab(
            &maximum,
            &expectation(&maximum, 0, u64::MAX, "changed"),
            maximum.clone(),
        ),
        Err(ClientProjectionDivergence)
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
    wrong.session_id = session_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.location = {
        let fixture_context_id = context_id(999);
        ClientContextLocation::Workspace {
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap(),
            terminal_context_id: fixture_context_id,
        }
    };
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs[1].panel_id = panel_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs[1].presentation_id = presentation_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs[1].binding.as_mut().unwrap().terminal_id = terminal_id(999);
    failures.push(wrong);
    let mut wrong = projected(3, 12);
    wrong.tabs.push(wrong.tabs[0].clone());
    failures.push(wrong);
    let mut wrong = projected(MAXIMUM_TABS_PER_CONTEXT, 12);
    let extra = project_tab(&tab(MAXIMUM_TABS_PER_CONTEXT), &[]).unwrap();
    wrong.tabs.push(extra);
    failures.push(wrong);
    for wrong in failures {
        assert_eq!(
            validate_renamed_tab(&baseline, &expected, wrong),
            Err(ClientProjectionDivergence)
        );
    }
}

#[test]
fn later_history_accepts_member_owner_migration_when_the_original_tab_vanishes() {
    // Same authorized delta as the canonical rule: the member keeps its
    // complete immutable tuple under a fresh owning Tab while the original
    // Tab is gone — the folded shape of relocating to a fresh Tab plus
    // closing the old Tab that held only the temporary leaf. The committed
    // rename target is simply absent from later history, which the
    // expectation already tolerates.
    let baseline = projected(3, 10);
    let expected = expectation(&baseline, 1, 11, "changed");
    let mut migrated = projected(3, 12);
    migrated.tabs[1].tab_id = tab_id(999);
    let validated = validate_renamed_tab(&baseline, &expected, migrated.clone()).unwrap();
    assert_eq!(validated.renamed_index, None);
    assert_eq!(validated.candidate, migrated);
}

fn expectation(
    baseline: &ClientContextTabs,
    target: usize,
    revision: u64,
    requested_label: &str,
) -> RenamedTabExpectation {
    RenamedTabExpectation::new(
        WorkbenchRevision::new(revision),
        baseline.terminal_context_id().unwrap().clone(),
        baseline.tabs[target].tab_id.clone(),
        label(requested_label),
    )
}
