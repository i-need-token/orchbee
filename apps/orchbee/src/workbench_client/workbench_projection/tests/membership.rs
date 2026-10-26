use super::*;

#[test]
fn removed_member_cannot_disguise_retained_structural_identity_as_a_fresh_add() {
    let current = projected(1, 10);
    for retained in 0..4 {
        let mut candidate = projected(2, 11);
        candidate.tabs.remove(0);
        match retained {
            0 => candidate.tabs[0].tab_id = current.tabs[0].tab_id.clone(),
            1 => candidate.tabs[0].panel_id = current.tabs[0].panel_id.clone(),
            2 => candidate.tabs[0].presentation_id = current.tabs[0].presentation_id.clone(),
            3 => {
                candidate.tabs[0].binding.as_mut().unwrap().terminal_id = current.tabs[0]
                    .binding
                    .as_ref()
                    .unwrap()
                    .terminal_id
                    .clone()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_canonical_refresh(&current, candidate),
            Err(ClientProjectionDivergence)
        );
    }
    let mut fresh = projected(2, 11);
    fresh.tabs.remove(0);
    fresh.tabs[0].binding.as_mut().unwrap().worker_id =
        current.tabs[0].binding.as_ref().unwrap().worker_id.clone();
    fresh.tabs[0].panels[0].primary.binding = fresh.tabs[0].binding.clone();
    assert_eq!(
        validate_canonical_refresh(&current, fresh.clone()),
        Ok(fresh)
    );
}

#[test]
fn canonical_membership_accepts_higher_revision_removal_at_every_boundary() {
    for count in [0, 1, 255, 256] {
        let current = projected(count, 10);
        let mut empty = current.clone();
        empty.revision = WorkbenchRevision::new(11);
        empty.tabs.clear();
        assert_eq!(
            validate_canonical_refresh(&current, empty.clone()),
            Ok(empty)
        );
        if count == 0 {
            continue;
        }
        for removed in [0, count / 2, count - 1] {
            let mut candidate = current.clone();
            candidate.revision = WorkbenchRevision::new(11);
            candidate.tabs.remove(removed);
            candidate.tabs.reverse();
            for tab in &mut candidate.tabs {
                tab.tab_label = label("surviving renamed member");
            }
            assert_eq!(
                validate_canonical_refresh(&current, candidate.clone()),
                Ok(candidate.clone())
            );
            candidate.revision = current.revision;
            assert_eq!(
                validate_canonical_refresh(&current, candidate),
                Err(ClientProjectionDivergence)
            );
        }
    }
}

#[test]
fn later_created_projection_can_omit_the_committed_target_and_old_members() {
    let current = projected(2, 10);
    let committed = created_candidate(2, 3, 11, "created");
    let expected = expectation_for(&committed, 2, 11, "created");
    let mut snapshot = committed.canonical_snapshot().clone();
    snapshot.revision = WorkbenchRevision::new(12);
    snapshot.terminal_contexts[0].tabs.remove(2);
    snapshot.terminal_contexts[0].tabs.remove(0);
    let later = project_preferred_workspace(&snapshot).unwrap();
    let validated = validate_created_tab(&current, &expected, later.clone()).unwrap();
    assert_eq!(validated.candidate, later);
    assert_eq!(validated.created_index, None);
    let mut missing_at_commit = committed;
    missing_at_commit.tabs.remove(2);
    assert!(validate_created_tab(&current, &expected, missing_at_commit).is_err());
}

#[test]
fn later_move_projection_keeps_commit_when_target_has_been_removed() {
    let current = projected(3, 10);
    let expected = MovedTabExpectation::new(
        WorkbenchRevision::new(11),
        current.terminal_context_id().unwrap().clone(),
        current.tabs[1].tab_id.clone(),
        WorkbenchTabMoveDirection::Last,
    );
    let mut later = current.clone();
    later.revision = WorkbenchRevision::new(12);
    later.tabs.remove(1);
    let validated = validate_moved_tab(&current, &expected, later.clone()).unwrap();
    assert_eq!(validated.candidate, later);
    assert_eq!(validated.moved_index, None);
    later.revision = WorkbenchRevision::new(11);
    assert!(validate_moved_tab(&current, &expected, later).is_err());
}
