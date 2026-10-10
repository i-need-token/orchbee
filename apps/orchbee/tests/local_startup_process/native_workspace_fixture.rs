use super::*;
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response, write_workbench_request,
};
use std::os::{fd::AsFd, unix::net::UnixStream};

// Shared empty Workspace scene for directory and location-label process witnesses.
// CLI seeds the explicit empty object; native UI steps retain their frame gates.
// The public-protocol observer remains read-only.
pub(super) const STANDALONE_HEADER: &str = "WORKBENCH · Standalone terminal";
pub(super) const WORKSPACE_HEADER: &str = "WORKBENCH · Setup process / Chosen workspace";
pub(super) const EMPTY: &str = "No terminal tabs";
pub(super) const DIRECTORY_NEEDED: &str =
    "No directory configured. Ctrl+B e · configure the first directory";
pub(super) const DIRECTORY_READY: &str = "Ctrl+B c · launch a terminal in this directory";
pub(super) const DIRECTORY_PATH: &str = "Configure Workspace directory";
pub(super) const TEXT_ACTIONS: &str =
    "Enter · submit    Esc · cancel    Backspace · delete    Ctrl+U · clear";
pub(super) const DIRECTORY_REFUSED: &str =
    "Workspace setup not applied · check the target and input before trying again";
pub(super) const DIRECTORY_ACCEPTED: &str = "Directory configured · Ctrl+B c to launch a terminal";

pub(super) fn modal(client: &mut Client, checkpoint: usize, header: &str, rows: &[&str]) {
    client.wait_modal_prompt_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        header,
        rows,
        &mut [],
    );
}

// Empty object creation is an explicit fixture prerequisite. Default TUI creation
// now launches one shell immediately and has no Project prerequisite wizard.
pub(super) fn seed_empty_workspace(fixture: &Fixture, client: &mut Client) -> TestResult {
    use super::native_cli_query::run_json;
    let (output, project) = run_json(
        fixture,
        &[
            "project",
            "create",
            "--label",
            "Setup process",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let project_id = project["result"]["projectId"]
        .as_str()
        .ok_or("Project receipt id")?;
    let (output, _) = run_json(
        fixture,
        &[
            "workspace",
            "create",
            "--project",
            project_id,
            "--label",
            "Chosen workspace",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A navigator read observes the externally seeded context before selection.
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["Workbench navigator", "Chosen workspace"],
        &[],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[STANDALONE_HEADER],
        &["Workbench navigator"],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02]");
    wait_empty(client, checkpoint, None);
    Ok(())
}

pub(super) fn submit_directory(client: &mut Client, path: &str) -> usize {
    edit_directory(client, path);
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    checkpoint
}

pub(super) fn edit_directory(client: &mut Client, path: &str) {
    finish_modal_handoff(client, &[WORKSPACE_HEADER, EMPTY]);
    let checkpoint = client.checkpoint();
    client.send(b"\x02e");
    // The modal wraps the long hint onto two interior rows inside its inner width.
    let hint = [
        "Literal absolute path, at most 4,096 UTF-8 bytes, without NUL. No expansion or",
        "directory creation.",
    ];
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
            hint[0],
            hint[1],
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
            hint[0],
            hint[1],
            "",
            TEXT_ACTIONS,
        ],
    );
}

pub(super) fn finish_modal_handoff(client: &mut Client, required: &[&str]) {
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

// The observer has no command API: after explicit empty-object seeding, directory
// and label mutations pass through native controls and their complete-draw gate.
pub(super) struct SnapshotObserver {
    stream: UnixStream,
    next: u128,
}

impl SnapshotObserver {
    pub(super) fn connect(fixture: &Fixture) -> Result<Self, Box<dyn Error>> {
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

    pub(super) fn snapshot(&mut self) -> Result<Snapshot, Box<dyn Error>> {
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

pub(super) fn wait_empty(client: &mut Client, checkpoint: usize, notice: Option<&str>) {
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
