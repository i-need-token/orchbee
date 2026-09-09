use super::*;
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response, write_workbench_request,
};
use std::os::{fd::AsFd, unix::net::UnixStream};

const STANDALONE_HEADER: &str = "WORKBENCH · Standalone terminal";
const WORKSPACE_HEADER: &str = "WORKBENCH · Setup process / Chosen workspace";
const EMPTY: &str = "No terminal tabs";
const DIRECTORY_NEEDED: &str = "No directory configured. Ctrl+B e · configure the first directory";
const DIRECTORY_READY: &str = "Ctrl+B c · launch a terminal in this directory";
const PROJECT_CREATED: &str = "Project created · the committed Project is retained";
const CHOOSE_PROJECT: &str = "Create Workspace · choose Project";
const PROJECT_NAME: &str = "Create Workspace · new Project name";
const WORKSPACE_NAME: &str = "Create Workspace · Workspace name";
const DIRECTORY_PATH: &str = "Configure Workspace directory";
const LABEL_HINT: &str = "Use 1–512 characters. Submit each step separately.";
const TEXT_ACTIONS: &str = "Enter · submit    Esc · cancel    Backspace · delete    Ctrl+U · clear";
const PROJECT_RETAINED: &str = "Project created. Cancelling this step keeps the Project.";
const DIRECTORY_REFUSED: &str =
    "Workspace setup not applied · check the target and input before trying again";
const DIRECTORY_ACCEPTED: &str = "Directory configured · Ctrl+B c to launch a terminal";

fn modal(client: &mut Client, checkpoint: usize, header: &str, rows: &[&str]) {
    client.wait_modal_prompt_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        header,
        rows,
        &mut [],
    );
}

fn open_new_project(client: &mut Client) {
    let checkpoint = client.checkpoint();
    client.send(b"\x02a");
    modal(
        client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            CHOOSE_PROJECT,
            "",
            "> 1. New Project",
            "",
            "[ / ] · choose Project    Enter · continue    Esc · cancel",
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    modal(
        client,
        checkpoint,
        STANDALONE_HEADER,
        &[PROJECT_NAME, "", "", "", LABEL_HINT, "", TEXT_ACTIONS],
    );
    finish_modal_handoff(client, &[STANDALONE_HEADER, PROJECT_NAME]);
}

fn submit_project(client: &mut Client, label: &str) {
    let checkpoint = client.checkpoint();
    client.send(label.as_bytes());
    modal(
        client,
        checkpoint,
        STANDALONE_HEADER,
        &[PROJECT_NAME, "", label, "", LABEL_HINT, "", TEXT_ACTIONS],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    modal(
        client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            WORKSPACE_NAME,
            "",
            &format!("Project: {label}"),
            "",
            "",
            "",
            LABEL_HINT,
            "",
            PROJECT_RETAINED,
            "",
            TEXT_ACTIONS,
        ],
    );
    finish_modal_handoff(client, &[STANDALONE_HEADER, WORKSPACE_NAME]);
}

fn submit_workspace(client: &mut Client, label: &str) -> usize {
    let checkpoint = client.checkpoint();
    client.send(label.as_bytes());
    modal(
        client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            WORKSPACE_NAME,
            "",
            "Project: Setup process",
            "",
            label,
            "",
            LABEL_HINT,
            "",
            PROJECT_RETAINED,
            "",
            TEXT_ACTIONS,
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    checkpoint
}

fn submit_directory(client: &mut Client, path: &str) -> usize {
    finish_modal_handoff(client, &[WORKSPACE_HEADER, EMPTY]);
    let checkpoint = client.checkpoint();
    client.send(b"\x02e");
    let hint = "Literal absolute path, at most 4,096 UTF-8 bytes, without NUL. No expansion or directory creation.";
    modal(
        client,
        checkpoint,
        WORKSPACE_HEADER,
        &[
            DIRECTORY_PATH,
            "",
            "Project: Setup process",
            "",
            "Workspaces: Chosen workspace",
            "",
            "",
            "",
            hint,
            "",
            TEXT_ACTIONS,
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(path.as_bytes());
    modal(
        client,
        checkpoint,
        WORKSPACE_HEADER,
        &[
            DIRECTORY_PATH,
            "",
            "Project: Setup process",
            "",
            "Workspaces: Chosen workspace",
            "",
            path,
            "",
            hint,
            "",
            TEXT_ACTIONS,
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    checkpoint
}

fn finish_modal_handoff(client: &mut Client, required: &[&str]) {
    // ModalDecoder deliberately consumes unframed input for 50 ms after submit/cancel.
    // Observe the completed context frame throughout that existing window, with a small
    // scheduling margin; never send retry input that could mask a dropped stage transition.
    let until = Instant::now() + Duration::from_millis(75);
    while Instant::now() < until {
        client.collect();
        assert!(
            required.iter().all(|text| client.contains(text)),
            "completed context or setup stage changed during modal handoff: {required:?}",
        );
        thread::yield_now();
    }
}

// The observer has no command API: every Project, Workspace and Directory mutation below
// must pass through the visible native controls, including their complete-draw gate.
struct SnapshotObserver {
    stream: UnixStream,
    next: u128,
}

impl SnapshotObserver {
    fn connect(fixture: &Fixture) -> Result<Self, Box<dyn Error>> {
        let deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
        let cohort = NativeCohort::load(&fixture.cohort, deadline)?;
        let admitted = ClientSessionConnection::connect_before(
            fixture.site().client_socket_path(),
            cohort.identity(),
            deadline,
        )?;
        Ok(Self {
            stream: UnixStream::from(admitted.as_fd().try_clone_to_owned()?),
            next: 1,
        })
    }

    fn snapshot(&mut self) -> Result<Snapshot, Box<dyn Error>> {
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        self.stream.set_read_timeout(Some(OBSERVATION_TIMEOUT))?;
        self.stream.set_write_timeout(Some(OBSERVATION_TIMEOUT))?;
        let request_id = WorkbenchRequestId::parse(&format!("{:032x}", self.next))?;
        self.next += 1;
        write_workbench_request(
            &mut self.stream,
            &WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
            }),
        )?;
        let WorkbenchResponse::Snapshot(result) = read_workbench_response(&mut self.stream)? else {
            return Err("expected read-only Snapshot response".into());
        };
        assert!(
            Instant::now() < deadline,
            "snapshot observation arrived late"
        );
        assert_eq!(result.request_id, request_id);
        let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
            return Err("Snapshot unavailable".into());
        };
        Ok(snapshot)
    }
}

fn wait_empty(client: &mut Client, checkpoint: usize, notice: Option<&str>) {
    // The sidebar clips the generic phase text. Observe the active context's complete
    // Empty-only panel guidance, alongside the absence of Tabs and canonical snapshots.
    let guidance = if notice == Some(DIRECTORY_ACCEPTED) {
        DIRECTORY_READY
    } else {
        DIRECTORY_NEEDED
    };
    let mut required = vec![WORKSPACE_HEADER, EMPTY, guidance];
    required.extend(notice);
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &required,
        &["[*1", "Workbench navigator"],
        &[],
        &mut [],
    );
}

#[test]
fn native_workspace_setup_uses_literal_directory_and_only_explicitly_launches() -> TestResult {
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
    client.send(shell_identity_command(&fixture, "standalone", "setup-before", true).as_bytes());
    let standalone = wait_file(
        &fixture.base.join("setup-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let standalone_pid = require_shell_identity(&standalone, "standalone");
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let initial = observer.snapshot()?;
    assert!(
        initial.navigation.projects.is_empty(),
        "fresh Site has no Projects"
    );
    assert_eq!(initial.terminal_contexts.len(), 1);
    assert_eq!(initial.terminal_contexts[0].tabs.len(), 1);

    open_new_project(&mut client);
    submit_project(&mut client, "Setup process");
    let partial = observer.snapshot()?;
    assert_eq!(partial.site_id, initial.site_id);
    assert!(partial.revision > initial.revision);
    assert_eq!(partial.terminal_contexts, initial.terminal_contexts);
    assert_eq!(partial.navigation.projects.len(), 1);
    let project = &partial.navigation.projects[0];
    assert_eq!(project.label, "Setup process");
    assert!(project.workspaces.is_empty());
    assert!(project.directories.is_empty());

    let checkpoint = submit_workspace(&mut client, "Chosen workspace");
    wait_empty(&mut client, checkpoint, None);
    let created = observer.snapshot()?;
    assert_eq!(created.site_id, initial.site_id);
    assert!(created.revision > partial.revision);
    assert_eq!(created.navigation.projects.len(), 1);
    let project = &created.navigation.projects[0];
    assert_eq!(
        project.project_id,
        partial.navigation.projects[0].project_id
    );
    assert_eq!(project.workspaces.len(), 1);
    assert!(project.directories.is_empty());
    let workspace = &project.workspaces[0];
    assert_eq!(workspace.label, "Chosen workspace");
    assert!(workspace.primary_directory_id.is_none());
    assert_eq!(created.terminal_contexts.len(), 2);
    let empty_context = created
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)
        .ok_or("created Workspace context missing")?;
    assert!(empty_context.tabs.is_empty());
    assert!(empty_context.groups.is_empty());
    let retained_standalone = created
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == initial.navigation.standalone_terminal_context_id
        })
        .ok_or("standalone context missing")?;
    assert_eq!(retained_standalone, &initial.terminal_contexts[0]);

    // A shell-looking suffix stays literal. The missing parent makes it invalid even if
    // a buggy implementation tries to expand "$HOME" or otherwise choose a fallback.
    let invalid = fixture.base.join("missing directory/$HOME");
    let checkpoint = submit_directory(&mut client, invalid.to_str().ok_or("path UTF-8")?);
    wait_empty(&mut client, checkpoint, Some(DIRECTORY_REFUSED));
    assert!(!invalid.exists());
    assert_eq!(
        observer.snapshot()?,
        created,
        "refusal has no fallback or canonical mutation"
    );

    let directory = fixture.base.join("chosen directory with spaces");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let checkpoint = submit_directory(&mut client, directory.to_str().ok_or("path UTF-8")?);
    wait_empty(&mut client, checkpoint, Some(DIRECTORY_ACCEPTED));
    let configured = observer.snapshot()?;
    assert!(configured.revision > created.revision);
    assert_eq!(configured.site_id, created.site_id);
    assert_eq!(
        configured.terminal_contexts, created.terminal_contexts,
        "directory acceptance does not create any Workspace Tab or terminal binding"
    );
    let configured_project = &configured.navigation.projects[0];
    assert_eq!(configured.navigation.projects.len(), 1);
    assert_eq!(configured_project.project_id, project.project_id);
    assert_eq!(configured_project.workspaces.len(), 1);
    assert_eq!(configured_project.directories.len(), 1);
    let configured_workspace = &configured_project.workspaces[0];
    assert_eq!(configured_workspace.workspace_id, workspace.workspace_id);
    assert_eq!(
        configured_workspace.terminal_context_id,
        workspace.terminal_context_id
    );
    let accepted_directory = &configured_project.directories[0];
    assert_eq!(
        configured_workspace.primary_directory_id.as_ref(),
        Some(&accepted_directory.directory_id)
    );
    assert_eq!(
        accepted_directory.path.as_str(),
        directory.to_str().ok_or("path UTF-8")?
    );
    let mut expected_configured = created.clone();
    expected_configured.revision = configured.revision;
    expected_configured.navigation.projects[0].directories = configured_project.directories.clone();
    expected_configured.navigation.projects[0].workspaces[0].primary_directory_id =
        Some(accepted_directory.directory_id.clone());
    assert_eq!(
        configured, expected_configured,
        "acceptance changes only this Workspace's directory association"
    );

    finish_modal_handoff(&mut client, &[WORKSPACE_HEADER, EMPTY]);
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        &[WORKSPACE_HEADER, "[*1 1]", "Tab 1 · Ready · controlling"],
        &[EMPTY],
        &[],
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "workspace", "setup-workspace", true).as_bytes());
    let workspace_shell = wait_file(
        &fixture.base.join("setup-workspace"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    assert_ne!(
        require_shell_identity(&workspace_shell, "workspace"),
        standalone_pid
    );
    client.send(format!("pwd -P > {}/setup-cwd\r", fixture.base.display()).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("setup-cwd"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        )
        .trim_end(),
        directory.to_str().ok_or("path UTF-8")?,
    );
    let launched = observer.snapshot()?;
    let launched_context = launched
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)
        .ok_or("launched Workspace context missing")?;
    assert_eq!(launched_context.tabs.len(), 1);
    assert!(launched.revision > configured.revision);
    let mut expected = configured.clone();
    expected.revision = launched.revision;
    expected
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)
        .ok_or("configured Workspace context missing")?
        .tabs = launched_context.tabs.clone();
    assert_eq!(
        launched, expected,
        "explicit c changes only the selected Workspace's Tabs"
    );

    let checkpoint = client.checkpoint();
    client.send(b"\x02[");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[STANDALONE_HEADER, "[*1 1]", "Tab 1 · Ready · controlling"],
        &[WORKSPACE_HEADER, EMPTY],
        &[],
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "", "setup-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("setup-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        standalone,
        "native setup retains the exact standalone PID and shell-local variable",
    );
    assert_eq!(observer.snapshot()?, launched);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_workspace_setup_cancel_keeps_the_already_committed_project() -> TestResult {
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
    client.send(shell_identity_command(&fixture, "cancel", "cancel-before", true).as_bytes());
    let standalone = wait_file(
        &fixture.base.join("cancel-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    require_shell_identity(&standalone, "cancel");
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let initial = observer.snapshot()?;
    assert!(initial.navigation.projects.is_empty());
    open_new_project(&mut client);
    submit_project(&mut client, "Retained project");
    let committed = observer.snapshot()?;
    assert!(committed.revision > initial.revision);
    assert_eq!(committed.terminal_contexts, initial.terminal_contexts);
    assert_eq!(committed.navigation.projects.len(), 1);
    assert_eq!(committed.navigation.projects[0].label, "Retained project");
    assert!(committed.navigation.projects[0].workspaces.is_empty());
    assert!(committed.navigation.projects[0].directories.is_empty());

    let checkpoint = client.checkpoint();
    client.send(b"\x1b");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[STANDALONE_HEADER, "[*1 1]", PROJECT_CREATED],
        &[WORKSPACE_NAME],
        &[],
        &mut [],
    );
    assert_eq!(
        observer.snapshot()?,
        committed,
        "cancel must retain the known Project commit"
    );
    finish_modal_handoff(&mut client, &[STANDALONE_HEADER, "[*1 1]", PROJECT_CREATED]);
    // The retained commit notice legitimately occupies the footer after cancellation.
    // Prove control with actual shell execution instead of its clipped sidebar label.
    assert_shell_survives(&fixture, &mut client, &standalone, "cancel-after");

    // Reopening sees the retained empty Project, but standalone still defaults to New Project.
    let checkpoint = client.checkpoint();
    client.send(b"\x02a");
    modal(
        &mut client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            CHOOSE_PROJECT,
            "",
            "1. Retained project",
            "> 2. New Project",
            "",
            "[ / ] · choose Project    Enter · continue    Esc · cancel",
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"[");
    modal(
        &mut client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            CHOOSE_PROJECT,
            "",
            "> 1. Retained project",
            "2. New Project",
            "",
            "[ / ] · choose Project    Enter · continue    Esc · cancel",
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    modal(
        &mut client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            WORKSPACE_NAME,
            "",
            "Project: Retained project",
            "",
            "",
            "",
            LABEL_HINT,
            "",
            TEXT_ACTIONS,
        ],
    );
    assert_eq!(
        observer.snapshot()?,
        committed,
        "selecting an existing Project does not create one"
    );
    finish_modal_handoff(&mut client, &[STANDALONE_HEADER, WORKSPACE_NAME]);
    let checkpoint = client.checkpoint();
    client.send(b"\x1b");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[STANDALONE_HEADER, "[*1 1]", "Tab 1 · Ready · controlling"],
        &[WORKSPACE_NAME],
        &[],
        &mut [],
    );
    finish_modal_handoff(
        &mut client,
        &[STANDALONE_HEADER, "Tab 1 · Ready · controlling"],
    );
    assert_eq!(observer.snapshot()?, committed);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
