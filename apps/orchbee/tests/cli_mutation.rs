//! Process-level mutation evidence: the real `orchbee` binary runs against a
//! scripted local peer bound at the Session's client socket. The peer speaks the
//! genuine OBLC admission and OBWP snapshot + command protocol, so every
//! mutation class — committed, unchanged, accepted, rejected, denied,
//! unavailable, ambiguous transport — is exercised against the production
//! code path. Every test also asserts the wire shape the CLI submitted:
//! exactly one snapshot read, then exactly one `workbench.command` carrying
//! the snapshot's Session identity and revision under a fresh request identity.
#![cfg(target_os = "linux")]

use std::{
    fs,
    io::{ErrorKind, Read},
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener, UnixStream},
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchRejectionReason,
    WorkbenchRequest, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePhase,
    WorkbenchTabCloseStatus, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
    WorkbenchWorkspaceCloseCounts, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResumeOutcome,
    WorkbenchWorkspaceCloseResumeResult, parse_snapshot, read_workbench_request,
    write_workbench_response,
};
use sha2::{Digest, Sha256};

use crate::support;

type TestResult = Result<(), Box<dyn std::error::Error>>;

// ---- staged cohort --------------------------------------------------------

/// One cohort directory per test process: copies of the four build artifacts
/// plus a `cohort.json` manifest, so the spawned CLI passes the real
/// `Cohort::current` verification.
fn cohort() -> &'static Path {
    static COHORT: OnceLock<PathBuf> = OnceLock::new();
    COHORT
        .get_or_init(|| {
            let directory = std::env::temp_dir().join(format!(
                "obcli-mutation-cohort-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            fs::create_dir(&directory).expect("create staged cohort directory");
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .expect("chmod staged cohort directory");
            let mut artifacts = serde_json::Map::new();
            for name in [
                "orchbee",
                "orchbee-server",
                "orchbee-worker",
                "orchbee-terminal-host",
            ] {
                let target = directory.join(crate::support::staged_artifact_name(name));
                if name == "orchbee" {
                    fs::copy(env!("CARGO_BIN_EXE_orchbee"), &target).expect("copy cohort artifact");
                } else {
                    fs::write(&target, format!("staged stand-in for {name}\n"))
                        .expect("write stand-in artifact");
                }
                fs::set_permissions(&target, fs::Permissions::from_mode(0o700))
                    .expect("chmod cohort artifact");
                let digest = Sha256::digest(fs::read(&target).expect("read artifact"));
                artifacts.insert((*name).to_owned(), format!("{digest:x}").into());
            }
            // A controlled dummy digest stands in for the adapters payload —
            // this fixture stages no sidecar; admission stays lazy.
            artifacts.insert(
                "agent-observation-adapters".to_owned(),
                format!("{:x}", Sha256::digest(b"staged stand-in adapters\n")).into(),
            );
            let manifest = directory.join("cohort.json");
            fs::write(
                &manifest,
                serde_json::json!({"schemaVersion": 1, "artifacts": artifacts}).to_string(),
            )
            .expect("write cohort manifest");
            // The verifier rejects group/other-writable cohort files; the
            // inherited umask is not guaranteed to be restrictive.
            fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600))
                .expect("chmod cohort manifest");
            directory
        })
        .as_path()
}

// ---- Session scope staging ---------------------------------------------------

/// Thin wrapper over the shared minter: a long TMPDIR must not push the
/// rendezvous socket path past `sun_path`, so names stay short and exclusive.
fn base_dir(tag: &str) -> PathBuf {
    support::runtime_base(tag)
}

/// Create the existing-only Session scope directory the rendezvous validator
/// expects (mode 0700, owned by the caller).
fn stage_scope(base: &Path) -> PathBuf {
    support::stage_scope(base)
}

// ---- scripted peer --------------------------------------------------------

/// Bind the scope's client socket and run `script` on the first admitted
/// connection. Returns the join handle; the CLI must exit first, so the peer
/// is joined after the child completes.
fn spawn_peer(
    scope: &Path,
    script: impl Fn(UnixStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    spawn_peer_n(scope, 1, script)
}

/// Every peer-side wait is bounded so a CLI that dies before connecting — or
/// stalls mid-exchange — fails the test quickly instead of hanging the whole
/// suite on `accept()`/`join()`. A peer timeout panics inside the peer thread
/// and propagates through `join`: it is a test failure, never a quiet pass.
const PEER_ACCEPT_DEADLINE: Duration = Duration::from_secs(60);
const PEER_IO_DEADLINE: Duration = Duration::from_secs(30);

fn spawn_peer_n(
    scope: &Path,
    connections: usize,
    script: impl Fn(UnixStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    let socket = support::client_socket(scope);
    // A socket file survives its listener: sequential peers in one test must
    // unlink the stale node before rebinding.
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    thread::spawn(move || {
        for _ in 0..connections {
            // A CLI that exits before connecting must fail the test, not hang
            // the join: bound the accept so the peer thread always retires.
            let stream = accept_bounded(&listener);
            // Accepted streams are blocking; per-call timeouts cap the
            // script's own reads and writes so `join` stays bounded as well.
            stream
                .set_read_timeout(Some(PEER_IO_DEADLINE))
                .expect("peer read timeout");
            stream
                .set_write_timeout(Some(PEER_IO_DEADLINE))
                .expect("peer write timeout");
            script(stream);
        }
    })
}

/// Accept the next connection within `PEER_ACCEPT_DEADLINE`; a CLI that never
/// arrives is a test failure, not an infinite block. The listener must already
/// be nonblocking.
fn accept_bounded(listener: &UnixListener) -> UnixStream {
    let deadline = Instant::now() + PEER_ACCEPT_DEADLINE;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "peer accept timed out waiting for the CLI"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("peer accept failed: {error}"),
        }
    }
}

/// After a deliberate peer stall the CLI must retire the session on its own
/// deadline; a read timeout here fails the test instead of passing as the
/// scripted silence.
fn expect_peer_retire(stream: &UnixStream) {
    let mut byte = [0_u8; 1];
    match (&*stream).read(&mut byte) {
        Ok(0) => {}
        Ok(_) => panic!("the CLI wrote after its exchange budget elapsed"),
        Err(error) => panic!("the CLI did not retire the stalled session: {error}"),
    }
}

/// OBLC admission: read the hello, answer `ClientAccepted`, return the stream.
fn admit(stream: &UnixStream) {
    orchbee_workbench_client::test_support::local_client::handshake::answer_offer(&mut &*stream)
        .unwrap();
    match read_message(&mut &*stream) {
        Ok(LocalClientMessage::ClientHello) => {}
        other => panic!("peer expected ClientHello, got {other:?}"),
    }
    write_message(
        &mut &*stream,
        &LocalClientMessage::ClientAccepted {
            generation: std::num::NonZeroU64::new(1).unwrap(),
        },
    )
    .expect("peer writes acceptance");
}

fn read_snapshot_request(stream: &UnixStream) -> orchbee_workbench_protocol::WorkbenchRequestId {
    match read_workbench_request(&mut &*stream) {
        Ok(WorkbenchRequest::SnapshotRead(intent)) => intent.request_id,
        other => panic!("peer expected workbench.snapshot.read, got {other:?}"),
    }
}

fn snapshot_response(
    stream: &UnixStream,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    outcome: WorkbenchSnapshotOutcome,
) {
    write_workbench_response(
        &mut &*stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id,
            outcome,
        }),
    )
    .expect("peer writes snapshot result");
}

/// Admit the session and answer the snapshot read with the fixture; returns
/// the snapshot request identity so callers can prove the command's identity
/// is a different, fresh one.
fn admit_and_serve_snapshot(stream: &UnixStream) -> orchbee_workbench_protocol::WorkbenchRequestId {
    admit(stream);
    let request_id = read_snapshot_request(stream);
    snapshot_response(
        stream,
        request_id.clone(),
        WorkbenchSnapshotOutcome::Available {
            snapshot: parse_snapshot(fixture_document()).expect("fixture"),
        },
    );
    request_id
}

fn read_command(stream: &UnixStream) -> WorkbenchCommand {
    match read_workbench_request(&mut &*stream) {
        Ok(WorkbenchRequest::Command(command)) => command,
        other => panic!("peer expected workbench.command, got {other:?}"),
    }
}

fn command_response(
    stream: &UnixStream,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    outcome: WorkbenchCommandOutcome,
) {
    write_workbench_response(
        &mut &*stream,
        &WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id,
            outcome,
        }),
    )
    .expect("peer writes command result");
}

/// Every spawned CLI child runs under the shared 45s wall-clock backstop in
/// `tests/support` — the same guard `cli_query` uses.
fn run(base: &Path, arguments: &[&str]) -> Output {
    support::run(
        &cohort().join(crate::support::staged_artifact_name("orchbee")),
        base,
        arguments,
    )
}

fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("answer is JSON")
}

/// A canonical typed id for fixtures and response identities.
fn id(n: u8) -> String {
    format!("{n:032x}")
}

fn revision(value: u64) -> WorkbenchRevision {
    WorkbenchRevision::new(value)
}

fn compiled_schema(base: &Path, schema_id: &str) -> jsonschema::Validator {
    let output = run(base, &["api", "schema", schema_id]);
    assert_eq!(output.status.code(), Some(0), "schema {schema_id}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    jsonschema::draft202012::new(&document["result"]["schema"]["document"])
        .expect("schema compiles")
}

// ---- fixture ---------------------------------------------------------------
//
// One project "web" (10) with workspace "main" (11) on Terminal Context 20,
// the standalone context 01, two workspace tabs (pinned "ops" 30 with a plain
// panel 31, "logs" 40 with a horizontal split holding panels 43 and 46), and
// one standalone tab "solo" (50).
fn fixture_document() -> &'static [u8] {
    br#"{
  "kind": "workbench.snapshot",
  "protocolVersion": 1,
  "revision": "7",
  "sessionId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "navigation": {"projects":[{"projectId":"00000000000000000000000000000010", "label":"web"}], "directories":[], "workspaces":[{"workspaceId":"00000000000000000000000000000001","terminalContextId":"00000000000000000000000000000001","projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":"00000000000000000000000000000011", "label":"main", "primaryDirectoryId":null, "terminalContextId":"00000000000000000000000000000020", "projectId":"00000000000000000000000000000010"}]},
  "terminalContexts": [
    {
      "terminalContextId": "00000000000000000000000000000001",
      "groups": [],
      "tabs": [
        {
          "tabId": "00000000000000000000000000000050",
          "label": "solo",
          "pinned": false,
          "groupId": null,
          "root": {
            "kind": "panel",
            "panelId": "00000000000000000000000000000051",
            "primaryPresentation": {
              "presentationId": "00000000000000000000000000000052",
              "label": "s",
              "binding": null
            },
            "childPresentations": []
          }
        }
      ]
    },
    {
      "terminalContextId": "00000000000000000000000000000020",
      "groups": [],
      "tabs": [
        {
          "tabId": "00000000000000000000000000000030",
          "label": "ops",
          "pinned": true,
          "groupId": null,
          "root": {
            "kind": "panel",
            "panelId": "00000000000000000000000000000031",
            "primaryPresentation": {
              "presentationId": "00000000000000000000000000000032",
              "label": "a",
              "binding": {
                "terminalId": "00000000000000000000000000000001",
                "workerId": "6f7263686265652d6c6f63616c2d7631"
              }
            },
            "childPresentations": []
          }
        },
        {
          "tabId": "00000000000000000000000000000040",
          "label": "logs",
          "pinned": false,
          "groupId": null,
          "root": {
            "kind": "split",
            "splitId": "00000000000000000000000000000041",
            "orientation": "horizontal",
            "children": [
              {
                "slotId": "00000000000000000000000000000042",
                "node": {
                  "kind": "panel",
                  "panelId": "00000000000000000000000000000043",
                  "primaryPresentation": {
                    "presentationId": "00000000000000000000000000000044",
                    "label": "l",
                    "binding": null
                  },
                  "childPresentations": []
                }
              },
              {
                "slotId": "00000000000000000000000000000045",
                "node": {
                  "kind": "panel",
                  "panelId": "00000000000000000000000000000046",
                  "primaryPresentation": {
                    "presentationId": "00000000000000000000000000000047",
                    "label": "r",
                    "binding": null
                  },
                  "childPresentations": []
                }
              }
            ]
          }
        }
      ]
    }
  ]
}"#
}

const SESSION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STANDALONE_CONTEXT: &str = "00000000000000000000000000000001";
const PROJECT: &str = "00000000000000000000000000000010";
const WORKSPACE: &str = "00000000000000000000000000000011";
const CONTEXT: &str = "00000000000000000000000000000020";
const OPS_TAB: &str = "00000000000000000000000000000030";
const LOGS_TAB: &str = "00000000000000000000000000000040";
const LEFT_PANEL: &str = "00000000000000000000000000000043";
const RIGHT_PANEL: &str = "00000000000000000000000000000046";

/// Assert the one-command contract the wire just proved: Session identity and
/// expected revision come from the snapshot, the request identity is fresh
/// (never the snapshot read's), and the action is the expected one.
fn assert_command_base(command: &WorkbenchCommand, snapshot_request: &str) {
    assert_eq!(
        command.session_id.as_str(),
        SESSION,
        "Session identity from snapshot"
    );
    assert_eq!(
        command.expected_revision,
        revision(7),
        "expected_revision is the snapshot revision"
    );
    assert_ne!(
        command.request_id.as_str(),
        snapshot_request,
        "command identity must be fresh, not the snapshot read's"
    );
}

// ---- committed commands -----------------------------------------------------

#[test]
fn project_create_commits_under_a_fresh_request() -> TestResult {
    let base = base_dir("project-create");
    let scope = stage_scope(&base);
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert_command_base(&command, snapshot_request.as_str());
        *seen.lock().unwrap() = command.request_id.as_str().to_owned();
        let WorkbenchAction::ProjectCreate { label } = &command.action else {
            panic!("expected project.create, got {:?}", command.action)
        };
        assert_eq!(label.as_str(), "平台-api");
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::ProjectCreated {
                revision: revision(8),
                project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(&id(0x60))
                    .unwrap(),
            },
        );
    });
    let output = run(
        &base,
        &[
            "project",
            "create",
            "--label",
            "平台-api",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.mutation.result");
    assert_eq!(document["version"], 1);
    assert_eq!(document["source"]["kind"], "workbench-command");
    assert_eq!(document["source"]["sessionId"], SESSION);
    assert_eq!(document["source"]["expectedRevision"], "7");
    assert_eq!(document["result"]["action"], "project.create");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["revision"], "8");
    // The committed id is the Server-allocated one from the outcome, never a
    // resolved target — project create has no target operand.
    assert_eq!(document["result"]["projectId"], id(0x60));
    assert_eq!(document["result"]["label"], "平台-api");
    assert_eq!(
        document["source"]["requestId"].as_str().unwrap(),
        submitted.lock().unwrap().as_str()
    );
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));
    Ok(())
}

#[test]
fn workspace_create_commits_under_a_fresh_request() -> TestResult {
    let base = base_dir("ws-create");
    let scope = stage_scope(&base);
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert_command_base(&command, snapshot_request.as_str());
        *seen.lock().unwrap() = command.request_id.as_str().to_owned();
        let WorkbenchAction::WorkspaceCreate {
            project_id, label, ..
        } = &command.action
        else {
            panic!("expected workspace.create, got {:?}", command.action)
        };
        assert_eq!(
            project_id.as_ref().unwrap().as_str(),
            PROJECT,
            "project label resolved"
        );
        assert_eq!(label.as_str(), "api");
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::WorkspaceCreated {
                revision: revision(8),
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&id(0x61))
                    .unwrap(),
                terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                    &id(0x62),
                )
                .unwrap(),
            },
        );
    });
    let output = run(
        &base,
        &[
            "workspace",
            "create",
            "--project",
            "web",
            "--label",
            "api",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.mutation.result");
    assert_eq!(document["version"], 1);
    assert_eq!(document["source"]["kind"], "workbench-command");
    assert_eq!(document["source"]["sessionId"], SESSION);
    assert_eq!(document["source"]["expectedRevision"], "7");
    assert_eq!(document["result"]["action"], "workspace.create");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["revision"], "8");
    assert_eq!(document["result"]["projectId"], PROJECT);
    assert_eq!(document["result"]["workspaceId"], id(0x61));
    assert_eq!(document["result"]["terminalContextId"], id(0x62));
    assert_eq!(document["result"]["label"], "api");
    // The receipt's request id is the submitted command's, not the read's.
    assert_eq!(
        document["source"]["requestId"].as_str().unwrap(),
        submitted.lock().unwrap().as_str()
    );
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));
    Ok(())
}

#[test]
fn workspace_rename_resolves_label_and_reports_unchanged() -> TestResult {
    let base = base_dir("ws-rename");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 2, |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert_command_base(&command, snapshot_request.as_str());
        let WorkbenchAction::WorkspaceRename {
            workspace_id,
            label,
        } = &command.action
        else {
            panic!("expected workspace.rename, got {:?}", command.action)
        };

        assert_eq!(workspace_id.as_str(), WORKSPACE, "label 'main' resolved");
        // First call renames, second call reports the unchanged outcome.
        static CALL: Mutex<u32> = Mutex::new(0);
        let call = {
            let mut guard = CALL.lock().unwrap();
            *guard += 1;
            *guard
        };
        let outcome = if call == 1 {
            assert_eq!(label.as_str(), "ops");
            WorkbenchCommandOutcome::Renamed {
                revision: revision(8),
            }
        } else {
            assert_eq!(label.as_str(), "ops-again");
            WorkbenchCommandOutcome::Unchanged {
                revision: revision(7),
            }
        };
        command_response(&stream, command.request_id, outcome);
    });

    let renamed = run(
        &base,
        &[
            "workspace",
            "rename",
            "main",
            "--label",
            "ops",
            "--format=json",
        ],
    );
    assert_eq!(
        renamed.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    let document = envelope(&renamed);
    assert_eq!(document["result"]["action"], "workspace.rename");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["revision"], "8");
    assert!(document["result"].get("projectId").is_none());
    assert_eq!(document["result"]["workspaceId"], WORKSPACE);
    assert_eq!(document["result"]["label"], "ops");

    let unchanged = run(
        &base,
        &[
            "workspace",
            "rename",
            WORKSPACE,
            "--label",
            "ops-again",
            "--format=json",
        ],
    );
    assert_eq!(unchanged.status.code(), Some(0));
    let document = envelope(&unchanged);
    assert_eq!(document["result"]["outcome"], "unchanged");
    assert_eq!(document["result"]["revision"], "7");
    peer.join().expect("peer finished");
    Ok(())
}

#[test]
fn tab_create_against_standalone_context_reports_terminal() -> TestResult {
    let base = base_dir("tab-create");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert_command_base(&command, snapshot_request.as_str());
        let WorkbenchAction::TabCreate {
            terminal_context_id,
            label,
            geometry,
            launch: None,
            profile: None,
            ..
        } = &command.action
        else {
            panic!("expected tab.create, got {:?}", command.action)
        };
        assert_eq!(terminal_context_id.as_str(), STANDALONE_CONTEXT);
        assert_eq!(label.as_str(), "scratch");
        assert_eq!(
            *geometry,
            WorkbenchTerminalGeometry::new(80, 24, 9, 18).unwrap()
        );
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::TabCreated {
                revision: revision(8),
                terminal_context_id: terminal_context_id.clone(),
                tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&id(0x63)).unwrap(),
                panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(0x64)).unwrap(),
                presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(
                    0x65,
                ))
                .unwrap(),
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: *geometry,
                },
            },
        );
    });
    let output = run(
        &base,
        &[
            "tab",
            "create",
            "--workspace",
            "Workspace",
            "--label",
            "scratch",
            "--cols",
            "80",
            "--rows",
            "24",
            "--cell-width",
            "9",
            "--cell-height",
            "18",
            "--format=json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["result"]["action"], "tab.create");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["terminalContextId"], STANDALONE_CONTEXT);
    assert_eq!(document["result"]["tabId"], id(0x63));
    assert_eq!(document["result"]["panelId"], id(0x64));
    assert_eq!(document["result"]["presentationId"], id(0x65));
    assert_eq!(document["result"]["label"], "scratch");
    assert_eq!(document["result"]["terminal"]["outcome"], "started");
    assert_eq!(document["result"]["terminal"]["geometry"]["columns"], 80);
    assert_eq!(document["result"]["terminal"]["geometry"]["rows"], 24);
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));
    Ok(())
}

#[test]
fn tab_create_in_workspace_context_resolves_context() -> TestResult {
    let base = base_dir("tab-create-ws");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let WorkbenchAction::TabCreate {
            terminal_context_id,
            ..
        } = &command.action
        else {
            panic!("expected tab.create, got {:?}", command.action)
        };
        assert_eq!(
            terminal_context_id.as_str(),
            CONTEXT,
            "workspace label 'main' resolved to its Terminal Context"
        );
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::TabCreatePending {
                revision: revision(8),
                terminal_context_id: terminal_context_id.clone(),
                tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&id(0x66)).unwrap(),
                panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(0x67)).unwrap(),
                presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(
                    0x68,
                ))
                .unwrap(),
            },
        );
    });
    let output = run(
        &base,
        &[
            "tab",
            "create",
            "--workspace",
            "main",
            "--label",
            "logs2",
            "--cols",
            "120",
            "--rows",
            "40",
            "--cell-width",
            "8",
            "--cell-height",
            "16",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["terminal"]["outcome"], "pending");
    Ok(())
}

/// With all four geometry flags absent the request still carries an explicit
/// geometry — the documented 80x24@(0,0) default — and the receipt echoes the
/// spawned geometry back to the operator.
#[test]
fn tab_create_without_geometry_requests_the_default() -> TestResult {
    let base = base_dir("tab-create-default-geometry");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert_command_base(&command, snapshot_request.as_str());
        let WorkbenchAction::TabCreate {
            terminal_context_id,
            geometry,
            ..
        } = &command.action
        else {
            panic!("expected tab.create, got {:?}", command.action)
        };
        assert_eq!(
            *geometry,
            WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
            "the request-bound geometry is the documented default"
        );
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::TabCreated {
                revision: revision(8),
                terminal_context_id: terminal_context_id.clone(),
                tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&id(0x69)).unwrap(),
                panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(0x6a)).unwrap(),
                presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(
                    0x6b,
                ))
                .unwrap(),
                terminal_outcome: WorkbenchTerminalOutcome::Started {
                    geometry: *geometry,
                },
            },
        );
    });
    let output = run(
        &base,
        &[
            "tab",
            "create",
            "--workspace",
            "Workspace",
            "--label",
            "scratch",
            "--format=json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["result"]["action"], "tab.create");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["terminal"]["outcome"], "started");
    assert_eq!(document["result"]["terminal"]["geometry"]["columns"], 80);
    assert_eq!(document["result"]["terminal"]["geometry"]["rows"], 24);
    assert_eq!(
        document["result"]["terminal"]["geometry"]["cellPixelWidth"],
        0
    );
    assert_eq!(
        document["result"]["terminal"]["geometry"]["cellPixelHeight"],
        0
    );
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));
    Ok(())
}

#[test]
fn tab_rename_move_and_close_each_submit_one_command() -> TestResult {
    let base = base_dir("tab-ops");
    let scope = stage_scope(&base);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let peer = spawn_peer_n(&scope, 3, move |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        seen.lock()
            .unwrap()
            .push(command.request_id.as_str().to_owned());
        let outcome = match &command.action {
            WorkbenchAction::TabRename {
                terminal_context_id,
                tab_id,
                label,
            } => {
                assert_eq!(terminal_context_id.as_str(), CONTEXT);
                assert_eq!(tab_id.as_str(), OPS_TAB, "tab label 'ops' resolved");
                assert_eq!(label.as_str(), "renamed");
                WorkbenchCommandOutcome::Renamed {
                    revision: revision(8),
                }
            }
            WorkbenchAction::TabMove {
                terminal_context_id,
                tab_id,
                direction,
            } => {
                assert_eq!(terminal_context_id.as_str(), CONTEXT);
                assert_eq!(tab_id.as_str(), LOGS_TAB);
                assert_eq!(
                    *direction,
                    orchbee_workbench_protocol::WorkbenchTabMoveDirection::Last
                );
                WorkbenchCommandOutcome::TabMoved {
                    revision: revision(8),
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                }
            }
            WorkbenchAction::TabClose {
                terminal_context_id,
                tab_id,
            } => {
                assert_eq!(terminal_context_id.as_str(), CONTEXT);
                assert_eq!(tab_id.as_str(), OPS_TAB);
                WorkbenchCommandOutcome::TabCloseAccepted {
                    close_request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&id(
                        0x70,
                    ))
                    .unwrap(),
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    accepted_revision: revision(7),
                }
            }
            other => panic!("unexpected action {other:?}"),
        };
        command_response(&stream, command.request_id, outcome);
    });

    let renamed = run(
        &base,
        &[
            "tab",
            "rename",
            "ops",
            "--label",
            "renamed",
            "--format=json",
        ],
    );
    assert_eq!(
        renamed.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    let document = envelope(&renamed);
    assert_eq!(document["result"]["action"], "tab.rename");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["revision"], "8");
    assert_eq!(document["result"]["terminalContextId"], CONTEXT);
    assert_eq!(document["result"]["tabId"], OPS_TAB);
    assert_eq!(document["result"]["label"], "renamed");

    let moved = run(
        &base,
        &["tab", "move", "logs", "--order", "last", "--format=json"],
    );
    assert_eq!(moved.status.code(), Some(0));
    let document = envelope(&moved);
    assert_eq!(document["result"]["action"], "tab.move");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["direction"], "last");
    assert_eq!(document["result"]["tabId"], LOGS_TAB);

    let closed = run(
        &base,
        &["tab", "close", "ops", "--confirm", "--format=json"],
    );
    assert_eq!(closed.status.code(), Some(0));
    let document = envelope(&closed);
    assert_eq!(document["result"]["action"], "tab.close");
    assert_eq!(document["result"]["outcome"], "accepted");
    assert_eq!(document["result"]["acceptedRevision"], "7");
    assert_eq!(document["result"]["closeRequestId"], id(0x70));
    assert_eq!(document["result"]["tabId"], OPS_TAB);
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));

    peer.join().expect("peer finished");
    // Three invocations, three distinct fresh request identities.
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_ne!(requests[0], requests[1]);
    assert_ne!(requests[1], requests[2]);
    assert_ne!(requests[0], requests[2]);
    Ok(())
}

#[test]
fn panel_split_close_and_move_each_submit_one_command() -> TestResult {
    let base = base_dir("panel-ops");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 3, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let outcome = match &command.action {
            WorkbenchAction::PanelSplit {
                terminal_context_id,
                tab_id,
                panel_id,
                direction,
                label,
                geometry,
                launch: None,
                profile: None,
                ..
            } => {
                assert_eq!(terminal_context_id.as_str(), CONTEXT);
                assert_eq!(tab_id.as_str(), LOGS_TAB);
                assert_eq!(panel_id.as_str(), LEFT_PANEL);
                assert_eq!(
                    *direction,
                    orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Right
                );
                assert_eq!(label.as_str(), "right");
                assert_eq!(
                    *geometry,
                    WorkbenchTerminalGeometry::new(80, 24, 9, 18).unwrap()
                );
                WorkbenchCommandOutcome::PanelSplitCreated {
                    revision: revision(8),
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                    new_panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(0x71))
                        .unwrap(),
                    split_id: orchbee_workbench_protocol::WorkbenchSplitId::parse(&id(0x72))
                        .unwrap(),
                    first_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(
                        0x73,
                    ))
                    .unwrap(),
                    second_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(
                        0x74,
                    ))
                    .unwrap(),
                    presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(
                        &id(0x75),
                    )
                    .unwrap(),
                    terminal_outcome: WorkbenchTerminalOutcome::Started {
                        geometry: *geometry,
                    },
                }
            }
            WorkbenchAction::PanelClose {
                terminal_context_id,
                tab_id,
                panel_id,
            } => {
                assert_eq!(terminal_context_id.as_str(), CONTEXT);
                assert_eq!(tab_id.as_str(), LOGS_TAB);
                assert_eq!(panel_id.as_str(), RIGHT_PANEL);
                WorkbenchCommandOutcome::PanelCloseAccepted {
                    close_request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&id(
                        0x76,
                    ))
                    .unwrap(),
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                    accepted_revision: revision(6),
                }
            }
            WorkbenchAction::PanelMove {
                terminal_context_id,
                tab_id,
                panel_id,
                target_panel_id,
                direction,
            } => {
                assert_eq!(terminal_context_id.as_str(), CONTEXT);
                assert_eq!(tab_id.as_str(), LOGS_TAB);
                assert_eq!(panel_id.as_str(), LEFT_PANEL);
                assert_eq!(
                    target_panel_id.as_str(),
                    RIGHT_PANEL,
                    "target panel resolves inside the source tab"
                );
                assert_eq!(
                    *direction,
                    orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Below
                );
                WorkbenchCommandOutcome::PanelMoved {
                    revision: revision(8),
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                    target_panel_id: target_panel_id.clone(),
                }
            }
            other => panic!("unexpected action {other:?}"),
        };
        command_response(&stream, command.request_id, outcome);
    });

    let split = run(
        &base,
        &[
            "panel",
            "split",
            LEFT_PANEL,
            "--direction",
            "right",
            "--label",
            "right",
            "--cols",
            "80",
            "--rows",
            "24",
            "--cell-width",
            "9",
            "--cell-height",
            "18",
            "--format=json",
        ],
    );
    assert_eq!(
        split.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&split.stderr)
    );
    let document = envelope(&split);
    assert_eq!(document["result"]["action"], "panel.split");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["panelId"], LEFT_PANEL);
    assert_eq!(document["result"]["newPanelId"], id(0x71));
    assert_eq!(document["result"]["splitId"], id(0x72));
    assert_eq!(document["result"]["firstSlotId"], id(0x73));
    assert_eq!(document["result"]["secondSlotId"], id(0x74));
    assert_eq!(document["result"]["presentationId"], id(0x75));
    assert_eq!(document["result"]["direction"], "right");
    assert_eq!(document["result"]["terminal"]["outcome"], "started");
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));

    let closed = run(
        &base,
        &["panel", "close", RIGHT_PANEL, "--confirm", "--format=json"],
    );
    assert_eq!(closed.status.code(), Some(0));
    let document = envelope(&closed);
    assert_eq!(document["result"]["action"], "panel.close");
    assert_eq!(document["result"]["outcome"], "accepted");
    assert_eq!(document["result"]["acceptedRevision"], "6");
    assert_eq!(document["result"]["closeRequestId"], id(0x76));
    assert_eq!(document["result"]["panelId"], RIGHT_PANEL);

    let moved = run(
        &base,
        &[
            "panel",
            "move",
            LEFT_PANEL,
            "--into-tab",
            OPS_TAB,
            "--target-panel",
            RIGHT_PANEL,
            "--direction",
            "down",
            "--format=json",
        ],
    );
    assert_eq!(moved.status.code(), Some(0));
    let document = envelope(&moved);
    assert_eq!(document["result"]["action"], "panel.move");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["direction"], "down");
    assert_eq!(document["result"]["panelId"], LEFT_PANEL);
    assert_eq!(document["result"]["targetPanelId"], RIGHT_PANEL);
    peer.join().expect("peer finished");
    Ok(())
}

/// A move that lands where the target already is comes back `unchanged` with
/// the Server's own revision — the same definite arm `workspace rename`
/// already covers.
#[test]
fn tab_and_panel_move_report_unchanged() -> TestResult {
    let base = base_dir("move-unchanged");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 2, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        match &command.action {
            WorkbenchAction::TabMove { direction, .. } => assert_eq!(
                *direction,
                orchbee_workbench_protocol::WorkbenchTabMoveDirection::Next
            ),
            WorkbenchAction::PanelMove { direction, .. } => assert_eq!(
                *direction,
                orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Above
            ),
            other => panic!("unexpected action {other:?}"),
        }
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Unchanged {
                revision: revision(7),
            },
        );
    });

    let tab_moved = run(
        &base,
        &["tab", "move", "ops", "--order", "next", "--format=json"],
    );
    assert_eq!(tab_moved.status.code(), Some(0));
    let document = envelope(&tab_moved);
    assert_eq!(document["result"]["action"], "tab.move");
    assert_eq!(document["result"]["outcome"], "unchanged");
    assert_eq!(document["result"]["revision"], "7");
    assert_eq!(document["result"]["direction"], "next");
    assert_eq!(document["result"]["tabId"], OPS_TAB);
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));

    let panel_moved = run(
        &base,
        &[
            "panel",
            "move",
            LEFT_PANEL,
            "--into-tab",
            OPS_TAB,
            "--target-panel",
            RIGHT_PANEL,
            "--direction",
            "up",
            "--format=json",
        ],
    );
    assert_eq!(panel_moved.status.code(), Some(0));
    let document = envelope(&panel_moved);
    assert_eq!(document["result"]["action"], "panel.move");
    assert_eq!(document["result"]["outcome"], "unchanged");
    assert_eq!(document["result"]["revision"], "7");
    assert_eq!(document["result"]["direction"], "up");
    peer.join().expect("peer finished");
    Ok(())
}

// ---- definite negative outcomes ---------------------------------------------

#[test]
fn rejected_command_maps_to_exit_three() -> TestResult {
    let base = base_dir("rejected");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Rejected {
                revision: revision(7),
                reason: WorkbenchRejectionReason::RevisionMismatch,
            },
        );
    });
    let output = run(
        &base,
        &[
            "workspace",
            "rename",
            "main",
            "--label",
            "x",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "rejected");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "revision-mismatch"
    );
    assert_eq!(document["failure"]["details"]["revision"], "7");
    assert_eq!(document["source"]["kind"], "workbench-command");
    assert_eq!(
        document["source"]["requestId"].as_str().unwrap().len(),
        32,
        "the failed request identity is reported"
    );
    assert!(
        compiled_schema(&base, "orchbee.cli.failure").is_valid(&document),
        "rejected envelope validates"
    );
    assert!(!output.stderr.is_empty());
    Ok(())
}

#[test]
fn project_create_rejected_maps_to_exit_three() -> TestResult {
    let base = base_dir("project-rejected");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let WorkbenchAction::ProjectCreate { .. } = &command.action else {
            panic!("expected project.create, got {:?}", command.action)
        };
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Rejected {
                revision: revision(7),
                reason: WorkbenchRejectionReason::ResourceLimit,
            },
        );
    });
    let output = run(
        &base,
        &["project", "create", "--label", "x", "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "rejected");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(document["failure"]["details"]["reason"], "resource-limit");
    assert_eq!(document["failure"]["details"]["revision"], "7");
    assert!(
        compiled_schema(&base, "orchbee.cli.failure").is_valid(&document),
        "rejected envelope validates"
    );
    Ok(())
}

#[test]
fn denied_and_unavailable_map_to_the_existing_matrix() -> TestResult {
    let base = base_dir("precommit");
    let scope = stage_scope(&base);
    let denied_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Denied {},
        );
    });
    let denied = run(
        &base,
        &["tab", "rename", "ops", "--label", "x", "--format=json"],
    );
    denied_peer.join().expect("peer finished");
    assert_eq!(denied.status.code(), Some(3));
    assert_eq!(envelope(&denied)["failure"]["kind"], "denied");
    assert_eq!(
        envelope(&denied)["source"]["kind"],
        "workbench-command",
        "denied carries the command source"
    );

    let unavailable_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy,
            },
        );
    });
    let unavailable = run(
        &base,
        &["tab", "close", "ops", "--confirm", "--format=json"],
    );
    unavailable_peer.join().expect("peer finished");
    assert_eq!(unavailable.status.code(), Some(4));
    let document = envelope(&unavailable);
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(document["failure"]["details"]["reason"], "terminal-busy");
    assert!(compiled_schema(&base, "orchbee.cli.failure").is_valid(&document));
    Ok(())
}

// ---- peer absence and silence stay inside the CLI's own budgets -------------

#[test]
fn no_listening_peer_fails_discovery_or_admission_fast() -> TestResult {
    let base = base_dir("no-peer");
    let scope = stage_scope(&base);
    // No client socket at all: rendezvous finds the scope but no listener, so
    // discovery itself answers without any peer involvement.
    let output = run(
        &base,
        &["tab", "rename", "ops", "--label", "x", "--format=json"],
    );
    assert_eq!(output.status.code(), Some(4));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["stage"], "discovery");

    // A stale socket with nobody listening: connect fails on the spot.
    let socket = support::client_socket(&scope);
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    drop(listener);
    let output = run(
        &base,
        &["tab", "rename", "ops", "--label", "x", "--format=json"],
    );
    assert_eq!(output.status.code(), Some(4));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["stage"], "admission");
    Ok(())
}

#[test]
fn a_connected_but_silent_peer_cannot_hold_the_cli_past_its_budget() -> TestResult {
    let base = base_dir("peer-silence");
    let scope = stage_scope(&base);
    // Admission silence: the hello lands but no acceptance ever comes back.
    // The CLI's shared exchange budget must end the exchange; the peer then
    // observes the disconnect instead of timing the test out itself.
    // `run` arms the 45s wall-clock backstop on the child, but the assertions
    // below (exit 5, transport-loss) prove the CLI retired on its own 5s
    // exchange budget — a backstop kill would panic before reaching them.
    let silent_admission = spawn_peer(&scope, |stream| {
        orchbee_workbench_client::test_support::local_client::handshake::answer_offer(&mut &stream)
            .unwrap();
        match read_message(&mut &stream) {
            Ok(LocalClientMessage::ClientHello) => {}
            other => panic!("peer expected ClientHello, got {other:?}"),
        }
        expect_peer_retire(&stream);
    });
    let output = run(
        &base,
        &["tab", "rename", "ops", "--label", "x", "--format=json"],
    );
    silent_admission.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "transport-loss");
    assert_eq!(document["failure"]["details"]["stage"], "admission");

    // Post-admission silence: admitted, the snapshot read lands, and no
    // response ever follows — the same shared budget ends it.
    let silent_snapshot = spawn_peer(&scope, |stream| {
        admit(&stream);
        let _ = read_snapshot_request(&stream);
        expect_peer_retire(&stream);
    });
    let output = run(
        &base,
        &["tab", "rename", "ops", "--label", "x", "--format=json"],
    );
    silent_snapshot.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "transport-loss");
    assert_eq!(document["failure"]["details"]["stage"], "snapshot");
    Ok(())
}

// ---- run() child backstop ---------------------------------------------------

#[test]
fn run_backstop_preserves_large_two_stream_output_and_status() -> TestResult {
    // Two MiB per stream far exceeds the pipe buffer: without concurrent
    // drains the child would block on a full pipe and die by the backstop.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "head -c 2000000 /dev/zero | tr '\\0' o; \
         head -c 2000000 /dev/zero | tr '\\0' e >&2; exit 3",
    ]);
    let output = support::bounded_output(&mut command, Duration::from_secs(30))
        .expect("large-output child completes inside the backstop");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(output.stdout.len(), 2_000_000);
    assert_eq!(output.stderr.len(), 2_000_000);
    assert!(output.stdout.iter().all(|byte| *byte == b'o'));
    assert!(output.stderr.iter().all(|byte| *byte == b'e'));
    Ok(())
}

#[test]
fn spawned_child_with_large_piped_streams_exits_normally() -> TestResult {
    // The already-spawned entry must drain while the child runs: waiting for
    // exit first would block a healthy writer once stderr fills the pipe.
    let mut command = Command::new("sh");
    command
        .args([
            "-c",
            "head -c 2000000 /dev/zero | tr '\\0' o; \
             head -c 2000000 /dev/zero | tr '\\0' e >&2; exit 3",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().expect("spawn writer");
    let output = support::bounded_wait_with_output(child, Duration::from_secs(30))
        .expect("large-output spawned child completes inside the backstop");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(output.stdout.len(), 2_000_000);
    assert_eq!(output.stderr.len(), 2_000_000);
    assert!(output.stdout.iter().all(|byte| *byte == b'o'));
    assert!(output.stderr.iter().all(|byte| *byte == b'e'));
    Ok(())
}

#[test]
fn run_backstop_kills_and_reaps_a_child_that_never_exits() -> TestResult {
    // `exec` makes the spawned pid the sleeper itself, so the backstop kills
    // exactly the spawned child — no grandchildren, nothing global. Exercised
    // through the `bounded_wait_with_output` entry: the same deadline applies
    // to an already-spawned child with custom stdio.
    let child = Command::new("sh")
        .args(["-c", "exec sleep 60"])
        .spawn()
        .expect("spawn sleeper");
    let started = Instant::now();
    let failure = support::bounded_wait_with_output(child, Duration::from_secs(2))
        .expect_err("a non-exiting child must surface as a timeout, not pass");
    match failure {
        support::BoundedChildFailure::ChildTimeout { status, .. } => {
            assert_eq!(status.signal(), Some(9), "reaped as SIGKILL");
        }
        other => panic!("expected child-timeout, got {other}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the short test budget fired, not the 45s backstop"
    );
    Ok(())
}

#[test]
fn run_backstop_fails_when_a_descendant_holds_the_pipe() -> TestResult {
    // The direct child exits at once; its backgrounded sibling sleeper keeps
    // the stdout pipe's write end open. The deadline must expire as an
    // explicit drain timeout — not a fabricated Ok, and not a wait for the
    // descendant (5s fixture vs 2s budget; the sleeper ends on its own and is
    // reaped by init, so nothing is left behind).
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 5 & exit 0"]);
    let started = Instant::now();
    let failure = support::bounded_output(&mut command, Duration::from_secs(2))
        .expect_err("a pipe held by a descendant must fail explicitly");
    match failure {
        support::BoundedChildFailure::DrainTimeout { status, .. } => {
            assert_eq!(status.code(), Some(0), "the direct child exited cleanly");
        }
        other => panic!("expected drain-timeout, got {other}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "the deadline returned before the descendant released the pipe"
    );
    Ok(())
}

#[test]
fn run_backstop_killed_child_does_not_wait_on_a_descendant() -> TestResult {
    // The direct child is killed at the deadline while its backgrounded
    // sleeper still holds the pipe: the result is the child-timeout, and it
    // must come back near the budget — never at the descendant's pace.
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 5 & exec sleep 60"]);
    let started = Instant::now();
    let failure = support::bounded_output(&mut command, Duration::from_secs(2))
        .expect_err("a non-exiting child must surface as a timeout, not pass");
    match failure {
        support::BoundedChildFailure::ChildTimeout { status, .. } => {
            assert_eq!(status.signal(), Some(9), "reaped as SIGKILL");
        }
        other => panic!("expected child-timeout, got {other}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "kill+reap returned inside the deadline, not at the descendant's pace"
    );
    Ok(())
}

// ---- run() capture budget ---------------------------------------------------
//
// The two streams share one 8MiB retain budget (support::CAPTURE_BYTE_BUDGET
// stays private; these fixtures pin the observable behavior, not the const).
// Every fixture is a finite `head -c` writer — no unbounded generators — and
// a writer blocked mid-pipe is still alive when the limit fires, so the kill
// arm is exercised deterministically.

const CAPTURE_LIMIT: usize = 8 * 1024 * 1024;

#[test]
fn capture_budget_limits_a_stdout_flood_and_kills_the_writer() -> TestResult {
    // 9MiB on stdout alone: the pipe holds ~64KiB, so the child is still
    // blocked on write when the retained prefix reaches the budget — the
    // limit kills and reaps it rather than growing the capture.
    let mut command = Command::new("sh");
    command.args(["-c", "head -c 9000000 /dev/zero | tr '\\0' o"]);
    let failure = support::bounded_output(&mut command, Duration::from_secs(30))
        .expect_err("a flooding child must fail, not capture without bound");
    match failure {
        support::BoundedChildFailure::OutputLimit {
            limit,
            status,
            stdout,
            stderr,
        } => {
            assert_eq!(limit, CAPTURE_LIMIT);
            assert_eq!(status.signal(), Some(9), "blocked writer killed");
            assert_eq!(stdout.len(), CAPTURE_LIMIT, "truncated at the budget");
            assert!(stdout.iter().all(|byte| *byte == b'o'));
            assert!(stderr.is_empty());
        }
        other => panic!("expected output-limit, got {other}"),
    }
    Ok(())
}

#[test]
fn capture_budget_limits_a_stderr_flood() -> TestResult {
    // The same shared budget covers a stderr-only flood; stdout is an early
    // EOF and captures nothing.
    let mut command = Command::new("sh");
    command.args(["-c", "head -c 9000000 /dev/zero | tr '\\0' e >&2"]);
    let failure = support::bounded_output(&mut command, Duration::from_secs(30))
        .expect_err("a stderr flood must fail, not capture without bound");
    match failure {
        support::BoundedChildFailure::OutputLimit {
            limit,
            status,
            stdout,
            stderr,
        } => {
            assert_eq!(limit, CAPTURE_LIMIT);
            assert_eq!(status.signal(), Some(9), "blocked writer killed");
            assert!(stdout.is_empty());
            assert_eq!(stderr.len(), CAPTURE_LIMIT, "truncated at the budget");
            assert!(stderr.iter().all(|byte| *byte == b'e'));
        }
        other => panic!("expected output-limit, got {other}"),
    }
    Ok(())
}

#[test]
fn capture_budget_is_shared_across_both_streams() -> TestResult {
    // Sequential writers make the split deterministic: stdout drains its
    // 6MiB fully, leaving 2,388,608B of the shared budget for stderr — so the
    // retained total is exactly the budget, never a per-stream allotment.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "head -c 6000000 /dev/zero | tr '\\0' o; \
         head -c 6000000 /dev/zero | tr '\\0' e >&2",
    ]);
    let failure = support::bounded_output(&mut command, Duration::from_secs(30))
        .expect_err("a two-stream flood must share one budget");
    match failure {
        support::BoundedChildFailure::OutputLimit {
            limit,
            status,
            stdout,
            stderr,
        } => {
            assert_eq!(limit, CAPTURE_LIMIT);
            assert_eq!(status.signal(), Some(9), "blocked writer killed");
            assert_eq!(stdout.len(), 6_000_000);
            assert_eq!(stderr.len(), CAPTURE_LIMIT - 6_000_000);
        }
        other => panic!("expected output-limit, got {other}"),
    }
    Ok(())
}

#[test]
fn capture_budget_accepts_output_exactly_at_the_limit() -> TestResult {
    // Exactly 8MiB then EOF is a legal capture: the limit fires only on the
    // first byte *past* the budget, not on reaching it.
    let mut command = Command::new("sh");
    command.args(["-c", "head -c 8388608 /dev/zero | tr '\\0' o; exit 7"]);
    let output = support::bounded_output(&mut command, Duration::from_secs(30))
        .expect("output at exactly the budget must succeed");
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout.len(), CAPTURE_LIMIT);
    assert!(output.stdout.iter().all(|byte| *byte == b'o'));
    assert!(output.stderr.is_empty());
    Ok(())
}

#[test]
fn capture_budget_fires_on_the_byte_past_the_limit() -> TestResult {
    // 8MiB+1: a single byte over the shared budget is already a failure —
    // the retained prefix stops at exactly the limit.
    let mut command = Command::new("sh");
    command.args(["-c", "head -c 8388609 /dev/zero | tr '\\0' o"]);
    let failure = support::bounded_output(&mut command, Duration::from_secs(30))
        .expect_err("one byte past the budget must fail");
    match failure {
        support::BoundedChildFailure::OutputLimit { stdout, stderr, .. } => {
            assert_eq!(stdout.len(), CAPTURE_LIMIT);
            assert!(stderr.is_empty());
        }
        other => panic!("expected output-limit, got {other}"),
    }
    Ok(())
}

#[test]
fn capture_budget_reports_an_already_exited_child() -> TestResult {
    // The direct child exits 0 at once while a backgrounded finite writer
    // keeps flooding the inherited stdout pipe: the limit reports the real
    // observed exit status — no kill is needed or fabricated — and dropping
    // the pipes SIGPIPEs the descendant, so nothing outlives the test.
    let mut command = Command::new("sh");
    command.args(["-c", "head -c 20000000 /dev/zero | tr '\\0' o & exit 0"]);
    let failure = support::bounded_output(&mut command, Duration::from_secs(30))
        .expect_err("a descendant flood must hit the same budget");
    match failure {
        support::BoundedChildFailure::OutputLimit {
            status,
            stdout,
            stderr,
            ..
        } => {
            assert_eq!(status.code(), Some(0), "the direct child exited 0");
            assert_eq!(stdout.len(), CAPTURE_LIMIT);
            assert!(stderr.is_empty());
        }
        other => panic!("expected output-limit, got {other}"),
    }
    Ok(())
}

// ---- short runtime base -----------------------------------------------------
//
// The shared minter's observable contract — the real sun_path boundary,
// exclusivity, 0700 — under whatever TMPDIR the suite actually runs.

#[test]
fn sun_path_boundary_is_exact_and_limit_plus_one_is_red() -> TestResult {
    // Pin the derived limit against the real bind boundary on this platform:
    // a path of exactly `limit` bytes binds; `limit`+1 must be refused up
    // front — an explicit red, never a skip.
    let dir = base_dir("bound");
    let dir_len = dir.as_os_str().as_encoded_bytes().len();
    let limit = support::sun_path_limit();
    assert!(
        dir_len + 2 <= limit,
        "runtime base {dir:?} ({dir_len}B) leaves no room for the boundary probe"
    );
    let exact = dir.join("s".repeat(limit - dir_len - 1));
    assert_eq!(exact.as_os_str().as_encoded_bytes().len(), limit);
    let listener = UnixListener::bind(&exact).expect("exact-limit path binds");
    drop(listener);
    let _ = fs::remove_file(&exact);
    let over = dir.join("s".repeat(limit - dir_len));
    assert_eq!(over.as_os_str().as_encoded_bytes().len(), limit + 1);
    let error = UnixListener::bind(&over).expect_err("limit+1 must be refused");
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    Ok(())
}

#[test]
fn runtime_base_dirs_are_exclusive_and_never_shared() -> TestResult {
    // Sequential mints with the same tag differ; concurrent mints across
    // threads (same pid) also differ — uniqueness comes from exclusive
    // create_dir, not from the truncated entropy.
    let first = base_dir("shared");
    let second = base_dir("shared");
    assert_ne!(first, second);
    let handles: Vec<_> = (0..8)
        .map(|_| thread::spawn(|| support::runtime_base("shared")))
        .collect();
    let minted: Vec<PathBuf> = handles
        .into_iter()
        .map(|handle| handle.join().expect("minter finished"))
        .collect();
    let mut sorted = minted.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), minted.len(), "no shared directory");
    Ok(())
}

// ---- ambiguous outcomes: typed unknown, never replayed -----------------------

#[test]
fn dropped_response_is_outcome_unknown_without_replay() -> TestResult {
    let base = base_dir("drop");
    let scope = stage_scope(&base);
    // The peer admits one session, answers the snapshot, reads the command,
    // then drops the connection mid-flight. The listener is bound but serves
    // exactly this one script; a CLI replay would need a new connection.
    let socket = support::client_socket(&scope);
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = thread::spawn(move || {
        let stream = accept_bounded(&listener);
        stream
            .set_read_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer read timeout");
        stream
            .set_write_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer write timeout");
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
        let command = read_command(&stream);
        *seen.lock().unwrap() = command.request_id.as_str().to_owned();
        drop(stream);
        // Prove nothing reconnects: the CLI has already exited by the time its
        // caller asserts below, so any backlog entry now is a replay attempt.
        thread::sleep(Duration::from_millis(300));
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Ok(_) => panic!("the CLI must never replay a submitted command"),
            Err(error) => panic!("listener accept failed: {error}"),
        }
    });
    let output = run(
        &base,
        &["tab", "rename", "ops", "--label", "x", "--format=json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(document["failure"]["details"]["reason"], "response-loss");
    assert_eq!(
        document["failure"]["details"]["requestId"]
            .as_str()
            .unwrap(),
        submitted.lock().unwrap().as_str(),
        "the typed unknown reports the submitted request identity"
    );
    assert_eq!(document["source"]["kind"], "workbench-command");
    assert!(
        compiled_schema(&base, "orchbee.cli.failure").is_valid(&document),
        "outcome-unknown envelope validates"
    );
    // stderr tells the operator the effect is undetermined and unrepeated.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not retried"), "{stderr}");
    Ok(())
}

#[test]
fn malformed_and_miscorrelated_responses_are_outcome_unknown() -> TestResult {
    let base = base_dir("ambiguous-wire");
    let scope = stage_scope(&base);
    let malformed_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let _ = read_command(&stream);
        use std::io::Write;
        (&mut &stream)
            .write_all(b"NOT-A-WORKBENCH-FRAME")
            .expect("peer writes garbage");
    });
    let output = run(
        &base,
        &["tab", "move", "ops", "--order", "next", "--format=json"],
    );
    malformed_peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(document["failure"]["details"]["reason"], "response-invalid");

    let mismatch_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let _ = read_command(&stream);
        let wrong = orchbee_workbench_protocol::WorkbenchRequestId::parse(&id(0x77)).unwrap();
        command_response(&stream, wrong, WorkbenchCommandOutcome::Denied {});
    });
    let output = run(
        &base,
        &["tab", "move", "ops", "--order", "next", "--format=json"],
    );
    mismatch_peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "correlation-mismatch"
    );
    Ok(())
}

#[test]
fn wrong_response_kind_is_outcome_unknown() -> TestResult {
    let base = base_dir("wrong-kind");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let _ = read_command(&stream);
        // Answer the command with a snapshot result: a valid frame, wrong kind.
        snapshot_response(
            &stream,
            orchbee_workbench_protocol::WorkbenchRequestId::parse(&id(0x78)).unwrap(),
            WorkbenchSnapshotOutcome::Denied {},
        );
    });
    let output = run(
        &base,
        &[
            "workspace",
            "rename",
            "main",
            "--label",
            "x",
            "--format=json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(document["failure"]["details"]["reason"], "response-invalid");
    Ok(())
}

#[test]
fn incompatible_outcome_is_outcome_unknown() -> TestResult {
    let base = base_dir("incompatible-outcome");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        // A wire-valid but action-incompatible outcome: TabMoved for a rename.
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::TabMoved {
                revision: revision(8),
                terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                    CONTEXT,
                )
                .unwrap(),
                tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(OPS_TAB).unwrap(),
            },
        );
    });
    let output = run(
        &base,
        &["tab", "rename", "ops", "--label", "x", "--format=json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "action-incompatible"
    );
    Ok(())
}

#[test]
fn project_create_dropped_response_is_unknown_without_replay() -> TestResult {
    let base = base_dir("project-drop");
    let scope = stage_scope(&base);
    // Same mid-flight drop as `dropped_response_is_outcome_unknown_without_replay`,
    // against the project arm: the listener stays bound so a replay would need a
    // fresh connection — any accept after the drop is a retry attempt.
    let socket = support::client_socket(&scope);
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = thread::spawn(move || {
        let stream = accept_bounded(&listener);
        stream
            .set_read_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer read timeout");
        stream
            .set_write_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer write timeout");
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
        let command = read_command(&stream);
        let WorkbenchAction::ProjectCreate { .. } = &command.action else {
            panic!("expected project.create, got {:?}", command.action)
        };
        *seen.lock().unwrap() = command.request_id.as_str().to_owned();
        drop(stream);
        thread::sleep(Duration::from_millis(300));
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Ok(_) => panic!("the CLI must never replay a submitted command"),
            Err(error) => panic!("listener accept failed: {error}"),
        }
    });
    let output = run(
        &base,
        &["project", "create", "--label", "x", "--format=json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(document["failure"]["details"]["reason"], "response-loss");
    assert_eq!(
        document["failure"]["details"]["requestId"]
            .as_str()
            .unwrap(),
        submitted.lock().unwrap().as_str(),
        "the typed unknown reports the submitted request identity"
    );
    assert!(
        compiled_schema(&base, "orchbee.cli.failure").is_valid(&document),
        "outcome-unknown envelope validates"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not retried"), "{stderr}");
    Ok(())
}

#[test]
fn project_create_wrong_outcome_is_not_success() -> TestResult {
    let base = base_dir("project-wrong-outcome");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let WorkbenchAction::ProjectCreate { .. } = &command.action else {
            panic!("expected project.create, got {:?}", command.action)
        };
        // A wire-valid but action-incompatible outcome: WorkspaceCreated for a
        // project create must never build a committed receipt.
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::WorkspaceCreated {
                revision: revision(8),
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&id(0x61))
                    .unwrap(),
                terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                    &id(0x62),
                )
                .unwrap(),
            },
        );
    });
    let output = run(
        &base,
        &["project", "create", "--label", "x", "--format=json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "action-incompatible"
    );
    Ok(())
}

// ---- workspace accept-directory ---------------------------------------------
//
// The literal path `LITERAL_PATH` deliberately contains '~' and '$'
// fragments so the wire and receipt assertions prove the CLI never expands,
// canonicalizes, or probes the value.
const LITERAL_PATH: &str = "/srv/$HOME/~/repo";
const DIRECTORY: &str = "00000000000000000000000000000044";
const WORKER: &str = "00000000000000000000000000000045";

fn accept_directory_args(target: &str) -> Vec<&str> {
    vec![
        "workspace",
        "accept-directory",
        target,
        "--path",
        LITERAL_PATH,
    ]
}

#[test]
fn workspace_accept_directory_commits_under_a_fresh_request() -> TestResult {
    let base = base_dir("accept-dir");
    let scope = stage_scope(&base);
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = spawn_peer_n(&scope, 2, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert_command_base(&command, snapshot_request.as_str());
        let WorkbenchAction::WorkspacePrimaryDirectoryAccept { workspace_id, path } =
            &command.action
        else {
            panic!(
                "expected workspace.primary-directory.accept, got {:?}",
                command.action
            )
        };

        assert_eq!(workspace_id.as_str(), WORKSPACE, "label 'main' resolved");
        assert_eq!(
            path.as_str(),
            LITERAL_PATH,
            "the literal path reaches the wire unexpanded"
        );
        *seen.lock().unwrap() = command.request_id.as_str().to_owned();
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
                revision: revision(8),
                directory_id: orchbee_workbench_protocol::WorkbenchDirectoryId::parse(DIRECTORY)
                    .unwrap(),
                worker_id: orchbee_workbench_protocol::WorkerId::parse(WORKER).unwrap(),
            },
        );
    });
    let mut argv = accept_directory_args("main");
    argv.push("--format=json");
    let output = run(&base, &argv);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.mutation.result");
    assert_eq!(document["source"]["kind"], "workbench-command");
    assert_eq!(document["source"]["sessionId"], SESSION);
    assert_eq!(document["source"]["expectedRevision"], "7");
    assert_eq!(document["result"]["action"], "workspace.accept-directory");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["revision"], "8");
    assert!(document["result"].get("projectId").is_none());
    assert_eq!(document["result"]["workspaceId"], WORKSPACE);
    assert_eq!(document["result"]["directoryId"], DIRECTORY);
    assert_eq!(document["result"]["workerId"], WORKER);
    assert_eq!(
        document["result"]["path"], LITERAL_PATH,
        "the receipt echoes the literal request path"
    );
    assert_eq!(
        document["source"]["requestId"].as_str().unwrap(),
        submitted.lock().unwrap().as_str()
    );
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));

    // Text output renders the same committed fields, path included.
    let text = run(&base, &accept_directory_args(WORKSPACE));
    assert_eq!(
        text.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&text.stderr)
    );
    let stdout = String::from_utf8(text.stdout)?;
    assert!(stdout.contains("workspace.accept-directory"), "{stdout}");
    assert!(stdout.contains("committed"), "{stdout}");
    assert!(stdout.contains(DIRECTORY), "{stdout}");
    assert!(stdout.contains(WORKER), "{stdout}");
    assert!(stdout.contains(LITERAL_PATH), "{stdout}");
    assert!(text.stderr.is_empty(), "clean success has no stderr");
    peer.join().expect("peer finished");
    Ok(())
}

#[test]
fn workspace_accept_directory_conflict_and_unavailable_map_matrix() -> TestResult {
    let base = base_dir("accept-dir-matrix");
    let scope = stage_scope(&base);
    // A Workspace that already holds a directory — even the same path — is the
    // Server's `WorkspaceDirectoryConflict` rejection, never a client-side
    // idempotent success.
    let conflict_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. } = &command.action else {
            panic!(
                "expected workspace.primary-directory.accept, got {:?}",
                command.action
            )
        };
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Rejected {
                revision: revision(7),
                reason: WorkbenchRejectionReason::WorkspaceDirectoryConflict,
            },
        );
    });
    let mut argv = accept_directory_args("main");
    argv.push("--format=json");
    let output = run(&base, &argv);
    conflict_peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "rejected");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "workspace-directory-conflict"
    );
    assert_eq!(document["failure"]["details"]["revision"], "7");
    assert!(compiled_schema(&base, "orchbee.cli.failure").is_valid(&document));

    // A path the Server's probe cannot accept as a directory is the
    // `DirectoryUnavailable` precommit — service-unavailable, exit 4.
    let unavailable_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::DirectoryUnavailable,
            },
        );
    });
    let output = run(&base, &argv);
    unavailable_peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "directory-unavailable"
    );
    assert!(compiled_schema(&base, "orchbee.cli.failure").is_valid(&document));
    Ok(())
}

#[test]
fn workspace_accept_directory_dropped_response_is_unknown_without_replay() -> TestResult {
    let base = base_dir("accept-dir-drop");
    let scope = stage_scope(&base);
    // Same mid-flight drop as the other mutation arms: the listener stays
    // bound, so any second accept is a replay attempt.
    let socket = support::client_socket(&scope);
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = thread::spawn(move || {
        let stream = accept_bounded(&listener);
        stream
            .set_read_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer read timeout");
        stream
            .set_write_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer write timeout");
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
        let command = read_command(&stream);
        let WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. } = &command.action else {
            panic!(
                "expected workspace.primary-directory.accept, got {:?}",
                command.action
            )
        };
        *seen.lock().unwrap() = command.request_id.as_str().to_owned();
        drop(stream);
        thread::sleep(Duration::from_millis(300));
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Ok(_) => panic!("the CLI must never replay a submitted command"),
            Err(error) => panic!("listener accept failed: {error}"),
        }
    });
    let mut argv = accept_directory_args("main");
    argv.push("--format=json");
    let output = run(&base, &argv);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(document["failure"]["details"]["reason"], "response-loss");
    assert_eq!(
        document["failure"]["details"]["requestId"]
            .as_str()
            .unwrap(),
        submitted.lock().unwrap().as_str(),
        "the typed unknown reports the submitted request identity"
    );
    assert!(
        compiled_schema(&base, "orchbee.cli.failure").is_valid(&document),
        "outcome-unknown envelope validates"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not retried"), "{stderr}");
    Ok(())
}

#[test]
fn workspace_accept_directory_wrong_outcome_is_not_success() -> TestResult {
    let base = base_dir("accept-dir-wrong-outcome");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. } = &command.action else {
            panic!(
                "expected workspace.primary-directory.accept, got {:?}",
                command.action
            )
        };
        // A wire-valid but action-incompatible outcome: Renamed for an
        // accept-directory must never build a committed receipt.
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Renamed {
                revision: revision(8),
            },
        );
    });
    let mut argv = accept_directory_args("main");
    argv.push("--format=json");
    let output = run(&base, &argv);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "action-incompatible"
    );
    Ok(())
}

#[test]
fn workspace_accept_directory_ambiguous_label_but_id_resolves() -> TestResult {
    let base = base_dir("accept-dir-ambiguous");
    let scope = stage_scope(&base);
    // Two projects each own a workspace labelled "dup": the label is
    // ambiguous across the snapshot, while each typed id still resolves to
    // its own row — and to the true owning project on that row.
    let dup_fixture: &[u8] = br#"{
  "kind": "workbench.snapshot",
  "protocolVersion": 1,
  "revision": "7",
  "sessionId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "navigation": {"projects":[{"projectId":"00000000000000000000000000000010", "label":"pa"}, {"projectId":"00000000000000000000000000000011", "label":"pb"}], "directories":[], "workspaces":[{"workspaceId":"00000000000000000000000000000001","terminalContextId":"00000000000000000000000000000001","projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":"00000000000000000000000000000012", "label":"dup", "primaryDirectoryId":null, "terminalContextId":"00000000000000000000000000000020", "projectId":"00000000000000000000000000000010"}, {"workspaceId":"00000000000000000000000000000013", "label":"dup", "primaryDirectoryId":null, "terminalContextId":"00000000000000000000000000000021", "projectId":"00000000000000000000000000000011"}]},
  "terminalContexts": [
    {
      "terminalContextId": "00000000000000000000000000000001",
      "groups": [],
      "tabs": []
    },
    {
      "terminalContextId": "00000000000000000000000000000020",
      "groups": [],
      "tabs": []
    },
    {
      "terminalContextId": "00000000000000000000000000000021",
      "groups": [],
      "tabs": []
    }
  ]
}"#;
    let peer = spawn_peer_n(&scope, 2, move |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(dup_fixture).expect("dup fixture"),
            },
        );
        static CALL: Mutex<u32> = Mutex::new(0);
        let call = {
            let mut guard = CALL.lock().unwrap();
            *guard += 1;
            *guard
        };
        if call == 1 {
            // The ambiguous label must stop before any command: nothing more
            // arrives on this connection.
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read timeout");
            if let Ok(request) = read_workbench_request(&mut &stream) {
                panic!("unexpected request after ambiguity: {request:?}");
            }
            return;
        }
        let command = read_command(&stream);
        let WorkbenchAction::WorkspacePrimaryDirectoryAccept { workspace_id, path } =
            &command.action
        else {
            panic!(
                "expected workspace.primary-directory.accept, got {:?}",
                command.action
            )
        };
        // The typed id resolved to its own snapshot row — and to the true
        // project on that row, not the sibling workspace's owner.
        assert_eq!(workspace_id.as_str(), id(0x12));

        assert_eq!(path.as_str(), LITERAL_PATH);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
                revision: revision(8),
                directory_id: orchbee_workbench_protocol::WorkbenchDirectoryId::parse(&id(0x50))
                    .unwrap(),
                worker_id: orchbee_workbench_protocol::WorkerId::parse(&id(0x51)).unwrap(),
            },
        );
    });
    let mut argv = accept_directory_args("dup");
    argv.push("--format=json");
    let output = run(&base, &argv);
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "ambiguous");
    assert_eq!(document["failure"]["details"]["resource"], "workspace");

    let workspace_id = id(0x12);
    let mut argv = accept_directory_args(&workspace_id);
    argv.push("--format=json");
    let output = run(&base, &argv);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["result"]["projectId"], id(0x10));
    assert_eq!(document["result"]["workspaceId"], id(0x12));
    assert_eq!(document["result"]["directoryId"], id(0x50));
    assert_eq!(document["result"]["workerId"], id(0x51));
    assert_eq!(document["result"]["path"], LITERAL_PATH);
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));
    Ok(())
}

// ---- failures that must precede any command --------------------------------

#[test]
fn snapshot_unavailable_never_reaches_a_command() -> TestResult {
    let base = base_dir("no-command");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Unavailable {
                reason: orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        );
        // The CLI must stop at the snapshot failure: nothing more arrives on
        // this connection before it is retired.
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        if let Ok(request) = read_workbench_request(&mut &stream) {
            panic!("unexpected request after snapshot failure: {request:?}");
        }
    });
    let output = run(
        &base,
        &[
            "workspace",
            "rename",
            "main",
            "--label",
            "x",
            "--format=json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["stage"], "snapshot");
    Ok(())
}

#[test]
fn unresolved_targets_never_reach_a_command() -> TestResult {
    let base = base_dir("unresolved");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 2, |stream| {
        admit_and_serve_snapshot(&stream);
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        if let Ok(request) = read_workbench_request(&mut &stream) {
            panic!("unexpected request after resolve failure: {request:?}");
        }
    });
    // Unknown workspace label → not-found; the command is never written.
    let output = run(
        &base,
        &[
            "workspace",
            "rename",
            "ghost",
            "--label",
            "x",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(envelope(&output)["failure"]["kind"], "not-found");
    assert_eq!(
        envelope(&output)["failure"]["details"]["resource"],
        "workspace"
    );
    // Unknown tab id → not-found on the tab resource.
    let ghost = id(0x7e);
    let output = run(
        &base,
        &["tab", "close", ghost.as_str(), "--confirm", "--format=json"],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(envelope(&output)["failure"]["kind"], "not-found");
    peer.join().expect("peer finished");
    Ok(())
}

#[test]
fn ambiguous_label_never_reaches_a_command() -> TestResult {
    let base = base_dir("ambiguous-target");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        // A snapshot with two tabs sharing the label "dup".
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(
                    br#"{
  "kind": "workbench.snapshot",
  "protocolVersion": 1,
  "revision": "3",
  "sessionId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "navigation": {"projects":[], "directories":[], "workspaces":[{"workspaceId":"00000000000000000000000000000001","terminalContextId":"00000000000000000000000000000001","projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
  "terminalContexts": [
    {
      "terminalContextId": "00000000000000000000000000000001",
      "groups": [],
      "tabs": [
        {
          "tabId": "00000000000000000000000000000002",
          "label": "dup",
          "pinned": true,
          "groupId": null,
          "root": {
            "kind": "panel",
            "panelId": "00000000000000000000000000000003",
            "primaryPresentation": {
              "presentationId": "00000000000000000000000000000004",
              "label": "a",
              "binding": null
            },
            "childPresentations": []
          }
        },
        {
          "tabId": "00000000000000000000000000000005",
          "label": "dup",
          "pinned": false,
          "groupId": null,
          "root": {
            "kind": "panel",
            "panelId": "00000000000000000000000000000006",
            "primaryPresentation": {
              "presentationId": "00000000000000000000000000000007",
              "label": "b",
              "binding": null
            },
            "childPresentations": []
          }
        }
      ]
    }
  ]
}"#,
                )
                .expect("fixture"),
            },
        );
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        if let Ok(request) = read_workbench_request(&mut &stream) {
            panic!("unexpected request after ambiguity: {request:?}");
        }
    });
    let output = run(
        &base,
        &["tab", "rename", "dup", "--label", "x", "--format=json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "ambiguous");
    assert_eq!(document["failure"]["details"]["resource"], "tab");
    Ok(())
}

// ---- usage gates ------------------------------------------------------------

#[test]
fn close_commands_fail_before_runtime_without_confirm() -> TestResult {
    // A runtime base that never exists proves rejection precedes discovery.
    let base = base_dir("confirm").join("never-created");
    let tab_id = id(0x30);
    let panel_id = id(0x43);
    for arguments in [
        &["tab", "close", tab_id.as_str()][..],
        &["panel", "close", panel_id.as_str()][..],
        &["tab", "close", tab_id.as_str(), "--yes"][..],
    ] {
        let mut argv: Vec<&str> = arguments.to_vec();
        argv.extend(["--format", "json"]);
        let output = run(&base, &argv);
        assert_eq!(output.status.code(), Some(2), "{argv:?}");
        assert_eq!(
            envelope(&output)["failure"]["kind"],
            "invalid-usage",
            "{argv:?}"
        );
    }
    assert!(
        !base.exists(),
        "missing --confirm must not touch the runtime"
    );
    Ok(())
}

#[test]
fn mutation_usage_failures_precede_runtime_access() -> TestResult {
    let base = base_dir("usage").join("never-created");
    let cases: &[&[&str]] = &[
        &["workspace", "create", "--label", "x"],
        &["workspace", "create", "--project", "p"],
        // A partial geometry set is still a usage failure before any
        // runtime access — the group is all-or-none.
        &[
            "tab",
            "create",
            "--workspace",
            "w",
            "--label",
            "t",
            "--cols",
            "80",
        ],
        &[
            "tab",
            "create",
            "--standalone",
            "--workspace",
            "w",
            "--label",
            "t",
        ],
        &["tab", "move", "t", "--direction", "above"],
        &["panel", "move", "p", "--direction", "left"],
        &[
            "panel",
            "split",
            "not-an-id",
            "--direction",
            "right",
            "--label",
            "p",
            "--cols",
            "80",
            "--rows",
            "24",
            "--cell-width",
            "9",
            "--cell-height",
            "18",
        ],
        &["workspace", "rename", "w", "--label", "x", "--confirm"],
        &["workspace", "accept-directory", "w"],
        &["workspace", "accept-directory", "w", "--path", "rel/dir"],
        &[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            "/a",
            "--label",
            "x",
        ],
        &[
            "workspace",
            "accept-directory",
            "00000000000000000000000000000000",
            "--path",
            "/a",
        ],
        &["status", "--label", "x"],
    ];
    for arguments in cases {
        let mut argv: Vec<&str> = arguments.to_vec();
        argv.extend(["--format", "json"]);
        let output = run(&base, &argv);
        assert_eq!(output.status.code(), Some(2), "{argv:?}");
        let kind = envelope(&output)["failure"]["kind"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(
            kind == "invalid-usage" || kind == "invalid-identifier",
            "{argv:?} → {kind}"
        );
    }
    assert!(!base.exists(), "usage failures must precede discovery");
    Ok(())
}

// ---- formats and hostile input ----------------------------------------------

#[test]
fn text_receipt_is_human_readable() -> TestResult {
    let base = base_dir("text");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let WorkbenchAction::WorkspaceRename { .. } = &command.action else {
            panic!("expected workspace.rename")
        };
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Renamed {
                revision: revision(8),
            },
        );
    });
    let output = run(&base, &["workspace", "rename", "main", "--label", "ops"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout)?;
    assert!(text.contains("workspace.rename"), "{text}");
    assert!(text.contains("committed"), "{text}");
    assert!(text.contains("8"), "{text}");
    assert!(text.contains(WORKSPACE), "{text}");
    assert!(output.stderr.is_empty(), "clean success has no stderr");
    Ok(())
}

#[test]
fn hostile_labels_round_trip_through_the_receipt() -> TestResult {
    let base = base_dir("hostile");
    let scope = stage_scope(&base);
    // Quotes, backslashes, newlines, and control-adjacent bytes stay inside
    // the JSON encoding untouched; the peer sees them verbatim.
    let hostile = "x\"\\/\n\t end — 中文";
    let peer = spawn_peer(&scope, move |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        let WorkbenchAction::WorkspaceRename { label, .. } = &command.action else {
            panic!("expected workspace.rename")
        };
        assert_eq!(label.as_str(), hostile, "label reaches the wire verbatim");
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Renamed {
                revision: revision(8),
            },
        );
    });
    let output = run(
        &base,
        &[
            "workspace",
            "rename",
            "main",
            "--label",
            hostile,
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let document = envelope(&output);
    assert_eq!(document["result"]["label"], hostile);
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));

    // Text output escapes nothing (labels are data, not markup) but stays
    // bounded and parses as ordinary rows.
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Unchanged {
                revision: revision(7),
            },
        );
    });
    let output = run(&base, &["workspace", "rename", "main", "--label", hostile]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.len() < 8192, "text receipt stays small");
    Ok(())
}

// ---- project rename -----------------------------------------------------------

/// `project rename` submits the already-modeled `ProjectRename` action through
/// the shared setup transport. A label target resolves to the snapshot's own
/// project id; an id target resolves by identity. The committed receipt
/// reports under the dedicated `orchbee.project.rename` document — the shared
/// mutation envelope's action enum stays closed.
#[test]
fn project_rename_commits_by_id_and_label() -> TestResult {
    let base = base_dir("project-rename");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 2, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert_command_base(&command, snapshot_request.as_str());
        let WorkbenchAction::ProjectRename { project_id, label } = &command.action else {
            panic!("expected project.rename, got {:?}", command.action)
        };
        assert_eq!(
            project_id.as_str(),
            PROJECT,
            "id and 'web' resolve to the same row"
        );
        assert_eq!(label.as_str(), "new");
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Renamed {
                revision: revision(8),
            },
        );
    });

    for target in [PROJECT, "web"] {
        let output = run(
            &base,
            &[
                "project",
                "rename",
                target,
                "--label",
                "new",
                "--format=json",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document = envelope(&output);
        assert_eq!(document["schema"], "orchbee.project.rename");
        assert_eq!(document["version"], 1);
        assert_eq!(document["source"]["kind"], "workbench-command");
        assert_eq!(document["source"]["sessionId"], SESSION);
        assert_eq!(document["source"]["expectedRevision"], "7");
        assert_eq!(document["result"]["action"], "project.rename");
        assert_eq!(document["result"]["outcome"], "committed");
        assert_eq!(document["result"]["revision"], "8");
        assert_eq!(document["result"]["projectId"], PROJECT);
        assert_eq!(document["result"]["label"], "new");
        assert!(
            compiled_schema(&base, "orchbee.project.rename").is_valid(&document),
            "receipt validates under its dedicated schema"
        );
    }
    peer.join().expect("peer finished");
    Ok(())
}

/// An equal-label rename is the Server's `Unchanged` verdict: the receipt
/// reports `unchanged` with the base revision, still under the dedicated
/// schema.
#[test]
fn project_rename_unchanged_reports_base_revision() -> TestResult {
    let base = base_dir("project-rename-unchanged");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        assert!(
            matches!(&command.action, WorkbenchAction::ProjectRename { .. }),
            "expected project.rename, got {:?}",
            command.action
        );
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Unchanged {
                revision: revision(7),
            },
        );
    });
    let output = run(
        &base,
        &[
            "project", "rename", PROJECT, "--label", "web", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.project.rename");
    assert_eq!(document["result"]["outcome"], "unchanged");
    assert_eq!(document["result"]["revision"], "7");
    assert!(compiled_schema(&base, "orchbee.project.rename").is_valid(&document));
    Ok(())
}

/// The shared precommit matrix maps unchanged: rejected→3, denied→3,
/// unavailable→4 — no new failure kinds for the new command.
#[test]
fn project_rename_precommit_verdicts_map_matrix() -> TestResult {
    let base = base_dir("project-rename-matrix");
    let scope = stage_scope(&base);
    let rejected_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Rejected {
                revision: revision(7),
                reason: WorkbenchRejectionReason::ProjectUnavailable,
            },
        );
    });
    let output = run(
        &base,
        &[
            "project",
            "rename",
            PROJECT,
            "--label",
            "x",
            "--format=json",
        ],
    );
    rejected_peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "rejected");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "project-unavailable"
    );
    assert_eq!(document["failure"]["details"]["revision"], "7");
    assert!(compiled_schema(&base, "orchbee.cli.failure").is_valid(&document));

    let denied_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Denied {},
        );
    });
    let output = run(
        &base,
        &[
            "project",
            "rename",
            PROJECT,
            "--label",
            "x",
            "--format=json",
        ],
    );
    denied_peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(envelope(&output)["failure"]["kind"], "denied");

    let unavailable_peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
            },
        );
    });
    let output = run(
        &base,
        &[
            "project",
            "rename",
            PROJECT,
            "--label",
            "x",
            "--format=json",
        ],
    );
    unavailable_peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    Ok(())
}

/// Resolve failures submit nothing: an unknown label is `not-found` and two
/// same-label Projects are `ambiguous` — both stop after the snapshot read
/// with zero commands on the wire.
#[test]
fn project_rename_resolve_failures_submit_nothing() -> TestResult {
    let base = base_dir("project-rename-unresolved");
    let scope = stage_scope(&base);
    // Connection 1: the stock fixture has no "ghost" project → not-found.
    // Connection 2: an inline snapshot with two "web" Projects → ambiguous.
    // Either way the CLI must retire without writing a command: any further
    // request on the connection is a test failure.
    let peer = spawn_peer_n(&scope, 2, |stream| {
        static CALL: Mutex<u32> = Mutex::new(0);
        let call = {
            let mut guard = CALL.lock().unwrap();
            *guard += 1;
            *guard
        };
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        if call == 1 {
            snapshot_response(
                &stream,
                request_id,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: parse_snapshot(fixture_document()).expect("fixture"),
                },
            );
        } else {
            // Two "web" Projects: derive the ambiguous snapshot from the
            // stock fixture so every cross-reference stays intact — only the
            // second row's identity (and its empty workspace list) changes.
            let mut document: serde_json::Value =
                serde_json::from_slice(fixture_document()).expect("fixture");
            let mut duplicate = document["navigation"]["projects"][0].clone();
            duplicate["projectId"] = serde_json::json!(id(0x12));
            duplicate["workspaces"] = serde_json::json!([]);
            document["navigation"]["projects"]
                .as_array_mut()
                .unwrap()
                .push(duplicate);
            let bytes = serde_json::to_vec(&document).expect("fixture bytes");
            snapshot_response(
                &stream,
                request_id,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: parse_snapshot(&bytes).expect("duplicate-label fixture"),
                },
            );
        }
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        if let Ok(request) = read_workbench_request(&mut &stream) {
            panic!("unexpected request after resolve failure: {request:?}");
        }
    });

    let output = run(
        &base,
        &[
            "project", "rename", "ghost", "--label", "x", "--format", "json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "not-found");
    assert_eq!(document["failure"]["details"]["resource"], "project");

    let output = run(
        &base,
        &[
            "project", "rename", "web", "--label", "x", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "ambiguous");
    assert_eq!(document["failure"]["details"]["resource"], "project");
    assert_eq!(
        document["failure"]["details"]["candidates"],
        serde_json::json!([PROJECT, id(0x12)])
    );
    assert_eq!(document["failure"]["details"]["totalCount"], 2);
    Ok(())
}

/// A dropped post-submission answer is `outcome-unknown` with the submitted
/// request identity — the listener sees exactly one connection and one
/// command; nothing is replayed.
#[test]
fn project_rename_dropped_response_is_outcome_unknown() -> TestResult {
    let base = base_dir("project-rename-drop");
    let scope = stage_scope(&base);
    let socket = support::client_socket(&scope);
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = thread::spawn(move || {
        let stream = accept_bounded(&listener);
        stream
            .set_read_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer read timeout");
        stream
            .set_write_timeout(Some(PEER_IO_DEADLINE))
            .expect("peer write timeout");
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
        let command = read_command(&stream);
        *seen.lock().unwrap() = command.request_id.as_str().to_owned();
        drop(stream);
        thread::sleep(Duration::from_millis(300));
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Ok(_) => panic!("the CLI must never replay a submitted command"),
            Err(error) => panic!("listener accept failed: {error}"),
        }
    });
    let output = run(
        &base,
        &[
            "project",
            "rename",
            PROJECT,
            "--label",
            "x",
            "--format=json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(document["failure"]["details"]["stage"], "command");
    assert_eq!(
        document["failure"]["details"]["requestId"]
            .as_str()
            .unwrap(),
        submitted.lock().unwrap().as_str(),
        "the typed unknown reports the submitted request identity"
    );
    assert!(compiled_schema(&base, "orchbee.cli.failure").is_valid(&document));
    Ok(())
}

/// The machine document is locale-neutral: en-US and zh-CN runs emit the
/// same schema and every semantic field; each run's `source.requestId` is a
/// fresh 32-hex identity of its own — identities are never claimed equal.
#[test]
fn project_rename_machine_document_is_locale_neutral() -> TestResult {
    let base = base_dir("project-rename-locale");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 2, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Renamed {
                revision: revision(8),
            },
        );
    });
    let argv = [
        "project", "rename", PROJECT, "--label", "new", "--format", "json",
    ];
    let english = run(&base, &argv);
    let chinese = support::run_lang(
        &cohort().join(crate::support::staged_artifact_name("orchbee")),
        &base,
        &argv,
        "zh_CN.UTF-8",
    );
    peer.join().expect("peer finished");
    assert_eq!(english.status.code(), Some(0));
    assert_eq!(chinese.status.code(), Some(0));
    let english_doc = envelope(&english);
    let chinese_doc = envelope(&chinese);
    for pointer in [
        "/schema",
        "/version",
        "/source/kind",
        "/source/sessionId",
        "/source/expectedRevision",
        "/result/action",
        "/result/outcome",
        "/result/revision",
        "/result/projectId",
        "/result/label",
    ] {
        assert_eq!(
            english_doc.pointer(pointer),
            chinese_doc.pointer(pointer),
            "semantic field {pointer} must be locale-neutral"
        );
    }
    // Each run submitted under its own fresh request identity: both are
    // legal 32-hex ids and never equal to each other.
    let english_id = english_doc["source"]["requestId"].as_str().unwrap();
    let chinese_id = chinese_doc["source"]["requestId"].as_str().unwrap();
    for request_id in [english_id, chinese_id] {
        assert_eq!(request_id.len(), 32, "request id shape: {request_id}");
        assert!(
            request_id
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "request id is lowercase hex: {request_id}"
        );
    }
    assert_ne!(english_id, chinese_id, "request identities stay fresh");
    // A successful mutation emits no diagnostics under either locale: the
    // stream split is itself locale-neutral.
    assert!(english.stderr.is_empty() && chinese.stderr.is_empty());
    Ok(())
}

/// The text receipt renders the same fields one per line; nothing
/// locale-specific or schema-shaped leaks into it.
#[test]
fn project_rename_text_receipt_rows() -> TestResult {
    let base = base_dir("project-rename-text");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit_and_serve_snapshot(&stream);
        let command = read_command(&stream);
        command_response(
            &stream,
            command.request_id,
            WorkbenchCommandOutcome::Renamed {
                revision: revision(8),
            },
        );
    });
    let output = run(&base, &["project", "rename", "web", "--label", "new"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    for row in [
        "action",
        "outcome",
        "revision",
        "requestId",
        "projectId",
        "label",
    ] {
        assert!(text.contains(row), "text receipt names {row}: {text}");
    }
    assert!(text.contains("project.rename"), "{text}");
    assert!(text.contains("committed"), "{text}");
    assert!(text.contains(PROJECT), "{text}");
    Ok(())
}

// ---- workspace close route ---------------------------------------------------

/// The runtime base this test mints under TMPDIR, reclaimed on every exit:
/// `Drop` runs on the normal path and during unwind alike, so a panic —
/// including one inside the peer script surfaced through `peer.join` —
/// cannot leave the directory behind. Deliberately best-effort and
/// Unix-only like the rest of this file: no handle-release semantics are
/// needed because the CLI child is always reaped by `support`'s bounded
/// wait and the peer thread retires on its own accept/IO deadlines — the
/// guard never joins it, so a drop cannot block the suite. Not a shared
/// fixture: it exists for this test's owned directory only.
struct BaseGuard(PathBuf);

impl BaseGuard {
    fn new(tag: &str) -> Self {
        Self(base_dir(tag))
    }
}

impl std::ops::Deref for BaseGuard {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for BaseGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `workspace close` → `workspace close-status` → `workspace close-resume` →
/// `workspace close-status`: four real CLI invocations against one scripted
/// peer prove the command route end to end — argv parse, `main` label →
/// `(project, workspace)` resolution on the real snapshot, a fresh request
/// identity per invocation, the Server-issued stable close id flowing from
/// the accepted receipt into the resume request, and the typed result
/// envelopes the user reads. The peer — not the CLI — is the wire oracle:
/// it asserts the real scope, the stable operation id, and the explicit
/// attempt-generation fence on every frame.
#[test]
fn workspace_close_route_reaches_typed_receipts_and_closed_status() -> TestResult {
    let base = BaseGuard::new("ws-close-route");
    let scope = stage_scope(&base);
    // The stable close operation id the Server allocates; the chain then
    // keys status and resume by it.
    const CLOSE_ID: &str = "00000000000000000000000000000066";
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = spawn_peer_n(&scope, 4, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        match read_workbench_request(&mut &stream).expect("peer reads request") {
            WorkbenchRequest::Command(command) => {
                assert_command_base(&command, snapshot_request.as_str());
                *seen.lock().unwrap() = command.request_id.as_str().to_owned();
                let WorkbenchAction::WorkspaceClose { workspace_id } = &command.action else {
                    panic!("expected workspace.close, got {:?}", command.action)
                };
                // The `main` positional resolved against the real snapshot.

                assert_eq!(workspace_id.as_str(), WORKSPACE);
                command_response(
                    &stream,
                    command.request_id,
                    WorkbenchCommandOutcome::WorkspaceCloseAccepted {
                        close_request_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(
                            CLOSE_ID,
                        )
                        .unwrap(),

                        workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                            WORKSPACE,
                        )
                        .unwrap(),
                        terminal_context_id:
                            orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(CONTEXT)
                                .unwrap(),
                        accepted_revision: revision(7),
                    },
                );
            }
            WorkbenchRequest::WorkspaceCloseRead(read) => {
                assert_eq!(read.session_id.as_str(), SESSION);

                assert_eq!(read.workspace_id.as_str(), WORKSPACE);
                assert_ne!(
                    read.request_id.as_str(),
                    snapshot_request.as_str(),
                    "close-status request identity is fresh"
                );
                // First status answers Closing at generation 1; the one
                // after resume answers Closed at generation 2.
                static READ: Mutex<u32> = Mutex::new(0);
                let read_call = {
                    let mut guard = READ.lock().unwrap();
                    *guard += 1;
                    *guard
                };
                let (attempt_generation, status) = if read_call == 1 {
                    (
                        WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                        WorkbenchTabCloseStatus::Closing {
                            phase: WorkbenchTabClosePhase::Queued,
                        },
                    )
                } else {
                    (
                        WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
                        WorkbenchTabCloseStatus::Closed {
                            revision: revision(8),
                        },
                    )
                };
                write_workbench_response(
                    &mut &stream,
                    &WorkbenchResponse::WorkspaceCloseRead(WorkbenchWorkspaceCloseReadResult {
                        protocol_version: ProtocolVersion,
                        request_id: read.request_id,
                        outcome: WorkbenchWorkspaceCloseReadOutcome::Available {
                            close_request_id:
                                orchbee_workbench_protocol::WorkbenchRequestId::parse(CLOSE_ID)
                                    .unwrap(),

                            workspace_id: read.workspace_id,
                            terminal_context_id:
                                orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                                    CONTEXT,
                                )
                                .unwrap(),
                            attempt_generation,
                            status,
                            counts: WorkbenchWorkspaceCloseCounts::default(),
                            members: Vec::new(),
                        },
                    }),
                )
                .expect("peer writes close read result");
            }
            WorkbenchRequest::WorkspaceCloseResume(resume) => {
                assert_eq!(resume.session_id.as_str(), SESSION);

                assert_eq!(resume.workspace_id.as_str(), WORKSPACE);
                // The stable id the accepted receipt carried reaches the
                // wire verbatim under the caller's generation fence.
                assert_eq!(resume.close_request_id.as_str(), CLOSE_ID);
                assert_eq!(resume.expected_attempt_generation.value(), 1);
                write_workbench_response(
                    &mut &stream,
                    &WorkbenchResponse::WorkspaceCloseResume(WorkbenchWorkspaceCloseResumeResult {
                        protocol_version: ProtocolVersion,
                        request_id: resume.request_id,
                        session_id: resume.session_id,

                        workspace_id: resume.workspace_id,
                        close_request_id: resume.close_request_id,
                        outcome: WorkbenchWorkspaceCloseResumeOutcome::Resumed {
                            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(2).unwrap(),
                        },
                    }),
                )
                .expect("peer writes close resume result");
            }
            other => panic!("peer expected a close-family request, got {other:?}"),
        }
    });

    let output = run(
        &base,
        &[
            "workspace",
            "close",
            "main",
            "--confirm",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.mutation.result");
    assert_eq!(document["version"], 1);
    assert_eq!(document["source"]["kind"], "workbench-command");
    assert_eq!(document["source"]["sessionId"], SESSION);
    assert_eq!(document["source"]["expectedRevision"], "7");
    assert_eq!(
        document["source"]["requestId"].as_str().unwrap(),
        submitted.lock().unwrap().as_str()
    );
    assert_eq!(document["result"]["action"], "workspace.close");
    assert_eq!(document["result"]["outcome"], "accepted");
    assert_eq!(document["result"]["closeRequestId"], CLOSE_ID);
    assert_eq!(document["result"]["acceptedRevision"], "7");
    assert!(document["result"].get("projectId").is_none());
    assert_eq!(document["result"]["workspaceId"], WORKSPACE);
    assert_eq!(document["result"]["terminalContextId"], CONTEXT);
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));

    let output = run(
        &base,
        &[
            "workspace",
            "close-status",
            "--workspace",
            WORKSPACE,
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.workspace.close-status");
    assert_eq!(document["version"], 1);
    assert_eq!(document["source"]["kind"], "workbench-snapshot");
    assert_eq!(document["source"]["sessionId"], SESSION);
    assert_eq!(document["source"]["revision"], "7");
    assert_eq!(document["result"]["kind"], "available");
    assert_eq!(document["result"]["closeRequestId"], CLOSE_ID);
    assert_eq!(document["result"]["attemptGeneration"], "1");
    assert_eq!(document["result"]["status"]["kind"], "closing");
    assert_eq!(document["result"]["status"]["phase"], "queued");
    assert!(compiled_schema(&base, "orchbee.workspace.close-status").is_valid(&document));

    let output = run(
        &base,
        &[
            "workspace",
            "close-resume",
            "--workspace",
            WORKSPACE,
            "--request",
            CLOSE_ID,
            "--generation",
            "1",
            "--confirm",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.mutation.result");
    assert_eq!(document["version"], 1);
    // `close-resume` reports its own request echo — there is no command.
    assert_eq!(document["source"]["kind"], "workspace-close-resume");
    assert_eq!(document["source"]["sessionId"], SESSION);
    assert_eq!(document["source"]["closeRequestId"], CLOSE_ID);
    assert_eq!(document["source"]["expectedAttemptGeneration"], "1");
    assert_eq!(document["result"]["action"], "workspace.close-resume");
    assert_eq!(document["result"]["outcome"], "resumed");
    assert_eq!(document["result"]["closeRequestId"], CLOSE_ID);
    assert_eq!(document["result"]["attemptGeneration"], "2");
    assert!(document["result"].get("projectId").is_none());
    assert_eq!(document["result"]["workspaceId"], WORKSPACE);
    assert!(compiled_schema(&base, "orchbee.mutation.result").is_valid(&document));

    let output = run(
        &base,
        &[
            "workspace",
            "close-status",
            "--workspace",
            WORKSPACE,
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.workspace.close-status");
    assert_eq!(document["result"]["kind"], "available");
    assert_eq!(document["result"]["closeRequestId"], CLOSE_ID);
    assert_eq!(document["result"]["attemptGeneration"], "2");
    assert_eq!(document["result"]["status"]["kind"], "closed");
    assert_eq!(document["result"]["status"]["revision"], "8");
    assert!(compiled_schema(&base, "orchbee.workspace.close-status").is_valid(&document));

    peer.join().expect("peer finished");
    Ok(())
}
