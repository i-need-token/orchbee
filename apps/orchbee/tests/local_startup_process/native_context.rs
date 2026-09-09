use super::*;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response,
    write_workbench_request,
};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use std::os::{fd::AsFd, unix::net::UnixStream};

const EMPTY: &str = "No terminal tabs · Ctrl+B c to create a Tab";
const STANDALONE_HEADER: &str = "WORKBENCH · Standalone terminal";
const CONFIGURED_HEADER: &str = "WORKBENCH · Context process / Configured";
const UNCONFIGURED_HEADER: &str = "WORKBENCH · Context process / Unconfigured";
const CLOSE_TIMEOUT: Duration = Duration::from_secs(45);

// Fixture preparation and canonical observation use the public OBLC/OBWP connection.
// Only the PTY-driven native Client selects contexts or creates terminal Tabs.
struct WorkbenchPeer {
    stream: UnixStream,
    next: u128,
}

impl WorkbenchPeer {
    fn connect(fixture: &Fixture, deadline: Instant) -> Result<Self, Box<dyn Error>> {
        let cohort = NativeCohort::load(&fixture.cohort, deadline)?;
        let admitted = ClientSessionConnection::connect_before(
            fixture.site().client_socket_path(),
            cohort.identity(),
            deadline,
        )?;
        let stream = UnixStream::from(admitted.as_fd().try_clone_to_owned()?);
        Ok(Self { stream, next: 1 })
    }

    fn request_id(&mut self) -> Result<WorkbenchRequestId, Box<dyn Error>> {
        let id = WorkbenchRequestId::parse(&format!("{:032x}", self.next))?;
        self.next += 1;
        Ok(id)
    }

    fn exchange(
        &mut self,
        request: WorkbenchRequest,
        deadline: Instant,
    ) -> Result<WorkbenchResponse, Box<dyn Error>> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or("Workbench observation deadline expired")?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.set_write_timeout(Some(remaining))?;
        write_workbench_request(&mut self.stream, &request)?;
        let response = read_workbench_response(&mut self.stream)?;
        assert!(Instant::now() < deadline, "Workbench response arrived late");
        Ok(response)
    }

    fn snapshot(&mut self, deadline: Instant) -> Result<Snapshot, Box<dyn Error>> {
        let request_id = self.request_id()?;
        let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
        });
        let WorkbenchResponse::Snapshot(result) = self.exchange(request, deadline)? else {
            return Err("expected Snapshot response".into());
        };
        assert_eq!(result.request_id, request_id);
        let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
            return Err("Snapshot unavailable".into());
        };
        Ok(snapshot)
    }

    fn prepare(
        &mut self,
        action: WorkbenchAction,
        deadline: Instant,
    ) -> Result<WorkbenchCommandOutcome, Box<dyn Error>> {
        let snapshot = self.snapshot(deadline)?;
        let request_id = self.request_id()?;
        let request = WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            site_id: snapshot.site_id,
            expected_revision: snapshot.revision,
            action,
        });
        let WorkbenchResponse::Command(result) = self.exchange(request, deadline)? else {
            return Err("expected preparation Command response".into());
        };
        assert_eq!(result.request_id, request_id);
        Ok(result.outcome)
    }
}

fn select_context(client: &mut Client, command: &[u8], header: &str, empty: bool) {
    let checkpoint = client.checkpoint();
    client.send(command);
    // The sidebar always says "Standalone terminal". Only the active header distinguishes
    // a completed context switch from a fresh draw of the previous Ready Workspace.
    let required = if empty {
        vec![header, EMPTY]
    } else {
        vec![header, "[*1 1]", "Tab 1 · Ready · controlling"]
    };
    let forbidden = if empty {
        vec!["Workbench navigator", "[*1"]
    } else {
        vec!["Workbench navigator", EMPTY]
    };
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &required,
        &forbidden,
        &[],
        &mut [],
    );
}

fn observe_independent_standalone(client: &mut Client, revision: u64, active: &mut Client) {
    let deadline = Instant::now() + SWITCH_TIMEOUT;
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    let refreshed = format!("Tabs refreshed · revision {revision}");
    client.wait_frame_text_after(
        checkpoint,
        deadline,
        &[
            "Workbench navigator",
            &refreshed,
            "[*1] Standalone terminal",
        ],
        &[],
        &[],
        &mut [active],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        deadline,
        &[
            STANDALONE_HEADER,
            "[*1 1]",
            "Tab 1 · Ready ·",
            "STANDALONE_PEER_BINDING",
        ],
        &[
            "Workbench navigator",
            CONFIGURED_HEADER,
            UNCONFIGURED_HEADER,
            EMPTY,
        ],
        &[],
        &mut [active],
    );
}

#[test]
fn native_workspace_selection_creates_only_explicitly_and_retains_exact_shells() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    client
        .send(shell_identity_command(&fixture, "standalone", "standalone-before", true).as_bytes());
    let standalone = wait_file(
        &fixture.base.join("standalone-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let standalone_pid = require_shell_identity(&standalone, "standalone");
    let deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, deadline)?;
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = peer.prepare(
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("Context process")?,
        },
        deadline,
    )?
    else {
        return Err("Project preparation failed".into());
    };
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id: configured_context,
        ..
    } = peer.prepare(
        WorkbenchAction::WorkspaceCreate {
            project_id: project_id.clone(),
            label: WorkbenchLabel::parse("Configured")?,
        },
        deadline,
    )?
    else {
        return Err("configured Workspace preparation failed".into());
    };
    let directory = fixture.base.join("workspace with spaces");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    assert!(matches!(
        peer.prepare(
            WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                project_id: project_id.clone(),
                workspace_id,
                path: WorkbenchDirectoryPath::parse(directory.to_str().ok_or("directory UTF-8")?)?,
            },
            deadline,
        )?,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    assert!(matches!(
        peer.prepare(
            WorkbenchAction::WorkspaceCreate {
                project_id,
                label: WorkbenchLabel::parse("Unconfigured")?,
            },
            deadline,
        )?,
        WorkbenchCommandOutcome::WorkspaceCreated { .. }
    ));
    let baseline = peer.snapshot(deadline)?;
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    let refreshed = format!("Tabs refreshed · revision {}", baseline.revision.value());
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Workbench navigator",
            &refreshed,
            "Configured",
            "Unconfigured",
        ],
        &[],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["Tab 1 · Ready · controlling"],
        &["Workbench navigator"],
        &[],
        &mut [],
    );

    // A second native Client observes the retained binding without acquiring control.
    // The joined output token does not occur in the input command's shell echo.
    let checkpoint = client.checkpoint();
    client.send(b"printf '%s%s\\n' STANDALONE _PEER_BINDING\r");
    client.wait_terminal_output_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        "STANDALONE_PEER_BINDING",
    );
    let mut independent = Client::spawn(&fixture, "en_US.UTF-8");
    independent.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::ViewOnly,
    );
    select_context(&mut client, b"\x02]", CONFIGURED_HEADER, true);
    observe_independent_standalone(&mut independent, baseline.revision.value(), &mut client);
    assert_eq!(
        peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        baseline
    );
    // Leaving and returning to Empty must neither mutate navigation nor start a shell.
    select_context(&mut client, b"\x02[", STANDALONE_HEADER, false);
    select_context(&mut client, b"\x02]", CONFIGURED_HEADER, true);
    assert_eq!(
        peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        baseline
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        &[CONFIGURED_HEADER, "[*1 1]", "Tab 1 · Ready · controlling"],
        &[EMPTY],
        &[],
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "workspace", "workspace-before", true).as_bytes());
    let workspace = wait_file(
        &fixture.base.join("workspace-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let workspace_pid = require_shell_identity(&workspace, "workspace");
    assert_ne!(workspace_pid, standalone_pid);
    let workspace_shell = pidfd_open(
        Pid::from_raw(i32::try_from(workspace_pid)?).ok_or("invalid Workspace shell PID")?,
        PidfdFlags::empty(),
    )?;
    client.send(format!("pwd -P > {}/workspace-cwd\r", fixture.base.display()).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("workspace-cwd"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        )
        .trim_end(),
        directory.to_str().ok_or("directory UTF-8")?
    );
    let created = peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?;
    let mut expected = baseline.clone();
    let created_context = created
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == configured_context)
        .ok_or("configured context missing")?;
    assert_eq!(created_context.tabs.len(), 1);
    assert!(created.revision > baseline.revision);
    expected.revision = created.revision;
    expected
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == configured_context)
        .ok_or("baseline configured context missing")?
        .tabs = created_context.tabs.clone();
    assert_eq!(
        created, expected,
        "Create changes only active Workspace membership"
    );
    observe_independent_standalone(&mut independent, created.revision.value(), &mut client);

    select_context(&mut client, b"\x02[", STANDALONE_HEADER, false);
    client.send(shell_identity_command(&fixture, "", "standalone-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("standalone-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        standalone
    );
    select_context(&mut client, b"\x02]", CONFIGURED_HEADER, false);
    client.send(shell_identity_command(&fixture, "", "workspace-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("workspace-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        workspace
    );

    // The native confirmation must target the active Workspace's retained Tab.
    let checkpoint = client.checkpoint();
    client.send(b"\x02x");
    client.wait_close_prompt_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        CONFIGURED_HEADER,
        &[
            "Close terminal Tab?",
            "",
            "1",
            "",
            "Closing this Tab ends its execution for all attached Clients.",
            "",
            "y · confirm    n / Esc · cancel",
        ],
        &mut [&mut independent],
    );
    let checkpoint = client.checkpoint();
    let deadline = Instant::now() + CLOSE_TIMEOUT;
    client.send(b"y");
    client.wait_frame_text_after(
        checkpoint,
        deadline,
        &[CONFIGURED_HEADER, EMPTY],
        &["Close terminal Tab?", "[*1", "Workbench navigator"],
        &[],
        &mut [&mut independent],
    );
    let closed = peer.snapshot(deadline)?;
    assert!(closed.revision > created.revision);
    let mut expected_closed = created.clone();
    expected_closed.revision = closed.revision;
    expected_closed
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == configured_context)
        .ok_or("configured context missing after Close")?
        .tabs
        .clear();
    assert_eq!(
        closed, expected_closed,
        "Close removes only the Workspace Tab and preserves standalone membership"
    );
    loop {
        let mut descriptor = [PollFd::new(workspace_shell.as_fd(), PollFlags::POLLIN)];
        poll(&mut descriptor, PollTimeout::ZERO)?;
        assert!(
            Instant::now() < deadline,
            "Workspace shell exit arrived late"
        );
        if descriptor[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN))
        {
            break;
        }
        client.collect();
        independent.collect();
        thread::yield_now();
    }
    observe_independent_standalone(&mut independent, closed.revision.value(), &mut client);

    select_context(&mut client, b"\x02]", UNCONFIGURED_HEADER, true);
    observe_independent_standalone(&mut independent, closed.revision.value(), &mut client);
    assert_eq!(peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?, closed);
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        &[UNCONFIGURED_HEADER, EMPTY, "Tab not created"],
        &["[*1"],
        &[],
        &mut [],
    );
    assert_eq!(
        peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        closed,
        "directory rejection must not create a fallback terminal"
    );
    select_context(&mut client, b"\x02]", STANDALONE_HEADER, false);
    client.send(shell_identity_command(&fixture, "", "standalone-after-close", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("standalone-after-close"),
            &mut [&mut client, &mut independent],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        standalone,
        "Workspace Close preserves the exact standalone shell and its variable"
    );
    assert_eq!(peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?, closed);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    independent.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
