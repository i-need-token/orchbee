use super::{PROCESS_TIMEOUT, RunningServer, TestDirectory, TestResult, connect_bounded_client};
use orchbee_client_server_protocol::{
    ClientMessage, RequestId, ServerMessage, SiteId as ClientSiteId,
    TerminalId as ClientTerminalId, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, WorkerId as ClientWorkerId, read_server_message, write_client_message,
};
use orchbee_workbench_protocol::{
    Directory, PanelNode, PresentationBinding, ProtocolVersion, Snapshot, SnapshotReadIntent, Tab,
    TerminalContext, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchRejectionReason,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchTabMoveDirection, read_workbench_response,
    write_workbench_request,
};
use std::{
    fs,
    io::{self, Read},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Instant,
};

const LOCAL_WORKER: orchbee_local_site_protocol::WorkerId =
    orchbee_local_site_protocol::WorkerId::from_bytes(*b"orchbee-local-v1");

fn workbench_id(value: u128) -> TestResult<WorkbenchRequestId> {
    Ok(WorkbenchRequestId::parse(&format!("{value:032x}"))?)
}

fn client_id(value: u128) -> TestResult<RequestId> {
    Ok(RequestId::parse(&format!("{value:032x}"))?)
}

fn geometry() -> TestResult<orchbee_workbench_protocol::WorkbenchTerminalGeometry> {
    Ok(orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(
        80, 24, 8, 16,
    )?)
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

fn write_workbench(client: &mut UnixStream, request: &WorkbenchRequest) -> TestResult {
    write_workbench_request(client, request)?;
    Ok(())
}

fn read_command(client: &mut UnixStream) -> TestResult<WorkbenchCommandResult> {
    match read_workbench_response(client)? {
        WorkbenchResponse::Command(result) => Ok(result),
        response => Err(io::Error::other(format!(
            "expected Workbench command response, received {response:?}"
        ))
        .into()),
    }
}

fn exchange_command(
    client: &mut UnixStream,
    request: &WorkbenchRequest,
) -> TestResult<WorkbenchCommandResult> {
    write_workbench(client, request)?;
    read_command(client)
}

fn read_snapshot(client: &mut UnixStream, request_value: u128) -> TestResult<Snapshot> {
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_value)?,
    });
    write_workbench(client, &request)?;
    match read_workbench_response(client)? {
        WorkbenchResponse::Snapshot(result) => match result.outcome {
            WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
            outcome => Err(io::Error::other(format!(
                "expected available Workbench snapshot, received {outcome:?}"
            ))
            .into()),
        },
        response => Err(io::Error::other(format!(
            "expected Workbench snapshot response, received {response:?}"
        ))
        .into()),
    }
}

fn context<'a>(snapshot: &'a Snapshot, context_id: &str) -> TestResult<&'a TerminalContext> {
    snapshot
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id.as_str() == context_id)
        .ok_or_else(|| io::Error::other("missing Workspace Terminal Context").into())
}

fn workspace_context(snapshot: &Snapshot) -> TestResult<&TerminalContext> {
    let workspace = snapshot
        .navigation
        .workspaces
        .iter()
        .find(|workspace| workspace.project_id.is_some())
        .ok_or_else(|| io::Error::other("missing created Workspace"))?;
    context(snapshot, workspace.terminal_context_id.as_str())
}

fn first_tab_binding(tab: &Tab) -> TestResult<&PresentationBinding> {
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &tab.root
    else {
        return Err(io::Error::other("Workspace Tab root was not a Panel").into());
    };
    primary_presentation
        .binding
        .as_ref()
        .ok_or_else(|| io::Error::other("Workspace Tab did not retain a binding").into())
}

fn hex_identity(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}

fn local_worker_wire_id() -> String {
    hex_identity(LOCAL_WORKER.as_bytes())
}

fn single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\\"'\\\"'"))
}

fn wait_for_marker(marker: &Path, expected_lines: usize) -> TestResult<Vec<String>> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Ok(contents) = fs::read_to_string(marker) {
            let lines = contents.lines().map(str::to_owned).collect::<Vec<_>>();
            if lines.len() >= expected_lines {
                return Ok(lines);
            }
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("launch marker did not reach {expected_lines} lines"),
            )
            .into());
        }
        thread::yield_now();
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn spawn(command: &mut Command, wait_for_ready_marker: bool) -> TestResult<Self> {
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut guard = Self(Some(child));
        if wait_for_ready_marker {
            let stdout = guard
                .0
                .as_mut()
                .and_then(|child| child.stdout.take())
                .ok_or_else(|| io::Error::other("Worker stdout pipe is unavailable"))?;
            let (sender, receiver) = mpsc::sync_channel(1);
            thread::spawn(move || {
                let mut output = stdout;
                let mut marker = [0_u8; 6];
                let _ignored = sender.send(output.read_exact(&mut marker).map(|()| marker));
            });
            let marker = receiver.recv_timeout(PROCESS_TIMEOUT)??;
            if marker != *b"ready\n" {
                return Err(io::Error::other(format!(
                    "unexpected Worker readiness marker: {marker:?}"
                ))
                .into());
            }
        }
        Ok(guard)
    }

    fn kill_and_wait(&mut self) -> io::Result<()> {
        let Some(mut child) = self.0.take() else {
            return Ok(());
        };
        if child.try_wait()?.is_none() {
            child.kill()?;
        }
        let _status = child.wait()?;
        Ok(())
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let Some(mut child) = self.0.take() else {
            return;
        };
        let _ignored = child.kill();
        let _ignored = child.wait();
    }
}

struct RealWorkerFixture {
    _host: ChildGuard,
    _worker: ChildGuard,
    _fixture: TestDirectory,
    directory: PathBuf,
    marker: PathBuf,
}

impl RealWorkerFixture {
    fn start(server: &RunningServer) -> TestResult<Self> {
        let fixture = TestDirectory::new("workspace-directory")?;
        let runtime = fixture.path().join("host-runtime");
        fs::create_dir(&runtime)?;
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))?;
        let directory = fixture.path().join("workspace");
        let marker = fixture.path().join("launch-cwds");
        let shell = runtime.join("orchbee-test-shell");
        fs::write(
            &shell,
            format!(
                "#!/bin/sh\npwd >> {}\nexec /bin/sh \"$@\"\n",
                single_quote(marker.to_string_lossy().as_ref())
            ),
        )?;
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o700))?;

        let endpoint_id = "1234567890abcdef1234567890abcdef";
        let endpoint = runtime.join(format!("{endpoint_id}.s"));
        let host_binary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../terminal-host/zig-out/bin/orchbee-terminal-host");
        let worker_binary =
            Path::new(env!("CARGO_BIN_EXE_orchbee-server")).with_file_name("orchbee-worker");
        for binary in [&host_binary, &worker_binary] {
            if !binary.is_file() {
                return Err(io::Error::other(format!(
                    "build the Worker and Terminal Host prerequisites before this process witness: {}",
                    binary.display()
                ))
                .into());
            }
        }

        let mut host = ChildGuard::spawn(
            Command::new(&host_binary)
                .args(["serve", "--runtime-directory"])
                .arg(&runtime)
                .args(["--endpoint-id", endpoint_id]),
            false,
        )?;
        wait_for_host_endpoint(&mut host, &endpoint)?;
        let worker = ChildGuard::spawn(
            Command::new(&worker_binary)
                .env("SHELL", &shell)
                .args(["--local-endpoint"])
                .arg(server.worker_endpoint())
                .arg("--terminal-host-runtime-directory")
                .arg(&runtime)
                .args(["--terminal-host-endpoint-id", endpoint_id]),
            true,
        )?;
        Ok(Self {
            _host: host,
            _worker: worker,
            _fixture: fixture,
            directory,
            marker,
        })
    }

    fn kill_worker(&mut self) -> io::Result<()> {
        self._worker.kill_and_wait()
    }
}

fn wait_for_host_endpoint(host: &mut ChildGuard, endpoint: &Path) -> TestResult {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    while !endpoint.exists() {
        if let Some(child) = host.0.as_mut()
            && let Some(status) = child.try_wait()?
        {
            return Err(io::Error::other(format!(
                "Terminal Host exited before readiness: {status}"
            ))
            .into());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Terminal Host endpoint did not appear",
            )
            .into());
        }
        thread::yield_now();
    }
    Ok(())
}

#[test]
fn real_server_worker_workspace_directory_association_and_tab_lifecycle() -> TestResult {
    let server = RunningServer::start()?;
    let mut fixture = RealWorkerFixture::start(&server)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;

    let initial = read_snapshot(&mut first, 1)?;
    assert_eq!(initial, read_snapshot(&mut second, 2)?);

    let project_request = command(
        workbench_id(10)?,
        &initial,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("Workspace directory process")?,
        },
    );
    let project_result = exchange_command(&mut first, &project_request)?;
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = project_result.outcome else {
        return Err("ProjectCreate did not return ProjectCreated".into());
    };
    let after_project = read_snapshot(&mut second, 3)?;
    assert_eq!(after_project.revision.value(), 1);
    let _project = after_project
        .navigation
        .projects
        .iter()
        .find(|candidate| candidate.project_id == project_id)
        .ok_or_else(|| io::Error::other("created Project was not projected"))?;
    assert!(after_project.navigation.directories.is_empty());

    let workspace_request = command(
        workbench_id(11)?,
        &after_project,
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(project_id.clone()),
            label: WorkbenchLabel::parse("Workspace")?,
        },
    );
    let workspace_result = exchange_command(&mut second, &workspace_request)?;
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id,
        ..
    } = workspace_result.outcome
    else {
        return Err("WorkspaceCreate did not return WorkspaceCreated".into());
    };
    let after_workspace = read_snapshot(&mut first, 4)?;
    assert_eq!(after_workspace.revision.value(), 2);
    let workspace = after_workspace
        .navigation
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == workspace_id)
        .ok_or_else(|| io::Error::other("created Workspace was not projected"))?;
    assert_eq!(workspace.terminal_context_id, terminal_context_id);
    assert!(workspace.primary_directory_id.is_none());
    assert!(workspace_context(&after_workspace)?.tabs.is_empty());

    let before_association_tab = command(
        workbench_id(12)?,
        &after_workspace,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: terminal_context_id.clone(),
            label: WorkbenchLabel::parse("Must have directory")?,
            geometry: geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    assert!(matches!(
        exchange_command(&mut second, &before_association_tab)?.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: orchbee_workbench_protocol::WorkbenchRejectionReason::WorkspaceDirectoryUnavailable,
            revision,
        } if revision == after_workspace.revision
    ));
    // The Host is already running in this fixture; this marker records shell launches only.
    assert!(!fixture.marker.exists());

    let path = WorkbenchDirectoryPath::parse(
        fixture
            .directory
            .to_str()
            .ok_or_else(|| io::Error::other("Workspace directory path was not UTF-8"))?,
    )?;
    let association_request = command(
        workbench_id(13)?,
        &after_workspace,
        WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id: workspace_id.clone(),
            path: path.clone(),
        },
    );
    let unavailable_association = exchange_command(&mut first, &association_request)?;
    assert!(matches!(
        unavailable_association.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason:
                orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::DirectoryUnavailable,
        }
    ));
    assert!(!fixture.marker.exists());
    let after_unavailable_association = read_snapshot(&mut first, 5)?;
    assert_eq!(
        after_unavailable_association.revision,
        after_workspace.revision
    );
    assert_eq!(after_unavailable_association, after_workspace);
    assert_eq!(
        after_unavailable_association,
        read_snapshot(&mut second, 6)?
    );
    let _unavailable_project = after_unavailable_association
        .navigation
        .projects
        .iter()
        .find(|candidate| candidate.project_id == project_id)
        .ok_or_else(|| io::Error::other("Project disappeared after unavailable directory"))?;
    assert!(
        after_unavailable_association
            .navigation
            .directories
            .is_empty()
    );
    let unavailable_workspace = after_unavailable_association
        .navigation
        .workspaces
        .iter()
        .find(|candidate| candidate.workspace_id == workspace_id)
        .ok_or_else(|| io::Error::other("Workspace disappeared after unavailable directory"))?;
    assert!(unavailable_workspace.primary_directory_id.is_none());

    fs::create_dir(&fixture.directory)?;
    fs::set_permissions(&fixture.directory, fs::Permissions::from_mode(0o700))?;
    let association = exchange_command(&mut first, &association_request)?;
    let WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
        revision: association_revision,
        directory_id,
        worker_id,
    } = association.outcome.clone()
    else {
        return Err(format!(
            "directory association did not succeed: {:?}",
            association.outcome
        )
        .into());
    };
    assert_eq!(association_revision.value(), 3);
    assert_eq!(worker_id.as_str(), local_worker_wire_id());

    let replay = exchange_command(&mut second, &association_request)?;
    assert_eq!(replay, association);
    let after_association = read_snapshot(&mut second, 7)?;
    assert_eq!(after_association.revision, association_revision);
    assert_eq!(after_association, read_snapshot(&mut first, 8)?);
    let _associated_project = after_association
        .navigation
        .projects
        .iter()
        .find(|candidate| candidate.project_id == project_id)
        .ok_or_else(|| io::Error::other("associated Project was not projected"))?;
    assert_eq!(after_association.navigation.directories.len(), 1);
    let associated_directory: &Directory = after_association
        .navigation
        .directories
        .first()
        .ok_or_else(|| io::Error::other("associated Directory was not projected"))?;
    assert_eq!(associated_directory.directory_id, directory_id);
    assert_eq!(
        associated_directory.worker_id.as_str(),
        local_worker_wire_id()
    );
    assert_eq!(associated_directory.path, path);
    let associated_workspace = after_association
        .navigation
        .workspaces
        .iter()
        .find(|candidate| candidate.workspace_id == workspace_id)
        .ok_or_else(|| io::Error::other("associated Workspace was not projected"))?;
    assert_eq!(
        associated_workspace.primary_directory_id.as_ref(),
        Some(&directory_id)
    );
    // Directory association validates through Worker and must not launch a shell.
    assert!(!fixture.marker.exists());

    let first_tab_request = command(
        workbench_id(14)?,
        &after_association,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: terminal_context_id.clone(),
            label: WorkbenchLabel::parse("Directory shell")?,
            geometry: geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    let first_tab_result = exchange_command(&mut second, &first_tab_request)?;
    assert!(matches!(
        first_tab_result.outcome,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome: orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));
    let marker_lines = wait_for_marker(&fixture.marker, 1)?;
    assert_eq!(marker_lines[0], fixture.directory.to_string_lossy());
    let after_tab = read_snapshot(&mut first, 9)?;
    assert_eq!(after_tab.revision.value(), 4);
    let workspace_after_tab = workspace_context(&after_tab)?;
    assert_eq!(workspace_after_tab.tabs.len(), 1);
    let binding = first_tab_binding(
        workspace_after_tab
            .tabs
            .first()
            .ok_or_else(|| io::Error::other("Workspace Tab was not projected"))?,
    )?;
    assert_eq!(binding.worker_id.as_str(), local_worker_wire_id());
    assert_eq!(
        after_association.navigation.directories[0]
            .worker_id
            .as_str(),
        binding.worker_id.as_str()
    );

    let site_id = ClientSiteId::parse(after_tab.site_id.as_str())?;
    let worker_id = ClientWorkerId::parse(binding.worker_id.as_str())?;
    let terminal_id = ClientTerminalId::parse(binding.terminal_id.as_str())?;
    let attach_request_id = client_id(21)?;
    write_client_message(
        &mut first,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            attach_request_id.clone(),
            TerminalViewerAction::AttachBinding {
                site_id,
                worker_id,
                terminal_id,
            },
        )),
    )?;
    let ServerMessage::TerminalViewerResult(attached) = read_server_message(&mut first)? else {
        return Err("AttachBinding did not return a viewer result".into());
    };
    assert_eq!(attached.request_id(), &attach_request_id);
    let TerminalViewerOutcome::Attached { attachment_id, .. } = attached.into_outcome() else {
        return Err("AttachBinding failed".into());
    };
    let detach_request_id = client_id(22)?;
    write_client_message(
        &mut first,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            detach_request_id.clone(),
            TerminalViewerAction::Detach { attachment_id },
        )),
    )?;
    let ServerMessage::TerminalViewerResult(detached) = read_server_message(&mut first)? else {
        return Err("AttachBinding Detach did not return a viewer result".into());
    };
    assert_eq!(detached.request_id(), &detach_request_id);
    assert_eq!(detached.outcome(), &TerminalViewerOutcome::Detached {});

    let before_deleted_directory = read_snapshot(&mut second, 10)?;
    assert_eq!(workspace_context(&before_deleted_directory)?.tabs.len(), 1);
    let marker_lines = wait_for_marker(&fixture.marker, 1)?;
    assert_eq!(marker_lines[0], fixture.directory.to_string_lossy());

    fs::remove_dir(&fixture.directory)?;
    let _before_deleted_directory_project = before_deleted_directory
        .navigation
        .projects
        .iter()
        .find(|candidate| candidate.project_id == project_id)
        .ok_or_else(|| io::Error::other("Project disappeared before deleted-directory test"))?;
    let deleted_tab_request = command(
        workbench_id(15)?,
        &before_deleted_directory,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id,
            label: WorkbenchLabel::parse("Deleted directory")?,
            geometry: geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    let deleted_tab_result = exchange_command(&mut first, &deleted_tab_request)?;
    let failed_tab_revision = match deleted_tab_result.outcome {
        WorkbenchCommandOutcome::TabCreated {
            revision,
            terminal_outcome:
                orchbee_workbench_protocol::WorkbenchTerminalOutcome::Failed {
                    reason:
                        orchbee_workbench_protocol::WorkbenchTerminalFailureReason::TerminalCreateFailed,
                },
            ..
        } => revision,
        outcome => {
            return Err(format!(
                "deleted-directory tab did not settle as an exact launch failure: {outcome:?}"
            )
            .into());
        }
    };
    assert_eq!(
        failed_tab_revision.value(),
        before_deleted_directory.revision.value() + 1
    );
    let marker_lines = fs::read_to_string(&fixture.marker)?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(marker_lines, wait_for_marker(&fixture.marker, 1)?);
    assert_eq!(marker_lines.len(), 1);
    let final_snapshot = read_snapshot(&mut second, 11)?;
    assert_eq!(final_snapshot.revision, failed_tab_revision);
    let _final_project = final_snapshot
        .navigation
        .projects
        .iter()
        .find(|candidate| candidate.project_id == project_id)
        .ok_or_else(|| io::Error::other("Project disappeared after failed launch"))?;
    assert_eq!(
        final_snapshot.navigation.directories,
        before_deleted_directory.navigation.directories
    );
    let final_workspace = final_snapshot
        .navigation
        .workspaces
        .iter()
        .find(|candidate| candidate.workspace_id == workspace_id)
        .ok_or_else(|| io::Error::other("Workspace disappeared after failed launch"))?;
    assert_eq!(
        final_workspace.primary_directory_id.as_ref(),
        Some(&directory_id)
    );
    let final_workspace_context = workspace_context(&final_snapshot)?;
    assert_eq!(final_workspace_context.tabs.len(), 2);
    let failed_tab = final_workspace_context
        .tabs
        .get(1)
        .ok_or_else(|| io::Error::other("failed Workspace Tab was not retained"))?;
    assert_eq!(failed_tab.label, "Deleted directory");
    let failed_binding = first_tab_binding(failed_tab)?;
    assert_eq!(failed_binding.worker_id.as_str(), local_worker_wire_id());
    assert_ne!(failed_binding.terminal_id, binding.terminal_id);

    fixture.kill_worker()?;
    let new_workspace_request = command(
        workbench_id(16)?,
        &final_snapshot,
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(project_id.clone()),
            label: WorkbenchLabel::parse("Worker loss workspace")?,
        },
    );
    let new_workspace_result = exchange_command(&mut first, &new_workspace_request)?;
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id: new_workspace_id,
        terminal_context_id: new_workspace_context_id,
        revision: new_workspace_revision,
    } = new_workspace_result.outcome
    else {
        return Err("Worker loss WorkspaceCreate did not succeed".into());
    };
    assert_eq!(
        new_workspace_revision.value(),
        failed_tab_revision.value() + 1
    );
    let after_new_workspace = read_snapshot(&mut first, 12)?;
    assert_eq!(after_new_workspace, read_snapshot(&mut second, 13)?);
    assert_eq!(after_new_workspace.revision, new_workspace_revision);
    let new_workspace = after_new_workspace
        .navigation
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == new_workspace_id)
        .ok_or_else(|| io::Error::other("Worker loss Workspace was not projected"))?;
    assert_eq!(new_workspace.project_id.as_ref(), Some(&project_id));
    assert_eq!(new_workspace.terminal_context_id, new_workspace_context_id);
    assert!(new_workspace.primary_directory_id.is_none());
    assert!(
        context(&after_new_workspace, new_workspace_context_id.as_str())?
            .tabs
            .is_empty()
    );

    let worker_loss_association = command(
        workbench_id(17)?,
        &after_new_workspace,
        WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id: new_workspace_id,
            path,
        },
    );
    assert!(matches!(
        exchange_command(&mut first, &worker_loss_association)?.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason:
                orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        }
    ));
    let after_worker_loss_association_first = read_snapshot(&mut first, 14)?;
    let after_worker_loss_association = read_snapshot(&mut second, 15)?;
    assert_eq!(after_worker_loss_association_first, after_new_workspace);
    assert_eq!(after_worker_loss_association, after_new_workspace);
    let marker_lines = wait_for_marker(&fixture.marker, 1)?;
    assert_eq!(marker_lines.len(), 1);

    let old_workspace_context_id = final_snapshot
        .navigation
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == workspace_id)
        .ok_or_else(|| io::Error::other("configured Workspace disappeared before Worker loss"))?
        .terminal_context_id
        .clone();
    let old_workspace_tab_request = command(
        workbench_id(18)?,
        &after_worker_loss_association,
        WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: old_workspace_context_id,
            label: WorkbenchLabel::parse("Worker unavailable")?,
            geometry: geometry()?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    assert!(matches!(
        exchange_command(&mut first, &old_workspace_tab_request)?.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason:
                orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        }
    ));
    let final_after_worker_loss_first = read_snapshot(&mut first, 16)?;
    let final_after_worker_loss = read_snapshot(&mut second, 17)?;
    assert_eq!(final_after_worker_loss_first, after_worker_loss_association);
    assert_eq!(final_after_worker_loss, after_worker_loss_association);
    assert_eq!(wait_for_marker(&fixture.marker, 1)?.len(), 1);

    let before_move_first = final_after_worker_loss_first;
    let before_move_second = final_after_worker_loss;
    assert_eq!(before_move_first, before_move_second);
    let workspace_before_move = workspace_context(&before_move_first)?;
    assert_eq!(workspace_before_move.tabs.len(), 2);
    let first_tab_before_move = workspace_before_move
        .tabs
        .first()
        .ok_or_else(|| io::Error::other("first Workspace Tab was not projected"))?;
    let second_tab_before_move = workspace_before_move
        .tabs
        .get(1)
        .ok_or_else(|| io::Error::other("second Workspace Tab was not projected"))?;
    let first_tab_id = first_tab_before_move.tab_id.clone();
    let second_tab_id = second_tab_before_move.tab_id.clone();
    let first_binding_before_move = first_tab_binding(first_tab_before_move)?.clone();
    let second_binding_before_move = first_tab_binding(second_tab_before_move)?.clone();
    let workspace_context_id = workspace_before_move.terminal_context_id.clone();
    let move_request = command(
        workbench_id(19)?,
        &before_move_first,
        WorkbenchAction::TabMove {
            terminal_context_id: workspace_context_id.clone(),
            tab_id: first_tab_id.clone(),
            direction: WorkbenchTabMoveDirection::Last,
        },
    );
    let move_result = exchange_command(&mut first, &move_request)?;
    let WorkbenchCommandOutcome::TabMoved {
        revision: move_revision,
        terminal_context_id: moved_context_id,
        tab_id: moved_tab_id,
    } = &move_result.outcome
    else {
        return Err(format!(
            "Workspace Tab move did not succeed: {:?}",
            move_result.outcome
        )
        .into());
    };
    assert_eq!(
        *move_revision,
        WorkbenchRevision::new(before_move_first.revision.value() + 1)
    );
    assert_eq!(moved_context_id, &workspace_context_id);
    assert_eq!(moved_tab_id, &first_tab_id);

    let mut expected_after_move = before_move_first.clone();
    expected_after_move.revision = *move_revision;
    let workspace_index = expected_after_move
        .terminal_contexts
        .iter()
        .position(|context| context.terminal_context_id == workspace_context_id)
        .ok_or_else(|| io::Error::other("Workspace context disappeared after move"))?;
    expected_after_move.terminal_contexts[workspace_index].tabs = vec![
        before_move_first.terminal_contexts[workspace_index].tabs[1].clone(),
        before_move_first.terminal_contexts[workspace_index].tabs[0].clone(),
    ];
    assert_eq!(read_snapshot(&mut first, 23)?, expected_after_move);
    let after_move_second = read_snapshot(&mut second, 24)?;
    assert_eq!(after_move_second, expected_after_move);
    let moved_workspace = workspace_context(&after_move_second)?;
    assert_eq!(moved_workspace.tabs[0].tab_id, second_tab_id);
    assert_eq!(moved_workspace.tabs[1].tab_id, first_tab_id);
    assert_eq!(
        first_tab_binding(&moved_workspace.tabs[0])?,
        &second_binding_before_move
    );
    assert_eq!(
        first_tab_binding(&moved_workspace.tabs[1])?,
        &first_binding_before_move
    );
    assert_eq!(wait_for_marker(&fixture.marker, 1)?.len(), 1);

    let replay = exchange_command(&mut second, &move_request)?;
    assert_eq!(replay, move_result);
    assert_eq!(read_snapshot(&mut first, 25)?, expected_after_move);
    assert_eq!(read_snapshot(&mut second, 26)?, expected_after_move);

    let WorkbenchRequest::Command(move_command) = &move_request else {
        unreachable!("Workspace Tab move request must be a command")
    };
    let WorkbenchAction::TabMove {
        tab_id: move_tab_id,
        direction: move_direction,
        ..
    } = &move_command.action
    else {
        unreachable!("Workspace Tab move request must move a Tab")
    };
    let conflict_request = WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: move_command.request_id.clone(),
        site_id: move_command.site_id.clone(),
        expected_revision: move_command.expected_revision,
        action: WorkbenchAction::TabMove {
            terminal_context_id: expected_after_move.navigation.workspaces[1]
                .terminal_context_id
                .clone(),
            tab_id: move_tab_id.clone(),
            direction: *move_direction,
        },
    });
    let conflict = exchange_command(&mut first, &conflict_request)?;
    assert!(matches!(
        conflict.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision,
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
        } if revision == expected_after_move.revision
    ));
    assert_eq!(read_snapshot(&mut first, 27)?, expected_after_move);
    assert_eq!(read_snapshot(&mut second, 28)?, expected_after_move);
    assert_eq!(wait_for_marker(&fixture.marker, 1)?.len(), 1);
    Ok(())
}

#[path = "workspace_directory/close.rs"]
mod close;
