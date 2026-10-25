use super::*;
use crate::test_support::WorkbenchFixture;

#[test]
fn every_ordinary_writer_preserves_reserved_removal_credit_and_noops_still_settle() {
    for operation in 0..7 {
        let (mut authority, grant) = fixture(2);
        let project = command(
            &authority,
            9000,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("project").unwrap(),
            },
        );
        let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } =
            submit(&mut authority, &grant, project).outcome
        else {
            panic!()
        };
        let workspace = command(
            &authority,
            9001,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id.clone()),
                label: WorkbenchLabel::parse("workspace").unwrap(),
            },
        );
        let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } =
            submit(&mut authority, &grant, workspace).outcome
        else {
            panic!()
        };
        set_revision(&mut authority, u64::MAX - 1);
        let now = Instant::now();
        let close = close_command(&authority, 10000, 0);
        accept(&mut authority, &grant, close.clone(), now);
        let member = authority.fixture_membership().unwrap().members()[0].clone();
        let context = authority
            .fixture_membership()
            .unwrap()
            .terminal_context_id()
            .clone();
        let action = match operation {
            0 => WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("new").unwrap(),
            },
            1 => WorkbenchAction::ProjectRename {
                project_id,
                label: WorkbenchLabel::parse("changed").unwrap(),
            },
            2 => WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id),
                label: WorkbenchLabel::parse("new").unwrap(),
            },
            3 => WorkbenchAction::WorkspaceRename {
                workspace_id,
                label: WorkbenchLabel::parse("changed").unwrap(),
            },
            4 => WorkbenchAction::TabRename {
                terminal_context_id: context.clone(),
                tab_id: member.tab_id().clone(),
                label: WorkbenchLabel::parse("changed").unwrap(),
            },
            5 => WorkbenchAction::TabMove {
                terminal_context_id: context.clone(),
                tab_id: member.tab_id().clone(),
                direction: WorkbenchTabMoveDirection::Last,
            },
            _ => WorkbenchAction::TabCreate {
                placement: None,
                automatic_label: false,
                terminal_context_id: context.clone(),
                label: WorkbenchLabel::parse("new").unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        };
        let request = command(&authority, 10001, action);
        let response = match operation {
            5 => {
                let WorkbenchTabMoveBegin::Admitted(admission) =
                    authority.begin_terminal_tab_move(Some(&grant), request)
                else {
                    panic!()
                };
                let WorkbenchTabMovePreparation::Prepared(prepared) = admission.finish() else {
                    panic!()
                };
                prepared.commit()
            }
            6 => {
                let WorkbenchTabCreateBegin::Ready(response) =
                    authority.begin_terminal_tab_create(Some(&grant), request)
                else {
                    panic!()
                };
                response
            }
            _ => submit(&mut authority, &grant, request),
        };
        assert!(
            matches!(
                response.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::RevisionExhausted,
                    ..
                }
            ),
            "writer {operation}"
        );
        let equal_automatic = command(
            &authority,
            10002,
            WorkbenchAction::TabRename {
                terminal_context_id: context.clone(),
                tab_id: member.tab_id().clone(),
                label: WorkbenchLabel::parse("1").unwrap(),
            },
        );
        assert!(matches!(
            submit(&mut authority, &grant, equal_automatic).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RevisionExhausted,
                ..
            }
        ));
        let noop = command(
            &authority,
            10003,
            WorkbenchAction::TabRename {
                terminal_context_id: context,
                tab_id: authority.fixture_membership().unwrap().members()[1]
                    .tab_id()
                    .clone(),
                label: WorkbenchLabel::parse("tab 1").unwrap(),
            },
        );
        assert!(matches!(
            submit(&mut authority, &grant, noop).outcome,
            WorkbenchCommandOutcome::Unchanged { .. }
        ));
        dispose_and_cleanup(&mut authority, &close.request_id, now);
        let closed = authority
            .prepare_terminal_tab_close_publication(&close.request_id, now)
            .unwrap()
            .commit();
        assert_eq!(
            closed,
            WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(u64::MAX)
            }
        );
        assert_eq!(authority.outstanding_close_credits().ok(), Some(0));
    }
}

#[test]
fn bootstrap_and_primary_reject_exhaustion_and_two_credits_are_consumed_separately() {
    let (mut empty, grant) = fixture(0);
    set_revision(&mut empty, u64::MAX);

    assert!(matches!(
        empty.begin_terminal_tab_create(Some(&grant), create_command(&empty, 10, "new")),
        WorkbenchTabCreateBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RevisionExhausted,
                ..
            },
            ..
        })
    ));
    let (mut authority, grant) = fixture(2);
    let now = Instant::now();
    set_revision(&mut authority, u64::MAX);
    let close = close_command(&authority, 10000, 0);
    let primary = primary(&mut authority, &grant, close.clone());
    let runtime = runtime(primary.member());
    assert!(matches!(
        primary.finish(runtime, None, now),
        WorkbenchTabClosePreparation::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RevisionExhausted,
                ..
            },
            ..
        })
    ));
    set_revision(&mut authority, u64::MAX - 2);
    let first = close_command(&authority, 10001, 0);
    let second = close_command(&authority, 10002, 1);
    accept(&mut authority, &grant, first.clone(), now);
    accept(&mut authority, &grant, second.clone(), now);
    for (request, revision, credits) in [(first, u64::MAX - 1, 1), (second, u64::MAX, 0)] {
        dispose_and_cleanup(&mut authority, &request.request_id, now);
        assert_eq!(
            authority
                .prepare_terminal_tab_close_publication(&request.request_id, now)
                .unwrap()
                .commit()
                .metadata()
                .clone(),
            WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(revision)
            }
        );
        assert_eq!(authority.outstanding_close_credits().ok(), Some(credits));
    }
}
