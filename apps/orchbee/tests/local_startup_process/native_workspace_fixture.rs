use super::tui_observer::{ModalRow, WorkbenchHeader};
use super::*;
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response, write_workbench_request,
};
use std::os::{fd::AsFd, unix::net::UnixStream};

// Shared empty Workspace scene for directory and location-label process witnesses.
// CLI seeds the explicit empty object; native UI steps retain their frame gates.
// The public-protocol observer remains read-only.
pub(super) const INITIAL_WORKSPACE_LOCATION: &str = "Workspace";
pub(super) const WORKSPACE_LOCATION: &str = "Setup process / Chosen workspace";
pub(super) const EMPTY: &str = "No terminal tabs";
pub(super) const DIRECTORY_NEEDED: &str = "No directory configured.";
pub(super) const DIRECTORY_NEEDED_ACTION: &str =
    "Ctrl+b e Configure an empty Workspace's first directory";
pub(super) const DIRECTORY_READY_ACTION: &str = "Ctrl+b c Create a Tab (requires connection)";
pub(super) const DIRECTORY_PATH: &str = "Configure Workspace directory";
pub(super) const DIRECTORY_INPUT_HINT: &str = "Enter the full path to an existing folder.";
pub(super) const TEXT_HINTS: &str = "Esc cancel · Enter submit · Backspace delete · Ctrl+u clear";
pub(super) const TEXT_ACTIONS: &str = "[ Cancel ]   [ Submit ]";
pub(super) const DIRECTORY_REFUSED: &str =
    "Workspace setup not applied · check the target and input before trying again";
pub(super) const DIRECTORY_ACCEPTED: &str = "Directory configured · ready to launch a terminal";

pub(super) fn modal(
    client: &mut Client,
    checkpoint: usize,
    header: WorkbenchHeader<'_>,
    rows: &[ModalRow<'_>],
) {
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
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        WorkbenchHeader::active(INITIAL_WORKSPACE_LOCATION, 1, "1"),
        &[],
        &["Workbench navigator"],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02]");
    wait_empty(client, checkpoint, None, None);
    Ok(())
}

pub(super) fn submit_directory(client: &mut Client, path: &str) -> usize {
    edit_directory(client, path);
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    checkpoint
}

pub(super) fn edit_directory(client: &mut Client, path: &str) {
    finish_modal_handoff(client, WorkbenchHeader::empty(WORKSPACE_LOCATION), &[EMPTY]);
    let checkpoint = client.checkpoint();
    client.send(b"\x02e");
    modal(
        client,
        checkpoint,
        WorkbenchHeader::empty(WORKSPACE_LOCATION),
        &[
            ModalRow::exact(DIRECTORY_PATH),
            ModalRow::exact(""),
            ModalRow::exact("Project: Setup process"),
            ModalRow::exact(""),
            ModalRow::exact("Workspaces: Chosen workspace"),
            ModalRow::exact(""),
            ModalRow::field_at_end(""),
            ModalRow::exact(""),
            ModalRow::exact(DIRECTORY_INPUT_HINT),
            ModalRow::exact(""),
            ModalRow::exact(TEXT_HINTS),
            ModalRow::exact(TEXT_ACTIONS),
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(path.as_bytes());
    modal(
        client,
        checkpoint,
        WorkbenchHeader::empty(WORKSPACE_LOCATION),
        &[
            ModalRow::exact(DIRECTORY_PATH),
            ModalRow::exact(""),
            ModalRow::exact("Project: Setup process"),
            ModalRow::exact(""),
            ModalRow::exact("Workspaces: Chosen workspace"),
            ModalRow::exact(""),
            ModalRow::field_at_end(path),
            ModalRow::exact(""),
            ModalRow::exact(DIRECTORY_INPUT_HINT),
            ModalRow::exact(""),
            ModalRow::exact(TEXT_HINTS),
            ModalRow::exact(TEXT_ACTIONS),
        ],
    );
}

pub(super) fn finish_modal_handoff(
    client: &mut Client,
    header: WorkbenchHeader<'_>,
    required: &[&str],
) {
    // ModalDecoder deliberately consumes unframed input for 50 ms after submit/cancel.
    // Observe the completed context frame throughout that existing window, with a small
    // scheduling margin; never send retry input that could mask a dropped stage transition.
    let until = Instant::now() + Duration::from_millis(75);
    while Instant::now() < until {
        assert!(
            client.current_frame_matches(header, required),
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
        let cohort = Cohort::load(&fixture.cohort, deadline)?;
        let admitted = ClientConnection::connect_before(
            fixture.session().client_socket_path(),
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

pub(super) fn wait_empty(
    client: &mut Client,
    checkpoint: usize,
    directory: Option<&str>,
    notice: Option<&str>,
) {
    // The sidebar clips the generic phase text. Observe the active context's complete
    // Empty-only panel guidance, alongside the absence of Tabs and canonical snapshots.
    let fact = if let Some(directory) = directory {
        format!("Directory: {directory}")
    } else {
        DIRECTORY_NEEDED.to_owned()
    };
    let action = if directory.is_some() {
        DIRECTORY_READY_ACTION
    } else {
        DIRECTORY_NEEDED_ACTION
    };
    let mut required = vec![EMPTY, fact.as_str(), action];
    required.extend(notice);
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        WorkbenchHeader::empty(WORKSPACE_LOCATION),
        &required,
        &["Workbench navigator"],
        &[],
        &mut [],
    );
}
