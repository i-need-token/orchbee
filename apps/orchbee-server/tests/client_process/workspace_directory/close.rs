use super::*;
use orchbee_client_server_protocol::{
    ControlInput, ControllerId, TerminalControlAction, TerminalControlOutcome,
    TerminalControlRequest, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    ViewerRejectedReason,
};
use orchbee_workbench_protocol::{
    WorkbenchPanelSplitDirection, WorkbenchTabCloseRead, WorkbenchTabCloseReadOutcome,
    WorkbenchTabCloseStatus, WorkbenchTabMoveDirection,
};
use std::{
    fs, io,
    num::NonZeroU64,
    os::unix::net::UnixStream,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct AssociatedWorkspace {
    snapshot: Snapshot,
    project_id: orchbee_workbench_protocol::WorkbenchProjectId,
    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId,
    terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId,
    standalone_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId,
}

fn create_associated_workspace(
    fixture: &RealWorkerFixture,
    first: &mut UnixStream,
    second: &mut UnixStream,
) -> TestResult<AssociatedWorkspace> {
    let initial = read_snapshot(first, 0x100)?;
    let standalone_context_id = initial.navigation.standalone_terminal_context_id.clone();
    assert!(
        context(&initial, standalone_context_id.as_str())?
            .tabs
            .is_empty()
    );

    let project = exchange_command(
        first,
        &command(
            workbench_id(0x101)?,
            &initial,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("Workspace close process")?,
            },
        ),
    )?;
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = project.outcome else {
        return Err("ProjectCreate did not return ProjectCreated".into());
    };
    let after_project = read_snapshot(second, 0x102)?;

    let workspace = exchange_command(
        second,
        &command(
            workbench_id(0x103)?,
            &after_project,
            WorkbenchAction::WorkspaceCreate {
                project_id: project_id.clone(),
                label: WorkbenchLabel::parse("Workspace")?,
            },
        ),
    )?;
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id,
        ..
    } = workspace.outcome
    else {
        return Err("WorkspaceCreate did not return WorkspaceCreated".into());
    };
    let after_workspace = read_snapshot(first, 0x104)?;
    assert!(workspace_context(&after_workspace)?.tabs.is_empty());

    fs::create_dir(&fixture.directory)?;
    fs::set_permissions(&fixture.directory, fs::Permissions::from_mode(0o700))?;
    let path = WorkbenchDirectoryPath::parse(
        fixture
            .directory
            .to_str()
            .ok_or_else(|| io::Error::other("Workspace directory path was not UTF-8"))?,
    )?;
    let association = exchange_command(
        first,
        &command(
            workbench_id(0x105)?,
            &after_workspace,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                path,
            },
        ),
    )?;
    assert!(matches!(
        association.outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    let associated = read_snapshot(second, 0x106)?;
    assert_eq!(associated, read_snapshot(first, 0x107)?);
    let project = associated
        .navigation
        .projects
        .iter()
        .find(|candidate| candidate.project_id == project_id)
        .ok_or_else(|| io::Error::other("associated Project was not projected"))?;
    assert_eq!(project.directories.len(), 1);
    let workspace = project
        .workspaces
        .iter()
        .find(|candidate| candidate.workspace_id == workspace_id)
        .ok_or_else(|| io::Error::other("associated Workspace was not projected"))?;
    assert_eq!(workspace.terminal_context_id, terminal_context_id);
    assert!(workspace.primary_directory_id.is_some());
    Ok(AssociatedWorkspace {
        snapshot: associated,
        project_id,
        workspace_id,
        terminal_context_id,
        standalone_context_id,
    })
}

fn read_close(
    client: &mut UnixStream,
    site_id: &orchbee_workbench_protocol::SiteId,
    terminal_context_id: &orchbee_workbench_protocol::WorkbenchTerminalContextId,
    tab_id: &orchbee_workbench_protocol::WorkbenchTabId,
    request_value: u128,
) -> TestResult<WorkbenchTabCloseReadOutcome> {
    let request_id = workbench_id(request_value)?;
    write_workbench_request(
        client,
        &WorkbenchRequest::TabCloseRead(WorkbenchTabCloseRead {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            site_id: site_id.clone(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
        }),
    )?;
    let WorkbenchResponse::TabCloseRead(result) = read_workbench_response(client)? else {
        return Err("TabCloseRead did not return a TabCloseRead response".into());
    };
    assert_eq!(result.request_id, request_id);
    Ok(result.outcome)
}

fn wait_for_closed(
    client: &mut UnixStream,
    snapshot: &Snapshot,
    terminal_context_id: &orchbee_workbench_protocol::WorkbenchTerminalContextId,
    tab_id: &orchbee_workbench_protocol::WorkbenchTabId,
    request_value: u128,
) -> TestResult<WorkbenchTabCloseReadOutcome> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let mut next_request = request_value;
    loop {
        let outcome = read_close(
            client,
            &snapshot.site_id,
            terminal_context_id,
            tab_id,
            next_request,
        )?;
        match &outcome {
            WorkbenchTabCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. },
                ..
            } => {
                return Ok(outcome);
            }
            WorkbenchTabCloseReadOutcome::Available {
                status:
                    status @ (WorkbenchTabCloseStatus::CleanupBlocked { .. }
                    | WorkbenchTabCloseStatus::OutcomeUnknown { .. }),
                ..
            } => {
                return Err(io::Error::other(format!(
                    "Workspace Tab close failed before Closed: {status:?}"
                ))
                .into());
            }
            WorkbenchTabCloseReadOutcome::Available { .. } => {}
            outcome => {
                return Err(io::Error::other(format!(
                    "Workspace Tab close became unreadable: {outcome:?}"
                ))
                .into());
            }
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Workspace Tab close did not settle as Closed",
            )
            .into());
        }
        next_request = next_request
            .checked_add(1)
            .ok_or_else(|| io::Error::other("TabCloseRead request id exhausted"))?;
        thread::sleep(Duration::from_millis(10));
    }
}

fn attach_binding(
    client: &mut UnixStream,
    snapshot: &Snapshot,
    binding: &PresentationBinding,
    request_value: u128,
) -> TestResult<orchbee_client_server_protocol::ViewerId> {
    let request_id = client_id(request_value)?;
    write_client_message(
        client,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            request_id.clone(),
            TerminalViewerAction::AttachBinding {
                site_id: ClientSiteId::parse(snapshot.site_id.as_str())?,
                worker_id: ClientWorkerId::parse(binding.worker_id.as_str())?,
                terminal_id: ClientTerminalId::parse(binding.terminal_id.as_str())?,
            },
        )),
    )?;
    let ServerMessage::TerminalViewerResult(result) = read_server_message(client)? else {
        return Err("standalone AttachBinding did not return a viewer result".into());
    };
    assert_eq!(result.request_id(), &request_id);
    let TerminalViewerOutcome::Attached { attachment_id, .. } = result.into_outcome() else {
        return Err("AttachBinding failed".into());
    };
    Ok(attachment_id)
}

fn acquire_controller(
    client: &mut UnixStream,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
    request_value: u128,
) -> TestResult<ControllerId> {
    let request_id = client_id(request_value)?;
    write_client_message(
        client,
        &ClientMessage::TerminalControl(TerminalControlRequest::new(
            request_id.clone(),
            TerminalControlAction::Acquire {
                attachment_id: attachment_id.clone(),
            },
        )),
    )?;
    let ServerMessage::TerminalControlResult(result) = read_server_message(client)? else {
        return Err("standalone Acquire did not return a control result".into());
    };
    assert_eq!(result.request_id(), &request_id);
    let TerminalControlOutcome::Acquired { controller_id } = result.into_outcome() else {
        return Err("standalone Acquire was not accepted".into());
    };
    Ok(controller_id)
}

fn input(
    client: &mut UnixStream,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
    controller_id: &ControllerId,
    request_value: u128,
    sequence: u64,
    bytes: Vec<u8>,
) -> TestResult<()> {
    let request_id = client_id(request_value)?;
    let sequence = NonZeroU64::new(sequence)
        .ok_or_else(|| io::Error::other("standalone input sequence must be positive"))?;
    write_client_message(
        client,
        &ClientMessage::TerminalControl(TerminalControlRequest::new(
            request_id.clone(),
            TerminalControlAction::Input {
                attachment_id: attachment_id.clone(),
                controller_id: controller_id.clone(),
                sequence,
                bytes: ControlInput::new(bytes)?,
            },
        )),
    )?;
    let ServerMessage::TerminalControlResult(result) = read_server_message(client)? else {
        return Err("standalone Input did not return a control result".into());
    };
    assert_eq!(result.request_id(), &request_id);
    assert_eq!(result.outcome(), &TerminalControlOutcome::InputAccepted {});
    Ok(())
}

fn wait_for_contents(path: &Path, expected: &str) -> TestResult<()> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if fs::read_to_string(path).ok().as_deref() == Some(expected) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("shell did not write expected witness {}", path.display()),
            )
            .into());
        }
        thread::yield_now();
    }
}

fn wait_for_pid(path: &Path) -> TestResult<u32> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Ok(contents) = fs::read_to_string(path)
            && let Ok(pid) = contents.trim().parse::<u32>()
        {
            return Ok(pid);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("shell did not write a PID witness {}", path.display()),
            )
            .into());
        }
        thread::yield_now();
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

fn wait_for_process_exit(pid: u32) -> TestResult<()> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if !process_is_alive(pid)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("closed Workspace shell PID {pid} is still alive"),
            )
            .into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn real_workspace_panel_split_and_aggregate_close_releases_both_terminals() -> TestResult {
    let server = RunningServer::start()?;
    let fixture = RealWorkerFixture::start(&server)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let setup = create_associated_workspace(&fixture, &mut first, &mut second)?;
    let geometry = geometry()?;

    let created = exchange_command(
        &mut first,
        &command(
            workbench_id(0x120)?,
            &setup.snapshot,
            WorkbenchAction::TabCreate {
                terminal_context_id: setup.terminal_context_id.clone(),
                label: WorkbenchLabel::parse("Workspace split source")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let (tab_id, panel_id) = match created.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => return Err(format!("Workspace TabCreate did not start: {outcome:?}").into()),
    };
    let after_create = read_snapshot(&mut second, 0x121)?;
    let split = exchange_command(
        &mut second,
        &command(
            workbench_id(0x122)?,
            &after_create,
            WorkbenchAction::PanelSplit {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("Workspace split peer")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let new_panel_id = match split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            new_panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => new_panel_id,
        outcome => return Err(format!("Workspace PanelSplit did not start: {outcome:?}").into()),
    };
    let after_split = read_snapshot(&mut first, 0x123)?;
    let split_tab = workspace_context(&after_split)?
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or_else(|| io::Error::other("Workspace split Tab was not projected"))?;
    let bindings: Vec<&PresentationBinding> = match &split_tab.root {
        PanelNode::Split { children, .. } => children
            .iter()
            .map(|slot| {
                let PanelNode::Panel {
                    panel_id: child_panel_id,
                    primary_presentation,
                    ..
                } = &slot.node
                else {
                    return Err(io::Error::other(
                        "Workspace PanelSplit produced an unexpected nested split",
                    ));
                };
                assert!(*child_panel_id == panel_id || *child_panel_id == new_panel_id);
                primary_presentation
                    .binding
                    .as_ref()
                    .ok_or_else(|| io::Error::other("Workspace split leaf is not bound"))
            })
            .collect::<Result<_, _>>()?,
        _ => {
            return Err(
                io::Error::other("Workspace PanelSplit did not project a split root").into(),
            );
        }
    };
    assert_eq!(bindings.len(), 2);
    assert_ne!(bindings[0].terminal_id, bindings[1].terminal_id);
    assert_eq!(bindings[0].worker_id, bindings[1].worker_id);
    let launch_lines = wait_for_marker(&fixture.marker, 2)?;
    assert_eq!(
        launch_lines,
        vec![
            fixture.directory.to_string_lossy().into_owned(),
            fixture.directory.to_string_lossy().into_owned(),
        ]
    );

    let closed = exchange_command(
        &mut first,
        &command(
            workbench_id(0x124)?,
            &after_split,
            WorkbenchAction::TabClose {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
            },
        ),
    )?;
    let close_request_id = match closed.outcome {
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id, ..
        } => close_request_id,
        outcome => {
            return Err(format!("Workspace aggregate close was not accepted: {outcome:?}").into());
        }
    };
    let close_status = wait_for_closed(
        &mut second,
        &after_split,
        &setup.terminal_context_id,
        &tab_id,
        0x125,
    )?;
    let WorkbenchTabCloseReadOutcome::Available {
        close_request_id: observed_request_id,
        status: WorkbenchTabCloseStatus::Closed { .. },
        ..
    } = close_status
    else {
        return Err("Workspace aggregate close did not return a Closed status".into());
    };
    assert_eq!(observed_request_id, close_request_id);
    let after_close = read_snapshot(&mut first, 0x126)?;
    assert!(workspace_context(&after_close)?.tabs.is_empty());
    assert!(
        context(&after_close, setup.standalone_context_id.as_str())?
            .tabs
            .is_empty()
    );
    Ok(())
}

#[test]
fn real_workspace_panel_move_relocates_a_leaf_without_touching_either_terminal() -> TestResult {
    let server = RunningServer::start()?;
    let fixture = RealWorkerFixture::start(&server)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let setup = create_associated_workspace(&fixture, &mut first, &mut second)?;
    let geometry = geometry()?;

    let created = exchange_command(
        &mut first,
        &command(
            workbench_id(0x140)?,
            &setup.snapshot,
            WorkbenchAction::TabCreate {
                terminal_context_id: setup.terminal_context_id.clone(),
                label: WorkbenchLabel::parse("Workspace move source")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let (tab_id, panel_id) = match created.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => return Err(format!("Workspace TabCreate did not start: {outcome:?}").into()),
    };
    let after_create = read_snapshot(&mut second, 0x141)?;
    let split = exchange_command(
        &mut second,
        &command(
            workbench_id(0x142)?,
            &after_create,
            WorkbenchAction::PanelSplit {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("Workspace move peer")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let new_panel_id = match split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            new_panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => new_panel_id,
        outcome => return Err(format!("Workspace PanelSplit did not start: {outcome:?}").into()),
    };
    let before_move = read_snapshot(&mut first, 0x143)?;
    let before_tab = workspace_context(&before_move)?
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or_else(|| io::Error::other("Workspace move Tab was not projected"))?;
    let PanelNode::Split {
        children: before_children,
        split_id: before_split_id,
        ..
    } = &before_tab.root
    else {
        return Err(io::Error::other("Workspace split did not project a split root").into());
    };
    let before_ids: Vec<&orchbee_workbench_protocol::WorkbenchPanelId> = before_children
        .iter()
        .map(|slot| match &slot.node {
            PanelNode::Panel {
                panel_id: child, ..
            } => Ok(child),
            node => Err(io::Error::other(format!(
                "Workspace split child was not a leaf: {node:?}"
            ))),
        })
        .collect::<Result<_, io::Error>>()?;
    assert_eq!(before_ids, [&panel_id, &new_panel_id]);
    let before_slot_ids: Vec<_> = before_children
        .iter()
        .map(|slot| slot.slot_id.clone())
        .collect();
    let before_bindings: Vec<PresentationBinding> = before_children
        .iter()
        .map(|slot| match &slot.node {
            PanelNode::Panel {
                primary_presentation,
                ..
            } => primary_presentation
                .binding
                .clone()
                .ok_or_else(|| io::Error::other("Workspace move leaf is not bound")),
            _ => unreachable!(),
        })
        .collect::<Result<_, io::Error>>()?;
    let launch_lines = wait_for_marker(&fixture.marker, 2)?;
    assert_eq!(
        launch_lines,
        vec![
            fixture.directory.to_string_lossy().into_owned(),
            fixture.directory.to_string_lossy().into_owned(),
        ]
    );

    let pid_paths: Vec<PathBuf> = (0..2)
        .map(|index| {
            fixture
                .marker
                .with_file_name(format!("move-shell-{index}-pid"))
        })
        .collect();
    for (pid_sequence, (binding, path)) in before_bindings.iter().zip(&pid_paths).enumerate() {
        let pid_sequence = pid_sequence as u128;
        let attachment = attach_binding(&mut first, &before_move, binding, 0x150 + pid_sequence)?;
        let controller = acquire_controller(&mut first, &attachment, 0x158 + pid_sequence)?;
        input(
            &mut first,
            &attachment,
            &controller,
            0x160 + pid_sequence,
            1,
            format!(
                "printf '%s\\n' \"$$\" > {}\n",
                single_quote(path.to_string_lossy().as_ref())
            )
            .into_bytes(),
        )?;
    }
    let pids: Vec<u32> = pid_paths
        .iter()
        .map(|path| wait_for_pid(path))
        .collect::<TestResult<_>>()?;

    let moved = exchange_command(
        &mut second,
        &command(
            workbench_id(0x144)?,
            &before_move,
            WorkbenchAction::PanelMove {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                target_panel_id: new_panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
            },
        ),
    )?;
    let move_revision = match moved.outcome {
        WorkbenchCommandOutcome::PanelMoved {
            revision,
            terminal_context_id: moved_context,
            tab_id: moved_tab,
            panel_id: moved_panel,
            target_panel_id: moved_target,
        } => {
            assert_eq!(moved_context, setup.terminal_context_id);
            assert_eq!(moved_tab, tab_id);
            assert_eq!(moved_panel, panel_id);
            assert_eq!(moved_target, new_panel_id);
            assert_eq!(revision.value(), before_move.revision.value() + 1);
            revision
        }
        outcome => return Err(format!("Workspace PanelMove did not commit: {outcome:?}").into()),
    };
    let after_move = read_snapshot(&mut first, 0x145)?;
    assert_eq!(after_move.revision, move_revision);
    let moved_tab = workspace_context(&after_move)?
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or_else(|| io::Error::other("moved Workspace Tab was not projected"))?;
    let PanelNode::Split {
        children: after_children,
        split_id: after_split_id,
        ..
    } = &moved_tab.root
    else {
        return Err(io::Error::other("Workspace move did not project a split root").into());
    };
    let after_ids: Vec<&orchbee_workbench_protocol::WorkbenchPanelId> = after_children
        .iter()
        .map(|slot| match &slot.node {
            PanelNode::Panel {
                panel_id: child, ..
            } => Ok(child),
            node => Err(io::Error::other(format!(
                "moved split child was not a leaf: {node:?}"
            ))),
        })
        .collect::<Result<_, io::Error>>()?;
    assert_eq!(after_ids, [&new_panel_id, &panel_id]);
    assert_ne!(after_split_id, before_split_id);
    for slot in after_children.iter() {
        assert!(!before_slot_ids.contains(&slot.slot_id));
    }
    let after_bindings: Vec<PresentationBinding> = after_children
        .iter()
        .map(|slot| match &slot.node {
            PanelNode::Panel {
                primary_presentation,
                ..
            } => primary_presentation
                .binding
                .clone()
                .ok_or_else(|| io::Error::other("moved Workspace leaf is not bound")),
            _ => unreachable!(),
        })
        .collect::<Result<_, io::Error>>()?;
    assert_eq!(
        after_bindings,
        [before_bindings[1].clone(), before_bindings[0].clone()]
    );

    // The move is lifecycle-free: no terminal launch and both retained shells still run.
    let launch_lines = wait_for_marker(&fixture.marker, 2)?;
    assert_eq!(launch_lines.len(), 2);
    for (pid, path) in pids.iter().zip(&pid_paths) {
        assert!(
            process_is_alive(*pid)?,
            "moved Workspace shell PID from {} is not alive",
            path.display()
        );
    }
    Ok(())
}

#[test]
fn real_workspace_multileaf_tab_move_reorders_without_retiring() -> TestResult {
    let server = RunningServer::start()?;
    let fixture = RealWorkerFixture::start(&server)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let setup = create_associated_workspace(&fixture, &mut first, &mut second)?;
    let geometry = geometry()?;

    let created = exchange_command(
        &mut first,
        &command(
            workbench_id(0x180)?,
            &setup.snapshot,
            WorkbenchAction::TabCreate {
                terminal_context_id: setup.terminal_context_id.clone(),
                label: WorkbenchLabel::parse("Workspace multileaf source")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let (tab_id, panel_id) = match created.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => return Err(format!("Workspace TabCreate did not start: {outcome:?}").into()),
    };
    let after_create = read_snapshot(&mut second, 0x181)?;
    let split = exchange_command(
        &mut second,
        &command(
            workbench_id(0x182)?,
            &after_create,
            WorkbenchAction::PanelSplit {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("Workspace multileaf peer")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let WorkbenchCommandOutcome::PanelSplitCreated {
        terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
        ..
    } = split.outcome
    else {
        return Err(format!("Workspace PanelSplit did not start: {:?}", split.outcome).into());
    };
    let after_split = read_snapshot(&mut first, 0x183)?;
    // A second workspace Tab makes the move observable while the first Tab is multileaf.
    let other = exchange_command(
        &mut first,
        &command(
            workbench_id(0x184)?,
            &after_split,
            WorkbenchAction::TabCreate {
                terminal_context_id: setup.terminal_context_id.clone(),
                label: WorkbenchLabel::parse("Workspace move sibling")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let other_tab_id = match other.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => tab_id,
        outcome => {
            return Err(format!("second Workspace TabCreate did not start: {outcome:?}").into());
        }
    };
    let before_move = read_snapshot(&mut second, 0x185)?;
    assert_eq!(workspace_context(&before_move)?.tabs.len(), 2);

    // Workspace TabMove admission evaluates tree membership across the context, so a
    // split sibling Tab no longer retires the authority.
    let moved = exchange_command(
        &mut second,
        &command(
            workbench_id(0x186)?,
            &before_move,
            WorkbenchAction::TabMove {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                direction: WorkbenchTabMoveDirection::Last,
            },
        ),
    )?;
    let WorkbenchCommandOutcome::TabMoved { revision, .. } = moved.outcome else {
        return Err(format!(
            "Workspace multileaf TabMove did not commit: {:?}",
            moved.outcome
        )
        .into());
    };
    assert_eq!(revision.value(), before_move.revision.value() + 1);
    let after_move_first = read_snapshot(&mut first, 0x187)?;
    let after_move_second = read_snapshot(&mut second, 0x188)?;
    assert_eq!(after_move_first, after_move_second);
    let moved_tabs = &workspace_context(&after_move_first)?.tabs;
    assert_eq!(moved_tabs.len(), 2);
    assert_eq!(moved_tabs[0].tab_id, other_tab_id);
    assert_eq!(moved_tabs[1].tab_id, tab_id);
    assert!(matches!(moved_tabs[1].root, PanelNode::Split { .. }));
    assert_eq!(wait_for_marker(&fixture.marker, 3)?.len(), 3);
    Ok(())
}

#[test]
fn real_workspace_close_preserves_standalone_and_can_recreate() -> TestResult {
    let server = RunningServer::start()?;
    let fixture = RealWorkerFixture::start(&server)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let setup = create_associated_workspace(&fixture, &mut first, &mut second)?;

    let workspace_create = command(
        workbench_id(0x108)?,
        &setup.snapshot,
        WorkbenchAction::TabCreate {
            terminal_context_id: setup.terminal_context_id.clone(),
            label: WorkbenchLabel::parse("Workspace shell")?,
            geometry: geometry()?,
            launch: None,
        },
    );
    let workspace_created = exchange_command(&mut first, &workspace_create)?;
    let WorkbenchCommandOutcome::TabCreated {
        tab_id: workspace_tab_id,
        terminal_context_id,
        terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
        ..
    } = workspace_created.outcome
    else {
        return Err("Workspace TabCreate did not start a terminal".into());
    };
    assert_eq!(terminal_context_id, setup.terminal_context_id);
    let after_workspace_create = read_snapshot(&mut second, 0x109)?;
    assert_eq!(after_workspace_create, read_snapshot(&mut first, 0x10a)?);
    let workspace_tab = workspace_context(&after_workspace_create)?
        .tabs
        .iter()
        .find(|tab| tab.tab_id == workspace_tab_id)
        .ok_or_else(|| io::Error::other("started Workspace Tab was not projected"))?;
    let old_binding = first_tab_binding(workspace_tab)?.clone();
    let launch_lines = wait_for_marker(&fixture.marker, 1)?;
    assert_eq!(launch_lines[0], fixture.directory.to_string_lossy());
    let workspace_pid_witness = fixture.marker.with_file_name("workspace-shell-pid");
    let workspace_attachment =
        attach_binding(&mut second, &after_workspace_create, &old_binding, 0x120)?;
    let workspace_controller = acquire_controller(&mut second, &workspace_attachment, 0x121)?;
    input(
        &mut second,
        &workspace_attachment,
        &workspace_controller,
        0x122,
        1,
        format!(
            "printf '%s\\n' \"$$\" > {}\n",
            single_quote(workspace_pid_witness.to_string_lossy().as_ref())
        )
        .into_bytes(),
    )?;
    let workspace_pid = wait_for_pid(&workspace_pid_witness)?;
    assert!(
        process_is_alive(workspace_pid)?,
        "Workspace shell PID marker was not alive before close"
    );

    let standalone_open_id = client_id(0x10b)?;
    write_client_message(
        &mut first,
        &ClientMessage::TerminalOpen(TerminalOpenRequest::new(
            standalone_open_id.clone(),
            client_geometry()?,
        )),
    )?;
    let ServerMessage::TerminalOpenResult(opened) = read_server_message(&mut first)? else {
        return Err("standalone bareOpen did not return an Open result".into());
    };
    assert_eq!(opened.request_id(), &standalone_open_id);
    assert!(matches!(opened.outcome(), OpenOutcome::Opened { .. }));
    let before_close = read_snapshot(&mut second, 0x10c)?;
    assert_eq!(before_close, read_snapshot(&mut first, 0x10d)?);
    let standalone_tab = context(&before_close, setup.standalone_context_id.as_str())?
        .tabs
        .first()
        .ok_or_else(|| io::Error::other("standalone bareOpen Tab was not projected"))?;
    let standalone_binding = first_tab_binding(standalone_tab)?.clone();
    let standalone_tab_id = standalone_tab.tab_id.clone();
    let standalone_attachment =
        attach_binding(&mut first, &before_close, &standalone_binding, 0x10e)?;
    let standalone_controller = acquire_controller(&mut first, &standalone_attachment, 0x10f)?;

    let before_close_project = before_close
        .navigation
        .projects
        .iter()
        .find(|project| project.project_id == setup.project_id)
        .ok_or_else(|| io::Error::other("Project disappeared before Workspace Tab close"))?;
    let before_close_directory_refs = before_close_project.directories.clone();
    let before_close_workspace = before_close_project
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == setup.workspace_id)
        .ok_or_else(|| io::Error::other("Workspace disappeared before Tab close"))?;
    let before_close_directory_id = before_close_workspace.primary_directory_id.clone();
    let original_close = command(
        workbench_id(0x110)?,
        &before_close,
        WorkbenchAction::TabClose {
            terminal_context_id: setup.terminal_context_id.clone(),
            tab_id: workspace_tab_id.clone(),
        },
    );
    let original_close_result = exchange_command(&mut first, &original_close)?;
    let close_request_id = match &original_close_result.outcome {
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id,
            terminal_context_id,
            tab_id,
            ..
        } => {
            assert_eq!(terminal_context_id, &setup.terminal_context_id);
            assert_eq!(tab_id, &workspace_tab_id);
            close_request_id.clone()
        }
        outcome => {
            return Err(format!("Workspace TabClose was not accepted: {outcome:?}").into());
        }
    };
    let closed = wait_for_closed(
        &mut second,
        &before_close,
        &setup.terminal_context_id,
        &workspace_tab_id,
        0x111,
    )?;
    let WorkbenchTabCloseReadOutcome::Available {
        close_request_id: closed_request_id,
        terminal_context_id: closed_context_id,
        tab_id: closed_tab_id,
        status: WorkbenchTabCloseStatus::Closed {
            revision: closed_revision,
        },
        ..
    } = &closed
    else {
        return Err("TabCloseRead did not return a Closed tombstone".into());
    };
    assert_eq!(closed_request_id, &close_request_id);
    assert_eq!(closed_context_id, &setup.terminal_context_id);
    assert_eq!(closed_tab_id, &workspace_tab_id);

    let after_close_first = read_snapshot(&mut first, 0x112)?;
    let after_close_second = read_snapshot(&mut second, 0x113)?;
    assert_eq!(after_close_first, after_close_second);
    assert_eq!(after_close_first.revision, *closed_revision);
    assert!(workspace_context(&after_close_first)?.tabs.is_empty());
    let after_close_project = after_close_first
        .navigation
        .projects
        .iter()
        .find(|project| project.project_id == setup.project_id)
        .ok_or_else(|| io::Error::other("Project disappeared after Workspace Tab close"))?;
    assert_eq!(after_close_project.directories, before_close_directory_refs);
    let after_close_workspace = after_close_project
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == setup.workspace_id)
        .ok_or_else(|| io::Error::other("Workspace disappeared after Tab close"))?;
    assert_eq!(
        after_close_workspace.primary_directory_id,
        before_close_directory_id
    );
    let after_close_standalone = context(&after_close_first, setup.standalone_context_id.as_str())?;
    assert_eq!(after_close_standalone.tabs.len(), 1);
    assert_eq!(after_close_standalone.tabs[0].tab_id, standalone_tab_id);
    assert_eq!(
        first_tab_binding(&after_close_standalone.tabs[0])?,
        &standalone_binding
    );

    let replay = exchange_command(&mut second, &original_close)?;
    assert_eq!(replay, original_close_result);
    assert_eq!(
        read_close(
            &mut first,
            &before_close.site_id,
            &setup.terminal_context_id,
            &workspace_tab_id,
            0x114,
        )?,
        closed
    );

    // The exact canonical binding is gone after Closed, while the unrelated standalone viewer
    // remains attached and can still drive its real shell.
    wait_for_process_exit(workspace_pid)?;
    let old_binding_request_id = client_id(0x115)?;
    write_client_message(
        &mut second,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            old_binding_request_id.clone(),
            TerminalViewerAction::AttachBinding {
                site_id: ClientSiteId::parse(before_close.site_id.as_str())?,
                worker_id: ClientWorkerId::parse(old_binding.worker_id.as_str())?,
                terminal_id: ClientTerminalId::parse(old_binding.terminal_id.as_str())?,
            },
        )),
    )?;
    let ServerMessage::TerminalViewerResult(old_binding_result) = read_server_message(&mut second)?
    else {
        return Err("old Workspace binding did not return a viewer result".into());
    };
    assert_eq!(old_binding_result.request_id(), &old_binding_request_id);
    assert_eq!(
        old_binding_result.outcome(),
        &TerminalViewerOutcome::Rejected {
            reason: ViewerRejectedReason::TerminalUnavailable,
        }
    );

    let standalone_witness = fixture.marker.with_file_name("standalone-alive");
    input(
        &mut first,
        &standalone_attachment,
        &standalone_controller,
        0x116,
        1,
        format!(
            "printf 'standalone-alive\\n' > {}\n",
            single_quote(standalone_witness.to_string_lossy().as_ref())
        )
        .into_bytes(),
    )?;
    wait_for_contents(&standalone_witness, "standalone-alive\n")?;

    let recreate = command(
        workbench_id(0x117)?,
        &after_close_first,
        WorkbenchAction::TabCreate {
            terminal_context_id: setup.terminal_context_id.clone(),
            label: WorkbenchLabel::parse("Workspace shell recreated")?,
            geometry: geometry()?,
            launch: None,
        },
    );
    let recreated = exchange_command(&mut second, &recreate)?;
    let WorkbenchCommandOutcome::TabCreated {
        tab_id: recreated_tab_id,
        terminal_context_id: recreated_context_id,
        terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
        ..
    } = recreated.outcome
    else {
        return Err("Workspace Tab recreate did not start a terminal".into());
    };
    assert_eq!(recreated_context_id, setup.terminal_context_id);
    assert_ne!(recreated_tab_id, workspace_tab_id);
    let after_recreate_first = read_snapshot(&mut first, 0x118)?;
    let after_recreate_second = read_snapshot(&mut second, 0x119)?;
    assert_eq!(after_recreate_first, after_recreate_second);
    let recreated_tab = workspace_context(&after_recreate_first)?
        .tabs
        .iter()
        .find(|tab| tab.tab_id == recreated_tab_id)
        .ok_or_else(|| io::Error::other("recreated Workspace Tab was not projected"))?;
    let recreated_binding = first_tab_binding(recreated_tab)?;
    assert_ne!(recreated_binding, &old_binding);
    assert_eq!(recreated_binding.worker_id.as_str(), local_worker_wire_id());
    let launch_lines = wait_for_marker(&fixture.marker, 3)?;
    assert_eq!(launch_lines[0], fixture.directory.to_string_lossy());
    assert_eq!(launch_lines[2], fixture.directory.to_string_lossy());
    Ok(())
}

#[test]
fn real_workspace_aggregate_close_fences_same_tab_split_and_move() -> TestResult {
    let server = RunningServer::start()?;
    let fixture = RealWorkerFixture::start(&server)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let setup = create_associated_workspace(&fixture, &mut first, &mut second)?;
    let geometry = geometry()?;

    let created = exchange_command(
        &mut first,
        &command(
            workbench_id(0x200)?,
            &setup.snapshot,
            WorkbenchAction::TabCreate {
                terminal_context_id: setup.terminal_context_id.clone(),
                label: WorkbenchLabel::parse("Workspace fenced source")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let (tab_id, panel_id) = match created.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => {
            return Err(format!("Workspace fenced TabCreate did not start: {outcome:?}").into());
        }
    };
    let after_create = read_snapshot(&mut second, 0x201)?;
    let split = exchange_command(
        &mut second,
        &command(
            workbench_id(0x202)?,
            &after_create,
            WorkbenchAction::PanelSplit {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("Workspace fenced peer")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let new_panel_id = match split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            new_panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => new_panel_id,
        outcome => {
            return Err(format!("Workspace fenced PanelSplit did not start: {outcome:?}").into());
        }
    };
    let after_split = read_snapshot(&mut first, 0x203)?;
    let other = exchange_command(
        &mut first,
        &command(
            workbench_id(0x204)?,
            &after_split,
            WorkbenchAction::TabCreate {
                terminal_context_id: setup.terminal_context_id.clone(),
                label: WorkbenchLabel::parse("Workspace unrelated Tab")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    let (other_tab_id, other_panel_id) = match other.outcome {
        WorkbenchCommandOutcome::TabCreated {
            tab_id,
            panel_id,
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => (tab_id, panel_id),
        outcome => {
            return Err(format!("Workspace unrelated TabCreate did not start: {outcome:?}").into());
        }
    };
    let pending_close = read_snapshot(&mut second, 0x205)?;
    let closed = exchange_command(
        &mut second,
        &command(
            workbench_id(0x206)?,
            &pending_close,
            WorkbenchAction::TabClose {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
            },
        ),
    )?;
    let WorkbenchCommandOutcome::TabCloseAccepted { .. } = closed.outcome else {
        return Err(format!(
            "Workspace fenced aggregate close was not accepted: {:?}",
            closed.outcome
        )
        .into());
    };

    // While the aggregate Close is pending, same-Tab structural admission is refused with
    // transient terminal-busy; the refusal stages no terminal launch and retains nothing.
    let fenced_split = command(
        workbench_id(0x207)?,
        &pending_close,
        WorkbenchAction::PanelSplit {
            terminal_context_id: setup.terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Workspace fenced during close")?,
            geometry,
            launch: None,
        },
    );
    for attempt in 0..2 {
        let fenced = exchange_command(&mut first, &fenced_split)?;
        let WorkbenchCommandOutcome::Unavailable {
            reason: orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::TerminalBusy,
        } = fenced.outcome
        else {
            return Err(format!(
                "Workspace same-Tab split during pending aggregate Close was not fenced (attempt {attempt}): {:?}",
                fenced.outcome
            )
            .into());
        };
    }
    let fenced_move = exchange_command(
        &mut second,
        &command(
            workbench_id(0x208)?,
            &pending_close,
            WorkbenchAction::PanelMove {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                target_panel_id: new_panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
            },
        ),
    )?;
    let WorkbenchCommandOutcome::Unavailable {
        reason: orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::TerminalBusy,
    } = fenced_move.outcome
    else {
        return Err(format!(
            "Workspace same-Tab move during pending aggregate Close was not fenced: {:?}",
            fenced_move.outcome
        )
        .into());
    };

    // An unrelated Tab still admits and launches structural work during the pending Close.
    let other_split = exchange_command(
        &mut first,
        &command(
            workbench_id(0x209)?,
            &pending_close,
            WorkbenchAction::PanelSplit {
                terminal_context_id: setup.terminal_context_id.clone(),
                tab_id: other_tab_id.clone(),
                panel_id: other_panel_id.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("Workspace unrelated split")?,
                geometry,
                launch: None,
            },
        ),
    )?;
    match other_split.outcome {
        WorkbenchCommandOutcome::PanelSplitCreated {
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        } => {}
        outcome => {
            return Err(
                format!("Workspace unrelated Tab split was not admitted: {outcome:?}").into(),
            );
        }
    }
    let launch_lines = wait_for_marker(&fixture.marker, 4)?;
    assert_eq!(
        launch_lines.len(),
        4,
        "fenced operations must not stage terminal launches: {launch_lines:?}"
    );

    let close_status = wait_for_closed(
        &mut second,
        &pending_close,
        &setup.terminal_context_id,
        &tab_id,
        0x20a,
    )?;
    let WorkbenchTabCloseReadOutcome::Available {
        status: WorkbenchTabCloseStatus::Closed { .. },
        ..
    } = close_status
    else {
        return Err("Workspace fenced aggregate close did not reach Closed".into());
    };
    let after_close = read_snapshot(&mut first, 0x20b)?;
    let remaining = &workspace_context(&after_close)?.tabs;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].tab_id, other_tab_id);
    let PanelNode::Split { children, .. } = &remaining[0].root else {
        return Err("Workspace unrelated Tab split did not project a split root".into());
    };
    assert_eq!(children.len(), 2);

    // The fenced request retained nothing: replaying it now resolves against the published
    // revision, and the authority keeps serving commands.
    let replayed = exchange_command(&mut second, &fenced_split)?;
    let WorkbenchCommandOutcome::Rejected {
        reason: WorkbenchRejectionReason::RevisionMismatch,
        ..
    } = replayed.outcome
    else {
        return Err(format!(
            "post-publication Workspace fenced split did not re-evaluate: {:?}",
            replayed.outcome
        )
        .into());
    };
    Ok(())
}
