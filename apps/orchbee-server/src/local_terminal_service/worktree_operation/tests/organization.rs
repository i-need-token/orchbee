use super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState, HostTerminalId,
    HostTerminalIdentity, TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchResult,
};
use orchbee_server::{
    WorkbenchGrant, WorkbenchTabClosePreparation, WorkbenchTabCloseResolution,
    WorkbenchWorkspaceCloseBegin,
};

fn open(source: &WorkbenchWorkspaceId) -> WorktreeIntent {
    WorktreeIntent::Open {
        source: WorktreeSource::Workspace {
            workspace_id: source.clone(),
        },
        selector: WorktreeSelector::Path {
            path: WorkbenchDirectoryPath::parse("C:/new-checkout").unwrap(),
        },
        label: None,
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        trust_repository: false,
    }
}

fn opened() -> Completion {
    Completion::Checkout(
        WorktreeCheckout::new(
            AbsoluteCwd::new("C:/repo").unwrap(),
            AbsoluteCwd::new("C:/repo/.git").unwrap(),
            AbsoluteCwd::new("C:/new-checkout").unwrap(),
            Some(GitReference::new("topic").unwrap()),
            false,
        )
        .unwrap(),
    )
}

fn complete_one_shell(fixture: &mut Fixture) {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) =
        read_message(&mut fixture.peer).unwrap()
    else {
        panic!("one terminal launch")
    };
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("shell geometry")
    };
    let outcome = TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
        HostTerminalIdentity::new(
            HostCreationId::from_bytes([5; 16]).unwrap(),
            HostEpoch::from_bytes([6; 16]).unwrap(),
            HostTerminalId::from_bytes([7; 16]).unwrap(),
        ),
        HostLaunchState::Started(geometry),
    ));
    write_message(
        &mut fixture.peer,
        &CapabilityNodeMessage::TerminalLaunchResult(
            TerminalLaunchResult::new(launch.authorization(), outcome).unwrap(),
        ),
    )
    .unwrap();
}

#[test]
fn asynchronous_worktree_completion_uses_the_sources_current_project_and_regroup_sends_no_host_requests()
 {
    let mut fixture = Fixture::new();
    let source = fixture.workspace.clone();
    assert!(matches!(
        fixture.start(open(&source)),
        Outcome::Available {
            project_id: None,
            ..
        }
    ));
    fixture.finish_job(checkout(false));
    // Git preparation is now admitted, but its completion is deliberately held.
    let preparing = fixture.receive();
    fixture.reply(
        &preparing,
        WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Accepted),
    );
    let completion = fixture.receive();
    let WorkbenchCommandOutcome::ProjectCreated {
        project_id: current,
        ..
    } = navigation(
        &fixture.service,
        200,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("current grouping").unwrap(),
        },
    )
    else {
        panic!("project")
    };
    assert!(matches!(
        navigation(
            &fixture.service,
            201,
            WorkbenchAction::WorkspaceSetProject {
                workspace_id: source.clone(),
                project_id: Some(current.clone())
            }
        ),
        WorkbenchCommandOutcome::WorkspaceProjectChanged { .. }
    ));
    let before_attach = snapshot(&fixture.service);
    fixture.reply(
        &completion,
        WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(opened())),
    );
    complete_one_shell(&mut fixture);
    let Status::Opened {
        workspace_id: created,
        reused: false,
        terminal_outcome: Some(orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. }),
        ..
    } = fixture.settled()
    else {
        panic!("opened")
    };
    let after = snapshot(&fixture.service);
    assert_eq!(
        after.navigation.workspace(&created).unwrap().project_id,
        Some(current.clone())
    );
    assert_eq!(
        after.navigation.workspace(&source),
        before_attach.navigation.workspace(&source)
    );
    assert_eq!(after.navigation.projects, before_attach.navigation.projects);
    assert_eq!(
        after.terminal_contexts.len(),
        before_attach.terminal_contexts.len() + 1
    );
    let checkout_before = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .worktree_authority()
        .unwrap()
        .worktree_checkout(&source)
        .unwrap();

    for (id, action) in [
        (
            202,
            WorkbenchAction::WorkspaceSetProject {
                workspace_id: created.clone(),
                project_id: None,
            },
        ),
        (
            203,
            WorkbenchAction::WorkspaceCreateProject {
                workspace_id: created.clone(),
                label: WorkbenchLabel::parse("new group").unwrap(),
            },
        ),
        (
            204,
            WorkbenchAction::ProjectDissolve {
                project_id: current,
            },
        ),
    ] {
        let before = snapshot(&fixture.service);
        assert!(matches!(
            navigation(&fixture.service, id, action),
            WorkbenchCommandOutcome::WorkspaceProjectChanged { .. }
                | WorkbenchCommandOutcome::ProjectDissolved { .. }
        ));
        let after = snapshot(&fixture.service);
        assert_eq!(
            after.terminal_contexts, before.terminal_contexts,
            "bound runtime identities and layouts are stable"
        );
        assert_eq!(after.navigation.directories, before.navigation.directories);
        assert_eq!(
            after.navigation.workspace_order,
            before.navigation.workspace_order
        );
    }
    assert_eq!(
        fixture
            .service
            .coordinator
            .lock()
            .unwrap()
            .worktree_authority()
            .unwrap()
            .worktree_checkout(&source),
        Some(checkout_before)
    );
    assert!(
        matches!(
            fixture.read(),
            Outcome::Available {
                project_id: None,
                ..
            }
        ),
        "receipt retains original requested metadata"
    );
    fixture
        .peer
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    assert!(
        read_message(&mut fixture.peer).is_err(),
        "regroup/dissolve must send neither PTY launch nor Git mutation requests"
    );
}

#[test]
fn directory_source_without_a_project_opens_an_ungrouped_workspace() {
    let mut fixture = Fixture::new();
    let before = snapshot(&fixture.service);
    assert!(matches!(
        fixture.apply(Action::Start {
            operation_id: id(40),
            project_id: None,
            intent: WorktreeIntent::Open {
                source: WorktreeSource::Directory {
                    path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
                },
                selector: WorktreeSelector::Path {
                    path: WorkbenchDirectoryPath::parse("C:/new-checkout").unwrap(),
                },
                label: None,
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                trust_repository: false,
            },
        }),
        Outcome::Available {
            project_id: None,
            ..
        }
    ));
    fixture.finish_job(checkout(false));
    fixture.finish_job(opened());
    complete_one_shell(&mut fixture);
    let Status::Opened {
        workspace_id: created,
        reused: false,
        ..
    } = fixture.settled()
    else {
        panic!("opened")
    };
    let after = snapshot(&fixture.service);
    assert_eq!(
        after.navigation.workspace(&created).unwrap().project_id,
        None
    );
    assert_eq!(
        after.navigation.projects, before.navigation.projects,
        "no Project is inferred from the repository"
    );
}

#[test]
fn delayed_worktree_result_cannot_resurrect_a_closed_or_closing_source() {
    for publish_close in [false, true] {
        let mut fixture = Fixture::with_close_owner(false);
        let source = fixture.workspace.clone();
        assert!(matches!(
            fixture.start(open(&source)),
            Outcome::Available { .. }
        ));
        fixture.finish_job(checkout(false));
        let preparing = fixture.receive();
        fixture.reply(
            &preparing,
            WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Accepted),
        );
        let completion = fixture.receive();
        {
            let mut coordinator = fixture.service.coordinator.lock().unwrap();
            let owner = coordinator.worktree_authority().unwrap();
            let grant = WorkbenchGrant::manage_navigation(owner.session_id().clone())
                .with_terminal_lifecycle();
            let close = WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: id(200),
                session_id: owner.session_id().clone(),
                expected_revision: owner.snapshot().revision,
                action: WorkbenchAction::WorkspaceClose {
                    workspace_id: source.clone(),
                },
            };
            let now = Instant::now();
            let WorkbenchWorkspaceCloseBegin::Admitted(admission) =
                owner.begin_workspace_close(Some(&grant), close.clone())
            else {
                panic!("close")
            };
            let WorkbenchTabCloseResolution::Primary(primary) =
                admission.resolve_after_composition()
            else {
                panic!("primary")
            };
            let WorkbenchTabClosePreparation::Prepared(prepared) =
                primary.finish_members(vec![], now)
            else {
                panic!("prepared")
            };
            let _accepted = prepared.commit();
            if publish_close {
                owner
                    .next_terminal_tab_close_step_target(&close.request_id, now)
                    .unwrap();
                let _closed = owner
                    .prepare_terminal_tab_close_publication(&close.request_id, now)
                    .unwrap()
                    .commit();
            }
        }
        let before_completion = snapshot(&fixture.service);
        assert_eq!(
            before_completion.navigation.workspace(&source).is_none(),
            publish_close
        );
        fixture.reply(
            &completion,
            WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(opened())),
        );
        assert!(matches!(
            fixture.settled(),
            Status::Failed {
                reason: Failure::SourceChanged {},
                resumable: false,
                ..
            }
        ));
        assert_eq!(snapshot(&fixture.service), before_completion);
        fixture
            .peer
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        assert!(
            read_message(&mut fixture.peer).is_err(),
            "no first shell or replacement source may be launched"
        );
    }
}
