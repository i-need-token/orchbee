use super::*;

#[test]
fn created_mode_is_pinned_at_commit_but_a_later_rename_may_fix_it() {
    let current = projected(1, 20);
    let mut candidate = created_candidate(1, 2, 21, "2");
    candidate.tabs[1].automatic_label = true;
    std::sync::Arc::make_mut(&mut candidate.canonical).terminal_contexts[0].tabs[1]
        .automatic_label = true;
    let expected = expectation_for(&candidate, 1, 21, "2");
    assert!(validate_created_tab(&current, &expected, candidate.clone()).is_ok());
    candidate.tabs[1].automatic_label = false;
    assert!(validate_created_tab(&current, &expected, candidate.clone()).is_err());
    candidate.revision = WorkbenchRevision::new(22);
    assert!(validate_created_tab(&current, &expected, candidate).is_ok());
}

#[test]
fn placed_creation_receipt_rejects_extra_writes_wrong_position_and_reused_group_identity() {
    use orchbee_workbench_protocol::{NewTerminalTab, TabCreationPlacement, create_terminal_tab};
    let mut baseline = snapshot(3, 20);
    baseline.terminal_contexts[0].groups.push(TabGroup {
        group_id: group_id(800),
        label: "".into(),
        color: TabGroupColor::Grey,
    });
    baseline.terminal_contexts[0].tabs[0].pinned = true;
    for tab in &mut baseline.terminal_contexts[0].tabs[1..] {
        tab.group_id = Some(group_id(800));
    }
    let current = project_preferred_workspace(&baseline).unwrap();
    for placement in [
        TabCreationPlacement::InGroup {
            group_id: group_id(800),
        },
        TabCreationPlacement::NewGroup {},
        TabCreationPlacement::Duplicate {
            tab_id: baseline.terminal_contexts[0].tabs[1].tab_id.clone(),
        },
    ] {
        let fresh = tab(3);
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = &fresh.root
        else {
            unreachable!()
        };
        let mut candidate = baseline.clone();
        candidate.revision = WorkbenchRevision::new(21);
        candidate.terminal_contexts[0] = create_terminal_tab(
            &baseline.terminal_contexts[0],
            Some(&placement),
            NewTerminalTab {
                tab_id: fresh.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: primary_presentation.presentation_id.clone(),
                label: label("4"),
                automatic_label: true,
                binding: primary_presentation.binding.clone().unwrap(),
            },
            matches!(placement, TabCreationPlacement::NewGroup {}).then_some(&group_id(801)),
        )
        .unwrap();
        let expected = CreatedTabExpectation::new(
            WorkbenchRevision::new(21),
            context_id(2),
            fresh.tab_id.clone(),
            panel_id.clone(),
            primary_presentation.presentation_id.clone(),
            label("4"),
            true,
        )
        .with_placement(Some(placement.clone()));
        assert!(
            validate_created_tab(
                &current,
                &expected,
                project_preferred_workspace(&candidate).unwrap()
            )
            .is_ok()
        );
        let index = candidate.terminal_contexts[0]
            .tabs
            .iter()
            .position(|tab| tab.tab_id == fresh.tab_id)
            .unwrap();
        for change in 0..7 {
            let mut hostile = candidate.clone();
            let context = &mut hostile.terminal_contexts[0];
            match change {
                0 => context.tabs[0].label = "unrelated rename".into(),
                1 => context.tabs[index].automatic_label = !context.tabs[index].automatic_label,
                2 => context.groups[0].color = TabGroupColor::Pink,
                3 => context.tabs.swap(index, index - 1),
                4 => context.tabs[index].group_id = None,
                5 => {
                    if let PanelNode::Panel {
                        primary_presentation,
                        ..
                    } = &mut context.tabs[index].root
                    {
                        primary_presentation.custom_label = Some(
                            orchbee_workbench_protocol::PanelCustomLabel::parse("injected")
                                .unwrap(),
                        );
                    }
                }
                _ => {
                    let extra = context.groups[0].clone();
                    context.groups.push(TabGroup {
                        group_id: group_id(900),
                        ..extra
                    });
                }
            }
            assert!(
                project_preferred_workspace(&hostile)
                    .ok()
                    .is_none_or(|next| validate_created_tab(&current, &expected, next).is_err()),
                "{placement:?} tamper {change}"
            );
        }
        if matches!(placement, TabCreationPlacement::NewGroup {}) {
            let mut reused = candidate.clone();
            reused.terminal_contexts[0].groups.pop();
            reused.terminal_contexts[0].tabs[index].group_id = Some(group_id(800));
            assert!(
                validate_created_tab(
                    &current,
                    &expected,
                    project_preferred_workspace(&reused).unwrap()
                )
                .is_err()
            );
        }
        // The known exact receipt remains historical when a later peer changes metadata/order.
        candidate.revision = WorkbenchRevision::new(22);
        candidate.terminal_contexts[0].groups[0].label = "later".into();
        assert!(
            validate_created_tab(
                &current,
                &expected,
                project_preferred_workspace(&candidate).unwrap()
            )
            .is_ok()
        );
    }
}

#[test]
fn created_tab_accepts_immediate_member_at_zero_and_256_boundaries() {
    for current_tab_count in [0, MAXIMUM_TABS_PER_CONTEXT - 1] {
        let current = projected(current_tab_count, 20);
        let candidate = created_candidate(
            current_tab_count,
            current_tab_count + 1,
            21,
            "  requested label  ",
        );
        let expected = expectation_for(&candidate, current_tab_count, 21, "  requested label  ");
        let expected_member = &candidate.tabs()[current_tab_count];
        assert_eq!(expected.committed_revision(), WorkbenchRevision::new(21));
        assert_eq!(
            Some(expected.terminal_context_id()),
            candidate.terminal_context_id()
        );
        assert_eq!(expected.tab_id(), expected_member.tab_id());
        assert_eq!(expected.panel_id(), expected_member.panel_id());
        assert_eq!(
            expected.presentation_id(),
            expected_member.presentation_id()
        );
        assert_eq!(expected.requested_label().as_str(), "  requested label  ");
        let ValidatedCreatedProjection {
            candidate: accepted,
            created_index,
        } = validate_created_tab(&current, &expected, candidate.clone()).unwrap();
        let index = created_index.unwrap();
        assert_eq!(accepted, candidate);
        assert_eq!(index, current_tab_count);
        assert_eq!(
            accepted.tabs()[index].tab_label().as_str(),
            "  requested label  "
        );
        assert_eq!(
            accepted.tabs()[index].presentation_label().as_str(),
            "  requested label  "
        );
    }
}

#[test]
fn created_tab_accepts_later_concurrent_members_and_snapshot_binding() {
    let current = projected(1, 5);
    let mut candidate = created_candidate(1, 4, 9, "created");
    candidate.tabs[0].tab_label = label("renamed old member");
    candidate.tabs[1].tab_label = label("renamed created member");
    candidate.tabs[1].binding = Some(ClientTerminalBinding {
        worker_id: worker_id(300),
        terminal_id: terminal_id(301),
    });
    candidate.tabs[1].panels[0].primary.binding = candidate.tabs[1].binding.clone();
    let expected = expectation_for(&candidate, 1, 6, "created");
    let ValidatedCreatedProjection {
        candidate: accepted,
        created_index,
    } = validate_created_tab(&current, &expected, candidate.clone()).unwrap();
    let index = created_index.unwrap();
    assert_eq!(accepted, candidate);
    assert_eq!(index, 1);
    assert_eq!(accepted.tabs().len(), 4);
    assert_eq!(
        accepted.tabs()[0].tab_label().as_str(),
        "renamed old member"
    );
    assert_eq!(
        accepted.tabs()[index].tab_label().as_str(),
        "renamed created member"
    );
    assert_eq!(
        accepted.tabs()[index].presentation_label().as_str(),
        "created"
    );
    assert_eq!(
        accepted.tabs()[index]
            .binding()
            .expect("bound fixture")
            .worker_id(),
        &worker_id(300)
    );
    assert_eq!(
        accepted.tabs()[index]
            .binding()
            .expect("bound fixture")
            .terminal_id(),
        &terminal_id(301)
    );
}

#[test]
fn created_tab_rejects_revision_context_prefix_and_immediate_position_divergence() {
    let current = projected(1, 5);
    let candidate = created_candidate(1, 3, 7, "created");
    let expected = expectation_for(&candidate, 1, 6, "created");

    let mut failures = Vec::new();

    let mut lower_than_commit = candidate.clone();
    lower_than_commit.revision = WorkbenchRevision::new(6);
    let mut later_commit = expected.clone();
    later_commit.committed_revision = WorkbenchRevision::new(7);
    failures.push((later_commit, lower_than_commit));

    let mut session_mismatch = candidate.clone();
    session_mismatch.session_id = session_id(400);
    failures.push((expected.clone(), session_mismatch));

    let mut context_mismatch = candidate.clone();
    context_mismatch.location = {
        let fixture_context_id = context_id(401);
        ClientContextLocation::Workspace {
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap(),
            terminal_context_id: fixture_context_id,
        }
    };
    failures.push((expected.clone(), context_mismatch));

    let mut expected_context_mismatch = expected.clone();
    expected_context_mismatch.terminal_context_id = context_id(402);
    failures.push((expected_context_mismatch, candidate.clone()));

    let mut same_revision_append = candidate.clone();
    same_revision_append.revision = current.revision;
    let mut same_revision_expectation = expected.clone();
    same_revision_expectation.committed_revision = current.revision;
    failures.push((same_revision_expectation, same_revision_append));

    for (expected, candidate) in failures {
        assert_eq!(
            validate_created_tab(&current, &expected, candidate),
            Err(ClientProjectionDivergence)
        );
    }
}

#[test]
fn created_tab_rejects_each_committed_tuple_or_label_mismatch() {
    let current = projected(1, 5);
    let candidate = created_candidate(1, 2, 6, "created");
    let expected = expectation_for(&candidate, 1, 6, "created");

    let mut expectations = Vec::new();
    let mut tab_mismatch = expected.clone();
    tab_mismatch.tab_id = tab_id(500);
    expectations.push(tab_mismatch);

    let mut panel_mismatch = expected.clone();
    panel_mismatch.panel_id = panel_id(501);
    expectations.push(panel_mismatch);

    let mut presentation_mismatch = expected.clone();
    presentation_mismatch.presentation_id = presentation_id(502);
    expectations.push(presentation_mismatch);

    let mut label_mismatch = expected.clone();
    label_mismatch.requested_label = label("different request");
    expectations.push(label_mismatch);

    for expected in expectations {
        assert_eq!(
            validate_created_tab(&current, &expected, candidate.clone()),
            Err(ClientProjectionDivergence)
        );
    }

    let mut tab_label_mismatch = candidate.clone();
    tab_label_mismatch.tabs[1].tab_label = label("different tab label");
    assert_eq!(
        validate_created_tab(&current, &expected, tab_label_mismatch),
        Err(ClientProjectionDivergence)
    );

    let mut presentation_label_mismatch = candidate;
    presentation_label_mismatch.tabs[1].presentation_label = label("different presentation label");
    assert_eq!(
        validate_created_tab(&current, &expected, presentation_label_mismatch),
        Err(ClientProjectionDivergence)
    );
}

#[test]
fn created_tab_requires_the_old_prefix_labels_exactly_at_the_commit_revision() {
    let current = projected(2, 5);
    let mut candidate = created_candidate(2, 3, 6, "created");
    let expected = expectation_for(&candidate, 2, 6, "created");
    candidate.tabs[0].tab_label = label("impossible concurrent rename");

    assert_eq!(
        validate_created_tab(&current, &expected, candidate),
        Err(ClientProjectionDivergence)
    );
}

#[test]
fn created_tab_requires_exactly_one_appended_member_at_the_commit_revision() {
    let current = projected(1, 5);
    let candidate = created_candidate(1, 3, 6, "created");
    let expected = expectation_for(&candidate, 1, 6, "created");

    assert_eq!(
        validate_created_tab(&current, &expected, candidate),
        Err(ClientProjectionDivergence)
    );
}
