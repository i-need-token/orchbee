use super::*;
use nix::poll::{PollFd, PollFlags, poll};
use orchbee_client_server_protocol::{
    SiteId as ClientSiteId, TerminalId as ClientTerminalId, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerRequest, ViewerUnavailableReason,
    WorkerId as ClientWorkerId,
};
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, ProtocolVersion, Snapshot, SnapshotReadIntent, TerminalContext,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response,
    write_workbench_request,
};
use std::{num::NonZeroU64, os::fd::AsFd};

pub(super) fn tab_request(snapshot: &Snapshot, value: u128) -> TestResult<WorkbenchRequest> {
    Ok(WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{value:032x}"))?,
        site_id: snapshot.site_id.clone(),
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            label: WorkbenchLabel::parse("1")?,
            geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    }))
}

pub(super) fn tab_outcome(client: &mut UnixStream) -> TestResult<WorkbenchCommandOutcome> {
    let WorkbenchResponse::Command(result) = read_workbench_response(client)? else {
        return Err("expected Tab command result".into());
    };
    Ok(result.outcome)
}

pub(super) fn read_snapshot(stream: &mut UnixStream, value: u128) -> TestResult<Snapshot> {
    write_workbench_request(
        stream,
        &WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{value:032x}"))?,
        }),
    )?;
    match read_workbench_response(stream)? {
        WorkbenchResponse::Snapshot(result) => match result.outcome {
            WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
            outcome => Err(io::Error::other(format!("snapshot unavailable: {outcome:?}")).into()),
        },
        response => Err(io::Error::other(format!("wrong response: {response:?}")).into()),
    }
}

fn navigate(
    stream: &mut UnixStream,
    value: u128,
    snapshot: &Snapshot,
    action: WorkbenchAction,
) -> TestResult<WorkbenchCommandOutcome> {
    let request_id = WorkbenchRequestId::parse(&format!("{value:032x}"))?;
    write_workbench_request(
        stream,
        &WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            site_id: snapshot.site_id.clone(),
            expected_revision: snapshot.revision,
            action,
        }),
    )?;
    match read_workbench_response(stream)? {
        WorkbenchResponse::Command(result) => {
            assert_eq!(result.request_id, request_id);
            Ok(result.outcome)
        }
        response => Err(io::Error::other(format!("wrong response: {response:?}")).into()),
    }
}

fn workspace_context(snapshot: &Snapshot) -> TestResult<&TerminalContext> {
    snapshot
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == snapshot.navigation.workspaces[0].terminal_context_id
        })
        .ok_or_else(|| io::Error::other("missing workspace terminal context").into())
}

pub(super) fn workspace_binding(snapshot: &Snapshot) -> TestResult<PresentationBinding> {
    let context = workspace_context(snapshot)?;
    assert!(context.groups.is_empty());
    assert_eq!(context.tabs.len(), 1);
    let tab = context
        .tabs
        .first()
        .ok_or_else(|| io::Error::other("missing workspace Tab"))?;
    assert!(tab.group_id.is_none());
    assert!(!tab.pinned);
    assert_eq!(tab.label, "1");
    let PanelNode::Panel {
        child_presentations,
        primary_presentation,
        ..
    } = &tab.root
    else {
        return Err(io::Error::other("workspace Tab root is not a Panel").into());
    };
    assert!(child_presentations.is_empty());
    assert_eq!(primary_presentation.label, "1");
    primary_presentation
        .binding
        .clone()
        .ok_or_else(|| io::Error::other("workspace primary Presentation is unbound").into())
}

fn assert_authorized_binding(
    binding: &PresentationBinding,
    authorization: orchbee_capability_node_protocol::TerminalOperationAuthorization,
) {
    assert_eq!(
        binding.worker_id.as_str(),
        hex_identity(authorization.worker_id().as_bytes())
    );
    assert_eq!(
        binding.terminal_id.as_str(),
        hex_identity(authorization.terminal_id().as_bytes())
    );
}

fn connect_worker_with_generation(
    endpoint: &Path,
    worker_id: orchbee_local_worker_protocol::WorkerId,
) -> TestResult<(UnixStream, NonZeroU64)> {
    let mut stream = super::super::connect_candidate(endpoint)?;
    orchbee_local_worker_protocol::write_message(
        &mut stream,
        &orchbee_local_worker_protocol::LocalWorkerMessage::WorkerHello(
            orchbee_local_worker_protocol::WorkerHello::new(env!("CARGO_PKG_VERSION"), worker_id)?,
        ),
    )?;
    match orchbee_local_worker_protocol::read_message(&mut stream)? {
        orchbee_local_worker_protocol::LocalWorkerMessage::WorkerAccepted { generation } => {
            Ok((stream, generation))
        }
        response => Err(io::Error::other(format!(
            "expected Worker admission, received {response:?}"
        ))
        .into()),
    }
}

fn wait_for_worker_record(stream: &UnixStream) -> TestResult {
    let mut descriptors = [PollFd::new(stream.as_fd(), PollFlags::POLLIN)];
    let ready = poll(
        &mut descriptors,
        u16::try_from(PROCESS_TIMEOUT.as_millis())?,
    )?;
    assert_eq!(ready, 1, "Worker Launch was not dispatched");
    assert!(
        descriptors
            .first()
            .and_then(PollFd::revents)
            .is_some_and(|events| events.contains(PollFlags::POLLIN)),
        "Worker socket became ready without a readable Launch"
    );
    Ok(())
}

fn hex_identity(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}

#[test]
fn workspace_tab_commits_membership_before_launch_and_preserves_it_through_navigation() -> TestResult
{
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut creator, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let empty = read_snapshot(&mut observer, 100)?;
    assert!(empty.navigation.workspaces.is_empty());
    assert!(empty.terminal_contexts.is_empty());
    let initial = super::super::tab_create::create_workspace(
        &mut observer,
        0x100_000,
        "/tmp",
        Instant::now() + PROCESS_TIMEOUT,
    )?;
    let create = tab_request(&initial, 1)?;
    write_workbench_request(&mut creator, &create)?;
    wait_for_worker_record(&worker)?;
    let committed = read_snapshot(&mut observer, 102)?;
    let binding = workspace_binding(&committed)?;
    assert_eq!(committed.revision.value(), initial.revision.value() + 1);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_obcn(&mut worker)? else {
        return Err("missing Launch".into());
    };
    let orchbee_capability_node_protocol::TerminalLaunchMode::Immediate(geometry) = launch.mode()
    else {
        return Err("expected immediate Launch".into());
    };
    assert!(launch.cwd().is_some());
    assert_authorized_binding(&binding, launch.authorization());
    write_workbench_request(&mut observer, &create)?;
    assert!(matches!(
        tab_outcome(&mut observer)?,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = navigate(
        &mut observer,
        201,
        &committed,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("While launch is pending")?,
        },
    )?
    else {
        return Err("project creation failed".into());
    };
    let after_create = read_snapshot(&mut observer, 103)?;
    assert_eq!(workspace_binding(&after_create)?, binding);
    assert!(matches!(
        navigate(
            &mut observer,
            202,
            &after_create,
            WorkbenchAction::ProjectRename {
                project_id,
                label: WorkbenchLabel::parse("Renamed while launch is pending")?,
            }
        )?,
        WorkbenchCommandOutcome::Renamed { .. }
    ));
    let after_rename = read_snapshot(&mut observer, 104)?;
    assert_eq!(workspace_binding(&after_rename)?, binding);
    drop(creator);
    let host_identity = identity()?;
    write_obcn(
        &mut worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                host_identity,
                HostLaunchState::Started(geometry),
            )),
        )?),
    )?;
    // A replay can observe pending until the disconnected handler publishes settlement.
    let limit = Instant::now() + PROCESS_TIMEOUT;
    loop {
        write_workbench_request(&mut observer, &create)?;
        match tab_outcome(&mut observer)? {
            WorkbenchCommandOutcome::TabCreated {
                terminal_outcome:
                    orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
                ..
            } => break,
            WorkbenchCommandOutcome::TabCreatePending { .. } => assert!(Instant::now() < limit),
            result => return Err(format!("unexpected creation replay: {result:?}").into()),
        }
        thread::yield_now();
    }
    for component in [
        host_identity.creation_id().as_bytes(),
        host_identity.host_epoch().as_bytes(),
        host_identity.terminal_id().as_bytes(),
    ] {
        assert_ne!(binding.terminal_id.as_str(), hex_identity(component));
    }
    let settled = read_snapshot(&mut observer, 105)?;
    assert_eq!(workspace_binding(&settled)?, binding);
    worker.set_read_timeout(Some(Duration::from_millis(40)))?;
    assert!(
        matches!(worker.read(&mut [0]), Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut))
    );
    Ok(())
}

#[test]
fn workspace_tab_unknown_after_worker_loss_retains_membership_across_worker_generation()
-> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = super::super::tab_create::create_workspace(
        &mut observer,
        0x100_000,
        "/tmp",
        Instant::now() + PROCESS_TIMEOUT,
    )?;
    let create = tab_request(&initial, 1)?;
    write_workbench_request(&mut first, &create)?;
    wait_for_worker_record(&worker)?;
    let committed = read_snapshot(&mut observer, 302)?;
    let binding = workspace_binding(&committed)?;
    assert_eq!(committed.revision.value(), initial.revision.value() + 1);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_obcn(&mut worker)? else {
        return Err("missing Launch".into());
    };
    assert_authorized_binding(&binding, launch.authorization());
    drop(worker);
    let result = tab_outcome(&mut first)?;
    assert!(matches!(
        result,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome:
                orchbee_workbench_protocol::WorkbenchTerminalOutcome::OutcomeUnknown {},
            ..
        }
    ));
    let after_loss = read_snapshot(&mut observer, 303)?;
    assert_eq!(after_loss.revision, committed.revision);
    assert_eq!(workspace_binding(&after_loss)?, binding);

    let (mut replacement, replacement_generation) =
        connect_worker_with_generation(server.worker_endpoint(), FIRST_WORKER)?;
    assert!(replacement_generation.get() > launch.authorization().worker_generation().get());
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let attach = ClientMessage::TerminalViewer(TerminalViewerRequest::new(
        id(3)?,
        TerminalViewerAction::AttachBinding {
            site_id: ClientSiteId::parse(after_loss.site_id.as_str())?,
            worker_id: ClientWorkerId::parse(binding.worker_id.as_str())?,
            terminal_id: ClientTerminalId::parse(binding.terminal_id.as_str())?,
        },
    ));
    assert!(
        matches!(exchange(&mut second, &attach)?, ServerMessage::TerminalViewerResult(result) if result.outcome() == &TerminalViewerOutcome::Unavailable { reason: ViewerUnavailableReason::TerminalRuntimeUnavailable })
    );
    write_workbench_request(&mut second, &create)?;
    let result = tab_outcome(&mut second)?;
    assert!(matches!(
        result,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome:
                orchbee_workbench_protocol::WorkbenchTerminalOutcome::OutcomeUnknown {},
            ..
        }
    ));
    let after_reopen = read_snapshot(&mut observer, 304)?;
    assert_eq!(workspace_binding(&after_reopen)?, binding);
    replacement.set_read_timeout(Some(Duration::from_millis(40)))?;
    let mut byte = [0];
    assert!(
        matches!(replacement.read(&mut byte), Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut))
    );
    Ok(())
}
