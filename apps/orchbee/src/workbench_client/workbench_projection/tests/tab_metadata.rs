use super::*;

fn grouped() -> Snapshot {
    let mut value = snapshot(4, 1);
    let context = &mut value.terminal_contexts[0];
    context.groups = (0..2)
        .map(|n| TabGroup {
            group_id: group_id(800 + n),
            label: format!("group {n}"),
            color: TabGroupColor::Cyan,
        })
        .collect();
    context.tabs[0].pinned = true;
    for tab in &mut context.tabs[1..3] {
        tab.group_id = Some(group_id(800));
    }
    value
}

#[test]
fn metadata_is_retained_and_only_newer_canonical_revisions_can_change_it() {
    let original = grouped();
    let baseline = project_preferred_workspace(&original).unwrap();
    assert_eq!(baseline.groups(), &original.terminal_contexts[0].groups);
    assert!(baseline.tabs()[0].pinned());
    assert_eq!(baseline.tabs()[0].group_label(), None);
    assert_eq!(baseline.tabs()[1].group_label(), Some("group 0"));
    for change in 0..3 {
        let mut next = original.clone();
        match change {
            0 => next.terminal_contexts[0].groups[1].label = "unused renamed".into(),
            1 => next.terminal_contexts[0].groups[0].color = TabGroupColor::Blue,
            _ => next.terminal_contexts[0].tabs[0].pinned = false,
        }
        let candidate = project_preferred_workspace(&next).unwrap();
        assert!(validate_canonical_refresh(&baseline, candidate.clone()).is_err());
        assert!(
            validate_retained_context(
                baseline.site_id(),
                baseline.location(),
                baseline.revision(),
                baseline.tabs(),
                baseline.groups(),
                &candidate
            )
            .is_err()
        );
        next.revision = WorkbenchRevision::new(2);
        let candidate = project_preferred_workspace(&next).unwrap();
        assert!(validate_canonical_refresh(&baseline, candidate).is_ok());
    }
}

#[test]
fn metadata_rejects_duplicate_missing_fragmented_and_pinned_group_records() {
    for change in 0..7 {
        let mut value = grouped();
        let context = &mut value.terminal_contexts[0];
        match change {
            0 => context.groups.push(context.groups[0].clone()),
            1 => context.tabs[3].group_id = Some(group_id(999)),
            2 => {
                context.tabs[2].group_id = None;
                context.tabs[3].group_id = Some(group_id(800));
            }
            3 => context.tabs[3].pinned = true,
            4 => context.tabs[0].group_id = Some(group_id(800)),
            5 => context.groups[1].label = "a".repeat(513),
            _ => {
                context.groups = (0..=orchbee_workbench_protocol::MAXIMUM_TAB_GROUPS_PER_CONTEXT)
                    .map(|n| TabGroup {
                        group_id: group_id(800 + n as u128),
                        label: format!("group {n}"),
                        color: TabGroupColor::Cyan,
                    })
                    .collect()
            }
        }
        assert_eq!(
            project_preferred_workspace(&value),
            Err(NativeProjectionError),
            "{change}"
        );
    }
}

#[test]
fn exact_move_receipt_cannot_also_change_group_or_pin_metadata() {
    let mut original = grouped();
    // Two pinned Tabs allow a legal one-position move.
    original.terminal_contexts[0].tabs[1].group_id = None;
    original.terminal_contexts[0].tabs[1].pinned = true;
    let baseline = project_preferred_workspace(&original).unwrap();
    let expected = MovedTabExpectation::new(
        WorkbenchRevision::new(2),
        context_id(2),
        baseline.tabs()[0].tab_id().clone(),
        WorkbenchTabMoveDirection::Next,
    );
    let mut moved = original.clone();
    moved.revision = WorkbenchRevision::new(2);
    moved.terminal_contexts[0].tabs.swap(0, 1);
    assert!(
        validate_moved_tab(
            &baseline,
            &expected,
            project_preferred_workspace(&moved).unwrap()
        )
        .is_ok()
    );
    moved.terminal_contexts[0].groups[1].label = "unexpected".into();
    assert!(
        validate_moved_tab(
            &baseline,
            &expected,
            project_preferred_workspace(&moved).unwrap()
        )
        .is_err()
    );
    moved.revision = WorkbenchRevision::new(3);
    assert!(
        validate_moved_tab(
            &baseline,
            &expected,
            project_preferred_workspace(&moved).unwrap()
        )
        .is_ok()
    );
}
