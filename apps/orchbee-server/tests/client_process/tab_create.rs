use super::{
    FIRST_WORKER, PROCESS_TIMEOUT, RunningServer, TestResult, connect_bounded_client,
    connect_worker,
};
use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, CapabilityNodeMessage, HostCreationId, HostEpoch as WorkerHostEpoch,
    HostLaunchAccepted, HostLaunchState, HostTerminalId, HostTerminalIdentity, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchResult, TerminalViewerAction as WorkerViewerAction,
    TerminalViewerOutcome as WorkerViewerOutcome, TerminalViewerResult as WorkerViewerResult,
    read_message as read_obcn, write_message as write_obcn,
};
use orchbee_client_server_protocol::{
    ClientMessage, Geometry, OpenOutcome, RequestId, ServerMessage, SiteId as ClientSiteId,
    TerminalId as ClientTerminalId, TerminalOpenRequest, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerRequest, WorkerId as ClientWorkerId, read_server_message,
    write_client_message,
};
use orchbee_workbench_protocol::{
    PanelNode, Presentation, PresentationBinding, ProtocolVersion, Snapshot, SnapshotReadIntent,
    Tab, TerminalContext, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchTerminalGeometry, read_workbench_response, write_workbench_request,
};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

const SECOND_TAB_LABEL: &str = "Process-created Tab";

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommittedTab {
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
}

fn deadline() -> Instant {
    Instant::now() + PROCESS_TIMEOUT
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "process witness deadline"))
}

fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "process witness deadline",
        ));
    }
    Ok(())
}

struct DeadlineStream<'a> {
    stream: &'a mut UnixStream,
    deadline: Instant,
}

impl<'a> DeadlineStream<'a> {
    fn new(stream: &'a mut UnixStream, deadline: Instant) -> io::Result<Self> {
        check_deadline(deadline)?;
        Ok(Self { stream, deadline })
    }

    fn finish<T>(&self, result: io::Result<T>) -> io::Result<T> {
        check_deadline(self.deadline)?;
        result
    }
}

impl Read for DeadlineStream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        let result = self.stream.read(buffer);
        self.finish(result)
    }
}

impl Write for DeadlineStream<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        let result = self.stream.write(buffer);
        self.finish(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        let result = self.stream.flush();
        self.finish(result)
    }
}

fn require_before(deadline: Instant) -> TestResult {
    check_deadline(deadline)?;
    Ok(())
}

fn client_id(value: u128) -> TestResult<RequestId> {
    Ok(RequestId::parse(&format!("{value:032x}"))?)
}

fn workbench_id(value: u128) -> TestResult<WorkbenchRequestId> {
    Ok(WorkbenchRequestId::parse(&format!("{value:032x}"))?)
}

fn client_geometry() -> TestResult<Geometry> {
    Ok(Geometry::new(80, 24, 8, 16)?)
}

fn workbench_geometry() -> TestResult<WorkbenchTerminalGeometry> {
    Ok(WorkbenchTerminalGeometry::new(100, 30, 9, 18)?)
}

fn host_identity(seed: u8) -> TestResult<HostTerminalIdentity> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([seed; 16])?,
        WorkerHostEpoch::from_bytes([seed.checked_add(1).ok_or("identity seed overflow")?; 16])?,
        HostTerminalId::from_bytes([seed.checked_add(2).ok_or("identity seed overflow")?; 16])?,
    ))
}

fn hex_identity(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}

fn read_worker_message(
    worker: &mut UnixStream,
    deadline: Instant,
) -> TestResult<CapabilityNodeMessage> {
    let mut stream = DeadlineStream::new(worker, deadline)?;
    Ok(read_obcn(&mut stream)?)
}

fn write_worker_message(
    worker: &mut UnixStream,
    message: &CapabilityNodeMessage,
    deadline: Instant,
) -> TestResult {
    let mut stream = DeadlineStream::new(worker, deadline)?;
    write_obcn(&mut stream, message)?;
    Ok(())
}

fn write_obcs(client: &mut UnixStream, message: &ClientMessage, deadline: Instant) -> TestResult {
    let mut stream = DeadlineStream::new(client, deadline)?;
    write_client_message(&mut stream, message)?;
    Ok(())
}

fn read_obcs(client: &mut UnixStream, deadline: Instant) -> TestResult<ServerMessage> {
    let mut stream = DeadlineStream::new(client, deadline)?;
    Ok(read_server_message(&mut stream)?)
}

fn write_obwp(
    client: &mut UnixStream,
    request: &WorkbenchRequest,
    deadline: Instant,
) -> TestResult {
    let mut stream = DeadlineStream::new(client, deadline)?;
    write_workbench_request(&mut stream, request)?;
    Ok(())
}

fn read_snapshot(
    client: &mut UnixStream,
    request_value: u128,
    deadline: Instant,
) -> TestResult<Snapshot> {
    let request_id = workbench_id(request_value)?;
    write_obwp(
        client,
        &WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
        }),
        deadline,
    )?;
    let mut stream = DeadlineStream::new(client, deadline)?;
    match read_workbench_response(&mut stream)? {
        WorkbenchResponse::Snapshot(result) => {
            assert_eq!(result.request_id, request_id);
            match result.outcome {
                WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
                outcome => {
                    Err(io::Error::other(format!("snapshot was unavailable: {outcome:?}")).into())
                }
            }
        }
        response => Err(io::Error::other(format!(
            "expected snapshot response, received {response:?}"
        ))
        .into()),
    }
}

fn read_command_result(
    client: &mut UnixStream,
    request_id: &WorkbenchRequestId,
    deadline: Instant,
) -> TestResult<WorkbenchCommandResult> {
    let mut stream = DeadlineStream::new(client, deadline)?;
    let WorkbenchResponse::Command(result) = read_workbench_response(&mut stream)? else {
        return Err(io::Error::other("expected Workbench command response").into());
    };
    assert_eq!(&result.request_id, request_id);
    Ok(result)
}

fn exchange_command(
    client: &mut UnixStream,
    request: &WorkbenchRequest,
    request_id: &WorkbenchRequestId,
    deadline: Instant,
) -> TestResult<WorkbenchCommandResult> {
    write_obwp(client, request, deadline)?;
    read_command_result(client, request_id, deadline)
}

fn command(
    request_id: WorkbenchRequestId,
    snapshot: &Snapshot,
    action: WorkbenchAction,
) -> WorkbenchRequest {
    WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id,
        site_id: snapshot.site_id.clone(),
        expected_revision: snapshot.revision,
        action,
    })
}

fn standalone_context(snapshot: &Snapshot) -> TestResult<&TerminalContext> {
    snapshot
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == snapshot.navigation.standalone_terminal_context_id
        })
        .ok_or_else(|| io::Error::other("missing standalone Terminal Context").into())
}

fn primary_presentation(tab: &Tab) -> TestResult<(&Presentation, &WorkbenchPanelId)> {
    let PanelNode::Panel {
        child_presentations,
        panel_id,
        primary_presentation,
    } = &tab.root
    else {
        return Err(io::Error::other("standalone Tab root was not a Panel").into());
    };
    assert!(child_presentations.is_empty());
    Ok((primary_presentation, panel_id))
}

fn assert_simple_tab<'a>(
    tab: &'a Tab,
    label: &str,
) -> TestResult<(&'a Presentation, &'a WorkbenchPanelId)> {
    assert!(tab.group_id.is_none());
    assert!(!tab.pinned);
    assert_eq!(tab.label, label);
    let (presentation, panel_id) = primary_presentation(tab)?;
    assert_eq!(presentation.label, label);
    Ok((presentation, panel_id))
}

fn binding_matches_launch(binding: &PresentationBinding, launch: &AuthorizedTerminalLaunch) {
    assert_eq!(
        binding.worker_id.as_str(),
        hex_identity(launch.authorization().worker_id().as_bytes())
    );
    assert_eq!(
        binding.terminal_id.as_str(),
        hex_identity(launch.authorization().terminal_id().as_bytes())
    );
}

fn assert_launch_geometry(
    launch: &AuthorizedTerminalLaunch,
    expected: WorkbenchTerminalGeometry,
) -> TestResult {
    let TerminalLaunchMode::Immediate(actual) = launch.mode() else {
        return Err(io::Error::other("tab.create did not dispatch an immediate Launch").into());
    };
    assert_eq!(actual.columns(), expected.columns());
    assert_eq!(actual.rows(), expected.rows());
    assert_eq!(actual.cell_pixel_width(), expected.cell_pixel_width());
    assert_eq!(actual.cell_pixel_height(), expected.cell_pixel_height());
    Ok(())
}

fn pending_tab(outcome: &WorkbenchCommandOutcome) -> TestResult<CommittedTab> {
    let WorkbenchCommandOutcome::TabCreatePending {
        revision,
        terminal_context_id,
        tab_id,
        panel_id,
        presentation_id,
    } = outcome
    else {
        return Err(io::Error::other(format!(
            "expected pending tab.create replay, received {outcome:?}"
        ))
        .into());
    };
    Ok(CommittedTab {
        revision: *revision,
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        panel_id: panel_id.clone(),
        presentation_id: presentation_id.clone(),
    })
}

fn final_tab(outcome: &WorkbenchCommandOutcome) -> TestResult<CommittedTab> {
    let WorkbenchCommandOutcome::TabCreated {
        revision,
        terminal_context_id,
        tab_id,
        panel_id,
        presentation_id,
        terminal_outcome,
    } = outcome
    else {
        return Err(io::Error::other(format!(
            "original tab.create did not receive a final result: {outcome:?}"
        ))
        .into());
    };
    assert_eq!(
        terminal_outcome,
        &orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started {
            geometry: workbench_geometry()?,
        }
    );
    Ok(CommittedTab {
        revision: *revision,
        terminal_context_id: terminal_context_id.clone(),
        tab_id: tab_id.clone(),
        panel_id: panel_id.clone(),
        presentation_id: presentation_id.clone(),
    })
}

#[test]
fn tab_create_commits_before_parked_launch_and_another_client_can_read_navigate_and_replay()
-> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut viewer, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut workbench_a, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut workbench_b, _) = connect_bounded_client(server.client_endpoint())?;

    let setup_deadline = deadline();
    let open_request_id = client_id(1)?;
    write_obcs(
        &mut viewer,
        &ClientMessage::TerminalOpen(TerminalOpenRequest::new(
            open_request_id.clone(),
            client_geometry()?,
        )),
        setup_deadline,
    )?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(first_launch) =
        read_worker_message(&mut worker, setup_deadline)?
    else {
        return Err(io::Error::other("missing first standalone Launch").into());
    };
    let first_geometry = match first_launch.mode() {
        TerminalLaunchMode::Immediate(geometry) => geometry,
        TerminalLaunchMode::Deferred => {
            return Err(io::Error::other("standalone Open dispatched a deferred Launch").into());
        }
    };
    let first_host = host_identity(3)?;
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            first_launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                first_host,
                HostLaunchState::Started(first_geometry),
            )),
        )?),
        setup_deadline,
    )?;
    let ServerMessage::TerminalOpenResult(opened) = read_obcs(&mut viewer, setup_deadline)? else {
        return Err(io::Error::other("missing standalone Open result").into());
    };
    assert_eq!(opened.request_id(), &open_request_id);
    assert!(matches!(opened.outcome(), OpenOutcome::Opened { .. }));

    let initial = read_snapshot(&mut workbench_a, 101, setup_deadline)?;
    let initial_context = standalone_context(&initial)?;
    assert!(initial_context.groups.is_empty());
    assert_eq!(initial_context.tabs.len(), 1);
    let initial_tab = initial_context.tabs[0].clone();
    let (first_presentation, _) = assert_simple_tab(&initial_tab, "1")?;
    let first_binding = first_presentation
        .binding
        .as_ref()
        .ok_or_else(|| io::Error::other("first standalone Tab was unbound"))?;
    binding_matches_launch(first_binding, &first_launch);
    require_before(setup_deadline)?;

    let attach_deadline = deadline();
    let attach_request_id = client_id(2)?;
    write_obcs(
        &mut viewer,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            attach_request_id.clone(),
            TerminalViewerAction::AttachBinding {
                site_id: ClientSiteId::parse(initial.site_id.as_str())?,
                worker_id: ClientWorkerId::parse(first_binding.worker_id.as_str())?,
                terminal_id: ClientTerminalId::parse(first_binding.terminal_id.as_str())?,
            },
        )),
        attach_deadline,
    )?;
    let CapabilityNodeMessage::TerminalViewerRequest(attached) =
        read_worker_message(&mut worker, attach_deadline)?
    else {
        return Err(io::Error::other("missing Worker viewer Attach").into());
    };
    assert_eq!(attached.worker_id, first_launch.authorization().worker_id());
    assert_eq!(
        attached.worker_generation,
        first_launch.authorization().worker_generation()
    );
    assert_eq!(
        attached.action,
        WorkerViewerAction::Attach {
            host_epoch: first_host.host_epoch(),
            terminal_id: first_host.terminal_id(),
        }
    );
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
            request_id: attached.request_id,
            attachment_id: attached.attachment_id,
            outcome: WorkerViewerOutcome::Attached,
        }),
        attach_deadline,
    )?;
    let ServerMessage::TerminalViewerResult(attach_result) =
        read_obcs(&mut viewer, attach_deadline)?
    else {
        return Err(io::Error::other("missing Client viewer Attach result").into());
    };
    assert_eq!(attach_result.request_id(), &attach_request_id);
    let TerminalViewerOutcome::Attached {
        attachment_id,
        host_epoch,
    } = attach_result.outcome()
    else {
        return Err(io::Error::other("Client viewer Attach was not accepted").into());
    };
    assert_eq!(
        attachment_id.as_str(),
        hex_identity(attached.attachment_id.as_bytes())
    );
    assert_eq!(
        host_epoch.as_str(),
        hex_identity(first_host.host_epoch().as_bytes())
    );
    require_before(attach_deadline)?;

    let create_request_id = workbench_id(102)?;
    let tab_geometry = workbench_geometry()?;
    let create_request = command(
        create_request_id.clone(),
        &initial,
        WorkbenchAction::TabCreate {
            terminal_context_id: initial_context.terminal_context_id.clone(),
            label: WorkbenchLabel::parse(SECOND_TAB_LABEL)?,
            geometry: tab_geometry,
        },
    );
    let parked_deadline = deadline();
    write_obwp(&mut workbench_a, &create_request, parked_deadline)?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(second_launch) =
        read_worker_message(&mut worker, parked_deadline)?
    else {
        return Err(io::Error::other("missing tab.create Launch").into());
    };
    assert_launch_geometry(&second_launch, tab_geometry)?;
    assert!(second_launch.cwd().is_none());
    assert_eq!(
        second_launch.authorization().worker_id(),
        first_launch.authorization().worker_id()
    );
    assert_eq!(
        second_launch.authorization().worker_generation(),
        first_launch.authorization().worker_generation()
    );
    assert_ne!(
        second_launch.authorization().terminal_id(),
        first_launch.authorization().terminal_id()
    );
    assert_ne!(
        second_launch.authorization().operation_id(),
        first_launch.authorization().operation_id()
    );

    // Receiving the parked Launch is the causal observation that membership committed.
    // Only independent OBWP traffic proceeds while this same-Worker request is in flight.
    let committed = read_snapshot(&mut workbench_b, 201, parked_deadline)?;
    assert_eq!(committed.revision.value(), initial.revision.value() + 1);
    let committed_context = standalone_context(&committed)?;
    assert!(committed_context.groups.is_empty());
    assert_eq!(
        committed_context.terminal_context_id,
        initial_context.terminal_context_id
    );
    assert_eq!(committed_context.tabs.len(), 2);
    assert_eq!(committed_context.tabs[0], initial_tab);
    let second_tab = &committed_context.tabs[1];
    let (second_presentation, second_panel_id) = assert_simple_tab(second_tab, SECOND_TAB_LABEL)?;
    let second_binding = second_presentation
        .binding
        .as_ref()
        .ok_or_else(|| io::Error::other("second standalone Tab was unbound"))?;
    binding_matches_launch(second_binding, &second_launch);

    let pending = exchange_command(
        &mut workbench_b,
        &create_request,
        &create_request_id,
        parked_deadline,
    )?;
    let committed_tab = pending_tab(&pending.outcome)?;
    assert_eq!(committed_tab.revision, committed.revision);
    assert_eq!(
        committed_tab.terminal_context_id,
        committed_context.terminal_context_id
    );
    assert_eq!(committed_tab.tab_id, second_tab.tab_id);
    assert_eq!(committed_tab.panel_id, *second_panel_id);
    assert_eq!(
        committed_tab.presentation_id,
        second_presentation.presentation_id
    );

    let project_request_id = workbench_id(202)?;
    let project_request = command(
        project_request_id.clone(),
        &committed,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("While tab Launch is parked")?,
        },
    );
    let project = exchange_command(
        &mut workbench_b,
        &project_request,
        &project_request_id,
        parked_deadline,
    )?;
    let WorkbenchCommandOutcome::ProjectCreated {
        revision: navigation_revision,
        project_id,
    } = &project.outcome
    else {
        return Err(io::Error::other("ProjectCreate did not complete during parked Launch").into());
    };
    assert_eq!(navigation_revision.value(), committed.revision.value() + 1);
    let after_navigation = read_snapshot(&mut workbench_b, 203, parked_deadline)?;
    assert_eq!(after_navigation.revision, *navigation_revision);
    assert_eq!(standalone_context(&after_navigation)?, committed_context);
    assert!(after_navigation.navigation.projects.iter().any(|project| {
        &project.project_id == project_id && project.label == "While tab Launch is parked"
    }));
    require_before(parked_deadline)?;

    let settlement_deadline = deadline();
    let second_host = host_identity(6)?;
    assert_ne!(second_host, first_host);
    let TerminalLaunchMode::Immediate(second_geometry) = second_launch.mode() else {
        unreachable!("tab.create geometry was checked above")
    };
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            second_launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                second_host,
                HostLaunchState::Started(second_geometry),
            )),
        )?),
        settlement_deadline,
    )?;
    let original = read_command_result(&mut workbench_a, &create_request_id, settlement_deadline)?;
    assert_eq!(final_tab(&original.outcome)?, committed_tab);
    let replayed = exchange_command(
        &mut workbench_b,
        &create_request,
        &create_request_id,
        settlement_deadline,
    )?;
    assert_eq!(replayed, original);
    assert_eq!(
        read_snapshot(&mut workbench_b, 204, settlement_deadline)?,
        after_navigation
    );

    // A known read-only request forms a positive FIFO boundary. If replay dispatched a
    // third Launch, it is the next Worker record and this exact match fails.
    let poll_request_id = client_id(3)?;
    write_obcs(
        &mut viewer,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            poll_request_id.clone(),
            TerminalViewerAction::Poll {},
        )),
        settlement_deadline,
    )?;
    let next = read_worker_message(&mut worker, settlement_deadline)?;
    let CapabilityNodeMessage::TerminalViewerRequest(polled) = next else {
        return Err(io::Error::other(format!(
            "expected viewer Poll after final replay, received {next:?}"
        ))
        .into());
    };
    assert_eq!(polled.action, WorkerViewerAction::Poll);
    assert_eq!(polled.worker_id, attached.worker_id);
    assert_eq!(polled.worker_generation, attached.worker_generation);
    assert_eq!(polled.attachment_id, attached.attachment_id);
    assert_ne!(polled.request_id, attached.request_id);
    write_worker_message(
        &mut worker,
        &CapabilityNodeMessage::TerminalViewerResult(WorkerViewerResult {
            request_id: polled.request_id,
            attachment_id: polled.attachment_id,
            outcome: WorkerViewerOutcome::Empty,
        }),
        settlement_deadline,
    )?;
    let ServerMessage::TerminalViewerResult(poll_result) =
        read_obcs(&mut viewer, settlement_deadline)?
    else {
        return Err(io::Error::other("missing Client viewer Poll result").into());
    };
    assert_eq!(poll_result.request_id(), &poll_request_id);
    assert_eq!(poll_result.outcome(), &TerminalViewerOutcome::Empty {});
    require_before(settlement_deadline)?;
    Ok(())
}

#[path = "tab_create/tab_move.rs"]
mod tab_move;
#[path = "tab_create/tab_rename.rs"]
mod tab_rename;
