use super::*;
use nix::poll::{PollFd, PollFlags, poll};
use orchbee_client_server_protocol::{
    OpenOutcome, SiteId as ClientSiteId, TerminalId as ClientTerminalId, TerminalOpenRequest,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerUnavailableReason,
    WorkerId as ClientWorkerId,
};
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, ProtocolVersion, Snapshot, SnapshotReadIntent, TerminalContext,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response,
    write_workbench_request,
};
use std::{num::NonZeroU64, os::fd::AsFd};

pub(super) fn open(value: u128, geometry: Geometry) -> TestResult<ClientMessage> {
    Ok(ClientMessage::TerminalOpen(TerminalOpenRequest::new(
        id(value)?,
        geometry,
    )))
}
pub(super) fn opened(message: ServerMessage, value: u128) -> TestResult<RequestId> {
    let ServerMessage::TerminalOpenResult(result) = message else {
        return Err("missing Open result".into());
    };
    assert_eq!(result.request_id(), &id(value)?);
    let OpenOutcome::Opened {
        terminal_create_request_id,
    } = result.into_outcome()
    else {
        return Err("Open failed".into());
    };
    Ok(terminal_create_request_id)
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

fn standalone_context(snapshot: &Snapshot) -> TestResult<&TerminalContext> {
    snapshot
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == snapshot.navigation.standalone_terminal_context_id
        })
        .ok_or_else(|| io::Error::other("missing standalone terminal context").into())
}

pub(super) fn standalone_binding(snapshot: &Snapshot) -> TestResult<PresentationBinding> {
    let context = standalone_context(snapshot)?;
    assert!(context.groups.is_empty());
    assert_eq!(context.tabs.len(), 1);
    let tab = context
        .tabs
        .first()
        .ok_or_else(|| io::Error::other("missing standalone Tab"))?;
    assert!(tab.group_id.is_none());
    assert!(!tab.pinned);
    assert_eq!(tab.label, "1");
    let PanelNode::Panel {
        child_presentations,
        primary_presentation,
        ..
    } = &tab.root
    else {
        return Err(io::Error::other("standalone Tab root is not a Panel").into());
    };
    assert!(child_presentations.is_empty());
    assert_eq!(primary_presentation.label, "1");
    primary_presentation
        .binding
        .clone()
        .ok_or_else(|| io::Error::other("standalone primary Presentation is unbound").into())
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
    worker_id: orchbee_local_site_protocol::WorkerId,
) -> TestResult<(UnixStream, NonZeroU64)> {
    let mut stream = super::super::connect_candidate(endpoint)?;
    orchbee_local_site_protocol::write_message(
        &mut stream,
        &orchbee_local_site_protocol::LocalSiteMessage::WorkerHello(
            orchbee_local_site_protocol::WorkerHello::new(env!("CARGO_PKG_VERSION"), worker_id)?,
        ),
    )?;
    match orchbee_local_site_protocol::read_message(&mut stream)? {
        orchbee_local_site_protocol::LocalSiteMessage::WorkerAccepted { generation } => {
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
fn standalone_open_commits_shared_membership_before_launch_and_preserves_it_through_navigation()
-> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut creator, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut joiner, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read_snapshot(&mut observer, 101)?;
    let initial_context = standalone_context(&initial)?;
    assert!(initial_context.groups.is_empty());
    assert!(initial_context.tabs.is_empty());

    write_client_message(&mut creator, &open(1, geometry()?)?)?;
    wait_for_worker_record(&worker)?;
    write_client_message(&mut joiner, &open(2, Geometry::new(100, 30, 8, 16)?)?)?;

    // Leave the Worker Launch unread: membership must already be a readable shared fact.
    let committed = read_snapshot(&mut observer, 102)?;
    let binding = standalone_binding(&committed)?;
    assert_eq!(committed.revision.value(), initial.revision.value() + 1);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_obcn(&mut worker)? else {
        return Err("missing Launch".into());
    };
    let orchbee_capability_node_protocol::TerminalLaunchMode::Immediate(geometry) = launch.mode()
    else {
        return Err("expected immediate Launch".into());
    };
    assert!(launch.cwd().is_none());
    assert_eq!(
        launch.authorization().worker_id().as_bytes(),
        FIRST_WORKER.as_bytes()
    );
    assert_authorized_binding(&binding, launch.authorization());

    let WorkbenchCommandOutcome::ProjectCreated {
        project_id,
        revision: created_revision,
    } = navigate(
        &mut observer,
        201,
        &committed,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("While launch is pending")?,
        },
    )?
    else {
        return Err("project creation failed while standalone Open was pending".into());
    };
    assert_eq!(created_revision.value(), committed.revision.value() + 1);
    let after_create = read_snapshot(&mut observer, 103)?;
    assert_eq!(standalone_binding(&after_create)?, binding);
    let WorkbenchCommandOutcome::Renamed {
        revision: renamed_revision,
    } = navigate(
        &mut observer,
        202,
        &after_create,
        WorkbenchAction::ProjectRename {
            project_id: project_id.clone(),
            label: WorkbenchLabel::parse("Renamed while launch is pending")?,
        },
    )?
    else {
        return Err("project rename failed while standalone Open was pending".into());
    };
    assert_eq!(renamed_revision.value(), after_create.revision.value() + 1);
    let after_rename = read_snapshot(&mut observer, 104)?;
    assert_eq!(standalone_binding(&after_rename)?, binding);
    assert!(after_rename.navigation.projects.iter().any(|project| {
        project.project_id == project_id && project.label == "Renamed while launch is pending"
    }));

    // Never consume the creator's response; its disconnect cannot undo shared membership.
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
    let reference = opened(read_server_message(&mut joiner)?, 2)?;
    assert_ne!(reference, id(1)?);
    assert_eq!(
        opened(
            exchange(&mut observer, &open(3, Geometry::new(80, 24, 8, 16)?)?)?,
            3
        )?,
        reference
    );
    assert_ne!(binding.terminal_id.as_str(), reference.as_str());
    for host_identity_component in [
        host_identity.creation_id().as_bytes(),
        host_identity.host_epoch().as_bytes(),
        host_identity.terminal_id().as_bytes(),
    ] {
        assert_ne!(
            binding.terminal_id.as_str(),
            hex_identity(host_identity_component)
        );
    }
    let settled = read_snapshot(&mut observer, 105)?;
    assert_eq!(standalone_binding(&settled)?, binding);
    worker.set_read_timeout(Some(Duration::from_millis(40)))?;
    let mut byte = [0];
    assert!(
        matches!(worker.read(&mut byte), Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut))
    );
    Ok(())
}

#[test]
fn standalone_unknown_after_worker_loss_retains_membership_across_worker_generation() -> TestResult
{
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read_snapshot(&mut observer, 301)?;
    write_client_message(&mut first, &open(1, geometry()?)?)?;
    wait_for_worker_record(&worker)?;
    let committed = read_snapshot(&mut observer, 302)?;
    let binding = standalone_binding(&committed)?;
    assert_eq!(committed.revision.value(), initial.revision.value() + 1);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_obcn(&mut worker)? else {
        return Err("missing Launch".into());
    };
    assert_authorized_binding(&binding, launch.authorization());
    drop(worker);
    let result = read_server_message(&mut first)?;
    assert!(
        matches!(result, ServerMessage::TerminalOpenResult(result) if result.outcome() == &OpenOutcome::OutcomeUnknown {})
    );
    let after_loss = read_snapshot(&mut observer, 303)?;
    assert_eq!(after_loss.revision, committed.revision);
    assert_eq!(standalone_binding(&after_loss)?, binding);

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
    let result = exchange(&mut second, &open(2, geometry()?)?)?;
    assert!(
        matches!(result, ServerMessage::TerminalOpenResult(result) if result.outcome() == &OpenOutcome::OutcomeUnknown {})
    );
    let after_reopen = read_snapshot(&mut observer, 304)?;
    assert_eq!(standalone_binding(&after_reopen)?, binding);
    replacement.set_read_timeout(Some(Duration::from_millis(40)))?;
    let mut byte = [0];
    assert!(
        matches!(replacement.read(&mut byte), Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut))
    );
    Ok(())
}
