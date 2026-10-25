use super::*;
use orchbee_workbench_protocol::{TabConfiguration, TabGroup, TabGroupColor, WorkbenchTabGroupId};

fn group(value: u128) -> TabGroup {
    TabGroup {
        group_id: WorkbenchTabGroupId::parse(&id(value)).unwrap(),
        label: "Group".into(),
        color: TabGroupColor::Blue,
    }
}

#[test]
fn tab_configuration_selected_set_receipt_requires_every_member_and_keeps_unselected_data() {
    let baseline = snapshot(3, 1, None);
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let ids = vec![
        baseline.terminal_contexts[0].tabs[0].tab_id.clone(),
        baseline.terminal_contexts[0].tabs[2].tab_id.clone(),
    ];
    let operation = state
        .plan_tab_configuration(
            TabConfiguration::SetTabsPinned {
                tab_ids: ids.try_into().unwrap(),
                pinned: true,
            },
            WorkbenchRequestId::parse(&id(95)).unwrap(),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let known = operation
        .known_commit(WorkbenchSetupCommit::TabsConfigured {
            revision: WorkbenchRevision::new(2),
            group_id: None,
            unchanged: false,
        })
        .unwrap();
    state.begin_setup(operation);
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    let mut exact = baseline.clone();
    exact.revision = WorkbenchRevision::new(2);
    let mut third = exact.terminal_contexts[0].tabs.remove(2);
    third.pinned = true;
    exact.terminal_contexts[0].tabs[0].pinned = true;
    exact.terminal_contexts[0].tabs.insert(1, third);
    assert!(matches!(
        state.prepare_setup_refresh(1, &exact),
        PreparedRefresh::Membership { .. }
    ));
    for fault in 0..3 {
        let mut hostile = exact.clone();
        match fault {
            0 => hostile.terminal_contexts[0].tabs[1].pinned = false,
            1 => hostile.terminal_contexts[0].tabs.swap(0, 1),
            _ => hostile.terminal_contexts[0].tabs[2].label = "Unrelated edit".into(),
        }
        assert!(
            matches!(
                state.prepare_setup_refresh(1, &hostile),
                PreparedRefresh::Divergent { .. }
            ),
            "fault {fault}"
        );
        assert_eq!(state.cache().canonical_snapshot(), &baseline);
    }
}
fn plan(state: &mut NativeSessionState, create: bool) -> SetupOperation {
    let tab_id = state.cache().tabs()[0].tab_id().clone();
    let operation = state
        .plan_tab_configuration(
            if create {
                TabConfiguration::CreateGroup {
                    tab_id,
                    label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse("Group")
                        .unwrap(),
                    color: TabGroupColor::Blue,
                }
            } else {
                TabConfiguration::SetPinned {
                    tab_id,
                    pinned: true,
                }
            },
            WorkbenchRequestId::parse(&id(91)).unwrap(),
            1,
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    state.begin_setup(operation.clone());
    operation
}

#[test]
fn tab_configuration_receipts_validate_new_identity_noop_bit_and_revision() {
    let mut baseline = workspace_snapshot(1, 1);
    baseline.terminal_contexts[1].groups.push(group(73));
    baseline.terminal_contexts[1].tabs[0].group_id = Some(group(73).group_id);
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let operation = plan(&mut state, true);
    for (revision, group_id, unchanged) in [
        (2, None, false),
        (1, Some(group(74).group_id), false),
        (2, Some(group(73).group_id), false),
        (2, Some(group(74).group_id), true),
    ] {
        assert!(
            operation
                .known_commit(WorkbenchSetupCommit::TabsConfigured {
                    revision: WorkbenchRevision::new(revision),
                    group_id,
                    unchanged
                })
                .is_none()
        );
    }
    assert!(
        operation
            .known_commit(WorkbenchSetupCommit::TabsConfigured {
                revision: WorkbenchRevision::new(2),
                group_id: Some(group(74).group_id),
                unchanged: false
            })
            .is_some()
    );
    assert_eq!(state.cache().canonical_snapshot(), &baseline);
}

#[test]
fn tab_configuration_exact_receipt_rejects_unrelated_metadata_and_later_revision_remains_recoverable()
 {
    let baseline = workspace_snapshot(1, 1);
    let mut state = ready_with_cache(project_preferred_workspace(&baseline).unwrap());
    let old = state.desired().cloned();
    let operation = plan(&mut state, true);
    let known = operation
        .known_commit(WorkbenchSetupCommit::TabsConfigured {
            revision: WorkbenchRevision::new(2),
            group_id: Some(group(74).group_id),
            unchanged: false,
        })
        .unwrap();
    assert!(state.retain_setup_worker(SetupWorkerEvidence::Known(known)));
    let mut exact = baseline.clone();
    exact.revision = WorkbenchRevision::new(2);
    exact.terminal_contexts[0].groups.push(group(74));
    exact.terminal_contexts[0].tabs[0].group_id = Some(group(74).group_id);
    assert!(matches!(
        state.prepare_setup_refresh(1, &exact),
        PreparedRefresh::Membership { .. }
    ));
    for mutation in 0..8 {
        let mut wrong = exact.clone();
        match mutation {
            0 => wrong.terminal_contexts[0].groups[0].color = TabGroupColor::Red,
            1 => wrong.terminal_contexts[0].groups[0].label = "Different".into(),
            2 => wrong.terminal_contexts[0].tabs[0].label = "Different".into(),
            3 => wrong.terminal_contexts[0].tabs[0].automatic_label = true,
            4 => wrong.terminal_contexts[1].tabs[0].label = "Unrelated context".into(),
            5 => wrong.navigation.projects[0].label = "Unrelated navigation".into(),
            6 => {
                let PanelNode::Panel {
                    primary_presentation,
                    ..
                } = &mut wrong.terminal_contexts[0].tabs[0].root
                else {
                    panic!("panel")
                };
                primary_presentation.label = "Changed presentation".into();
            }
            _ => wrong.terminal_contexts[0].tabs[0].group_id = None,
        }
        assert!(
            matches!(
                state.prepare_setup_refresh(1, &wrong),
                PreparedRefresh::Divergent { .. }
            ),
            "mutation {mutation}"
        );
        assert_eq!(state.cache().canonical_snapshot(), &baseline);
        assert_eq!(state.desired(), old.as_ref());
    }
    exact.revision = WorkbenchRevision::new(3);
    exact.terminal_contexts[0].groups[0].label = "Subsequent peer edit".into();
    assert_eq!(
        finish(&mut state, exact, 1),
        MembershipInstallDecision::Preserved
    );
    assert_eq!(state.desired(), old.as_ref());
    assert!(state.invariant_holds());
}
