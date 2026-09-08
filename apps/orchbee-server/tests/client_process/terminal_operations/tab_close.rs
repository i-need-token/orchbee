use super::*;
use orchbee_client_server_protocol::{
    ControlInput, ControllerId, TerminalControlAction, TerminalControlOutcome,
    TerminalControlRequest, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
};
use orchbee_workbench_protocol::{
    PanelNode, ProtocolVersion, Snapshot, SnapshotReadIntent, Tab, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchLabel,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus,
    WorkbenchTerminalGeometry, read_workbench_response, write_workbench_request,
};
use std::num::NonZeroU64;

fn workbench_id(value: u128) -> TestResult<WorkbenchRequestId> {
    Ok(WorkbenchRequestId::parse(&format!("{value:032x}"))?)
}

fn snapshot(client: &mut UnixStream, value: u128) -> TestResult<Snapshot> {
    let request_id = workbench_id(value)?;
    write_workbench_request(
        client,
        &WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
        }),
    )?;
    let WorkbenchResponse::Snapshot(result) = read_workbench_response(client)? else {
        return Err("missing snapshot".into());
    };
    assert_eq!(result.request_id, request_id);
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        return Err("unavailable snapshot".into());
    };
    Ok(snapshot)
}

fn tabs(snapshot: &Snapshot) -> TestResult<&[Tab]> {
    Ok(&snapshot
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == snapshot.navigation.standalone_terminal_context_id
        })
        .ok_or("standalone context")?
        .tabs)
}

fn command(
    client: &mut UnixStream,
    snapshot: &Snapshot,
    value: u128,
    action: WorkbenchAction,
) -> TestResult<WorkbenchCommandResult> {
    let request_id = workbench_id(value)?;
    write_workbench_request(
        client,
        &WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            site_id: snapshot.site_id.clone(),
            expected_revision: snapshot.revision,
            action,
        }),
    )?;
    let WorkbenchResponse::Command(result) = read_workbench_response(client)? else {
        return Err("missing command result".into());
    };
    assert_eq!(result.request_id, request_id);
    Ok(result)
}

fn create_tab(client: &mut UnixStream, value: u128) -> TestResult<(Snapshot, Tab)> {
    let before = snapshot(client, value)?;
    let created = command(
        client,
        &before,
        value + 1,
        WorkbenchAction::TabCreate {
            terminal_context_id: before.navigation.standalone_terminal_context_id.clone(),
            label: WorkbenchLabel::parse("idle Close capacity")?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
        },
    )?;
    let WorkbenchCommandOutcome::TabCreated { tab_id, .. } = created.outcome else {
        return Err(format!("Create: {:?}", created.outcome).into());
    };
    let after = snapshot(client, value + 2)?;
    let tab = tabs(&after)?
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or("created Tab missing")?
        .clone();
    Ok((after, tab))
}

fn attach(
    client: &mut UnixStream,
    snapshot: &Snapshot,
    tab: &Tab,
    value: u128,
) -> TestResult<orchbee_client_server_protocol::ViewerId> {
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &tab.root
    else {
        return Err("standalone panel".into());
    };
    let binding = primary_presentation
        .binding
        .as_ref()
        .ok_or("terminal binding")?;
    let ServerMessage::TerminalViewerResult(result) = exchange(
        client,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            id(value)?,
            TerminalViewerAction::AttachBinding {
                site_id: orchbee_client_server_protocol::SiteId::parse(snapshot.site_id.as_str())?,
                worker_id: orchbee_client_server_protocol::WorkerId::parse(
                    binding.worker_id.as_str(),
                )?,
                terminal_id: orchbee_client_server_protocol::TerminalId::parse(
                    binding.terminal_id.as_str(),
                )?,
            },
        )),
    )?
    else {
        return Err("Viewer result".into());
    };
    assert_eq!(result.request_id(), &id(value)?);
    let TerminalViewerOutcome::Attached { attachment_id, .. } = result.into_outcome() else {
        return Err("fresh Attach failed".into());
    };
    Ok(attachment_id)
}

fn acquire(client: &mut UnixStream, value: u128) -> TestResult<ControllerId> {
    let ServerMessage::TerminalControlResult(result) = exchange(
        client,
        &ClientMessage::TerminalControl(TerminalControlRequest::new(
            id(value)?,
            TerminalControlAction::Acquire {},
        )),
    )?
    else {
        return Err("control result".into());
    };
    assert_eq!(result.request_id(), &id(value)?);
    let TerminalControlOutcome::Acquired { controller_id } = result.into_outcome() else {
        return Err("fresh Acquire failed".into());
    };
    Ok(controller_id)
}

fn input(
    client: &mut UnixStream,
    controller: &ControllerId,
    value: u128,
    sequence: u64,
    bytes: Vec<u8>,
) -> TestResult {
    let ServerMessage::TerminalControlResult(result) = exchange(
        client,
        &ClientMessage::TerminalControl(TerminalControlRequest::new(
            id(value)?,
            TerminalControlAction::Input {
                controller_id: controller.clone(),
                sequence: NonZeroU64::new(sequence).ok_or("zero sequence")?,
                bytes: ControlInput::new(bytes)?,
            },
        )),
    )?
    else {
        return Err("Input result".into());
    };
    assert_eq!(result.request_id(), &id(value)?);
    assert_eq!(result.outcome(), &TerminalControlOutcome::InputAccepted {});
    Ok(())
}

fn close_tab(client: &mut UnixStream, snapshot: &Snapshot, tab: &Tab, value: u128) -> TestResult {
    let closed = command(
        client,
        snapshot,
        value,
        WorkbenchAction::TabClose {
            terminal_context_id: snapshot.navigation.standalone_terminal_context_id.clone(),
            tab_id: tab.tab_id.clone(),
        },
    )?;
    let WorkbenchCommandOutcome::TabCloseAccepted {
        close_request_id, ..
    } = closed.outcome
    else {
        return Err(format!("Close: {:?}", closed.outcome).into());
    };
    assert_eq!(close_request_id, workbench_id(value)?);
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        let request_id = workbench_id(value + 1)?;
        write_workbench_request(
            client,
            &WorkbenchRequest::TabCloseRead(WorkbenchTabCloseRead {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                site_id: snapshot.site_id.clone(),
                terminal_context_id: snapshot.navigation.standalone_terminal_context_id.clone(),
                tab_id: tab.tab_id.clone(),
            }),
        )?;
        let WorkbenchResponse::TabCloseRead(result) = read_workbench_response(client)? else {
            return Err("CloseRead result".into());
        };
        assert_eq!(result.request_id, request_id);
        let WorkbenchTabCloseReadOutcome::Available { status, .. } = result.outcome else {
            return Err("CloseRead unavailable".into());
        };
        if matches!(status, WorkbenchTabCloseStatus::Closed { .. }) {
            return Ok(());
        }
        assert!(
            Instant::now() < deadline,
            "Close did not settle: {status:?}"
        );
        assert!(
            !matches!(
                status,
                WorkbenchTabCloseStatus::OutcomeUnknown { .. }
                    | WorkbenchTabCloseStatus::CleanupBlocked { .. }
            ),
            "Close failed: {status:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn real_eight_idle_clients_close_and_reuse_one_session_without_detach_or_drop() -> TestResult {
    let server = RunningServer::start()?;
    let host_directory = HostDirectory::new()?;
    let runtime = host_directory.0.as_path();
    let endpoint_id = "1234567890abcdef1234567890abcdef";
    let endpoint = runtime.join(format!("{endpoint_id}.s"));
    let host_binary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../terminal-host/zig-out/bin/orchbee-terminal-host");
    let worker_binary =
        Path::new(env!("CARGO_BIN_EXE_orchbee-server")).with_file_name("orchbee-worker");
    assert!(
        host_binary.is_file() && worker_binary.is_file(),
        "build exact Host/Worker prerequisites first"
    );
    let mut host = ChildGuard::spawn(
        Command::new(host_binary)
            .args(["serve", "--runtime-directory"])
            .arg(runtime)
            .args(["--endpoint-id", endpoint_id]),
        false,
    )?;
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    while !endpoint.exists() {
        if let Some(status) = host.0.try_wait()? {
            return Err(format!("Host exited: {status}").into());
        }
        assert!(Instant::now() < deadline, "Host endpoint did not appear");
        thread::sleep(Duration::from_millis(5));
    }
    let _worker = ChildGuard::spawn(
        Command::new(worker_binary)
            .env("SHELL", "/bin/sh")
            .args(["--local-endpoint"])
            .arg(server.worker_endpoint())
            .arg("--terminal-host-runtime-directory")
            .arg(runtime)
            .args(["--terminal-host-endpoint-id", endpoint_id]),
        true,
    )?;
    let mut clients = Vec::new();
    let mut controllers = Vec::new();
    let mut attachments = Vec::new();
    let mut originals = Vec::new();
    for index in 0..8_u128 {
        let (mut client, _) = connect_bounded_client(server.client_endpoint())?;
        let (current, tab) = create_tab(&mut client, 100 + index * 10)?;
        attachments.push(attach(&mut client, &current, &tab, 1000 + index)?);
        controllers.push(acquire(&mut client, 1100 + index)?);
        originals.push(tab);
        clients.push(client);
    }
    let before = snapshot(&mut clients[0], 2000)?;
    assert_eq!(tabs(&before)?.len(), 8);
    close_tab(&mut clients[0], &before, &originals[0], 2100)?;
    let removed = snapshot(&mut clients[0], 2200)?;
    assert_eq!(tabs(&removed)?.len(), 7);
    assert!(
        tabs(&removed)?
            .iter()
            .all(|tab| tab.tab_id != originals[0].tab_id)
    );
    for original in &originals[1..] {
        assert!(tabs(&removed)?.iter().any(|tab| tab == original));
    }
    let (fresh, fresh_tab) = create_tab(&mut clients[0], 2300)?;
    assert_eq!(tabs(&fresh)?.len(), 8);
    let fresh_attachment = attach(&mut clients[0], &fresh, &fresh_tab, 2400)?;
    let fresh_controller = acquire(&mut clients[0], 2401)?;
    assert_ne!(fresh_attachment, attachments[0]);
    assert_ne!(fresh_controller, controllers[0]);
    assert_eq!(clients.len(), 8, "every original Client remains connected");
    let witness = runtime.join("unrelated-close-witness");
    let quoted = format!("'{}'", witness.display().to_string().replace('\'', "'\\''"));
    input(
        &mut clients[1],
        &controllers[1],
        2500,
        1,
        format!("printf 'still-alive\\n' > {quoted}\n").into_bytes(),
    )?;
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    while std::fs::read_to_string(&witness).ok().as_deref() != Some("still-alive\n") {
        assert!(Instant::now() < deadline, "unrelated shell did not execute");
        thread::sleep(Duration::from_millis(5));
    }
    // Witness complete. Settle fixture shells through the same lifecycle owner before shutdown.
    input(
        &mut clients[0],
        &fresh_controller,
        2600,
        1,
        b"exit\n".to_vec(),
    )?;
    for index in 1..8 {
        input(
            &mut clients[index],
            &controllers[index],
            2600 + u128::try_from(index)?,
            if index == 1 { 2 } else { 1 },
            b"exit\n".to_vec(),
        )?;
    }
    for index in 0..8_u128 {
        let current = snapshot(&mut clients[0], 3000 + index * 10)?;
        let tab = tabs(&current)?.first().ok_or("cleanup target")?.clone();
        close_tab(&mut clients[0], &current, &tab, 3001 + index * 10)?;
    }
    assert!(tabs(&snapshot(&mut clients[0], 4000)?)?.is_empty());
    Ok(())
}
