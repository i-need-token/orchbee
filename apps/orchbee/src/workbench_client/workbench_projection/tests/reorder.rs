use super::*;

#[test]
fn higher_revision_reorders_preserve_members_at_every_supported_length() {
    for count in 0..=MAXIMUM_TABS_PER_CONTEXT {
        let current = projected(count, 10);
        let mut candidate = projected(count, 11);
        candidate.tabs.reverse();
        for tab in &mut candidate.tabs {
            tab.tab_label = label("renamed");
        }
        assert_eq!(
            validate_canonical_refresh(&current, candidate.clone()),
            Ok(candidate)
        );
        if count < MAXIMUM_TABS_PER_CONTEXT {
            let mut candidate = projected(count + 1, 12);
            candidate.tabs.rotate_right(1);
            assert_eq!(
                validate_canonical_refresh(&current, candidate.clone()),
                Ok(candidate)
            );
        }
    }
}

#[test]
fn projector_rejects_duplicate_structural_ids_in_any_member() {
    for field in 0..3 {
        let mut candidate = snapshot(3, 11);
        let first = candidate.terminal_contexts[0].tabs[0].clone();
        let last = &mut candidate.terminal_contexts[0].tabs[2];
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = first.root
        else {
            unreachable!()
        };
        match field {
            0 => last.tab_id = first.tab_id,
            1 => {
                let PanelNode::Panel {
                    panel_id: target, ..
                } = &mut last.root
                else {
                    unreachable!()
                };
                *target = panel_id;
            }
            _ => {
                let PanelNode::Panel {
                    primary_presentation: target,
                    ..
                } = &mut last.root
                else {
                    unreachable!()
                };
                target.presentation_id = primary_presentation.presentation_id;
            }
        }
        assert_eq!(
            project_preferred_workspace(&candidate),
            Err(NativeProjectionError),
            "field {field}"
        );
    }
}

#[test]
fn projector_rejects_a_terminal_bound_twice_even_with_distinct_workers() {
    let mut candidate = snapshot(2, 11);
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut candidate.terminal_contexts[0].tabs[1].root
    else {
        unreachable!()
    };
    primary_presentation.binding.as_mut().unwrap().terminal_id = terminal_id(tab_seed(0) + 4);
    assert_eq!(
        project_preferred_workspace(&candidate),
        Err(NativeProjectionError)
    );
}

#[test]
fn created_member_is_found_after_later_moves_but_must_be_new_at_successor_revision() {
    for count in [0, 1, 2, MAXIMUM_TABS_PER_CONTEXT - 1] {
        let current = projected(count, 10);
        let immediate = created_candidate(count, count + 1, 11, "created");
        let expected = expectation_for(&immediate, count, 11, "created");
        let mut later = immediate.clone();
        later.revision = WorkbenchRevision::new(12);
        later.tabs.rotate_right(1);
        later.tabs[0].tab_label = label("renamed created");
        assert_eq!(
            validate_created_tab(&current, &expected, later.clone()),
            Ok(ValidatedCreatedProjection {
                candidate: later,
                created_index: Some(0)
            })
        );
        if count > 0 {
            let mut reordered_at_commit = immediate;
            reordered_at_commit.tabs.rotate_right(1);
            assert_eq!(
                validate_created_tab(&current, &expected, reordered_at_commit),
                Err(NativeProjectionDivergence)
            );
        }
    }
    let current = projected(2, 10);
    let candidate = projected(3, 12);
    let reused = expectation_for(&candidate, 0, 11, "1");
    assert_eq!(
        validate_created_tab(&current, &reused, candidate.clone()),
        Err(NativeProjectionDivergence)
    );
    let wrong_successor = expectation_for(&candidate, 2, 12, "3");
    assert_eq!(
        validate_created_tab(&current, &wrong_successor, candidate),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn reordered_members_still_reject_every_immutable_change_and_duplicate_identity() {
    let current = projected(2, 10);
    for field in 0..8 {
        let mut candidate = projected(3, 12);
        candidate.tabs.rotate_left(1);
        let tab = &mut candidate.tabs[2];
        match field {
            0 => tab.panel_id = panel_id(900),
            1 => tab.presentation_id = presentation_id(901),
            2 => tab.presentation_label = label("wrong"),
            3 => tab.binding.as_mut().unwrap().worker_id = worker_id(902),
            4 => tab.binding.as_mut().unwrap().terminal_id = terminal_id(903),
            5 => candidate.tabs[1].tab_id = candidate.tabs[0].tab_id.clone(),
            6 => candidate.tabs[1].panel_id = candidate.tabs[0].panel_id.clone(),
            _ => candidate.tabs[1].presentation_id = candidate.tabs[0].presentation_id.clone(),
        }
        assert_eq!(
            validate_canonical_refresh(&current, candidate),
            Err(NativeProjectionDivergence),
            "field {field}"
        );
    }
    let mut candidate = created_candidate(2, 3, 12, "created");
    let expected = expectation_for(&candidate, 2, 11, "created");
    candidate.tabs.rotate_right(1);
    candidate.tabs[0].presentation_label = label("renamed presentation");
    assert_eq!(
        validate_created_tab(&current, &expected, candidate),
        Err(NativeProjectionDivergence)
    );
}

#[test]
fn workspace_order_changes_navigation_without_changing_selected_context() {
    let mut source = snapshot(1, 10);
    for value in [201, 211] {
        source.navigation.workspaces.extend(vec![Workspace {
            project_id: Some(
                (WorkbenchProjectId::parse(&format!("{value:032x}")).unwrap()).clone(),
            ),
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", value + 1)).unwrap(),
            label: format!("workspace {value}"),
            primary_directory_id: None,
            terminal_context_id: context_id(value + 2),
        }]);
        source.navigation.projects.push(Project {
            project_id: WorkbenchProjectId::parse(&format!("{value:032x}")).unwrap(),
            label: format!("project {value}"),
        });
        source
            .terminal_contexts
            .push(terminal_context(value + 2, 0));
    }
    let old = project_preferred_workspace(&source).unwrap();
    let selected = old.navigation[1].location.clone();
    let before = project_native_context_tabs(&source, &selected).unwrap();
    source.navigation.workspace_order = source
        .navigation
        .workspaces
        .iter()
        .rev()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    source.revision = WorkbenchRevision::new(11);
    let after = project_native_context_tabs(&source, &selected).unwrap();
    assert_eq!(after.location, before.location);
    assert_eq!(after.navigation[0].location, old.navigation[2].location);
    assert_eq!(after.navigation[1].location, selected);
    assert_eq!(after.projects, before.projects);
    assert_eq!(after.tabs, before.tabs);
    assert_eq!(
        validate_canonical_refresh(&before, after.clone()),
        Ok(after)
    );
}
