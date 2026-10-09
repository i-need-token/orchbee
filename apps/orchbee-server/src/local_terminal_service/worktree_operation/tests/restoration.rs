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
    let context = state.navigation.projects[0].workspaces[0]
        .terminal_context_id
        .clone();
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(10000 + u128::from(index)),
        site_id: state.site_id,
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
                old.binding = new.binding.clone();
                old.presentation_id = new.presentation_id.clone();
            }
        }
        assert_eq!(
            after, expected,
            "only Presentation, binding and revision change"
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
