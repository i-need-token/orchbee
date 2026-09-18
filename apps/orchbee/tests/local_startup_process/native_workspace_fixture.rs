use super::*;
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchSnapshotOutcome, read_workbench_response, write_workbench_request,
};
use std::os::{fd::AsFd, unix::net::UnixStream};

// Shared native Workspace setup scene for creation and location-label process witnesses.
// UI steps retain their frame gates; the public-protocol observer cannot submit commands.
pub(super) const STANDALONE_HEADER: &str = "WORKBENCH · Standalone terminal";
pub(super) const WORKSPACE_HEADER: &str = "WORKBENCH · Setup process / Chosen workspace";
pub(super) const EMPTY: &str = "No terminal tabs";
const DIRECTORY_NEEDED: &str = "No directory configured. Ctrl+B e · configure the first directory";
const DIRECTORY_READY: &str = "Ctrl+B c · launch a terminal in this directory";
pub(super) const PROJECT_CREATED: &str = "Project created · the committed Project is retained";
pub(super) const CHOOSE_PROJECT: &str = "Create Workspace · choose Project";
const PROJECT_NAME: &str = "Create Workspace · new Project name";
pub(super) const WORKSPACE_NAME: &str = "Create Workspace · Workspace name";
const DIRECTORY_PATH: &str = "Configure Workspace directory";
pub(super) const LABEL_HINT: &str = "Use 1–512 characters. Submit each step separately.";
pub(super) const TEXT_ACTIONS: &str =
    "Enter · submit    Esc · cancel    Backspace · delete    Ctrl+U · clear";
const PROJECT_RETAINED: &str = "Project created. Cancelling this step keeps the Project.";
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

pub(super) fn open_new_project(client: &mut Client) {
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

pub(super) fn submit_project(client: &mut Client, label: &str) {
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

pub(super) fn submit_workspace(client: &mut Client, label: &str) -> usize {
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

pub(super) fn submit_directory(client: &mut Client, path: &str) -> usize {
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
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    checkpoint
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

// The observer has no command API: every Project, Workspace and Directory mutation below
// must pass through the visible native controls, including their complete-draw gate.
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
