//! Real framed exchanges prove the destructive scope, retained evidence, and WorkbenchClientUi settlement.
use super::super::ClientExecutor;
use super::setup::{TestResult, admit};
use crate::client_view::{ClientAction, ClientActionDisposition, ClientKey, ClientView};
use crate::local_client::test_support::server::FakeServer;
use crate::local_client::{LocalClient, LocalOperationClock};
use crate::localization::Locale;
use crate::tui_hit_map::HitMapBuilder;
use crate::tui_theme::Theme;
use crate::workbench_client::WorkspaceCloseStatus;
use crate::workbench_client::mailbox::{
    CommandLease, CommandMailbox, EventDeadline, QueuedCommand, WorkerEvent,
};
use crate::workbench_client::ui::WorkbenchClientUi;
use crate::workbench_projection::{ClientContextLocation, project_native_context_tabs};
use crate::workbench_state::workspace_close::{WorkspaceCloseOperation, WorkspaceCloseResult};
use crate::workbench_state::{CarrierFailure, OPERATION_TOTAL};
use orchbee_client_server_protocol::Geometry;
use orchbee_workbench_protocol::*;
use ratatui::{Terminal, backend::TestBackend};
use std::{
    io::{self, Read},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scope {
    Workspace,
    BackgroundWorkspace,
    UngroupedWorkspace,
    Project,
    EmptyProject,
}
impl Scope {
    fn project(self) -> bool {
        matches!(self, Self::Project | Self::EmptyProject)
    }
    fn index(self) -> usize {
        match self {
            Self::Workspace => 0,
            Self::BackgroundWorkspace => 1,
            _ => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cut {
    StaleSource,
    /// A peer commit advances the revision between the fresh read and the Close command.
    RevisionRaced,
    StaleTarget,
    RemovedTarget,
    UnknownStart,
    /// The canonical read that follows admission is lost with the connection.
    AcceptedSnapshotLost,
    UiDeliveryLost,
    Complete,
}
impl Cut {
    fn rejects_source(self) -> bool {
        matches!(
            self,
            Self::StaleSource | Self::StaleTarget | Self::RemovedTarget
        )
    }
}
fn id(value: u128) -> String {
    format!("{value:032x}")
}
fn project_id(scope: Scope) -> WorkbenchProjectId {
    WorkbenchProjectId::parse(&id(if scope == Scope::EmptyProject {
        280
    } else {
        270
    }))
    .unwrap()
}
fn snapshot() -> Snapshot {
    let directory_id = WorkbenchDirectoryId::parse(&id(260)).unwrap();
    let workspaces: Vec<_> = (0..3)
        .map(|index| Workspace {
            terminal_directory: None,
            automatic_label: false,
            workspace_id: WorkbenchWorkspaceId::parse(&id(271 + index)).unwrap(),
            project_id: (index < 2).then(|| project_id(Scope::Project)),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(290 + index)).unwrap(),
            label: format!("项目 space 界 {index}"),
            primary_directory_id: Some(directory_id.clone()),
        })
        .collect();
    let contexts = workspaces
        .iter()
        .map(|workspace| TerminalContext {
            terminal_context_id: workspace.terminal_context_id.clone(),
            tabs: Vec::new(),
            groups: Vec::new(),
        })
        .collect();
    Snapshot {
        protocol_version: ProtocolVersion,
        session_id: SessionId::parse(&id(1)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            project_order: Vec::new(),
            directories: vec![Directory {
                directory_id,
                worker_id: WorkerId::parse(&id(261)).unwrap(),
                path: WorkbenchDirectoryPath::parse("C:/shared-checkout").unwrap(),
            }],
            projects: vec![
                Project {
                    project_id: project_id(Scope::Project),
                    label: "Project 项目".into(),
                },
                Project {
                    project_id: project_id(Scope::EmptyProject),
                    label: "Empty 空项目".into(),
                },
            ],
            workspace_order: workspaces
                .iter()
                .map(|workspace| workspace.workspace_id.clone())
                .collect(),
            workspaces,
        },
        terminal_contexts: contexts,
    }
}
fn ready(
    scope: Scope,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    ready_at(scope, Arc::new(AtomicU64::new(0)))
}
/// The ready fixture on an injected clock whose elapsed milliseconds the caller advances.
fn ready_at(
    scope: Scope,
    elapsed_millis: Arc<AtomicU64>,
) -> (
    WorkbenchClientUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Snapshot,
) {
    let clock =
        crate::local_client::test_support::server::adjustable_operation_clock(elapsed_millis);
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(Arc::clone(&mailbox), clock.clone());
    let snapshot = snapshot();
    let origin = if scope == Scope::BackgroundWorkspace {
        0
    } else {
        scope.index()
    };
    let workspace = &snapshot.navigation.workspaces[origin];
    let location = ClientContextLocation::Workspace {
        workspace_id: workspace.workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache: project_native_context_tabs(&snapshot, &location).unwrap(),
            },
            EventDeadline::Startup(clock.now() + OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    (ui, mailbox, clock, snapshot)
}
fn open(ui: &mut WorkbenchClientUi, scope: Scope) {
    if scope.project() {
        assert_eq!(
            ui.request_project_close_all(&project_id(scope)),
            ClientActionDisposition::DismissNavigator
        );
    } else {
        let target = snapshot().navigation.workspaces[scope.index()]
            .workspace_id
            .clone();
        assert_eq!(
            crate::workbench_client::ui::tests::workspace_close::open_confirmation(
                ui,
                Some(&target)
            ),
            ClientActionDisposition::DismissNavigator
        );
    }
}

fn draw(
    ui: &mut WorkbenchClientUi,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let view = ClientView::new(width);
    let mut receipt = None;
    terminal
        .draw(|frame| {
            receipt = Some(crate::tui_render::render_terminal_shell(
                frame,
                &view,
                locale,
                Some(ui.view()),
                theme,
                &mut HitMapBuilder::new(),
            ));
        })
        .unwrap();
    ui.after_close_draw(receipt.unwrap());
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
fn submit(ui: &mut WorkbenchClientUi, mailbox: &CommandMailbox, scope: Scope) -> CommandLease {
    open(ui, scope);
    draw(ui, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    assert!(ui.capture_confirmation_key(ClientKey::Enter));
    mailbox.try_recv().unwrap().unwrap()
}
fn available(
    operation: &WorkspaceCloseOperation,
    scope: Scope,
    closed: bool,
) -> WorkbenchWorkspaceCloseReadOutcome {
    let snapshot = operation.baseline.canonical_snapshot();
    let workspace = &snapshot.navigation.workspaces[scope.index()];
    WorkbenchWorkspaceCloseReadOutcome::Available {
        close_request_id: operation.request_id.clone(),
        workspace_id: workspace.workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
        attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
        status: if closed {
            WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(2),
            }
        } else {
            WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            }
        },
        counts: WorkbenchWorkspaceCloseCounts::default(),
        members: Vec::new(),
    }
}
fn group_receipt(
    operation: &WorkspaceCloseOperation,
    scope: Scope,
    closed: bool,
) -> WorkbenchWorkspaceGroupCloseResult {
    WorkbenchWorkspaceGroupCloseResult {
        protocol_version: ProtocolVersion,
        request_id: operation.request_id.clone(),
        operation_id: operation.request_id.clone(),
        outcome: WorkspaceGroupCloseOutcome::Available {
            project_id: project_id(scope),
            attempt: 1,
            status: if closed {
                WorkspaceGroupCloseStatus::Closed {}
            } else {
                WorkspaceGroupCloseStatus::Pending {}
            },
            members: operation
                .baseline
                .canonical_snapshot()
                .navigation
                .workspaces_in_project(Some(&project_id(scope)))
                .enumerate()
                .map(|(index, workspace)| WorkspaceGroupCloseMember {
                    workspace_id: workspace.workspace_id.clone(),
                    label: WorkbenchLabel::parse(&workspace.label).unwrap(),
                    close: (closed || index == 0).then(|| WorkspaceGroupCloseChild {
                        request_id: WorkbenchRequestId::parse(&id(900 + index as u128)).unwrap(),
                        observation: closed.then(|| WorkspaceGroupCloseObservation {
                            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                            status: WorkbenchTabCloseStatus::Closed {
                                revision: WorkbenchRevision::new(2),
                            },
                        }),
                    }),
                })
                .collect(),
        },
    }
}
fn closed_snapshot(mut snapshot: Snapshot, scope: Scope) -> Snapshot {
    let closed: Vec<_> = if scope.project() {
        snapshot
            .navigation
            .workspaces_in_project(Some(&project_id(scope)))
            .map(|workspace| workspace.workspace_id.clone())
            .collect()
    } else {
        vec![
            snapshot.navigation.workspaces[scope.index()]
                .workspace_id
                .clone(),
        ]
    };
    let contexts: Vec<_> = snapshot
        .navigation
        .workspaces
        .iter()
        .filter(|workspace| closed.contains(&workspace.workspace_id))
        .map(|workspace| workspace.terminal_context_id.clone())
        .collect();
    snapshot
        .navigation
        .workspaces
        .retain(|workspace| !closed.contains(&workspace.workspace_id));
    snapshot
        .navigation
        .workspace_order
        .retain(|workspace| !closed.contains(workspace));
    snapshot
        .terminal_contexts
        .retain(|context| !contexts.contains(&context.terminal_context_id));
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot
}
fn exercise(scope: Scope, cut: Cut) -> TestResult {
    let (mut ui, mailbox, _, snapshot) = ready(scope);
    let original_location = ui
        .view()
        .context_at(ui.view().active_context_position().unwrap() - 1)
        .unwrap()
        .location()
        .clone();
    let lease = submit(&mut ui, &mailbox, scope);
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one Close lease");
    };
    let marker = marker.clone();
    let operation = marker.workspace_close().unwrap().clone();
    let expected = operation.clone();
    let surviving = closed_snapshot(snapshot.clone(), scope);
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("fresh source check precedes mutation"));
        };
        let mut fresh = snapshot.clone();
        if cut == Cut::StaleSource {
            fresh.revision = WorkbenchRevision::new(2);
        } else if cut == Cut::StaleTarget {
            let old = fresh.navigation.workspaces[scope.index()]
                .terminal_context_id
                .clone();
            let replacement = WorkbenchTerminalContextId::parse(&id(999)).unwrap();
            fresh.navigation.workspaces[scope.index()].terminal_context_id = replacement.clone();
            fresh
                .terminal_contexts
                .iter_mut()
                .find(|context| context.terminal_context_id == old)
                .unwrap()
                .terminal_context_id = replacement;
            fresh
                .terminal_contexts
                .sort_by(|a, b| a.terminal_context_id.cmp(&b.terminal_context_id));
            fresh.revision = WorkbenchRevision::new(2);
        } else if cut == Cut::RemovedTarget {
            fresh = closed_snapshot(fresh, scope);
        }
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchSnapshotOutcome::Available { snapshot: fresh },
            }),
        )
        .map_err(|error| io::Error::other(format!("{scope:?} {cut:?} fresh snapshot: {error}")))?;
        if !cut.rejects_source() {
            let start = read_workbench_request(&mut stream).map_err(io::Error::other)?;
            if cut == Cut::RevisionRaced {
                assert_eq!(
                    start,
                    WorkbenchRequest::Command(expected.workspace_command().unwrap())
                );
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Command(WorkbenchCommandResult {
                        protocol_version: ProtocolVersion,
                        request_id: expected.request_id.clone(),
                        outcome: WorkbenchCommandOutcome::Rejected {
                            revision: WorkbenchRevision::new(2),
                            reason: WorkbenchRejectionReason::RevisionMismatch,
                        },
                    }),
                )
                .map_err(io::Error::other)?;
                let WorkbenchRequest::PublicationRead(read) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    return Err(io::Error::other("a raced Close reads the newer projection"));
                };
                assert_eq!(read.known_revision, WorkbenchRevision::new(1));
                let mut raced = snapshot.clone();
                raced.revision = WorkbenchRevision::new(2);
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                        protocol_version: ProtocolVersion,
                        request_id: read.request_id,
                        outcome: WorkbenchPublicationOutcome::Published {
                            publications: vec![WorkbenchPublicationDocument::Snapshot(raced)],
                        },
                    }),
                )
                .map_err(io::Error::other)?;
                let mut trailing = [0];
                assert_eq!(
                    stream.read(&mut trailing)?,
                    0,
                    "the executor never replays a refused command itself"
                );
                return Ok(());
            }
            let initial = if scope.project() {
                assert_eq!(
                    start,
                    WorkbenchRequest::WorkspaceGroupClose(expected.project_request().unwrap())
                );
                WorkbenchResponse::WorkspaceGroupClose(group_receipt(
                    &expected,
                    scope,
                    scope == Scope::EmptyProject,
                ))
            } else {
                assert_eq!(
                    start,
                    WorkbenchRequest::Command(expected.workspace_command().unwrap()),
                    "ordinary Close must not submit a group"
                );
                let workspace = &snapshot.navigation.workspaces[scope.index()];
                WorkbenchResponse::Command(WorkbenchCommandResult {
                    protocol_version: ProtocolVersion,
                    request_id: expected.request_id.clone(),
                    outcome: WorkbenchCommandOutcome::WorkspaceCloseAccepted {
                        close_request_id: expected.request_id.clone(),
                        workspace_id: workspace.workspace_id.clone(),
                        terminal_context_id: workspace.terminal_context_id.clone(),
                        accepted_revision: snapshot.revision,
                    },
                })
            };
            if cut == Cut::UnknownStart {
                return Ok(());
            }
            write_workbench_response(&mut stream, &initial).map_err(io::Error::other)?;
            // The submission lease ends at admission: only the canonical read follows it, and the
            // target is still published then. Settlement reads run in leases of their own.
            let WorkbenchRequest::PublicationRead(read) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other(
                    "only the canonical read follows admission",
                ));
            };
            assert_eq!(read.known_revision, WorkbenchRevision::new(1));
            if cut == Cut::AcceptedSnapshotLost {
                return Ok(());
            }
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                    protocol_version: ProtocolVersion,
                    request_id: read.request_id,
                    outcome: WorkbenchPublicationOutcome::Current {
                        revision: snapshot.revision,
                    },
                }),
            )
            .map_err(io::Error::other)?;
        }
        let mut trailing = [0];
        assert_eq!(
            stream.read(&mut trailing)?,
            0,
            "no replay, Resume, Git mutation, or extra terminal effect"
        );
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "native-close-test")?;
    let worker_mailbox = Arc::clone(&mailbox);
    let worker = thread::spawn(move || {
        let mut executor = ClientExecutor::new(client, Arc::clone(&worker_mailbox));
        let result = executor.run_refresh(&lease, marker);
        let evidence = executor.pending_workspace_close.take();
        if result.is_ok() {
            worker_mailbox.finish_lease(lease).unwrap();
        }
        (result, evidence)
    });
    let deadline = Instant::now() + Duration::from_secs(4);
    while !worker.is_finished() && Instant::now() < deadline {
        if cut == Cut::UiDeliveryLost {
            if mailbox.cells().0 == "event" {
                mailbox.close(CarrierFailure::Transport);
            }
        } else {
            ui.tick();
        }
        thread::sleep(Duration::from_millis(1));
    }
    if !worker.is_finished() {
        mailbox.close(CarrierFailure::Deadline);
    }
    let (result, evidence) = worker.join().unwrap();
    server.finish()?;
    if cut.rejects_source() || matches!(cut, Cut::RevisionRaced | Cut::Complete) {
        assert_eq!(result, Ok(()), "{scope:?} {cut:?}");
        assert!(evidence.is_none());
        match cut {
            // A newer projection is not a refusal: the confirmed scope is unchanged, so it is
            // submitted once more against that projection, with no refusal status meanwhile.
            Cut::StaleSource | Cut::RevisionRaced => {
                assert!(
                    ui.view().workspace_close_status().is_none(),
                    "{scope:?} {cut:?}: an outdated revision is not shown as a refusal"
                );
                let retry = mailbox
                    .try_recv()
                    .unwrap()
                    .unwrap_or_else(|| panic!("{scope:?} {cut:?}: one automatic retry"));
                let QueuedCommand::Refresh(marker) = retry.record() else {
                    panic!("{scope:?} {cut:?}: the retry is a Close refresh");
                };
                let retried = marker.workspace_close().unwrap();
                assert_eq!(
                    mailbox
                        .workspace_close_worker_loss()
                        .map(|evidence| evidence.operation),
                    Some(retried.clone()),
                    "only the retry is admitted; nothing of the refused attempt remains"
                );
                assert_eq!(retried.target, operation.target);
                assert_eq!(retried.baseline.revision(), WorkbenchRevision::new(2));
                assert_ne!(retried.request_id, operation.request_id);
                if let Some(command) = retried.workspace_command() {
                    assert_eq!(
                        command.action,
                        operation.workspace_command().unwrap().action,
                        "the retry keeps the confirmed destructive action"
                    );
                    assert_eq!(command.expected_revision, WorkbenchRevision::new(2));
                }
            }
            // A changed or removed scope is never resubmitted; the user sees why.
            Cut::StaleTarget | Cut::RemovedTarget => {
                assert!(
                    ui.view().workspace_close_status().is_none(),
                    "{scope:?} {cut:?}: no refusal status beside the notice"
                );
                assert_eq!(
                    ui.view().notice(),
                    Some(crate::localization::WorkbenchClientNotice::WorkspaceChanged)
                );
                assert!(mailbox.try_recv().unwrap().is_none());
            }
            // Admission is announced at once, after the landing when the close takes the
            // current Workspace, while settlement continues behind it.
            _ => assert_eq!(
                ui.view()
                    .workspace_close_status()
                    .unwrap_or_else(|| panic!(
                        "{scope:?} {cut:?}: close status must retain its admitted result"
                    ))
                    .status(),
                WorkspaceCloseStatus::Closed
            ),
        }
        let active = ui
            .view()
            .context_at(ui.view().active_context_position().unwrap() - 1)
            .unwrap()
            .location()
            .clone();
        let surviving_locations: Vec<_> = surviving
            .navigation
            .ordered_workspaces()
            .unwrap()
            .iter()
            .map(|(_, workspace)| ClientContextLocation::Workspace {
                workspace_id: workspace.workspace_id.clone(),
                terminal_context_id: workspace.terminal_context_id.clone(),
            })
            .collect();
        let removes_current = !scope.project() && scope != Scope::BackgroundWorkspace;
        if removes_current && cut == Cut::Complete {
            assert!(
                surviving_locations.contains(&active),
                "{scope:?} {cut:?}: a removed current Workspace lands on a surviving Workspace"
            );
            assert_eq!(
                ui.current_location_for_test().as_ref(),
                Some(&active),
                "{scope:?} {cut:?}: the neighbour is installed before the close settles"
            );
        } else {
            assert_eq!(
                active, original_location,
                "an unrelated close must not steal focus"
            );
        }
        if cut == Cut::Complete {
            assert_eq!(
                ui.view().context_count(),
                surviving.navigation.workspaces.len()
            );
            assert!(
                (0..ui.view().context_count()).all(|index| matches!(
                    ui.view().context_at(index).unwrap().location(),
                    ClientContextLocation::Workspace { .. }
                )),
                "the empty compatibility slot is not a navigation row"
            );
            // Settlement continues behind the landing, followed by reads alone.
            let follow = mailbox.try_recv().unwrap();
            if scope == Scope::EmptyProject {
                assert!(follow.is_none(), "a close admitted as Closed needs no read");
            } else {
                assert!(
                    matches!(
                        follow.as_ref().map(CommandLease::record),
                        Some(QueuedCommand::WorkspaceCloseRead(_))
                    ),
                    "{scope:?}: an admitted close is followed by a read, never resubmitted"
                );
            }
        }
        if !matches!(cut, Cut::StaleSource | Cut::RevisionRaced) {
            assert!(mailbox.workspace_close_worker_loss().is_none());
        }
    } else {
        assert!(result.is_err(), "{scope:?} {cut:?}");
        let evidence = evidence.expect("admission and known facts survive worker exit");
        assert_eq!(
            mailbox.workspace_close_worker_loss(),
            Some(evidence.clone())
        );
        match cut {
            Cut::UnknownStart => assert_eq!(evidence.result, WorkspaceCloseResult::Unknown),
            Cut::AcceptedSnapshotLost | Cut::UiDeliveryLost => {
                assert!(evidence.result.is_pending(), "{scope:?} {cut:?}");
            }
            _ => unreachable!(),
        }
        assert!(ui.finalize_workspace_close_worker(Some(evidence)));
        assert_eq!(
            ui.view().workspace_close_status().unwrap().operation_id(),
            Some(&operation.request_id)
        );
        let cursors = mailbox.cursors();
        ui.apply_client_action(ClientAction::RequestWorkspaceClose);
        ui.request_project_close_all(&project_id(Scope::Project));
        assert_eq!(mailbox.cursors(), cursors);
        if cut != Cut::UiDeliveryLost {
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
    Ok(())
}
#[test]
fn background_workspace_close_carrier_validates_exact_target_and_preserves_origin_without_replay()
-> TestResult {
    for cut in [
        Cut::StaleTarget,
        Cut::RemovedTarget,
        Cut::AcceptedSnapshotLost,
        Cut::UiDeliveryLost,
        Cut::Complete,
    ] {
        exercise(Scope::BackgroundWorkspace, cut)?;
    }
    Ok(())
}
#[test]
fn workspace_close_carrier_single_scope_and_every_failure_boundary_preserve_truth_without_replay()
-> TestResult {
    for cut in [
        Cut::StaleSource,
        Cut::RevisionRaced,
        Cut::UnknownStart,
        Cut::AcceptedSnapshotLost,
        Cut::UiDeliveryLost,
        Cut::Complete,
    ] {
        exercise(Scope::Workspace, cut)?;
    }
    exercise(Scope::UngroupedWorkspace, Cut::Complete)
}
#[test]
fn project_close_all_carrier_captures_members_retains_empty_metadata_and_never_steals_other_focus()
-> TestResult {
    for cut in [
        Cut::StaleSource,
        Cut::UnknownStart,
        Cut::AcceptedSnapshotLost,
        Cut::UiDeliveryLost,
        Cut::Complete,
    ] {
        exercise(Scope::Project, cut)?;
    }
    exercise(Scope::EmptyProject, Cut::Complete)
}
/// Ticks the Client until the worker reports one more finished lease and the Client has applied
/// everything that lease published, then once more so its schedulers see the result.
fn tick_until_finished(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    finished: &mpsc::Receiver<()>,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut done = false;
    while Instant::now() < deadline {
        ui.tick();
        done = done || finished.try_recv().is_ok();
        if done && mailbox.cells().0 == "empty" {
            ui.tick();
            return true;
        }
        thread::sleep(Duration::from_millis(1));
    }
    false
}
fn displayed(ui: &WorkbenchClientUi) -> Vec<ClientContextLocation> {
    let view = ui.view();
    (0..view.context_count())
        .map(|index| view.context_at(index).unwrap().location().clone())
        .collect()
}
#[test]
fn accepted_current_close_lands_on_the_neighbour_before_settlement_and_is_never_resubmitted()
-> TestResult {
    let elapsed = Arc::new(AtomicU64::new(0));
    let (mut ui, mailbox, _, snapshot) = ready_at(Scope::Workspace, Arc::clone(&elapsed));
    let location = |workspace: &Workspace| ClientContextLocation::Workspace {
        workspace_id: workspace.workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    };
    let closing = snapshot.navigation.workspaces[0].clone();
    let closing_location = location(&closing);
    let neighbour = location(&snapshot.navigation.workspaces[1]);
    let lease = submit(&mut ui, &mailbox, Scope::Workspace);
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one Close lease");
    };
    let marker = marker.clone();
    let expected = marker.workspace_close().unwrap().clone();
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("fresh source check precedes mutation"));
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(expected.workspace_command().unwrap())
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Command(WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: expected.request_id.clone(),
                outcome: WorkbenchCommandOutcome::WorkspaceCloseAccepted {
                    close_request_id: expected.request_id.clone(),
                    workspace_id: closing.workspace_id.clone(),
                    terminal_context_id: closing.terminal_context_id.clone(),
                    accepted_revision: snapshot.revision,
                },
            }),
        )
        .map_err(io::Error::other)?;
        let WorkbenchRequest::PublicationRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other(
                "only the canonical read follows admission",
            ));
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchPublicationOutcome::Current {
                    revision: snapshot.revision,
                },
            }),
        )
        .map_err(io::Error::other)?;
        // The close may already have been delivered, so settlement is followed by reads alone.
        for status in [
            WorkbenchTabCloseStatus::Closing {
                phase: WorkbenchTabClosePhase::Queued,
            },
            WorkbenchTabCloseStatus::CleanupBlocked {
                reason: WorkbenchTabCloseBlockedReason::HostRefused,
                resumable: false,
            },
        ] {
            let WorkbenchRequest::WorkspaceCloseRead(read) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("only a status read follows admission"));
            };
            assert_eq!(read.workspace_id, closing.workspace_id);
            let mut outcome = available(&expected, Scope::Workspace, false);
            if let WorkbenchWorkspaceCloseReadOutcome::Available {
                status: observed, ..
            } = &mut outcome
            {
                *observed = status;
            }
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
                    protocol_version: ProtocolVersion,
                    request_id: read.request_id,
                    outcome,
                }),
            )
            .map_err(io::Error::other)?;
        }
        let mut trailing = [0];
        assert_eq!(
            stream.read(&mut trailing)?,
            0,
            "the close is never submitted again"
        );
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "native-close-test")?;
    let worker_mailbox = Arc::clone(&mailbox);
    let (report, finished) = mpsc::channel();
    let worker = thread::spawn(move || -> Result<(), CarrierFailure> {
        let mut executor = ClientExecutor::new(client, Arc::clone(&worker_mailbox));
        executor.run_refresh(&lease, marker)?;
        worker_mailbox
            .finish_lease(lease)
            .map_err(|closed| closed.failure())?;
        report.send(()).ok();
        for _ in 0..2 {
            let lease = worker_mailbox.recv().map_err(|closed| closed.failure())?;
            let QueuedCommand::WorkspaceCloseRead(read) = lease.record() else {
                return Err(CarrierFailure::Protocol);
            };
            let read = read.clone();
            executor.run_workspace_close_read(&lease, &read)?;
            worker_mailbox
                .finish_lease(lease)
                .map_err(|closed| closed.failure())?;
            report.send(()).ok();
        }
        Ok(())
    });

    assert!(tick_until_finished(&mut ui, &mailbox, &finished));
    // Accepted, not settled: the row has left, the neighbour is installed and receives input,
    // and the Closed notice shows now.
    assert_eq!(ui.current_location_for_test().as_ref(), Some(&neighbour));
    assert!(!displayed(&ui).contains(&closing_location));
    assert_eq!(
        ui.view().workspace_close_status().unwrap().status(),
        WorkspaceCloseStatus::Closed
    );
    // The close key pressed again at once does not close the neighbour it never saw.
    ui.apply_client_action(ClientAction::RequestWorkspaceClose);
    assert!(ui.view().workspace_close_confirmation().is_none());
    assert_eq!(
        ui.view().notice(),
        Some(crate::localization::WorkbenchClientNotice::WorkspaceChanged)
    );

    // Still closing: the next read waits for the follow interval.
    assert!(tick_until_finished(&mut ui, &mailbox, &finished));
    assert_eq!(ui.current_location_for_test().as_ref(), Some(&neighbour));
    assert!(!displayed(&ui).contains(&closing_location));
    elapsed.fetch_add(50, Ordering::SeqCst);

    // A later failure returns the row and its notice; the selection stays on the neighbour.
    assert!(tick_until_finished(&mut ui, &mailbox, &finished));
    assert_eq!(ui.current_location_for_test().as_ref(), Some(&neighbour));
    assert!(displayed(&ui).contains(&closing_location));
    assert_eq!(
        ui.view().workspace_close_status().unwrap().status(),
        WorkspaceCloseStatus::Failed
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(worker.join().unwrap(), Ok(()));
    server.finish()?;
    Ok(())
}
#[test]
fn workspace_and_project_close_shell_matrix_discloses_exact_scope_and_requires_a_complete_draw() {
    for scope in [Scope::Workspace, Scope::Project, Scope::EmptyProject] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                for (width, height, complete) in [
                    (80, 24, true),
                    (120, 40, true),
                    (60, 24, true),
                    (50, 30, true),
                    (22, 8, false),
                    (19, 4, false),
                ] {
                    let (mut ui, mailbox, _, _) = ready(scope);
                    let cursors = mailbox.cursors();
                    open(&mut ui, scope);
                    let prompt = ui.view().workspace_close_confirmation().unwrap();
                    assert_eq!(
                        prompt.project_member_count(),
                        match scope {
                            Scope::Project => Some(2),
                            Scope::EmptyProject => Some(0),
                            _ => None,
                        }
                    );
                    let text = draw(&mut ui, locale, &theme, width, height);
                    let normalized: String = text.chars().filter(|c| c.is_alphanumeric()).collect();
                    if complete {
                        let warning =
                            locale.workspace_close_consequence(scope != Scope::Workspace)[0];
                        let expected: String =
                            warning.chars().filter(|c| c.is_alphanumeric()).collect();
                        assert!(
                            normalized.contains(&expected),
                            "{scope:?} {locale:?} {width}x{height}\n{text}"
                        );
                        assert!(ui.view().workspace_close_confirmation().is_some());
                        ui.capture_confirmation_key(ClientKey::Plain('Y'));
                        assert_eq!(mailbox.cursors(), cursors);
                        assert!(mailbox.try_recv().unwrap().is_none());
                        ui.capture_confirmation_key(ClientKey::Enter);
                        assert!(mailbox.try_recv().unwrap().is_some());
                    } else {
                        assert!(ui.view().workspace_close_confirmation().is_none());
                        assert_eq!(mailbox.cursors(), cursors);
                        assert!(mailbox.try_recv().unwrap().is_none());
                    }
                }
            }
        }
    }
}
