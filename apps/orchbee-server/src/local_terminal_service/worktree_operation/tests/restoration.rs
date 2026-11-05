use super::*;
use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostTerminalId, HostTerminalIdentity, LifecycleInventoryPage, LifecycleTargetObservation,
    LifecycleTerminalPhase, TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchResult, TerminalLifecycleAction, TerminalLifecycleOutcome,
    TerminalLifecycleResult,
};
use orchbee_workbench_protocol::{PanelNode, WorkbenchTerminalOutcome};
use std::num::NonZeroU64;

fn launch(fixture: &mut Fixture) -> AuthorizedTerminalLaunch {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) =
        read_message(&mut fixture.peer).unwrap()
    else {
        panic!("launch")
    };
    launch
}
fn started(launch: &AuthorizedTerminalLaunch, index: u8) -> TerminalLaunchOutcome {
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("immediate shell")
    };
    TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
        HostTerminalIdentity::new(
            HostCreationId::from_bytes([index; 16]).unwrap(),
            HostEpoch::from_bytes([4; 16]).unwrap(),
            HostTerminalId::from_bytes([index; 16]).unwrap(),
        ),
        HostLaunchState::Started(geometry),
    ))
}
fn reply_launch(
    fixture: &mut Fixture,
    launch: &AuthorizedTerminalLaunch,
    outcome: TerminalLaunchOutcome,
) {
    write_message(
        &mut fixture.peer,
        &CapabilityNodeMessage::TerminalLaunchResult(
            TerminalLaunchResult::new(launch.authorization(), outcome).unwrap(),
        ),
    )
    .unwrap();
}
fn add_shell(fixture: &mut Fixture, index: u8) {
    let state = snapshot(&fixture.service);
    let context = state.navigation.workspaces[0].terminal_context_id.clone();
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(10000 + u128::from(index)),
        session_id: state.session_id,
        expected_revision: state.revision,
        action: WorkbenchAction::TabCreate {
            placement: None,
            terminal_context_id: context,
            automatic_label: false,
            label: WorkbenchLabel::parse("Agent task").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(100, 40, 9, 18).unwrap(),
            launch: Some(
                orchbee_workbench_protocol::LaunchSelection::new(
                    "C:/old-agent.exe".into(),
                    vec!["resume-old-task".into()],
                )
                .unwrap(),
            ),
            profile: None,
            options: Default::default(),
        },
    };
    let service = Arc::clone(&fixture.service);
    let caller = thread::spawn(move || service.execute_terminal_tab_create(command).unwrap());
    let request = launch(fixture);
    assert!(request.launch().is_some());
    reply_launch(fixture, &request, started(&request, index));
    assert!(matches!(
        caller.join().unwrap().outcome,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
}
fn disposed_before_remove(fixture: &mut Fixture, expected: usize) -> WorktreeMutationRequest {
    let mut disposed = 0;
    loop {
        match read_message(&mut fixture.peer).unwrap() {
            CapabilityNodeMessage::TerminalLifecycleRequest(request) => {
                let outcome = match request.action {
                    TerminalLifecycleAction::InventoryPage { .. } => {
                        TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
                            inventory_generation: NonZeroU64::MIN,
                            next_cursor: None,
                            returned_count: 1,
                            target: Some(LifecycleTargetObservation {
                                revision: 3,
                                phase: LifecycleTerminalPhase::Exited {
                                    terminate_requested: false,
                                },
                            }),
                        })
                    }
                    TerminalLifecycleAction::Dispose => {
                        disposed += 1;
                        TerminalLifecycleOutcome::Disposed
                    }
                    other => panic!("unexpected lifecycle {other:?}"),
                };
                write_message(
                    &mut fixture.peer,
                    &CapabilityNodeMessage::TerminalLifecycleResult(
                        TerminalLifecycleResult::new(request, outcome).unwrap(),
                    ),
                )
                .unwrap();
            }
            CapabilityNodeMessage::WorktreeMutationRequest(request) => {
                assert_eq!(disposed, expected);
                assert!(
                    matches!(&request.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::Remove { .. }))
                );
                return request;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
fn complete_job(fixture: &mut Fixture, request: &WorktreeMutationRequest, completion: Completion) {
    fixture.reply(
        request,
        WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Accepted),
    );
    let poll = fixture.receive();
    fixture.reply(
        &poll,
        WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(completion)),
    );
}
fn git_failed() -> Completion {
    Completion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Mutate,
            WorktreeMutationEffect::MayHaveChanged,
            WorktreeMutationError::GitFailed(Some(128)),
            b"locked".to_vec(),
        )
        .unwrap(),
    )
}

#[test]
fn failed_remove_restores_original_slots_with_fresh_default_shells_and_keeps_partial_receipts() {
    for mode in 0..3 {
        let mut fixture = Fixture::new();
        for index in 1..=2 {
            add_shell(&mut fixture, index);
        }
        let before = snapshot(&fixture.service);
        fixture.start(WorktreeIntent::Remove {
            workspace_id: fixture.workspace.clone(),
            force: true,
            trust_repository: false,
        });
        fixture.finish_job(checkout(false));
        let remove = disposed_before_remove(&mut fixture, 2);
        assert_eq!(
            snapshot(&fixture.service),
            before,
            "disposal must not remove layout"
        );
        complete_job(&mut fixture, &remove, git_failed());
        let verify = fixture.finish_job(checkout(false));
        assert!(
            matches!(verify.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::InspectRemove { .. }) && intent.source().as_str() == "C:/repo")
        );
        for index in 0..2 {
            let request = launch(&mut fixture);
            assert!(request.launch().is_none(), "never replay the Agent program");
            assert!(request.environment().entries().is_empty());
            assert_eq!(request.cwd().unwrap().as_str(), "C:/w");
            let Outcome::Available { status, .. } = fixture.read() else {
                panic!("pending")
            };
            let Status::Pending { progress, .. } = *status else {
                panic!("recovery pending")
            };
            assert_eq!(
                progress.restoration.unwrap().attempted,
                index + 1,
                "reserve receipt before dispatch"
            );
            let result = if index == 0 || mode == 0 {
                started(&request, 20 + index as u8)
            } else if mode == 1 {
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
            } else {
                TerminalLaunchOutcome::OutcomeUnknown {
                    host_epoch: HostEpoch::from_bytes([4; 16]).unwrap(),
                }
            };
            reply_launch(&mut fixture, &request, result);
        }
        let status = fixture.settled();
        let Status::Failed {
            reason: Failure::Git {
                status: Some(128), ..
            },
            progress,
            resumable,
        } = &status
        else {
            panic!("{status:?}")
        };
        assert!(matches!(
            progress.workspace,
            WorktreeWorkspaceDisposition::Open { .. }
        ));
        let receipt = progress.restoration.as_ref().unwrap();
        assert_eq!((receipt.total, receipt.attempted), (2, 2));
        assert_eq!(
            (receipt.started, receipt.failed, receipt.unknown),
            match mode {
                0 => (2, 0, 0),
                1 => (1, 1, 0),
                _ => (1, 0, 1),
            }
        );
        assert!(receipt.failure.is_none());
        assert_eq!(*resumable, mode != 2);
        let after = snapshot(&fixture.service);
        let mut expected = before.clone();
        expected.revision = after.revision;
        for (old_context, new_context) in expected
            .terminal_contexts
            .iter_mut()
            .zip(&after.terminal_contexts)
        {
            for (old_tab, new_tab) in old_context.tabs.iter_mut().zip(&new_context.tabs) {
                let PanelNode::Panel {
                    primary_presentation: old,
                    ..
                } = &mut old_tab.root
                else {
                    panic!("leaf")
                };
                let PanelNode::Panel {
                    primary_presentation: new,
                    ..
                } = &new_tab.root
                else {
                    panic!("leaf")
                };
                assert_ne!(old.binding, new.binding);
                assert_ne!(old.presentation_id, new.presentation_id);
                // The replacement shell never inherits the old agent's program; this fixture's
                // Worker reports no System shell, so the default launch names none.
                assert_eq!(
                    old.launch_program
                        .as_ref()
                        .map(orchbee_workbench_protocol::PresentationLaunchProgram::as_str),
                    Some("old-agent.exe")
                );
                assert_eq!(new.launch_program, None);
                old.binding = new.binding.clone();
                old.presentation_id = new.presentation_id.clone();
                old.launch_program = None;
            }
        }
        assert_eq!(
            after, expected,
            "only Presentation, binding, launched program and revision change"
        );
        let resumed = fixture.apply(Action::Resume {
            operation_id: id(40),
            expected_attempt: 1,
        });
        if mode == 2 {
            assert!(
                matches!(resumed, Outcome::Available { attempt: 1, status: old, .. } if *old == status)
            );
        } else {
            assert!(matches!(resumed, Outcome::Available { attempt: 2, .. }));
            fixture.finish_job(checkout(false));
            let remove = disposed_before_remove(&mut fixture, if mode == 0 { 2 } else { 1 });
            complete_job(
                &mut fixture,
                &remove,
                Completion::Removed {
                    path: AbsoluteCwd::new("C:/w").unwrap(),
                },
            );
            assert!(matches!(fixture.settled(), Status::Removed { .. }));
        }
        fixture
            .peer
            .set_read_timeout(Some(Duration::from_millis(40)))
            .unwrap();
        assert!(
            read_message(&mut fixture.peer).is_err(),
            "no duplicated launch or Git request"
        );
    }
}

#[test]
fn failed_checkout_verification_preserves_original_git_failure_and_starts_no_shell() {
    let mut fixture = Fixture::new();
    add_shell(&mut fixture, 1);
    let before = snapshot(&fixture.service);
    fixture.start(WorktreeIntent::Remove {
        workspace_id: fixture.workspace.clone(),
        force: true,
        trust_repository: false,
    });
    fixture.finish_job(checkout(false));
    let remove = disposed_before_remove(&mut fixture, 1);
    complete_job(&mut fixture, &remove, git_failed());
    fixture.finish_job(Completion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Select,
            WorktreeMutationEffect::None,
            WorktreeMutationError::CheckoutNotFound,
            vec![],
        )
        .unwrap(),
    ));
    let status = fixture.settled();
    let Status::Failed {
        reason: Failure::Git {
            status: Some(128), ..
        },
        progress,
        ..
    } = status
    else {
        panic!("original Git failure")
    };
    let receipt = progress.restoration.unwrap();
    assert_eq!((receipt.total, receipt.attempted), (1, 0));
    assert!(matches!(
        receipt.failure,
        Some(Failure::Git {
            error: WorktreeGitError::CheckoutNotFound,
            ..
        })
    ));
    assert!(matches!(
        progress.workspace,
        WorktreeWorkspaceDisposition::Closing { .. }
    ));
    assert_eq!(snapshot(&fixture.service), before);
    fixture
        .peer
        .set_read_timeout(Some(Duration::from_millis(40)))
        .unwrap();
    assert!(read_message(&mut fixture.peer).is_err());
}

fn checkout_not_found(stage: WorktreeMutationStage) -> Completion {
    Completion::Failed(
        WorktreeMutationFailure::new(
            stage,
            WorktreeMutationEffect::None,
            WorktreeMutationError::CheckoutNotFound,
            vec![],
        )
        .unwrap(),
    )
}

/// Git deleted the registration and contents, but a locked directory survived. The first
/// attempt ends with the Workspace still closing and an unverifiable checkout.
fn half_removed(fixture: &mut Fixture) {
    add_shell(fixture, 1);
    fixture.start(WorktreeIntent::Remove {
        workspace_id: fixture.workspace.clone(),
        force: false,
        trust_repository: false,
    });
    fixture.finish_job(checkout(false));
    let remove = disposed_before_remove(fixture, 1);
    complete_job(fixture, &remove, git_failed());
    fixture.finish_job(checkout_not_found(WorktreeMutationStage::Select));
    assert!(matches!(
        fixture.settled(),
        Status::Failed {
            resumable: true,
            progress,
            ..
        } if matches!(progress.workspace, WorktreeWorkspaceDisposition::Closing { .. })
    ));
}

/// A Close maintenance turn long after the 30 s attempt budget. A failed removal holds its
/// Workspace close while the user decides; that turn must leave the held close ready.
fn age_held_close(fixture: &Fixture) {
    let Outcome::Available { status, .. } = fixture.read() else {
        panic!("retained status")
    };
    let Status::Failed { progress, .. } = *status else {
        panic!("failed removal")
    };
    let WorktreeWorkspaceDisposition::Closing {
        close_request_id, ..
    } = progress.workspace
    else {
        panic!("held close")
    };
    let mut session = fixture.service.coordinator.lock().unwrap();
    let authority = session.worktree_authority().unwrap();
    let late = Instant::now() + Duration::from_secs(31);
    assert_eq!(
        authority
            .next_terminal_tab_close_step(&close_request_id, late)
            .unwrap(),
        orchbee_server::CloseContinuationNext::Parked
    );
}

/// Resume re-inspects the vanished registration, keeps the original checkout facts, and asks
/// the Worker to settle the residue without closing or launching anything again.
fn resume_to_remove(fixture: &mut Fixture, expected_attempt: u32) -> WorktreeMutationRequest {
    assert!(matches!(
        fixture.apply(Action::Resume {
            operation_id: id(40),
            expected_attempt,
        }),
        Outcome::Available { attempt, .. } if attempt == expected_attempt + 1
    ));
    let inspect = fixture.finish_job(checkout_not_found(WorktreeMutationStage::Select));
    assert!(
        matches!(&inspect.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::InspectRemove { .. }))
    );
    let remove = disposed_before_remove(fixture, 0);
    assert!(
        matches!(&remove.action, WorktreeMutationJobAction::Start(intent)
            if intent.source().as_str() == "C:/repo"
                && matches!(intent.mutation(), WorktreeMutation::Remove { path, expected_common_directory, force: false }
                    if path.as_str() == "C:/w" && expected_common_directory.as_str() == "C:/repo/.git"))
    );
    remove
}

#[test]
fn resumed_half_removal_settles_the_unregistered_residue_and_finishes_the_close() {
    let mut fixture = Fixture::new();
    half_removed(&mut fixture);
    age_held_close(&fixture);
    // The surviving directory is still locked: a retryable filesystem failure.
    let remove = resume_to_remove(&mut fixture, 1);
    complete_job(
        &mut fixture,
        &remove,
        Completion::Failed(
            WorktreeMutationFailure::new(
                WorktreeMutationStage::Mutate,
                WorktreeMutationEffect::None,
                WorktreeMutationError::Io,
                vec![],
            )
            .unwrap(),
        ),
    );
    fixture.finish_job(checkout_not_found(WorktreeMutationStage::Select));
    let status = fixture.settled();
    assert!(
        matches!(&status, Status::Failed {
            reason: Failure::Git { error: WorktreeGitError::Io, .. },
            resumable: true,
            progress,
        } if matches!(progress.workspace, WorktreeWorkspaceDisposition::Closing { .. })),
        "{status:?}"
    );
    // After the lock is released, the Worker proves absence and the held close completes.
    age_held_close(&fixture);
    let remove = resume_to_remove(&mut fixture, 2);
    complete_job(
        &mut fixture,
        &remove,
        Completion::Removed {
            path: AbsoluteCwd::new("C:/w").unwrap(),
        },
    );
    let status = fixture.settled();
    assert!(
        matches!(&status, Status::Removed { workspace_id, .. } if *workspace_id == fixture.workspace),
        "{status:?}"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !snapshot(&fixture.service).navigation.workspaces.is_empty() {
        assert!(Instant::now() < deadline, "the held Workspace close ends");
        thread::sleep(Duration::from_millis(10));
    }
    fixture
        .peer
        .set_read_timeout(Some(Duration::from_millis(40)))
        .unwrap();
    assert!(read_message(&mut fixture.peer).is_err());
}

#[test]
fn unprovable_residue_is_not_resumable_and_keeps_the_held_workspace_for_explicit_close() {
    let mut fixture = Fixture::new();
    half_removed(&mut fixture);
    age_held_close(&fixture);
    let remove = resume_to_remove(&mut fixture, 1);
    complete_job(
        &mut fixture,
        &remove,
        checkout_not_found(WorktreeMutationStage::Select),
    );
    fixture.finish_job(checkout_not_found(WorktreeMutationStage::Select));
    let status = fixture.settled();
    assert!(
        matches!(&status, Status::Failed {
            reason: Failure::Git { error: WorktreeGitError::CheckoutNotFound, .. },
            resumable: false,
            progress,
        } if matches!(progress.workspace, WorktreeWorkspaceDisposition::Closing { .. })),
        "{status:?}"
    );
    assert!(matches!(
        fixture.apply(Action::Resume {
            operation_id: id(40),
            expected_attempt: 2,
        }),
        Outcome::Available { attempt: 2, .. }
    ));
    fixture
        .peer
        .set_read_timeout(Some(Duration::from_millis(40)))
        .unwrap();
    assert!(read_message(&mut fixture.peer).is_err(), "no Git replay");
    // The explicit close the failure names ends the held Workspace on its own, even
    // after the hold outlasted the Close's original attempt budget.
    age_held_close(&fixture);
    let current = snapshot(&fixture.service);
    let accepted = fixture
        .service
        .execute_workspace_close(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: id(70),
            session_id: current.session_id,
            expected_revision: current.revision,
            action: WorkbenchAction::WorkspaceClose {
                workspace_id: fixture.workspace.clone(),
            },
        })
        .unwrap();
    assert!(
        matches!(
            accepted.outcome,
            WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
        ),
        "{:?}",
        accepted.outcome
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !snapshot(&fixture.service).navigation.workspaces.is_empty() {
        assert!(
            Instant::now() < deadline,
            "the explicit close ends the held Workspace"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(read_message(&mut fixture.peer).is_err(), "no Git replay");
}
