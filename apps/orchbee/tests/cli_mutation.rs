//! Process-level mutation evidence: the real `orchbee` binary runs against a
//! scripted local peer bound at the Site's client socket. The peer speaks the
//! genuine OBLC admission and OBWP snapshot + command protocol, so every
//! mutation class — committed, unchanged, accepted, rejected, denied,
//! unavailable, ambiguous transport — is exercised against the production
//! code path. Every test also asserts the wire shape the CLI submitted:
//! exactly one snapshot read, then exactly one `workbench.command` carrying
//! the snapshot's Site identity and revision under a fresh request identity.
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
    WorkbenchSnapshotResult, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, parse_snapshot,
    read_workbench_request, write_workbench_response,
};
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SCOPE_PREFIX: &str = "orchbee-native-";

// ---- staged cohort --------------------------------------------------------

/// One cohort directory per test process: copies of the four build artifacts
/// plus a `cohort.json` manifest, so the spawned CLI passes the real
/// `NativeCohort::current` verification.
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
                let target = directory.join(name);
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

// ---- Site scope staging ---------------------------------------------------

fn base_dir(tag: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "obcli-mutation-base-{}-{tag}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).expect("create runtime base");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("chmod base");
    directory
}

fn scope_dir(base: &Path) -> PathBuf {
    base.join(format!("{SCOPE_PREFIX}{}", nix::unistd::geteuid()))
}

/// Create the existing-only Site scope directory the rendezvous validator
/// expects (mode 0700, owned by the caller).
fn stage_scope(base: &Path) -> PathBuf {
    let scope = scope_dir(base);
    fs::create_dir(&scope).expect("create scope directory");
    fs::set_permissions(&scope, fs::Permissions::from_mode(0o700)).expect("chmod scope");
    scope
}

// ---- scripted peer --------------------------------------------------------

/// Bind `client.sock` inside the scope and run `script` on the first admitted
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
    let socket = scope.join("client.sock");
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
    match read_message(&mut &*stream) {
        Ok(LocalClientMessage::ClientHello(_)) => {}
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

/// Wall-clock backstop on every spawned CLI child. A healthy run retires on
/// the CLI's own 5s network budget; this bound only guards the suite against
/// a child that never exits at all, leaving ample slack for a loaded machine.
const RUN_WALL_CLOCK_BACKSTOP: Duration = Duration::from_secs(45);

fn run(base: &Path, arguments: &[&str]) -> Output {
    let mut command = Command::new(cohort().join("orchbee"));
    command
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    bounded_output(&mut command, RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("{timeout}"))
}

/// The spawned child never exited inside `budget` and had to be killed. The
/// reaped status and whatever was drained stay attached so the timeout is a
/// diagnosable failure, never a silent pass.
#[derive(Debug)]
struct BoundedChildTimeout {
    budget: Duration,
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl std::fmt::Display for BoundedChildTimeout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cli child exceeded the {:?} wall-clock backstop; killed and reaped \
             ({}) with {}B stdout / {}B stderr captured",
            self.budget,
            self.status,
            self.stdout.len(),
            self.stderr.len()
        )
    }
}

/// `Command::output` with a deadline. Each pipe drains on its own thread so a
/// child writing more than the pipe buffer cannot deadlock the wait; on expiry
/// only the spawned child is killed (`Child::kill` targets its exact pid) and
/// then reaped via `wait`. A child that exited on its own in the kill race
/// still returns its real `Output`.
fn bounded_output(command: &mut Command, budget: Duration) -> Result<Output, BoundedChildTimeout> {
    let mut child = command.spawn().expect("spawn cli child");
    let mut stdout_pipe = child.stdout.take().expect("stdout piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr piped");
    let stdout_drain = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut bytes);
        bytes
    });
    let stderr_drain = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut bytes);
        bytes
    });
    let deadline = Instant::now() + budget;
    let observed = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => break None,
            Err(error) => panic!("cli child wait failed: {error}"),
        }
    };
    let status = match observed {
        Some(status) => status,
        None => {
            match child.kill() {
                Ok(()) => {}
                // Exited between the last poll and the kill: nothing to
                // signal, `wait` reaps the real exit status below.
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => panic!("cli child kill failed: {error}"),
            }
            child.wait().expect("reap cli child")
        }
    };
    let stdout = stdout_drain.join().expect("stdout drain joined");
    let stderr = stderr_drain.join().expect("stderr drain joined");
    if observed.is_none() && status.signal().is_some() {
        return Err(BoundedChildTimeout {
            budget,
            status,
            stdout,
            stderr,
        });
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
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
  "siteId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "navigation": {
    "projects": [
      {
        "projectId": "00000000000000000000000000000010",
        "label": "web",
        "directories": [],
        "workspaces": [
          {
            "workspaceId": "00000000000000000000000000000011",
            "label": "main",
            "primaryDirectoryId": null,
            "terminalContextId": "00000000000000000000000000000020"
          }
        ]
      }
    ],
    "standaloneTerminalContextId": "00000000000000000000000000000001"
  },
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

const SITE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STANDALONE_CONTEXT: &str = "00000000000000000000000000000001";
const PROJECT: &str = "00000000000000000000000000000010";
const WORKSPACE: &str = "00000000000000000000000000000011";
const CONTEXT: &str = "00000000000000000000000000000020";
const OPS_TAB: &str = "00000000000000000000000000000030";
const LOGS_TAB: &str = "00000000000000000000000000000040";
const LEFT_PANEL: &str = "00000000000000000000000000000043";
const RIGHT_PANEL: &str = "00000000000000000000000000000046";

/// Assert the one-command contract the wire just proved: Site identity and
/// expected revision come from the snapshot, the request identity is fresh
/// (never the snapshot read's), and the action is the expected one.
fn assert_command_base(command: &WorkbenchCommand, snapshot_request: &str) {
    assert_eq!(
        command.site_id.as_str(),
        SITE,
        "Site identity from snapshot"
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
        assert_eq!(project_id.as_str(), PROJECT, "project label resolved");
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
    assert_eq!(document["source"]["siteId"], SITE);
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
            project_id,
            workspace_id,
            label,
        } = &command.action
        else {
            panic!("expected workspace.rename, got {:?}", command.action)
        };
        assert_eq!(project_id.as_str(), PROJECT);
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
    assert_eq!(document["result"]["projectId"], PROJECT);
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
            "--standalone",
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
        &[
            "tab",
            "move",
            "logs",
            "--direction",
            "last",
            "--format=json",
        ],
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
            "--target-panel",
            RIGHT_PANEL,
            "--direction",
            "below",
            "--format=json",
        ],
    );
    assert_eq!(moved.status.code(), Some(0));
    let document = envelope(&moved);
    assert_eq!(document["result"]["action"], "panel.move");
    assert_eq!(document["result"]["outcome"], "committed");
    assert_eq!(document["result"]["direction"], "below");
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
        &["tab", "move", "ops", "--direction", "next", "--format=json"],
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
            "--target-panel",
            RIGHT_PANEL,
            "--direction",
            "above",
            "--format=json",
        ],
    );
    assert_eq!(panel_moved.status.code(), Some(0));
    let document = envelope(&panel_moved);
    assert_eq!(document["result"]["action"], "panel.move");
    assert_eq!(document["result"]["outcome"], "unchanged");
    assert_eq!(document["result"]["revision"], "7");
    assert_eq!(document["result"]["direction"], "above");
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
    // No client.sock at all: rendezvous finds the scope but no listener, so
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
    let socket = scope.join("client.sock");
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
        match read_message(&mut &stream) {
            Ok(LocalClientMessage::ClientHello(_)) => {}
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
    command
        .args([
            "-c",
            "head -c 2000000 /dev/zero | tr '\\0' o; \
             head -c 2000000 /dev/zero | tr '\\0' e >&2; exit 3",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = bounded_output(&mut command, Duration::from_secs(30))
        .expect("large-output child completes inside the backstop");
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
    // exactly the spawned child — no grandchildren, nothing global.
    let mut command = Command::new("sh");
    command
        .args(["-c", "exec sleep 60"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = Instant::now();
    let timeout = bounded_output(&mut command, Duration::from_secs(2))
        .expect_err("a non-exiting child must surface as a timeout, not pass");
    assert_eq!(timeout.status.signal(), Some(9), "reaped as SIGKILL");
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the short test budget fired, not the 45s backstop"
    );
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
    let socket = scope.join("client.sock");
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
        &["tab", "move", "ops", "--direction", "next", "--format=json"],
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
        &["tab", "move", "ops", "--direction", "next", "--format=json"],
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
  "siteId": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "navigation": {
    "projects": [],
    "standaloneTerminalContextId": "00000000000000000000000000000001"
  },
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
        &["tab", "create", "--workspace", "w", "--label", "t"],
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
