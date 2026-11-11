use super::{authority, command, create, identity, project_id, submit};
use crate::{
    WorkbenchAuthority, WorkbenchGrant, WorkbenchWorkspaceCloseBegin, WorktreeOperationPlan,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchDirectoryPath,
    WorkbenchLabel, WorkbenchProjectId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseStatus, WorkbenchTerminalGeometry,
    WorkbenchWorkspaceGroupCloseRequest, WorkbenchWorkspaceId, WorkerId,
    WorkspaceGroupCloseAction as Action, WorkspaceGroupCloseChild,
    WorkspaceGroupCloseFailure as Failure, WorkspaceGroupCloseObservation,
    WorkspaceGroupCloseOutcome as Outcome, WorkspaceGroupCloseStatus as Status,
    WorkspaceGroupCloseUnavailable as Unavailable, WorktreeCheckoutFacts, WorktreeGitEffect,
    WorktreeIntent, WorktreeOperationPhase, WorktreeProgress, WorktreeSource,
    WorktreeWorkspaceDisposition,
};
use std::num::NonZeroU64;
fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&identity(value)).unwrap()
}
fn worker() -> (WorkerId, NonZeroU64) {
    (WorkerId::parse(&identity(20)).unwrap(), NonZeroU64::MIN)
}
struct Fixture {
    authority: WorkbenchAuthority,
    grant: WorkbenchGrant,
    project: WorkbenchProjectId,
    main: WorkbenchWorkspaceId,
    linked: WorkbenchWorkspaceId,
    independent: WorkbenchWorkspaceId,
    other_project: WorkbenchProjectId,
    excluded: WorkbenchWorkspaceId,
}
impl Fixture {
    fn new() -> Self {
        let (mut authority, navigation) = authority();
        let project = project_id(&submit(
            &mut authority,
            &navigation,
            command(10, 0, create("repo")),
        ));
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone())
                .with_directory_association()
                .with_terminal_lifecycle();
        let main = workspace(&mut authority, &grant, &project, 11, "C:/repo/subdir");
        let linked = workspace(&mut authority, &grant, &project, 13, "C:/linked");
        let independent = workspace(
            &mut authority,
            &grant,
            &project,
            15,
            "D:/independent-repository",
        );
        let revision = authority.snapshot().revision.value();
        let other_project = project_id(&submit(
            &mut authority,
            &navigation,
            command(17, revision, create("other group")),
        ));
        let excluded = workspace(&mut authority, &grant, &other_project, 18, "C:/repo/peer");
        let revision = authority.snapshot().revision.value();
        submit(
            &mut authority,
            &grant,
            command(
                20,
                revision,
                WorkbenchAction::WorkspaceMoveBlock {
                    workspace_ids: orchbee_workbench_protocol::WorkbenchWorkspaceList::new(vec![
                        linked.clone(),
                        independent.clone(),
                        main.clone(),
                        excluded.clone(),
                    ])
                    .unwrap(),
                    before_workspace_id: None,
                },
            ),
        );
        let mut fixture = Self {
            authority,
            grant,
            project,
            main,
            linked,
            independent,
            other_project,
            excluded,
        };
        fixture.associate(fixture.main.clone(), "C:/repo");
        fixture.associate(fixture.linked.clone(), "C:/linked");
        fixture.associate(fixture.excluded.clone(), "C:/repo/peer");
        fixture
    }
    fn associate(&mut self, workspace_id: WorkbenchWorkspaceId, path: &str) {
        let source = self
            .authority
            .workspace_directory_target(&workspace_id)
            .unwrap();
        let plan = WorktreeOperationPlan {
            operation_id: id(40),
            removal_complete: false,
            project_id: None,
            intent: WorktreeIntent::Create {
                source: WorktreeSource::Workspace {
                    workspace_id: workspace_id.clone(),
                },
                branch: None,
                base: None,
                path: None,
                label: None,
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                trust_repository: false,
                agent_start: None,
            },
            source,
            source_workspace_id: Some(workspace_id),
            worker_generation: worker().1,
            attempt: 1,
            phase: WorktreeOperationPhase::ObservingSource,
            progress: WorktreeProgress {
                path: None,
                checkout: None,
                workspace: WorktreeWorkspaceDisposition::Unassociated {},
                git_effect: WorktreeGitEffect::None,
                restoration: None,
            },
        };
        assert!(self.authority.associate_worktree_source(
            &plan,
            WorktreeCheckoutFacts {
                repository_root: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
                common_directory: WorkbenchDirectoryPath::parse("C:/repo/.git").unwrap(),
                path: WorkbenchDirectoryPath::parse(path).unwrap(),
                branch: None,
                dirty: false
            }
        ));
    }
    fn apply(&mut self, action: Action) -> Outcome {
        self.authority
            .execute_workspace_group_close(
                Some(&self.grant),
                WorkbenchWorkspaceGroupCloseRequest {
                    protocol_version: ProtocolVersion,
                    request_id: id(900),
                    session_id: self.authority.session_id().clone(),
                    action,
                },
                Some(worker()),
                true,
            )
            .outcome
    }
    fn start(&self, operation: u128) -> Action {
        Action::Start {
            operation_id: id(operation),
            project_id: self.project.clone(),
        }
    }
}
fn workspace(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    project: &WorkbenchProjectId,
    value: u128,
    path: &str,
) -> WorkbenchWorkspaceId {
    let revision = authority.snapshot().revision.value();
    let result = submit(
        authority,
        grant,
        command(
            value,
            revision,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project.clone()),
                label: WorkbenchLabel::parse(path).unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = result.outcome else {
        panic!("create");
    };
    let result = authority.accept_workspace_directory(
        Some(grant),
        command(
            value + 1,
            revision + 1,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                workspace_id: workspace_id.clone(),
                path: WorkbenchDirectoryPath::parse(path).unwrap(),
            },
        ),
        worker().0,
    );
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    workspace_id
}
fn child(value: u128, status: WorkbenchTabCloseStatus) -> WorkspaceGroupCloseChild {
    WorkspaceGroupCloseChild {
        request_id: id(value),
        observation: Some(WorkspaceGroupCloseObservation {
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
            status,
        }),
    }
}
fn closed(value: u128) -> WorkspaceGroupCloseChild {
    child(
        value,
        WorkbenchTabCloseStatus::Closed {
            revision: WorkbenchRevision::new(50),
        },
    )
}

#[test]
fn project_selection_captures_display_order_across_repositories_and_excludes_same_repo_peer() {
    let mut fixture = Fixture::new();
    let start = fixture.start(100);
    let result = fixture.apply(start.clone());
    let Outcome::Available {
        members,
        status: Status::Pending {},
        ..
    } = &result
    else {
        panic!("group");
    };
    assert_eq!(
        members
            .iter()
            .map(|member| &member.workspace_id)
            .collect::<Vec<_>>(),
        vec![&fixture.linked, &fixture.independent, &fixture.main]
    );
    assert_eq!(fixture.apply(start), result);
    assert_eq!(
        fixture.apply(Action::Start {
            operation_id: id(100),
            project_id: fixture.other_project.clone()
        }),
        Outcome::Conflict {}
    );
    assert_eq!(
        fixture.apply(fixture.start(101)),
        Outcome::Unavailable {
            reason: Unavailable::Busy
        }
    );
    let plan = fixture.authority.claim_workspace_group_close().unwrap();
    assert!(fixture.authority.claim_workspace_group_close().is_none());
    assert!(fixture.authority.publish_workspace_group_close(
        &plan,
        Status::Failed {
            reason: Failure::Deadline,
            resumable: true
        }
    ));
    let Outcome::Available { members, .. } = fixture.apply(fixture.start(101)) else {
        panic!("linked");
    };
    assert_eq!(members.len(), 3);
    assert_eq!(members[0].workspace_id, fixture.linked);
}

#[test]
fn group_child_tokens_reject_unclaimed_stale_and_out_of_order_submissions() {
    let mut fixture = Fixture::new();
    fixture.apply(fixture.start(100));
    let mut plan = fixture.authority.claim_workspace_group_close().unwrap();
    let close = command(
        201,
        fixture.authority.snapshot().revision.value(),
        WorkbenchAction::WorkspaceClose {
            workspace_id: fixture.main.clone(),
        },
    );
    assert!(matches!(
        fixture.authority.begin_workspace_group_member_close(
            Some(&fixture.grant),
            close.clone(),
            &id(100),
            1,
            2
        ),
        WorkbenchWorkspaceCloseBegin::Ready(_)
    ));
    for index in 0..2 {
        plan.members[index].member.close = Some(closed(300 + index as u128));
        assert!(
            fixture
                .authority
                .publish_workspace_group_close(&plan, Status::Pending {})
        );
    }
    assert!(matches!(
        fixture.authority.begin_workspace_group_member_close(
            Some(&fixture.grant),
            close.clone(),
            &id(100),
            2,
            2
        ),
        WorkbenchWorkspaceCloseBegin::Ready(_)
    ));
    assert!(matches!(
        fixture.authority.begin_workspace_group_member_close(
            Some(&fixture.grant),
            close,
            &id(100),
            1,
            2
        ),
        WorkbenchWorkspaceCloseBegin::Admitted(_)
    ));
}

#[test]
fn partial_failure_resume_preserves_children_and_fences_prior_attempt_publication() {
    let mut fixture = Fixture::new();
    fixture.apply(fixture.start(100));
    let mut plan = fixture.authority.claim_workspace_group_close().unwrap();
    plan.members[0].member.close = Some(closed(300));
    assert!(
        !fixture
            .authority
            .publish_workspace_group_close(&plan, Status::Closed {})
    );
    assert!(fixture.authority.publish_workspace_group_close(
        &plan,
        Status::Failed {
            reason: Failure::Deadline,
            resumable: true
        }
    ));
    assert_eq!(
        fixture.apply(Action::Resume {
            operation_id: id(100),
            expected_attempt: 2
        }),
        Outcome::Unavailable {
            reason: Unavailable::StaleAttempt
        }
    );
    assert!(matches!(
        fixture.apply(Action::Resume {
            operation_id: id(100),
            expected_attempt: 1
        }),
        Outcome::Available { attempt: 2, .. }
    ));
    let mut next = fixture.authority.claim_workspace_group_close().unwrap();
    assert_eq!(next.members, plan.members);
    assert!(!fixture.authority.publish_workspace_group_close(
        &plan,
        Status::Failed {
            reason: Failure::Deadline,
            resumable: true
        }
    ));
    next.members[0].member.close = Some(closed(301));
    assert!(
        !fixture
            .authority
            .publish_workspace_group_close(&next, Status::Pending {})
    );
    next.members[0].member.close = Some(closed(300));
    next.members[1].member.close = Some(closed(301));
    assert!(
        fixture
            .authority
            .publish_workspace_group_close(&next, Status::Pending {})
    );
    next.members[2].member.close = Some(closed(302));
    assert!(
        fixture
            .authority
            .publish_workspace_group_close(&next, Status::Closed {})
    );
    assert!(
        !fixture
            .authority
            .publish_workspace_group_close(&next, Status::Pending {})
    );
    assert!(matches!(
        fixture.apply(Action::Read {
            operation_id: id(100)
        }),
        Outcome::Available {
            attempt: 2,
            status: Status::Closed {},
            ..
        }
    ));
}

#[test]
fn capacity_preserves_pure_reads_and_equal_start_observations() {
    let mut fixture = Fixture::new();
    for index in 0..64 {
        assert!(matches!(
            fixture.apply(fixture.start(100 + index)),
            Outcome::Available { .. }
        ));
        let mut plan = fixture.authority.claim_workspace_group_close().unwrap();
        for (member_index, member) in plan.members.iter_mut().enumerate() {
            member.member.close = Some(closed(300 + index * 3 + member_index as u128));
        }
        assert!(
            fixture
                .authority
                .publish_workspace_group_close(&plan, Status::Closed {})
        );
    }
    assert_eq!(
        fixture.apply(fixture.start(164)),
        Outcome::Unavailable {
            reason: Unavailable::Capacity
        }
    );
    assert!(matches!(
        fixture.apply(fixture.start(100)),
        Outcome::Available {
            status: Status::Closed {},
            ..
        }
    ));
    assert!(matches!(
        fixture.apply(Action::Read {
            operation_id: id(100)
        }),
        Outcome::Available {
            status: Status::Closed {},
            ..
        }
    ));
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Close(usize),
    Fail,
    Resume,
    Stale,
    ChangeChild,
    Finish,
    Read,
}
#[test]
fn generated_group_traces_preserve_closed_prefix_identity_and_attempts() {
    for seed in 1_u64..=64 {
        let mut random = seed;
        let steps = (0..64)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                match random % 7 {
                    0 => Step::Close((random / 7 % 3) as usize),
                    1 => Step::Fail,
                    2 => Step::Resume,
                    3 => Step::Stale,
                    4 => Step::ChangeChild,
                    5 => Step::Finish,
                    _ => Step::Read,
                }
            })
            .collect::<Vec<_>>();
        if let Err(failure) = trace(&steps) {
            let mut reduced = steps;
            let mut index = 0;
            while index < reduced.len() {
                let mut candidate = reduced.clone();
                candidate.remove(index);
                if trace(&candidate).is_err() {
                    reduced = candidate;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!("seed={seed}; failure={failure}; reduced={reduced:?}");
        }
    }
}
fn trace(steps: &[Step]) -> Result<(), String> {
    let mut fixture = Fixture::new();
    fixture.apply(fixture.start(100));
    let mut plan = fixture.authority.claim_workspace_group_close().unwrap();
    let mut prefix = 0;
    let mut pending = true;
    let mut finished = false;
    let mut attempt = 1;
    for (turn, step) in steps.iter().enumerate() {
        match *step {
            Step::Close(index) => {
                let mut candidate = plan.clone();
                candidate.members[index].member.close = Some(closed(300 + index as u128));
                let accepted = fixture
                    .authority
                    .publish_workspace_group_close(&candidate, Status::Pending {});
                let expected = pending && index <= prefix;
                if accepted != expected {
                    return Err(format!(
                        "turn {turn}: close {index}, prefix {prefix}, pending {pending}"
                    ));
                }
                if accepted {
                    plan = candidate;
                    prefix = prefix.max(index + 1);
                }
            }
            Step::Fail => {
                let accepted = fixture.authority.publish_workspace_group_close(
                    &plan,
                    Status::Failed {
                        reason: Failure::Deadline,
                        resumable: true,
                    },
                );
                if accepted != pending {
                    return Err(format!("turn {turn}: fail admission"));
                }
                pending = false;
            }
            Step::Resume => {
                let result = fixture.apply(Action::Resume {
                    operation_id: id(100),
                    expected_attempt: attempt,
                });
                let accepted = matches!(result, Outcome::Available { .. });
                if accepted != (!pending && !finished && attempt < 64) {
                    return Err(format!("turn {turn}: resume {result:?}"));
                }
                if accepted {
                    attempt += 1;
                    pending = true;
                    plan = fixture.authority.claim_workspace_group_close().unwrap();
                }
            }
            Step::Stale => {
                let mut stale = plan.clone();
                stale.attempt = 0;
                if fixture
                    .authority
                    .publish_workspace_group_close(&stale, Status::Pending {})
                {
                    return Err(format!("turn {turn}: stale callback"));
                }
            }
            Step::ChangeChild if prefix > 0 => {
                let mut changed = plan.clone();
                changed.members[0].member.close = Some(closed(999));
                if fixture
                    .authority
                    .publish_workspace_group_close(&changed, Status::Pending {})
                {
                    return Err(format!("turn {turn}: child identity changed"));
                }
            }
            Step::Finish => {
                let accepted = fixture
                    .authority
                    .publish_workspace_group_close(&plan, Status::Closed {});
                if accepted != (pending && prefix == 3) {
                    return Err(format!("turn {turn}: premature complete"));
                }
                if accepted {
                    pending = false;
                    finished = true;
                }
            }
            Step::Read | Step::ChangeChild => {}
        }
        let read = fixture.apply(Action::Read {
            operation_id: id(100),
        });
        if read != fixture.apply(fixture.start(100)) {
            return Err(format!("turn {turn}: equal Start replay"));
        }
        let Outcome::Available {
            attempt: observed,
            members,
            status,
            ..
        } = read
        else {
            return Err(format!("turn {turn}: lost record"));
        };
        if observed != attempt
            || members.iter().enumerate().any(|(index, member)| {
                member.close.is_some() != (index < prefix)
                    || member
                        .close
                        .as_ref()
                        .is_some_and(|child| !child.is_closed())
            })
            || matches!(status, Status::Pending {}) != pending
            || matches!(status, Status::Closed {}) != finished
        {
            return Err(format!("turn {turn}: model observation"));
        }
    }
    Ok(())
}

#[test]
fn authorization_worker_availability_and_changed_directory_fail_before_child_admission() {
    let mut fixture = Fixture::new();
    let request = WorkbenchWorkspaceGroupCloseRequest {
        protocol_version: ProtocolVersion,
        request_id: id(901),
        session_id: fixture.authority.session_id().clone(),
        action: fixture.start(100),
    };
    assert_eq!(
        fixture
            .authority
            .execute_workspace_group_close(None, request.clone(), Some(worker()), true)
            .outcome,
        Outcome::Denied {}
    );
    let read_only = WorkbenchGrant::read_only(fixture.authority.session_id().clone());
    assert_eq!(
        fixture
            .authority
            .execute_workspace_group_close(Some(&read_only), request.clone(), Some(worker()), true)
            .outcome,
        Outcome::Denied {}
    );
    let mut foreign = request.clone();
    foreign.session_id = orchbee_workbench_protocol::SessionId::parse(&identity(902)).unwrap();
    assert_eq!(
        fixture
            .authority
            .execute_workspace_group_close(Some(&fixture.grant), foreign, Some(worker()), true)
            .outcome,
        Outcome::Denied {}
    );
    assert_eq!(
        fixture
            .authority
            .execute_workspace_group_close(Some(&fixture.grant), request.clone(), None, true)
            .outcome,
        Outcome::Unavailable {
            reason: Unavailable::WorkerUnavailable
        }
    );
    assert!(fixture.authority.claim_workspace_group_close().is_none());
    let accepted = fixture.apply(request.action);
    let plan = fixture.authority.claim_workspace_group_close().unwrap();
    let first = &plan.members[0].member;
    assert!(
        fixture
            .authority
            .workspace_group_member_matches(&id(100), 1, 0, &first.workspace_id)
    );
    let mut changed = fixture.authority.snapshot().clone();
    let directory_id = &plan.members[0].directory.as_ref().unwrap().directory_id;
    let directory = changed
        .navigation
        .directories
        .iter_mut()
        .find(|directory| Some(&directory.directory_id) == directory_id.as_ref())
        .unwrap();
    directory.path = WorkbenchDirectoryPath::parse("C:/changed").unwrap();
    fixture.authority.projection = crate::workbench_projection::WorkbenchProjectionStore::load(
        &orchbee_workbench_protocol::serialize_document(
            &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(changed),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        !fixture
            .authority
            .workspace_group_member_matches(&id(100), 1, 0, &first.workspace_id)
    );
    let read = WorkbenchWorkspaceGroupCloseRequest {
        protocol_version: ProtocolVersion,
        request_id: id(903),
        session_id: fixture.authority.session_id().clone(),
        action: Action::Read {
            operation_id: id(100),
        },
    };
    assert_eq!(
        fixture
            .authority
            .execute_workspace_group_close(Some(&read_only), read, None, false)
            .outcome,
        accepted
    );
}

#[test]
fn accepted_unobserved_child_is_retained_and_never_authorizes_resume() {
    let mut fixture = Fixture::new();
    fixture.apply(fixture.start(100));
    let mut plan = fixture.authority.claim_workspace_group_close().unwrap();
    plan.members[0].member.close = Some(WorkspaceGroupCloseChild {
        request_id: id(300),
        observation: None,
    });
    assert!(
        fixture
            .authority
            .publish_workspace_group_close(&plan, Status::Pending {})
    );
    assert!(!fixture.authority.publish_workspace_group_close(
        &plan,
        Status::Failed {
            reason: Failure::Deadline,
            resumable: true
        }
    ));
    assert!(fixture.authority.publish_workspace_group_close(
        &plan,
        Status::Failed {
            reason: Failure::OutcomeUnknown,
            resumable: false
        }
    ));
    assert_eq!(
        fixture.apply(Action::Resume {
            operation_id: id(100),
            expected_attempt: 1
        }),
        Outcome::Unavailable {
            reason: Unavailable::NotResumable
        }
    );
    let Outcome::Available { members, .. } = fixture.apply(Action::Read {
        operation_id: id(100),
    }) else {
        panic!("retained");
    };
    assert_eq!(members[0].close.as_ref().unwrap().request_id, id(300));
    assert!(members[0].close.as_ref().unwrap().observation.is_none());
    assert!(members[1..].iter().all(|member| member.close.is_none()));
}

#[test]
fn ordinary_workspace_close_removes_only_its_context_even_with_observed_git_peers() {
    use crate::workbench_authority::{WorkbenchTabClosePreparation, WorkbenchTabCloseResolution};
    let mut fixture = Fixture::new();
    let before = fixture.authority.snapshot().clone();
    let close = command(
        200,
        before.revision.value(),
        WorkbenchAction::WorkspaceClose {
            workspace_id: fixture.main.clone(),
        },
    );
    let now = std::time::Instant::now();
    let WorkbenchWorkspaceCloseBegin::Admitted(admission) = fixture
        .authority
        .begin_workspace_close(Some(&fixture.grant), close.clone())
    else {
        panic!("ordinary close must not demand a Git group close")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("primary")
    };
    assert!(primary.members().is_empty());
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(vec![], now)
    else {
        panic!("prepare")
    };
    let _accepted = prepared.commit();
    fixture
        .authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    let prepared = fixture
        .authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap();
    let revision = prepared.revision();
    let _closed = prepared.commit();
    let context = before
        .navigation
        .workspace(&fixture.main)
        .unwrap()
        .terminal_context_id
        .clone();
    let mut expected = before;
    expected.revision = revision;
    expected
        .navigation
        .workspaces
        .retain(|workspace| workspace.workspace_id != fixture.main);
    expected
        .navigation
        .workspace_order
        .retain(|id| *id != fixture.main);
    expected
        .terminal_contexts
        .retain(|entry| entry.terminal_context_id != context);
    assert_eq!(fixture.authority.snapshot(), &expected);
    assert!(
        fixture
            .authority
            .worktree_checkout(&fixture.linked)
            .is_some()
    );
    assert!(
        fixture
            .authority
            .worktree_checkout(&fixture.excluded)
            .is_some()
    );
}

#[test]
fn group_admission_freezes_members_and_order_and_preserves_project_until_explicit_dissolve() {
    let mut fixture = Fixture::new();
    let Outcome::Available {
        members: captured, ..
    } = fixture.apply(fixture.start(100))
    else {
        panic!("admit")
    };
    let newcomer = workspace(
        &mut fixture.authority,
        &fixture.grant,
        &fixture.project,
        3000,
        "E:/later",
    );
    let revision = fixture.authority.snapshot().revision.value();
    submit(
        &mut fixture.authority,
        &fixture.grant,
        command(
            3002,
            revision,
            WorkbenchAction::WorkspaceMove {
                workspace_id: newcomer.clone(),
                insert_index: orchbee_workbench_protocol::WorkbenchWorkspaceInsertIndex::new(0)
                    .unwrap(),
            },
        ),
    );
    let mut plan = fixture.authority.claim_workspace_group_close().unwrap();
    assert_eq!(
        plan.members
            .iter()
            .map(|member| member.member.clone())
            .collect::<Vec<_>>(),
        captured
    );
    assert!(
        plan.members
            .iter()
            .all(|member| member.member.workspace_id != newcomer)
    );
    for (index, member) in plan.members.iter_mut().enumerate() {
        member.member.close = Some(closed(4000 + index as u128));
    }
    assert!(
        fixture
            .authority
            .publish_workspace_group_close(&plan, Status::Closed {})
    );
    assert!(
        fixture
            .authority
            .snapshot()
            .navigation
            .project(&fixture.project)
            .is_some()
    );
    assert!(
        fixture
            .authority
            .snapshot()
            .navigation
            .workspace(&newcomer)
            .is_some()
    );
    // Dissolution is a separate organization action and leaves all Workspace identities intact.
    let before = fixture.authority.snapshot().clone();
    let revision = before.revision.value();
    assert!(matches!(
        submit(
            &mut fixture.authority,
            &fixture.grant,
            command(
                3003,
                revision,
                WorkbenchAction::ProjectDissolve {
                    project_id: fixture.project.clone()
                }
            )
        )
        .outcome,
        WorkbenchCommandOutcome::ProjectDissolved { .. }
    ));
    assert_eq!(
        fixture.authority.snapshot().terminal_contexts,
        before.terminal_contexts
    );
    assert_eq!(
        fixture.authority.snapshot().navigation.workspace_order,
        before.navigation.workspace_order
    );
}

#[test]
fn empty_project_close_is_valid_and_has_no_implicit_project_deletion() {
    let mut fixture = Fixture::new();
    let revision = fixture.authority.snapshot().revision.value();
    let empty = project_id(&submit(
        &mut fixture.authority,
        &fixture.grant,
        command(3000, revision, create("empty")),
    ));
    let before = fixture.authority.snapshot().clone();
    assert!(
        matches!(fixture.apply(Action::Start { operation_id: id(3001), project_id: empty.clone() }),
        Outcome::Available { members, .. } if members.is_empty())
    );
    let plan = fixture.authority.claim_workspace_group_close().unwrap();
    assert!(plan.members.is_empty());
    assert!(
        fixture
            .authority
            .publish_workspace_group_close(&plan, Status::Closed {})
    );
    assert_eq!(fixture.authority.snapshot(), &before);
    assert!(
        fixture
            .authority
            .snapshot()
            .navigation
            .project(&empty)
            .is_some()
    );
}
