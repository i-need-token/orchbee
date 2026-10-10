use super::*;
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, PublicationReadIntent, SnapshotReadIntent, TerminalId,
    WorkbenchLabel, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkerId,
};

mod launch_profile;
mod layout_create;
mod panel_input;
mod panel_labels;
mod split_ratios;
mod stateful;
mod tab_configuration;
mod workspace_directory;
mod workspace_group_close;
mod workspace_order;
mod workspace_organization;
mod worktree_operation;

fn authority() -> (WorkbenchAuthority, WorkbenchGrant) {
    let site = SiteId::parse(&identity(100)).unwrap();
    (
        WorkbenchAuthority::new(site.clone()).unwrap(),
        WorkbenchGrant::manage_navigation(site),
    )
}
fn command(id: u128, revision: u64, action: WorkbenchAction) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(id)).unwrap(),
        site_id: SiteId::parse(&identity(100)).unwrap(),
        expected_revision: WorkbenchRevision::new(revision),
        action,
    }
}
fn create(label: &str) -> WorkbenchAction {
    WorkbenchAction::ProjectCreate {
        label: WorkbenchLabel::parse(label).unwrap(),
    }
}
fn submit(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchCommandResult {
    let WorkbenchResponse::Command(result) = authority.execute(
        Some(grant),
        WorkbenchRequest::Command(command).try_into().unwrap(),
    ) else {
        panic!("expected command result")
    };
    result
}
fn snapshot(authority: &mut WorkbenchAuthority, grant: &WorkbenchGrant) -> Snapshot {
    let response = authority.execute(Some(grant), read().try_into().unwrap());
    let WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
        outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        ..
    }) = response
    else {
        panic!("expected snapshot")
    };
    snapshot
}
fn read() -> WorkbenchRequest {
    WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(999)).unwrap(),
    })
}
fn publication_read(id: u128, known_revision: u64) -> WorkbenchRequest {
    WorkbenchRequest::PublicationRead(PublicationReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(id)).unwrap(),
        known_revision: WorkbenchRevision::new(known_revision),
    })
}
fn publication_outcome(
    authority: &mut WorkbenchAuthority,
    grant: Option<&WorkbenchGrant>,
    request: WorkbenchRequest,
) -> WorkbenchPublicationOutcome {
    let WorkbenchResponse::Publication(WorkbenchPublicationResult { outcome, .. }) =
        authority.execute(grant, request.try_into().unwrap())
    else {
        panic!("expected publication result")
    };
    outcome
}
fn project_id(result: &WorkbenchCommandResult) -> WorkbenchProjectId {
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = &result.outcome else {
        panic!("expected project")
    };
    project_id.clone()
}

fn binding(value: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(value)).unwrap(),
        worker_id: WorkerId::parse(&identity(value + 1_000)).unwrap(),
    }
}

fn install_two_tabs(
    authority: &mut WorkbenchAuthority,
) -> (
    StandaloneIntentMetadata,
    CommittedWorkbenchTabCreateMetadata,
    PendingWorkbenchTabCreate,
) {
    let first = authority
        .prepare_standalone_intent(binding(700))
        .unwrap()
        .commit();
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap();
    let create = command(
        900,
        first.revision().value(),
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: first.terminal_context_id().clone(),
            label: WorkbenchLabel::parse("\0duplicate\u{2028}\u{2029}\n").unwrap(),
            geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    let terminal_grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone());
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&terminal_grant), create)
    else {
        panic!("expected tab create admission")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(701)) else {
        panic!("expected prepared tab create")
    };
    let committed = prepared.commit();
    let second = committed.metadata().clone();
    (first, second, committed.into_pending())
}

fn tab_rename(
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    label: &str,
) -> WorkbenchAction {
    WorkbenchAction::TabRename {
        terminal_context_id,
        tab_id,
        label: WorkbenchLabel::parse(label).unwrap(),
    }
}

#[test]
fn exact_replay_preserves_original_result_after_other_commits_and_conflicts_do_not_replace_it() {
    let (mut authority, grant) = authority();
    let first = command(1, 0, create(" First "));
    let settled = submit(&mut authority, &grant, first.clone());
    submit(&mut authority, &grant, command(2, 1, create("Second")));
    assert_eq!(submit(&mut authority, &grant, first.clone()), settled);
    let conflict = submit(&mut authority, &grant, command(1, 2, create("Changed")));
    assert_eq!(
        conflict.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(2),
            reason: WorkbenchRejectionReason::RequestIdentityConflict
        }
    );
    assert_eq!(submit(&mut authority, &grant, first), settled);
    assert_eq!(
        snapshot(&mut authority, &grant).navigation.projects.len(),
        2
    );
}

#[test]
fn authorization_precedes_ledger_revision_and_retirement_disclosure() {
    let (mut authority, grant) = authority();
    let first = command(1, 0, create("Private"));
    let accepted = submit(&mut authority, &grant, first.clone());
    let reader = WorkbenchGrant::read_only(grant.site_id.clone());
    let wrong_site = WorkbenchGrant::manage_navigation(SiteId::parse(&identity(200)).unwrap());
    for denied in [None, Some(&reader), Some(&wrong_site)] {
        let response = authority.execute(
            denied,
            WorkbenchRequest::Command(first.clone()).try_into().unwrap(),
        );
        assert_eq!(
            response,
            WorkbenchResponse::Command(result(&first, WorkbenchCommandOutcome::Denied {}))
        );
    }
    assert_eq!(authority.ledger.len(), 1);
    assert_eq!(submit(&mut authority, &grant, first.clone()), accepted);
    assert_eq!(
        snapshot(&mut authority, &reader).navigation.projects.len(),
        1
    );
    let response = authority.execute(Some(&wrong_site), read().try_into().unwrap());
    assert!(matches!(
        response,
        WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            outcome: WorkbenchSnapshotOutcome::Denied {},
            ..
        })
    ));
    authority.retire(&first);
    assert!(matches!(
        submit(&mut authority, &grant, first).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable
        }
    ));
    assert!(matches!(
        authority.execute(Some(&reader), read().try_into().unwrap()),
        WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            outcome: WorkbenchSnapshotOutcome::Unavailable { .. },
            ..
        })
    ));
    assert!(matches!(
        authority.execute(None, read().try_into().unwrap()),
        WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            outcome: WorkbenchSnapshotOutcome::Denied {},
            ..
        })
    ));
    assert_eq!(authority.ledger.len(), 1);
}

#[test]
fn workspace_creation_and_rename_keep_the_exact_parent_and_one_context() {
    let (mut authority, grant) = authority();
    let project = project_id(&submit(&mut authority, &grant, command(1, 0, create("P"))));
    let _second = project_id(&submit(
        &mut authority,
        &grant,
        command(2, 1, create("Other")),
    ));
    let outcome = submit(
        &mut authority,
        &grant,
        command(
            3,
            2,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project.clone()),
                label: WorkbenchLabel::parse("W").unwrap(),
            },
        ),
    )
    .outcome;
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id,
        ..
    } = outcome
    else {
        panic!("expected workspace")
    };
    let before = snapshot(&mut authority, &grant);
    assert_eq!(before.terminal_contexts.len(), 2);
    let context = before
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == terminal_context_id)
        .unwrap();
    assert!(context.tabs.is_empty() && context.groups.is_empty());
    let rename = |workspace_id| WorkbenchAction::WorkspaceRename {
        workspace_id,
        label: WorkbenchLabel::parse("Renamed").unwrap(),
    };
    assert!(matches!(
        submit(
            &mut authority,
            &grant,
            command(
                4,
                3,
                rename(WorkbenchWorkspaceId::parse(&identity(999_998)).unwrap())
            )
        )
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::WorkspaceUnavailable,
            ..
        }
    ));
    assert_eq!(snapshot(&mut authority, &grant), before);
    assert!(matches!(
        submit(
            &mut authority,
            &grant,
            command(5, 3, rename(workspace_id.clone()))
        )
        .outcome,
        WorkbenchCommandOutcome::Renamed { .. }
    ));
    let after = snapshot(&mut authority, &grant);
    assert_eq!(after.navigation.workspaces[0].label, "Renamed");
    assert_eq!(after.terminal_contexts, before.terminal_contexts);
}

#[test]
fn tab_rename_preserves_every_non_target_fact_and_replays_its_historical_revision() {
    let (mut authority, grant) = authority();
    let (first, second, pending) = install_two_tabs(&mut authority);
    let before = snapshot(&mut authority, &grant);
    assert_eq!(before.revision, WorkbenchRevision::new(2));
    let old_cursor = authority.last_identity;
    let context = first.terminal_context_id().clone();
    let duplicate_control_label = before.terminal_contexts[0].tabs[1].label.clone();
    let first_rename = command(
        1,
        2,
        tab_rename(
            context.clone(),
            first.tab_id().clone(),
            &duplicate_control_label,
        ),
    );

    let first_result = submit(&mut authority, &grant, first_rename.clone());
    assert_eq!(
        first_result.outcome,
        WorkbenchCommandOutcome::Renamed {
            revision: WorkbenchRevision::new(3)
        }
    );
    let after_first = snapshot(&mut authority, &grant);
    let mut expected = before.clone();
    expected.revision = WorkbenchRevision::new(3);
    expected.terminal_contexts[0].tabs[0].label = duplicate_control_label.clone();
    expected.terminal_contexts[0].tabs[0].automatic_label = false;
    assert_eq!(after_first, expected);
    assert_eq!(
        after_first.terminal_contexts[0].tabs[0].label,
        duplicate_control_label
    );
    assert_eq!(
        after_first.terminal_contexts[0].tabs[0].label,
        after_first.terminal_contexts[0].tabs[1].label
    );
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &after_first.terminal_contexts[0].tabs[0].root
    else {
        panic!("expected simple first tab")
    };
    assert_eq!(primary_presentation.label, "1");
    assert_eq!(primary_presentation.binding.as_ref(), Some(first.binding()));
    assert_eq!(authority.last_identity, old_cursor);

    let maximum_label = "\u{1f41d}".repeat(512);
    let second_result = submit(
        &mut authority,
        &grant,
        command(
            2,
            3,
            tab_rename(context, second.tab_id().clone(), &maximum_label),
        ),
    );
    assert_eq!(
        second_result.outcome,
        WorkbenchCommandOutcome::Renamed {
            revision: WorkbenchRevision::new(4)
        }
    );
    let after_second = snapshot(&mut authority, &grant);
    assert_eq!(
        after_second.terminal_contexts[0].tabs[0].label,
        duplicate_control_label
    );
    assert_eq!(
        after_second.terminal_contexts[0].tabs[1].label,
        maximum_label
    );
    assert_eq!(authority.last_identity, old_cursor);

    assert_eq!(submit(&mut authority, &grant, first_rename), first_result);
    assert_eq!(snapshot(&mut authority, &grant), after_second);

    let unchanged = submit(
        &mut authority,
        &grant,
        command(
            3,
            4,
            tab_rename(
                first.terminal_context_id().clone(),
                first.tab_id().clone(),
                &duplicate_control_label,
            ),
        ),
    );
    assert_eq!(
        unchanged.outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(4)
        }
    );
    assert_eq!(snapshot(&mut authority, &grant), after_second);
    assert_eq!(authority.last_identity, old_cursor);

    let settlement = authority
        .prepare_terminal_tab_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: second.geometry(),
            },
        )
        .unwrap();
    assert!(matches!(
        settlement.commit().outcome,
        WorkbenchCommandOutcome::TabCreated {
            revision,
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        } if revision == WorkbenchRevision::new(2)
    ));
    assert_eq!(snapshot(&mut authority, &grant), after_second);
}

#[test]
fn tab_rename_resolves_context_before_tab_and_rejections_are_atomic() {
    let (mut authority, grant) = authority();
    let first = authority
        .prepare_standalone_intent(binding(800))
        .unwrap()
        .commit();
    let project = project_id(&submit(&mut authority, &grant, command(1, 1, create("P"))));
    let workspace = submit(
        &mut authority,
        &grant,
        command(
            2,
            2,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project),
                label: WorkbenchLabel::parse("W").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated {
        terminal_context_id: empty_context,
        ..
    } = workspace.outcome
    else {
        panic!("expected workspace")
    };
    let before = snapshot(&mut authority, &grant);
    let old_cursor = authority.last_identity;
    let old_ledger = authority.ledger.len();

    let missing_context = WorkbenchTerminalContextId::parse(&identity(9_999)).unwrap();
    let first_rejection = submit(
        &mut authority,
        &grant,
        command(
            3,
            3,
            tab_rename(missing_context, first.tab_id().clone(), "missing context"),
        ),
    );
    assert_eq!(
        first_rejection.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(3),
            reason: WorkbenchRejectionReason::TerminalContextUnavailable,
        }
    );
    let second_rejection = submit(
        &mut authority,
        &grant,
        command(
            4,
            3,
            tab_rename(empty_context, first.tab_id().clone(), "missing tab"),
        ),
    );
    assert_eq!(
        second_rejection.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(3),
            reason: WorkbenchRejectionReason::TabUnavailable,
        }
    );
    assert_eq!(snapshot(&mut authority, &grant), before);
    assert_eq!(authority.last_identity, old_cursor);
    assert_eq!(authority.ledger.len(), old_ledger + 2);
}

#[test]
fn navigation_grant_can_rename_a_tab_without_terminal_grant_or_partial_ledger_effects() {
    let (mut authority, grant) = authority();
    let first = authority
        .prepare_standalone_intent(binding(900))
        .unwrap()
        .commit();
    let request = command(
        1,
        1,
        tab_rename(
            first.terminal_context_id().clone(),
            first.tab_id().clone(),
            "renamed",
        ),
    );
    let reader = WorkbenchGrant::read_only(grant.site_id.clone());
    let before = snapshot(&mut authority, &grant);
    assert_eq!(
        authority.execute(
            Some(&reader),
            WorkbenchRequest::Command(request.clone())
                .try_into()
                .unwrap(),
        ),
        WorkbenchResponse::Command(result(&request, WorkbenchCommandOutcome::Denied {}))
    );
    assert_eq!(snapshot(&mut authority, &grant), before);
    assert!(authority.ledger.is_empty());

    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    assert_eq!(
        submit(&mut authority, &grant, request.clone()).outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(snapshot(&mut authority, &grant), before);
    assert!(authority.ledger.is_empty());
    assert!(!authority.retired);

    authority.ledger_bytes = 0;
    assert_eq!(
        submit(&mut authority, &grant, request).outcome,
        WorkbenchCommandOutcome::Renamed {
            revision: WorkbenchRevision::new(2)
        }
    );
    let after = snapshot(&mut authority, &grant);
    assert_eq!(after.terminal_contexts[0].tabs[0].label, "renamed");
}

#[test]
fn identity_and_revision_exhaustion_are_atomic_and_equal_rename_is_still_unchanged() {
    let (mut authority, grant) = authority();
    let project = project_id(&submit(&mut authority, &grant, command(1, 0, create("P"))));
    authority.last_identity = u128::MAX - 1;
    let before = snapshot(&mut authority, &grant);
    let workspace = WorkbenchAction::WorkspaceCreate {
        project_id: Some(project.clone()),
        label: WorkbenchLabel::parse("W").unwrap(),
    };
    assert!(matches!(
        submit(&mut authority, &grant, command(2, 1, workspace)).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::IdentityExhausted,
            ..
        }
    ));
    assert_eq!(authority.last_identity, u128::MAX - 1);
    assert_eq!(snapshot(&mut authority, &grant), before);
    let mut exhausted = before;
    exhausted.revision = WorkbenchRevision::new(u64::MAX);
    let bytes =
        serialize_document(&WorkbenchProtocolDocument::Snapshot(exhausted.clone())).unwrap();
    authority.projection.advance_projection(&bytes).unwrap();
    let rename = |label| WorkbenchAction::ProjectRename {
        project_id: project.clone(),
        label: WorkbenchLabel::parse(label).unwrap(),
    };
    assert_eq!(
        submit(&mut authority, &grant, command(3, u64::MAX, rename("P"))).outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: WorkbenchRevision::new(u64::MAX)
        }
    );
    assert!(matches!(
        submit(&mut authority, &grant, command(4, u64::MAX, rename("Q"))).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionExhausted,
            ..
        }
    ));
    assert_eq!(snapshot(&mut authority, &grant), exhausted);
}

#[test]
fn ledger_count_rejects_new_requests_without_eviction_and_retained_replay_survives() {
    let (mut authority, grant) = authority();
    let first = command(1, 0, create("P"));
    let accepted = submit(&mut authority, &grant, first.clone());
    for id in 2..=u128::try_from(MAXIMUM_LEDGER_ENTRIES).unwrap() {
        let rejected = submit(&mut authority, &grant, command(id, 0, create("Stale")));
        assert!(matches!(
            rejected.outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RevisionMismatch,
                ..
            }
        ));
    }
    let before = snapshot(&mut authority, &grant);
    let full = submit(&mut authority, &grant, command(10000, 0, create("P")));
    assert_eq!(
        full.outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(submit(&mut authority, &grant, first), accepted);
    assert_eq!(snapshot(&mut authority, &grant), before);
}

#[test]
fn ledger_bytes_include_both_canonical_documents_and_stop_before_count_capacity() {
    let (mut authority, grant) = authority();
    let label = "\u{1f41d}".repeat(512);
    for id in 1..=u128::try_from(MAXIMUM_LEDGER_ENTRIES).unwrap() {
        let request = command(id, 0, create(&label));
        let response = submit(&mut authority, &grant, request.clone());
        if matches!(
            response.outcome,
            WorkbenchCommandOutcome::Unavailable { .. }
        ) {
            assert!(authority.ledger.len() < MAXIMUM_LEDGER_ENTRIES);
            let prospective = result(
                &request,
                authority.rejected(WorkbenchRejectionReason::RevisionMismatch),
            );
            assert!(
                authority.ledger_bytes + ledger_size(&request, &prospective).unwrap()
                    > MAXIMUM_LEDGER_BYTES
            );
            let actual: usize = authority
                .ledger
                .values()
                .map(|entry| ledger_size(entry.command(), entry.replay_result()).unwrap())
                .sum();
            assert_eq!(actual, authority.ledger_bytes);
            assert_eq!(snapshot(&mut authority, &grant).revision.value(), 1);
            return;
        }
    }
    panic!("expected byte exhaustion before count exhaustion");
}

#[test]
fn prepared_publication_is_discarded_when_ledger_byte_reservation_fails() {
    let (mut authority, grant) = authority();
    let request = command(1, 0, create("Prepared"));
    let before = snapshot(&mut authority, &grant);
    let old_identity = authority.last_identity;
    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;

    assert_eq!(
        submit(&mut authority, &grant, request.clone()).outcome,
        unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull)
    );
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, old_identity);
    assert!(authority.ledger.is_empty());
    assert_eq!(authority.ledger_bytes, MAXIMUM_LEDGER_BYTES);
    assert!(!authority.retired);

    authority.ledger_bytes = 0;
    assert!(matches!(
        submit(&mut authority, &grant, request).outcome,
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));
}

#[test]
fn publication_preflight_failure_precedes_ledger_byte_refusal_atomically() {
    let (mut authority, grant) = authority();
    submit(&mut authority, &grant, command(1, 0, create("Existing")));
    let before = snapshot(&mut authority, &grant);
    // Fault injection: a broken allocator would reuse the existing Project identity.
    authority.last_identity = 1;
    authority.ledger_bytes = MAXIMUM_LEDGER_BYTES;
    let response = submit(
        &mut authority,
        &grant,
        command(2, 1, create("Duplicate identity")),
    );
    assert_eq!(
        response.outcome,
        unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
    );
    assert!(authority.retired);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.ledger.len(), 1);
    assert_eq!(authority.ledger_bytes, MAXIMUM_LEDGER_BYTES);
    assert_eq!(authority.last_identity, 1);
}

#[test]
fn publication_read_replays_committed_history_under_the_read_grant() {
    let (mut authority, grant) = authority();
    let reader = WorkbenchGrant::read_only(grant.site_id.clone());

    assert_eq!(
        publication_outcome(&mut authority, Some(&reader), publication_read(800, 0)),
        WorkbenchPublicationOutcome::Current {
            revision: WorkbenchRevision::new(0)
        }
    );

    submit(&mut authority, &grant, command(1, 0, create("First")));
    submit(&mut authority, &grant, command(2, 1, create("Second")));

    let WorkbenchPublicationOutcome::Published { publications } =
        publication_outcome(&mut authority, Some(&reader), publication_read(801, 0))
    else {
        panic!("expected published publications")
    };
    assert_eq!(publications.len(), 2);
    let mut replayed = snapshot(&mut authority, &grant);
    // Rewind the reader baseline to revision 0 by re-reading the initial state shape.
    let base = Snapshot {
        revision: WorkbenchRevision::new(0),
        navigation: Navigation {
            directories: Vec::new(),
            workspaces: Vec::new(),
            workspace_order: Vec::new(),
            projects: Vec::new(),
            standalone_terminal_context_id: replayed
                .navigation
                .standalone_terminal_context_id
                .clone(),
        },
        terminal_contexts: replayed
            .terminal_contexts
            .iter()
            .cloned()
            .map(|mut context| {
                context.tabs.clear();
                context.groups.clear();
                context
            })
            .collect::<Vec<_>>(),
        ..replayed.clone()
    };
    replayed = base;
    for publication in &publications {
        let WorkbenchPublicationDocument::Delta(delta) = publication else {
            panic!("expected delta publications")
        };
        replayed = orchbee_workbench_protocol::apply_workbench_delta(&replayed, delta).unwrap();
    }
    assert_eq!(replayed, snapshot(&mut authority, &grant));

    let WorkbenchPublicationOutcome::Published { publications: tail } =
        publication_outcome(&mut authority, Some(&reader), publication_read(802, 1))
    else {
        panic!("expected tail publication")
    };
    assert_eq!(tail.len(), 1);
    assert_eq!(
        publication_outcome(&mut authority, Some(&reader), publication_read(803, 2)),
        WorkbenchPublicationOutcome::Current {
            revision: WorkbenchRevision::new(2)
        }
    );
}

#[test]
fn publication_read_denies_before_disclosure_and_skips_the_command_ledger() {
    let (mut authority, grant) = authority();
    submit(&mut authority, &grant, command(1, 0, create("Private")));
    let reader = WorkbenchGrant::read_only(grant.site_id.clone());
    let wrong_site = WorkbenchGrant::manage_navigation(SiteId::parse(&identity(200)).unwrap());

    for denied in [None, Some(&wrong_site)] {
        assert_eq!(
            publication_outcome(&mut authority, denied, publication_read(810, 0)),
            WorkbenchPublicationOutcome::Denied {}
        );
    }
    // A read-only grant may observe publications but a wrong Site still learns nothing.
    assert!(matches!(
        publication_outcome(&mut authority, Some(&reader), publication_read(811, 1)),
        WorkbenchPublicationOutcome::Current { .. } | WorkbenchPublicationOutcome::Published { .. }
    ));
    // Publication reads never enter the command ledger, so a later command may reuse the id.
    assert!(matches!(
        submit(&mut authority, &grant, command(810, 1, create("After"))).outcome,
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));

    let first = command(900, 2, create("Unused"));
    authority.retire(&first);
    assert_eq!(
        publication_outcome(&mut authority, Some(&reader), publication_read(812, 0)),
        WorkbenchPublicationOutcome::Unavailable {
            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable
        }
    );
    assert_eq!(
        publication_outcome(&mut authority, None, publication_read(813, 0)),
        WorkbenchPublicationOutcome::Denied {}
    );
}
