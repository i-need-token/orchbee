use super::super::super::{WorkbenchPanelCreateBegin, WorkbenchPanelCreatePreparation};
use super::*;
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchPanelSplitDirection, WorkbenchRevision, WorkbenchTabCloseResume,
    WorkbenchTabCloseStatus,
};

fn panel_close_command(
    authority: &WorkbenchAuthority,
    id: u128,
    member: &WorkbenchTerminalTab,
) -> WorkbenchCommand {
    command(
        authority,
        id,
        WorkbenchAction::PanelClose {
            terminal_context_id: authority
                .tree_standalone_membership()
                .unwrap()
                .terminal_context_id()
                .clone(),
            tab_id: member.tab_id().clone(),
            panel_id: member.panel_id().clone(),
        },
    )
}

/// Resolves the admission chain and returns a result only for decisions that need no runtime.
fn resolve(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchCommandResult {
    match authority.begin_terminal_tab_close(Some(grant), command) {
        WorkbenchTabCloseBegin::Ready(result) => result,
        WorkbenchTabCloseBegin::Admitted(admission) => {
            match admission.resolve_after_composition() {
                WorkbenchTabCloseResolution::Ready(result) => result,
                WorkbenchTabCloseResolution::Primary(_) => {
                    panic!("expected a Close decision without runtime admission")
                }
            }
        }
    }
}

/// Splits the standalone Tab's first leaf once; returns the original and created leaf members.
fn split_fixture(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command_id: u128,
) -> (WorkbenchTerminalTab, WorkbenchTerminalTab) {
    let membership = authority.tree_standalone_membership().unwrap();
    let source = membership.members().first().unwrap().clone();
    let WorkbenchPanelCreateBegin::Admitted(admission) = authority.begin_terminal_panel_create(
        Some(grant),
        command(
            authority,
            command_id,
            WorkbenchAction::PanelSplit {
                right_click: Default::default(),
                terminal_context_id: membership.terminal_context_id().clone(),
                tab_id: source.tab_id().clone(),
                panel_id: source.panel_id().clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("split").unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        ),
    ) else {
        panic!("panel split admission")
    };
    let WorkbenchPanelCreatePreparation::Prepared(prepared) =
        admission.finish(binding(9000 + command_id))
    else {
        panic!("panel split preparation")
    };
    let pending = prepared.commit_into_pending();
    let _ = authority
        .prepare_terminal_panel_create_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            },
        )
        .unwrap()
        .commit();
    let membership = authority.tree_standalone_membership().unwrap();
    let created = membership
        .members_for_tab(source.tab_id())
        .find(|member| member.panel_id() != source.panel_id())
        .cloned()
        .expect("split produced a second leaf");
    (source, created)
}

#[test]
fn panel_close_accepts_disposes_and_collapses_the_parent_split() {
    let (mut authority, grant) = fixture(1);
    let (source, created) = split_fixture(&mut authority, &grant, 20000);
    let now = Instant::now();
    let close = panel_close_command(&authority, 20001, &created);
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    let WorkbenchCommandOutcome::PanelCloseAccepted {
        close_request_id,
        terminal_context_id,
        tab_id,
        panel_id,
        accepted_revision,
    } = accepted.outcome
    else {
        panic!("panel.close must be accepted: {:?}", accepted.outcome)
    };
    assert_eq!(close_request_id, close.request_id);
    assert_eq!(tab_id, *created.tab_id());
    assert_eq!(panel_id, *created.panel_id());
    assert_eq!(accepted_revision, close.expected_revision);
    assert_eq!(
        terminal_context_id,
        *authority
            .tree_standalone_membership()
            .unwrap()
            .terminal_context_id()
    );
    // The leaf remains projected until the phased Close publishes.
    assert_eq!(
        authority
            .tree_standalone_membership()
            .unwrap()
            .members()
            .len(),
        2
    );
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available { .. }
    ));
    dispose_and_cleanup(&mut authority, &close.request_id, now);
    let prepared = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap();
    assert_eq!(prepared.exact_removed_members(), &[created]);
    let closed_revision = prepared.revision();
    let _ = prepared.commit();
    let snapshot = authority.projection.snapshot();
    assert_eq!(snapshot.revision, closed_revision);
    let membership = authority.tree_standalone_membership().unwrap();
    assert_eq!(membership.members(), std::slice::from_ref(&source));
    let tab = &snapshot.terminal_contexts[0].tabs[0];
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("collapsed Tab must restore a single leaf root")
    };
    assert_eq!(panel_id, source.panel_id());
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::Closed { revision },
            ..
        } if revision == closed_revision
    ));
}

#[test]
fn panel_close_on_the_only_leaf_closes_the_whole_tab() {
    let (mut authority, grant) = fixture(1);
    let now = Instant::now();
    let membership = authority.standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let close = panel_close_command(&authority, 20010, &member);
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));
    dispose_and_cleanup(&mut authority, &close.request_id, now);
    let prepared = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap();
    assert_eq!(prepared.exact_removed_member(), Some(&member));
    let _ = prepared.commit();
    let context = &authority.projection.snapshot().terminal_contexts[0];
    assert!(context.tabs.is_empty());
    assert!(authority.tree_standalone_membership().unwrap().is_empty());
}

#[test]
fn panel_close_rejects_unknown_panels_and_tabs() {
    let (mut authority, grant) = fixture(1);
    let (source, created) = split_fixture(&mut authority, &grant, 20020);
    let membership = authority.tree_standalone_membership().unwrap();
    let context_id = membership.terminal_context_id().clone();
    let missing_panel = command(
        &authority,
        20021,
        WorkbenchAction::PanelClose {
            terminal_context_id: context_id.clone(),
            tab_id: source.tab_id().clone(),
            panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&identity(424242))
                .unwrap(),
        },
    );
    assert!(matches!(
        resolve(&mut authority, &grant, missing_panel).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::PanelUnavailable,
            ..
        }
    ));
    let missing_tab = command(
        &authority,
        20022,
        WorkbenchAction::PanelClose {
            terminal_context_id: context_id,
            tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&identity(434343)).unwrap(),
            panel_id: created.panel_id().clone(),
        },
    );
    assert!(matches!(
        resolve(&mut authority, &grant, missing_tab).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::PanelUnavailable,
            ..
        }
    ));
}

#[test]
fn close_coverage_governs_join_busy_and_disjoint_panel_requests() {
    let (mut authority, grant) = fixture(1);
    let (source, created) = split_fixture(&mut authority, &grant, 20030);
    let now = Instant::now();
    let close = panel_close_command(&authority, 20031, &created);
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    // An identical in-flight panel.close joins the retained attempt without a runtime lookup.
    let alias = panel_close_command(&authority, 20032, &created);
    let joined = resolve(&mut authority, &grant, alias.clone());
    assert_eq!(joined.outcome, accepted.outcome);
    assert_eq!(joined.request_id, alias.request_id);
    // A tab.close overlapping the open panel.close is told the member is busy.
    let membership = authority.tree_standalone_membership().unwrap();
    let whole = command(
        &authority,
        20033,
        WorkbenchAction::TabClose {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: source.tab_id().clone(),
        },
    );
    let busy = resolve(&mut authority, &grant, whole);
    assert!(matches!(
        busy.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    // A disjoint leaf close proceeds independently.
    let disjoint = panel_close_command(&authority, 20034, &source);
    let accepted_disjoint = accept(&mut authority, &grant, disjoint.clone(), now);
    assert!(matches!(
        accepted_disjoint.outcome,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));
    assert_eq!(authority.outstanding_close_credits().ok(), Some(2));
}

#[test]
fn sibling_panel_closes_publish_in_turn_and_reach_single_leaf_then_empty() {
    let (mut authority, grant) = fixture(1);
    let (source, created) = split_fixture(&mut authority, &grant, 20040);
    let now = Instant::now();
    let first = panel_close_command(&authority, 20041, &created);
    let second = panel_close_command(&authority, 20042, &source);
    accept(&mut authority, &grant, first.clone(), now);
    accept(&mut authority, &grant, second.clone(), now);
    dispose_and_cleanup(&mut authority, &first.request_id, now);
    dispose_and_cleanup(&mut authority, &second.request_id, now);
    // Publish one leaf first; the surviving sibling collapses to the Tab root.
    let _ = authority
        .prepare_terminal_tab_close_publication(&first.request_id, now)
        .unwrap()
        .commit();
    let membership = authority.tree_standalone_membership().unwrap();
    assert_eq!(membership.members(), std::slice::from_ref(&source));
    let tab = authority.projection.snapshot().terminal_contexts[0].tabs[0].clone();
    let PanelNode::Panel { panel_id, .. } = &tab.root else {
        panic!("first publication must collapse the split to the surviving leaf")
    };
    assert_eq!(panel_id, source.panel_id());
    // The surviving leaf's panel-scoped Close now publishes whole-Tab removal.
    let _ = authority
        .prepare_terminal_tab_close_publication(&second.request_id, now)
        .unwrap()
        .commit();
    let context = &authority.projection.snapshot().terminal_contexts[0];
    assert!(context.tabs.is_empty());
    assert!(authority.tree_standalone_membership().unwrap().is_empty());
}

#[test]
fn completed_leaf_close_tombstone_does_not_fence_the_same_tab_close() {
    let (mut authority, grant) = fixture(1);
    let (source, created) = split_fixture(&mut authority, &grant, 20060);
    let now = Instant::now();
    let leaf = panel_close_command(&authority, 20061, &created);
    accept(&mut authority, &grant, leaf.clone(), now);
    dispose_and_cleanup(&mut authority, &leaf.request_id, now);
    let prepared = authority
        .prepare_terminal_tab_close_publication(&leaf.request_id, now)
        .unwrap();
    let leaf_revision = prepared.revision();
    let _ = prepared.commit();
    assert_eq!(authority.projection.snapshot().revision, leaf_revision);
    assert_eq!(
        authority.tree_standalone_membership().unwrap().members(),
        std::slice::from_ref(&source)
    );
    // The completed leaf record shares the Tab address but its captured member
    // is already removed; it cannot fence the surviving Tab's whole-Tab Close.
    let membership = authority.tree_standalone_membership().unwrap();
    let whole = command(
        &authority,
        20062,
        WorkbenchAction::TabClose {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: source.tab_id().clone(),
        },
    );
    let accepted = accept(&mut authority, &grant, whole.clone(), now);
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    dispose_and_cleanup(&mut authority, &whole.request_id, now);
    let prepared = authority
        .prepare_terminal_tab_close_publication(&whole.request_id, now)
        .unwrap();
    assert_eq!(prepared.exact_removed_members(), &[source]);
    let closed_revision = prepared.revision();
    let _ = prepared.commit();
    assert_eq!(closed_revision.value(), leaf_revision.value() + 1);
    let snapshot = authority.projection.snapshot();
    assert!(snapshot.terminal_contexts[0].tabs.is_empty());
    assert!(authority.tree_standalone_membership().unwrap().is_empty());
    assert!(matches!(
        authority.terminal_tab_close_status(&whole.request_id),
        Some(WorkbenchTabCloseStatus::Closed { revision }) if revision == closed_revision
    ));
    // The address-scoped read may return either retained tombstone for this
    // Tab; remaining available proves the authority and read surface survived.
    assert!(matches!(
        status(&authority, &grant, &whole),
        WorkbenchTabCloseReadOutcome::Available { .. }
    ));
    assert!(!authority.retired);
}

#[test]
fn completed_close_still_covering_a_projected_member_retires_the_authority() {
    let (mut authority, grant) = fixture(1);
    let (source, created) = split_fixture(&mut authority, &grant, 20070);
    let now = Instant::now();
    let leaf = panel_close_command(&authority, 20071, &created);
    accept(&mut authority, &grant, leaf.clone(), now);
    // Fabricate the invariant violation directly: a completed record whose
    // captured member is still projected cannot coexist with a clean Close.
    let revision = authority.projection.snapshot().revision;
    authority
        .close_record_mut(&leaf.request_id)
        .unwrap()
        .commit_closed(WorkbenchRevision::new(revision.value() + 1));
    let membership = authority.tree_standalone_membership().unwrap();
    let whole = command(
        &authority,
        20072,
        WorkbenchAction::TabClose {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: source.tab_id().clone(),
        },
    );
    let refused = resolve(&mut authority, &grant, whole);
    assert!(matches!(
        refused.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
        }
    ));
    assert!(authority.retired);
}

#[test]
fn panel_close_read_and_resume_reuse_the_close_control_surface() {
    let (mut authority, grant) = fixture(1);
    let (_source, created) = split_fixture(&mut authority, &grant, 20050);
    let now = Instant::now();
    let close = panel_close_command(&authority, 20051, &created);
    accept(&mut authority, &grant, close.clone(), now);
    // Reads address the containing Tab and observe the panel-scoped record.
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available {
            close_request_id,
            ..
        } if close_request_id == close.request_id
    ));
    // A resume while lifecycle work has nothing resumable is rejected without admission.
    let WorkbenchAction::PanelClose {
        terminal_context_id,
        ..
    } = &close.action
    else {
        panic!()
    };
    let resume = WorkbenchTabCloseResume {
        protocol_version: ProtocolVersion,
        request_id: request_id(99001),
        site_id: close.site_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_id: created.tab_id().clone(),
        close_request_id: close.request_id.clone(),
        expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
    };
    let WorkbenchTabCloseResumeBegin::Ready(resumed) =
        authority.begin_terminal_tab_close_resume(Some(&grant), resume)
    else {
        panic!("non-resumable resume must not admit")
    };
    assert!(matches!(
        resumed.outcome,
        WorkbenchTabCloseResumeOutcome::Rejected {
            reason: WorkbenchTabCloseResumeRejectionReason::NotResumable
        }
    ));
}
