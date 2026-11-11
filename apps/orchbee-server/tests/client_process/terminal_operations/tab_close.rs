use super::*;
use orchbee_client_server_protocol::{
    ControlInput, ControllerId, TerminalControlAction, TerminalControlOutcome,
    TerminalControlRequest, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
};
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, ProtocolVersion, Snapshot, SnapshotReadIntent, Tab,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchLabel, WorkbenchPanelSplitDirection, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus, WorkbenchTabMoveDirection,
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
            context.terminal_context_id == snapshot.navigation.workspaces[0].terminal_context_id
        })
        .ok_or("initial_workspace context")?
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
            session_id: snapshot.session_id.clone(),
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
            automatic_label: false,
            placement: None,
            terminal_context_id: before.navigation.workspaces[0].terminal_context_id.clone(),
            label: WorkbenchLabel::parse("idle Close capacity")?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
            launch: None,
            profile: None,
            options: Default::default(),
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
        return Err("initial_workspace panel".into());
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
                session_id: orchbee_client_server_protocol::SessionId::parse(
                    snapshot.session_id.as_str(),
                )?,
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

fn acquire(
    client: &mut UnixStream,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
    value: u128,
) -> TestResult<ControllerId> {
    let ServerMessage::TerminalControlResult(result) = exchange(
        client,
        &ClientMessage::TerminalControl(TerminalControlRequest::new(
            id(value)?,
            TerminalControlAction::Acquire {
                attachment_id: attachment_id.clone(),
            },
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
    attachment_id: &orchbee_client_server_protocol::ViewerId,
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
                attachment_id: attachment_id.clone(),
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
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
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
                session_id: snapshot.session_id.clone(),
                terminal_context_id: snapshot.navigation.workspaces[0]
                    .terminal_context_id
                    .clone(),
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
    let endpoint = runtime.join(format!("{endpoint_id}-host.sock"));
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
    let deadline = Instant::now() + Duration::from_secs(40);
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
        let attachment = attach(&mut client, &current, &tab, 1000 + index)?;
        controllers.push(acquire(&mut client, &attachment, 1100 + index)?);
        attachments.push(attachment);
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
    let fresh_controller = acquire(&mut clients[0], &fresh_attachment, 2401)?;
    assert_ne!(fresh_attachment, attachments[0]);
    assert_ne!(fresh_controller, controllers[0]);
    assert_eq!(clients.len(), 8, "every original Client remains connected");
    let witness = runtime.join("unrelated-close-witness");
    let quoted = format!("'{}'", witness.display().to_string().replace('\'', "'\\''"));
    input(
        &mut clients[1],
        &attachments[1],
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
        &fresh_attachment,
        &fresh_controller,
        2600,
        1,
        b"exit\n".to_vec(),
    )?;
    for index in 1..8 {
        input(
            &mut clients[index],
            &attachments[index],
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

#[test]
fn real_workbench_panel_split_and_aggregate_close_releases_both_terminals() -> TestResult {
    macro_rules! stage {
        ($name:literal, $operation:expr) => {
            $operation.map_err(|error| std::io::Error::other(format!("{}: {error}", $name)))?
        };
    }
    let server = stage!("server start", RunningServer::start());
    let mut real = stage!("real runtime start", RealTerminalRuntime::start(&server));
    let (mut client, _) = stage!(
        "workbench client connect",
        connect_bounded_client(server.client_endpoint())
    );
    let initial = stage!("initial snapshot", snapshot(&mut client, 1));
    let context_id = initial.navigation.workspaces[0].terminal_context_id.clone();
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16)?;
    let created = stage!(
        "tab.create",
        command(
            &mut client,
            &initial,
            2,
            WorkbenchAction::TabCreate {
                automatic_label: false,
                placement: None,
                terminal_context_id: context_id.clone(),
                label: WorkbenchLabel::parse("real split source")?,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let (tab_id, panel_id) = match created.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome,
            ..
        } => {
            assert!(matches!(
                terminal_outcome,
                orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. }
            ));
            (tab_id, panel_id)
        }
        outcome => return Err(format!("real tab.create did not start: {outcome:?}").into()),
    };
    let after_create = stage!("snapshot after tab.create", snapshot(&mut client, 3));
    tabs(&after_create)?
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or("real source Tab missing")?;
    let split = stage!(
        "panel.split",
        command(
            &mut client,
            &after_create,
            4,
            WorkbenchAction::PanelSplit {
                right_click: Default::default(),
                terminal_context_id: context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("real split peer")?,
                custom_label: None,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let new_panel_id = match split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            new_panel_id,
            terminal_outcome,
            ..
        } => {
            assert!(matches!(
                terminal_outcome,
                orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. }
            ));
            new_panel_id
        }
        outcome => return Err(format!("real panel.split did not start: {outcome:?}").into()),
    };
    let after_split = stage!("snapshot after panel.split", snapshot(&mut client, 5));
    let split_tab = tabs(&after_split)?
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or("real split Tab missing")?;
    let PanelNode::Split { children, .. } = &split_tab.root else {
        return Err("real panel.split did not project a split root".into());
    };
    let bindings: Vec<PresentationBinding> = children
        .iter()
        .map(|slot| {
            let PanelNode::Panel {
                primary_presentation,
                ..
            } = &slot.node
            else {
                return Err("real panel.split produced a nested split unexpectedly".into());
            };
            primary_presentation
                .binding
                .clone()
                .ok_or_else(|| "real panel.split leaf is not bound".into())
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(bindings.len(), 2);
    assert_ne!(bindings[0].terminal_id, bindings[1].terminal_id);
    assert_eq!(bindings[0].worker_id, bindings[1].worker_id);
    assert!(children.iter().any(|slot| {
        matches!(
            &slot.node,
            PanelNode::Panel { panel_id: candidate, .. } if *candidate == panel_id
        )
    }));
    assert!(children.iter().any(|slot| {
        matches!(
            &slot.node,
            PanelNode::Panel { panel_id: candidate, .. } if *candidate == new_panel_id
        )
    }));

    let closed = stage!(
        "tab.close",
        command(
            &mut client,
            &after_split,
            6,
            WorkbenchAction::TabClose {
                terminal_context_id: context_id.clone(),
                tab_id: tab_id.clone(),
            },
        )
    );
    let close_request_id = match closed.outcome {
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id, ..
        } => close_request_id,
        outcome => {
            return Err(format!("real aggregate close was not accepted: {outcome:?}").into());
        }
    };
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut request_value = 7;
    loop {
        let request_id = workbench_id(request_value)?;
        stage!(
            "tab-close.read write",
            write_workbench_request(
                &mut client,
                &WorkbenchRequest::TabCloseRead(WorkbenchTabCloseRead {
                    protocol_version: ProtocolVersion,
                    request_id: request_id.clone(),
                    session_id: after_split.session_id.clone(),
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                }),
            )
        );
        let response = stage!(
            "tab-close.read response",
            read_workbench_response(&mut client)
        );
        let WorkbenchResponse::TabCloseRead(result) = response else {
            return Err("real aggregate close returned the wrong response".into());
        };
        assert_eq!(result.request_id, request_id);
        let WorkbenchTabCloseReadOutcome::Available {
            close_request_id: observed_request_id,
            status,
            ..
        } = result.outcome
        else {
            return Err("real aggregate close became unavailable".into());
        };
        assert_eq!(observed_request_id, close_request_id);
        match &status {
            WorkbenchTabCloseStatus::Closed { .. } => break,
            WorkbenchTabCloseStatus::OutcomeUnknown { .. }
            | WorkbenchTabCloseStatus::CleanupBlocked { .. } => {
                return Err(format!("real aggregate close failed: {status:?}").into());
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err(format!("real aggregate close did not settle: {status:?}").into());
        }
        request_value += 1;
        thread::sleep(Duration::from_millis(10));
    }
    let after_close = stage!(
        "snapshot after aggregate close",
        snapshot(&mut client, request_value + 1)
    );
    assert!(tabs(&after_close)?.is_empty());
    drop(real.worker.take());
    // Releasing the last terminal retires the Host; its endpoint may briefly outlive it.
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if real._host.0.try_wait()?.is_some() {
            break;
        }
        if let Ok(inventory) = running_inventory(real.endpoint.as_path()) {
            assert_eq!(inventory, 0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Host stayed alive without serving a drained terminal inventory"
        );
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

#[test]
fn real_workbench_aggregate_close_fences_same_tab_split_and_move() -> TestResult {
    macro_rules! stage {
        ($name:literal, $operation:expr) => {
            $operation.map_err(|error| std::io::Error::other(format!("{}: {error}", $name)))?
        };
    }
    let server = stage!("server start", RunningServer::start());
    let mut real = stage!("real runtime start", RealTerminalRuntime::start(&server));
    let (mut client, _) = stage!(
        "workbench client connect",
        connect_bounded_client(server.client_endpoint())
    );
    let initial = stage!("initial snapshot", snapshot(&mut client, 1));
    let context_id = initial.navigation.workspaces[0].terminal_context_id.clone();
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16)?;
    let created = stage!(
        "tab.create",
        command(
            &mut client,
            &initial,
            2,
            WorkbenchAction::TabCreate {
                automatic_label: false,
                placement: None,
                terminal_context_id: context_id.clone(),
                label: WorkbenchLabel::parse("fenced split source")?,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let (tab_id, panel_id) = match created.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => return Err(format!("real tab.create did not start: {outcome:?}").into()),
    };
    let after_create = stage!("snapshot after tab.create", snapshot(&mut client, 3));
    let split = stage!(
        "panel.split",
        command(
            &mut client,
            &after_create,
            4,
            WorkbenchAction::PanelSplit {
                right_click: Default::default(),
                terminal_context_id: context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("fenced split peer")?,
                custom_label: None,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let new_panel_id = match split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            new_panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => new_panel_id,
        outcome => return Err(format!("real panel.split did not start: {outcome:?}").into()),
    };
    let after_split = stage!("snapshot after panel.split", snapshot(&mut client, 5));
    // Workspace create admission evaluates tree membership, so it still admits on a
    // context whose first Tab is already split.
    let other = stage!(
        "unrelated tab.create",
        command(
            &mut client,
            &after_split,
            6,
            WorkbenchAction::TabCreate {
                automatic_label: false,
                placement: None,
                terminal_context_id: context_id.clone(),
                label: WorkbenchLabel::parse("unrelated Tab")?,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let (other_tab_id, other_panel_id) = match other.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => return Err(format!("unrelated tab.create did not start: {outcome:?}").into()),
    };
    let pending_close = stage!("snapshot before close", snapshot(&mut client, 7));
    let closed = stage!(
        "tab.close",
        command(
            &mut client,
            &pending_close,
            8,
            WorkbenchAction::TabClose {
                terminal_context_id: context_id.clone(),
                tab_id: tab_id.clone(),
            },
        )
    );
    let close_request_id = match closed.outcome {
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id, ..
        } => close_request_id,
        outcome => {
            return Err(format!("real aggregate close was not accepted: {outcome:?}").into());
        }
    };

    // While the aggregate Close is pending, same-Tab structural admission is refused with
    // transient terminal-busy; the refusal stages no terminal launch and retains nothing.
    let fenced_split = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(9)?,
        session_id: pending_close.session_id.clone(),
        expected_revision: pending_close.revision,
        action: WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("fenced during close")?,
            custom_label: None,
            geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    for attempt in 0..2 {
        stage!(
            "fenced panel.split write",
            write_workbench_request(
                &mut client,
                &WorkbenchRequest::Command(fenced_split.clone())
            )
        );
        let WorkbenchResponse::Command(fenced) = stage!(
            "fenced panel.split response",
            read_workbench_response(&mut client)
        ) else {
            return Err("missing fenced split result".into());
        };
        assert_eq!(fenced.request_id, workbench_id(9)?);
        let WorkbenchCommandOutcome::Unavailable {
            reason: orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::TerminalBusy,
        } = fenced.outcome
        else {
            return Err(format!(
                "same-Tab split during pending aggregate Close was not fenced (attempt {attempt}): {:?}",
                fenced.outcome
            )
            .into());
        };
    }
    let fenced_move = stage!(
        "fenced panel.move",
        command(
            &mut client,
            &pending_close,
            10,
            WorkbenchAction::PanelMove {
                terminal_context_id: context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                target_panel_id: new_panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
            },
        )
    );
    let WorkbenchCommandOutcome::Unavailable {
        reason: orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::TerminalBusy,
    } = fenced_move.outcome
    else {
        return Err(format!(
            "same-Tab move during pending aggregate Close was not fenced: {:?}",
            fenced_move.outcome
        )
        .into());
    };

    // An unrelated Tab still admits and launches structural work during the pending Close.
    let other_split = stage!(
        "unrelated panel.split",
        command(
            &mut client,
            &pending_close,
            11,
            WorkbenchAction::PanelSplit {
                right_click: Default::default(),
                terminal_context_id: context_id.clone(),
                tab_id: other_tab_id.clone(),
                panel_id: other_panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("unrelated split")?,
                custom_label: None,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    match other_split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => {}
        outcome => {
            return Err(format!("unrelated Tab split was not admitted: {outcome:?}").into());
        }
    }

    let deadline = Instant::now() + Duration::from_secs(40);
    let mut request_value = 12;
    loop {
        let request_id = workbench_id(request_value)?;
        stage!(
            "tab-close.read write",
            write_workbench_request(
                &mut client,
                &WorkbenchRequest::TabCloseRead(WorkbenchTabCloseRead {
                    protocol_version: ProtocolVersion,
                    request_id: request_id.clone(),
                    session_id: pending_close.session_id.clone(),
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                }),
            )
        );
        let response = stage!(
            "tab-close.read response",
            read_workbench_response(&mut client)
        );
        let WorkbenchResponse::TabCloseRead(result) = response else {
            return Err("fenced aggregate close returned the wrong response".into());
        };
        assert_eq!(result.request_id, request_id);
        let WorkbenchTabCloseReadOutcome::Available {
            close_request_id: observed_request_id,
            status,
            ..
        } = result.outcome
        else {
            return Err("fenced aggregate close became unavailable".into());
        };
        assert_eq!(observed_request_id, close_request_id);
        match &status {
            WorkbenchTabCloseStatus::Closed { .. } => break,
            WorkbenchTabCloseStatus::OutcomeUnknown { .. }
            | WorkbenchTabCloseStatus::CleanupBlocked { .. } => {
                return Err(format!("fenced aggregate close failed: {status:?}").into());
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err(format!("fenced aggregate close did not settle: {status:?}").into());
        }
        request_value += 1;
        thread::sleep(Duration::from_millis(10));
    }
    let after_close = stage!(
        "snapshot after fenced close",
        snapshot(&mut client, request_value + 1)
    );
    let surviving = tabs(&after_close)?;
    assert_eq!(surviving.len(), 1);
    assert_eq!(surviving[0].tab_id, other_tab_id);
    let PanelNode::Split { children, .. } = &surviving[0].root else {
        return Err("unrelated Tab split did not project a split root".into());
    };
    assert_eq!(children.len(), 2);

    // The fenced request retained nothing: replaying it now resolves against the published
    // revision, and the authority keeps serving commands.
    stage!(
        "post-publication split replay write",
        write_workbench_request(&mut client, &WorkbenchRequest::Command(fenced_split))
    );
    let WorkbenchResponse::Command(replayed) = stage!(
        "post-publication split replay response",
        read_workbench_response(&mut client)
    ) else {
        return Err("missing post-publication split result".into());
    };
    let WorkbenchCommandOutcome::Rejected {
        reason: orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
        ..
    } = replayed.outcome
    else {
        return Err(format!(
            "post-publication fenced split did not re-evaluate: {:?}",
            replayed.outcome
        )
        .into());
    };

    close_tab(&mut client, &after_close, &surviving[0], request_value + 2)?;
    let emptied = stage!("final snapshot", snapshot(&mut client, request_value + 3));
    assert!(tabs(&emptied)?.is_empty());
    drop(real.worker.take());
    // Releasing the last terminal retires the Host; its endpoint may briefly outlive it.
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if real._host.0.try_wait()?.is_some() {
            break;
        }
        if let Ok(inventory) = running_inventory(real.endpoint.as_path()) {
            assert_eq!(inventory, 0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Host stayed alive without serving a drained terminal inventory"
        );
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn attach_binding(
    client: &mut UnixStream,
    snapshot: &Snapshot,
    binding: &PresentationBinding,
    value: u128,
) -> TestResult<orchbee_client_server_protocol::ViewerId> {
    let ServerMessage::TerminalViewerResult(result) = exchange(
        client,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            id(value)?,
            TerminalViewerAction::AttachBinding {
                session_id: orchbee_client_server_protocol::SessionId::parse(
                    snapshot.session_id.as_str(),
                )?,
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
        return Err("binding Attach result".into());
    };
    assert_eq!(result.request_id(), &id(value)?);
    let TerminalViewerOutcome::Attached { attachment_id, .. } = result.into_outcome() else {
        return Err("binding Attach failed".into());
    };
    Ok(attachment_id)
}

fn shell_pid(
    client: &mut UnixStream,
    snapshot: &Snapshot,
    binding: &PresentationBinding,
    path: &Path,
    value: u128,
) -> TestResult<u32> {
    let attachment = attach_binding(client, snapshot, binding, value)?;
    let controller = acquire(client, &attachment, value + 1)?;
    let quoted = format!("'{}'", path.display().to_string().replace('\'', "'\\''"));
    input(
        client,
        &attachment,
        &controller,
        value + 2,
        1,
        format!("printf '%s\\n' \"$$\" > {quoted}\n").into_bytes(),
    )?;
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Ok(contents) = std::fs::read_to_string(path)
            && let Ok(pid) = contents.trim().parse::<u32>()
        {
            return Ok(pid);
        }
        assert!(Instant::now() < deadline, "shell PID marker did not appear");
        thread::sleep(Duration::from_millis(5));
    }
}

fn process_is_alive(pid: u32) -> TestResult<bool> {
    Ok(Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}

fn wait_for_process_exit(pid: u32) -> TestResult {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    while process_is_alive(pid)? {
        assert!(Instant::now() < deadline, "closed shell PID {pid} is alive");
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn leaf_bindings(tab: &Tab) -> TestResult<Vec<PresentationBinding>> {
    let mut found = Vec::new();
    let mut stack = vec![&tab.root];
    while let Some(node) = stack.pop() {
        match node {
            PanelNode::Panel {
                primary_presentation,
                ..
            } => found.push(
                primary_presentation
                    .binding
                    .clone()
                    .ok_or("leaf Presentation is unbound")?,
            ),
            PanelNode::Split { children, .. } => {
                stack.extend(children.iter().map(|slot| &slot.node));
            }
        }
    }
    Ok(found)
}

#[test]
fn real_multileaf_create_and_move_reconcile_two_clients_and_retarget_close() -> TestResult {
    macro_rules! stage {
        ($name:literal, $operation:expr) => {
            $operation.map_err(|error| std::io::Error::other(format!("{}: {error}", $name)))?
        };
    }
    let server = stage!("server start", RunningServer::start());
    let mut real = stage!("real runtime start", RealTerminalRuntime::start(&server));
    let witnesses = HostDirectory::new()?;
    let (mut first, _) = stage!(
        "first client connect",
        connect_bounded_client(server.client_endpoint())
    );
    let (mut second, _) = stage!(
        "second client connect",
        connect_bounded_client(server.client_endpoint())
    );
    let initial = stage!("initial snapshot", snapshot(&mut first, 1));
    let context_id = initial.navigation.workspaces[0].terminal_context_id.clone();
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16)?;

    let created = stage!(
        "first tab.create",
        command(
            &mut first,
            &initial,
            2,
            WorkbenchAction::TabCreate {
                automatic_label: false,
                placement: None,
                terminal_context_id: context_id.clone(),
                label: WorkbenchLabel::parse("multileaf source")?,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let (tab_id, panel_id) = match created.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => return Err(format!("first tab.create did not start: {outcome:?}").into()),
    };
    let after_create = stage!("snapshot after create", snapshot(&mut second, 3));
    let split = stage!(
        "panel.split",
        command(
            &mut second,
            &after_create,
            4,
            WorkbenchAction::PanelSplit {
                right_click: Default::default(),
                terminal_context_id: context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("multileaf peer")?,
                custom_label: None,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let new_panel_id = match split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            new_panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => new_panel_id,
        outcome => return Err(format!("panel.split did not start: {outcome:?}").into()),
    };
    let after_split = stage!("snapshot after split", snapshot(&mut first, 5));
    let split_tab = tabs(&after_split)?
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or("split Tab missing")?;
    let split_bindings = leaf_bindings(split_tab)?;
    assert_eq!(split_bindings.len(), 2);
    assert_ne!(split_bindings[0].terminal_id, split_bindings[1].terminal_id);
    let PanelNode::Split { children, .. } = &split_tab.root else {
        return Err("split did not project a split root".into());
    };
    assert!(children.iter().any(|slot| matches!(
        &slot.node,
        PanelNode::Panel { panel_id: child, .. } if *child == new_panel_id
    )));

    // Workspace create admission on multileaf membership must not retire the authority.
    let second_create = stage!(
        "multileaf tab.create",
        command(
            &mut first,
            &after_split,
            6,
            WorkbenchAction::TabCreate {
                automatic_label: false,
                placement: None,
                terminal_context_id: context_id.clone(),
                label: WorkbenchLabel::parse("multileaf peer Tab")?,
                geometry,
                launch: None,
                profile: None,
                options: Default::default(),
            },
        )
    );
    let other_tab_id = match second_create.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => tab_id,
        outcome => {
            return Err(format!("tab.create on multileaf membership failed: {outcome:?}").into());
        }
    };
    let after_second = stage!("snapshot after second create", snapshot(&mut second, 7));
    assert_eq!(tabs(&after_second)?.len(), 2);

    // Workspace TabMove admission on the same multileaf membership.
    let moved = stage!(
        "multileaf tab.move",
        command(
            &mut second,
            &after_second,
            8,
            WorkbenchAction::TabMove {
                terminal_context_id: context_id.clone(),
                tab_id: tab_id.clone(),
                direction: WorkbenchTabMoveDirection::Last,
            },
        )
    );
    let WorkbenchCommandOutcome::TabMoved { .. } = moved.outcome else {
        return Err(format!(
            "tab.move on multileaf membership failed: {:?}",
            moved.outcome
        )
        .into());
    };
    let after_move = stage!("snapshot after move", snapshot(&mut first, 9));
    assert_eq!(
        after_move,
        stage!("second client snapshot", snapshot(&mut second, 10))
    );
    let ordered = tabs(&after_move)?;
    assert_eq!(ordered.len(), 2);
    assert_eq!(ordered[0].tab_id, other_tab_id);
    assert_eq!(ordered[1].tab_id, tab_id);
    assert_eq!(leaf_bindings(&ordered[1])?, split_bindings);

    // Every projected binding drives a distinct real shell process.
    let mut pids = Vec::new();
    for (index, binding) in leaf_bindings(&ordered[1])?
        .into_iter()
        .chain(leaf_bindings(&ordered[0])?)
        .enumerate()
    {
        let path = witnesses.0.join(format!("shell-{index}-pid"));
        let pid = stage!(
            "binding shell PID",
            shell_pid(
                &mut first,
                &after_move,
                &binding,
                &path,
                0x100 + index as u128 * 4
            )
        );
        assert!(process_is_alive(pid)?, "bound shell {pid} is not alive");
        pids.push(pid);
    }
    assert_eq!(pids.len(), 3);
    let distinct: std::collections::HashSet<u32> = pids.iter().copied().collect();
    assert_eq!(distinct.len(), 3, "two bindings share one shell PID");

    // Closing the split Tab retires exactly its two leaves through the real lifecycle.
    stage!(
        "aggregate tab.close",
        close_tab(&mut first, &after_move, &ordered[1], 20)
    );
    for pid in &pids[..2] {
        stage!("closed shell exit", wait_for_process_exit(*pid));
    }
    assert!(
        process_is_alive(pids[2])?,
        "surviving Tab shell was killed by the aggregate close"
    );
    let after_close = stage!("snapshot after close", snapshot(&mut second, 30));
    assert_eq!(
        after_close,
        stage!("first client snapshot", snapshot(&mut first, 31))
    );
    let surviving = tabs(&after_close)?;
    assert_eq!(surviving.len(), 1);
    assert_eq!(surviving[0].tab_id, other_tab_id);

    close_tab(&mut first, &after_close, &surviving[0], 40)?;
    let emptied = stage!("final snapshot", snapshot(&mut first, 41));
    assert!(tabs(&emptied)?.is_empty());
    drop(real.worker.take());
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if real._host.0.try_wait()?.is_some() {
            break;
        }
        if let Ok(inventory) = running_inventory(real.endpoint.as_path()) {
            assert_eq!(inventory, 0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Host stayed alive without serving a drained terminal inventory"
        );
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
