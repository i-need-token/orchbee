//! Process-level query evidence: the real `orchbee` binary runs against a
//! scripted local peer bound at the Site's client socket. The peer speaks the
//! genuine OBLC admission and OBWP snapshot protocol, so every admitted
//! outcome class — success, denied, unavailable, malformed, correlation
//! mismatch, timeout, disconnect, capacity, incompatibility — is exercised
//! against the production code path with no Site mutation.
#![cfg(target_os = "linux")]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{OnceLock, mpsc},
    thread,
    time::{Duration, Instant},
};

use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, MAXIMUM_READ_ROWS_BYTES, ServerMessage, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerRequest, TerminalViewerResult, ViewerId,
    ViewerRejectedReason, ViewerRowsRejectedReason, ViewerSnapshotReason, ViewerUnavailableReason,
    read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{
    LocalClientMessage, RejectionReason, read_message, write_message,
};
use orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, MessageRole, PayloadEncoding,
    ProjectionBlockReason, ProjectionBlocked, ProjectionDemandControl, decode_terminal_text_frame,
    decode_terminal_text_row_window, encode_projection_demand_control, encode_transport_envelope,
    inspect_terminal_frame_route,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, WorkbenchSnapshotUnavailableReason, parse_snapshot,
    read_workbench_request, write_workbench_response,
};
use sha2::{Digest, Sha256};

mod support;

type TestResult = Result<(), Box<dyn std::error::Error>>;

// ---- staged cohort --------------------------------------------------------

/// One cohort directory per test process: copies of the four build artifacts
/// plus a `cohort.json` manifest, so the spawned CLI passes the real
/// `NativeCohort::current` verification.
fn cohort() -> &'static Path {
    static COHORT: OnceLock<PathBuf> = OnceLock::new();
    COHORT
        .get_or_init(|| {
            let directory = std::env::temp_dir().join(format!(
                "obcli-query-cohort-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            fs::create_dir(&directory).expect("create staged cohort directory");
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .expect("chmod staged cohort directory");
            // The queried binary must be the real artifact; the other three
            // artifacts are never spawned under a scripted peer, so small
            // stand-ins keep the 5 s debug-build cohort hashing budget safe.
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
            // NativeCohort rejects group/other-writable manifests; pin the
            // permissions independently of the inherited process umask.
            fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600))
                .expect("chmod cohort manifest");
            directory
        })
        .as_path()
}

// ---- Site scope staging ---------------------------------------------------

/// Thin wrapper over the shared minter: a long TMPDIR must not push the
/// rendezvous socket path past `sun_path`, so names stay short and exclusive.
fn base_dir(tag: &str) -> PathBuf {
    support::runtime_base(tag)
}

fn scope_dir(base: &Path) -> PathBuf {
    support::scope_dir(base)
}

/// Create the existing-only Site scope directory the rendezvous validator
/// expects (mode 0700, owned by the caller).
fn stage_scope(base: &Path) -> PathBuf {
    support::stage_scope(base)
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
/// stops answering mid-exchange — fails the test quickly instead of hanging
/// the whole suite on `accept()`/`join()`.
const PEER_IO_DEADLINE: Duration = Duration::from_secs(30);

/// Same as `spawn_peer`, but serves `connections` sequential query sessions:
/// each CLI invocation admits its own Client Session on the shared socket.
fn spawn_peer_n(
    scope: &Path,
    connections: usize,
    script: impl Fn(UnixStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    // The rendezvous validator admits only owner-private sockets.
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("peer listener nonblocking");
    thread::spawn(move || {
        for _ in 0..connections {
            let stream = accept_bounded(&listener);
            // Accepted streams are blocking; per-call timeouts cap the script's
            // own reads and writes so `join` stays bounded as well.
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

/// Accept the next connection within `PEER_IO_DEADLINE`; a CLI that never
/// arrives is a test failure, not an infinite block.
fn accept_bounded(listener: &UnixListener) -> UnixStream {
    let deadline = Instant::now() + PEER_IO_DEADLINE;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "peer accept timed out waiting for the CLI to connect"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("peer accept: {error}"),
        }
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

/// Read the snapshot.read request and hand back its request identity.
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

/// Every spawned CLI child runs under the shared 45s wall-clock backstop in
/// `tests/support` — the same guard `cli_mutation` uses.
fn run(base: &Path, arguments: &[&str]) -> Output {
    support::run(&cohort().join("orchbee"), base, arguments)
}

/// `run` that keeps the `Child` handle so a test can signal this process —
/// and only this process — while an exchange is still open. Stdio and the
/// deterministic locale environment match `support::run` exactly; the
/// caller finishes through `support::bounded_wait_with_output`, keeping the
/// shared wall-clock backstop, drain discipline, and capture budget.
fn spawn_cli(base: &Path, arguments: &[&str]) -> Child {
    Command::new(cohort().join("orchbee"))
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cli child")
}

fn failure_kind(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("failure envelope is JSON")
}

/// A canonical typed id for schema fixtures.
fn id(n: u8) -> String {
    format!("{n:032x}")
}

// ---- scripted outcome matrix ----------------------------------------------

#[test]
fn missing_scope_directory_is_service_unavailable() -> TestResult {
    let base = base_dir("absent");
    let output = run(&base, &["status", "--format", "json"]);
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        failure_kind(&output)["failure"]["kind"],
        "service-unavailable"
    );
    assert_eq!(
        failure_kind(&output)["failure"]["details"]["stage"],
        "discovery"
    );
    assert!(
        !scope_dir(&base).exists(),
        "query must not create the scope"
    );
    Ok(())
}

#[test]
fn scope_without_client_socket_is_service_unavailable() -> TestResult {
    let base = base_dir("nosock");
    let scope = stage_scope(&base);
    let output = run(&base, &["status", "--format", "json"]);
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        failure_kind(&output)["failure"]["kind"],
        "service-unavailable"
    );
    assert_eq!(
        scope.read_dir()?.count(),
        0,
        "query must leave the Site scope untouched"
    );
    Ok(())
}

#[test]
fn denied_snapshot_maps_to_exit_three() -> TestResult {
    let base = base_dir("denied");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(&stream, request_id, WorkbenchSnapshotOutcome::Denied {});
    });
    let output = run(&base, &["status", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "denied");
    Ok(())
}

#[test]
fn unavailable_snapshot_maps_to_exit_four() -> TestResult {
    let base = base_dir("unavailable");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        );
    });
    let output = run(&base, &["terminal", "list", "--format=json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        failure_kind(&output)["failure"]["kind"],
        "service-unavailable"
    );
    assert_eq!(
        failure_kind(&output)["failure"]["details"]["stage"],
        "snapshot"
    );
    Ok(())
}

#[test]
fn malformed_response_maps_to_protocol_invalid() -> TestResult {
    let base = base_dir("malformed");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let _ = read_snapshot_request(&stream);
        use std::io::Write;
        (&mut &stream)
            .write_all(b"NOT-A-WORKBENCH-FRAME")
            .expect("peer writes garbage");
    });
    let output = run(&base, &["status", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "protocol-invalid");
    Ok(())
}

#[test]
fn correlation_mismatch_maps_to_protocol_invalid() -> TestResult {
    let base = base_dir("correlation");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let _ = read_snapshot_request(&stream);
        let wrong = orchbee_workbench_protocol::WorkbenchRequestId::parse(
            "0000000000000000000000000000000f",
        )
        .expect("valid request id");
        snapshot_response(&stream, wrong, WorkbenchSnapshotOutcome::Denied {});
    });
    let output = run(&base, &["status", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "protocol-invalid");
    assert_eq!(
        failure_kind(&output)["failure"]["details"]["reason"],
        "correlation-mismatch"
    );
    Ok(())
}

#[test]
fn slow_peer_hits_the_shared_deadline() -> TestResult {
    let base = base_dir("slow");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let _ = read_snapshot_request(&stream);
        // The CLI shares one monotonic 5-second budget; outlast it.
        thread::sleep(Duration::from_secs(30));
    });
    let started = Instant::now();
    let output = run(&base, &["status", "--format", "json"]);
    // Measure before joining the peer: the assertion is about the child
    // deadline, not the peer's own hold time.
    let elapsed = started.elapsed();
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "transport-loss");
    assert!(
        elapsed < Duration::from_secs(25),
        "query must not wait for the sleeping peer (elapsed {elapsed:?})"
    );
    peer.join().expect("peer finished");
    Ok(())
}

#[test]
fn disconnect_after_admission_is_transport_loss() -> TestResult {
    let base = base_dir("disconnect");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        drop(stream);
    });
    let output = run(&base, &["status", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "transport-loss");
    Ok(())
}

#[test]
fn capacity_rejection_maps_to_exit_four() -> TestResult {
    let base = base_dir("capacity");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        let _ = read_message(&mut &stream);
        write_message(
            &mut &stream,
            &LocalClientMessage::Rejected {
                reason: RejectionReason::CapacityExceeded,
            },
        )
        .expect("peer rejects");
    });
    let output = run(&base, &["status", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "session-capacity");
    Ok(())
}

#[test]
fn cohort_rejection_maps_to_exit_six() -> TestResult {
    let base = base_dir("cohort-reject");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        let _ = read_message(&mut &stream);
        write_message(
            &mut &stream,
            &LocalClientMessage::Rejected {
                reason: RejectionReason::IncompatibleCohort,
            },
        )
        .expect("peer rejects");
    });
    let output = run(&base, &["status", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(6));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "incompatible");
    Ok(())
}

// ---- scripted success surface ---------------------------------------------

/// One standalone context with two tabs sharing the label "dup" plus one
/// project ("proj") whose workspace ("ws") owns a second context holding a
/// split-root "ws-main" tab — every result shape emits at least one non-empty
/// row from this fixture, including the workspace-context variants.
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
        "label": "proj",
        "directories": [
          {
            "directoryId": "00000000000000000000000000000011",
            "workerId": "6f7263686265652d6c6f63616c2d7631",
            "path": "/repo"
          }
        ],
        "workspaces": [
          {
            "workspaceId": "00000000000000000000000000000012",
            "label": "ws",
            "primaryDirectoryId": "00000000000000000000000000000011",
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
              "binding": {
                "terminalId": "00000000000000000000000000000001",
                "workerId": "6f7263686265652d6c6f63616c2d7631"
              }
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
    },
    {
      "terminalContextId": "00000000000000000000000000000020",
      "groups": [],
      "tabs": [
        {
          "tabId": "00000000000000000000000000000021",
          "label": "ws-main",
          "pinned": false,
          "groupId": null,
          "root": {
            "kind": "split",
            "splitId": "00000000000000000000000000000022",
            "orientation": "horizontal",
            "children": [
              {
                "slotId": "00000000000000000000000000000023",
                "node": {
                  "kind": "panel",
                  "panelId": "00000000000000000000000000000024",
                  "primaryPresentation": {
                    "presentationId": "00000000000000000000000000000025",
                    "label": "left",
                    "binding": null
                  },
                  "childPresentations": []
                }
              },
              {
                "slotId": "00000000000000000000000000000026",
                "node": {
                  "kind": "panel",
                  "panelId": "00000000000000000000000000000027",
                  "primaryPresentation": {
                    "presentationId": "00000000000000000000000000000028",
                    "label": "right",
                    "binding": {
                      "terminalId": "00000000000000000000000000000030",
                      "workerId": "6f7263686265652d6c6f63616c2d7631"
                    }
                  },
                  "childPresentations": [
                    {
                      "presentationId": "00000000000000000000000000000029",
                      "label": "aux",
                      "binding": null
                    }
                  ]
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

#[test]
fn available_snapshot_projects_every_resource() -> TestResult {
    let base = base_dir("available");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 2, |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
    });

    // status
    let output = run(&base, &["status"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    assert!(text.contains("revision 7"), "{text}");
    assert!(text.contains("bound terminals 2"), "{text}");

    // both duplicated labels are listed
    let output = run(&base, &["tab", "list"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    assert_eq!(text.matches("dup").count(), 2, "{text}");

    peer.join().expect("peer finished");
    Ok(())
}

#[test]
fn duplicate_label_is_ambiguous_not_arbitrary() -> TestResult {
    let base = base_dir("ambiguous");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
    });
    let output = run(&base, &["tab", "get", "dup", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "ambiguous");
    Ok(())
}

/// The whole snapshot-derived surface over the extended fixture: every
/// emitted document validates against its own embedded schema and carries
/// real rows — non-empty project/workspace, workspace-context tab, split
/// panel structure — not just shapes the empty fixture happened to cover.
#[test]
fn snapshot_derived_documents_validate_against_schemas() -> TestResult {
    let base = base_dir("derived-schemas");
    let scope = stage_scope(&base);
    // Each query admits one fresh Client Session on the shared scripted
    // socket; `api schema` lookups are local and consume no connection.
    let queries: &[(&[&str], &str)] = &[
        (
            &["project", "list", "--format", "json"],
            "orchbee.project.list",
        ),
        (
            &["project", "get", "proj", "--format", "json"],
            "orchbee.project.get",
        ),
        (
            &["workspace", "list", "--format", "json"],
            "orchbee.workspace.list",
        ),
        (
            &["workspace", "get", "ws", "--format", "json"],
            "orchbee.workspace.get",
        ),
        (&["tab", "list", "--format", "json"], "orchbee.tab.list"),
        (
            &["tab", "list", "--workspace", "ws", "--format", "json"],
            "orchbee.tab.list",
        ),
        (
            &["tab", "get", "ws-main", "--format", "json"],
            "orchbee.tab.get",
        ),
        (&["panel", "list", "--format", "json"], "orchbee.panel.list"),
        (
            &["panel", "list", "--tab", "ws-main", "--format", "json"],
            "orchbee.panel.list",
        ),
        (
            &[
                "panel",
                "get",
                "00000000000000000000000000000024",
                "--format",
                "json",
            ],
            "orchbee.panel.get",
        ),
        (
            &[
                "panel",
                "get",
                "00000000000000000000000000000027",
                "--format",
                "json",
            ],
            "orchbee.panel.get",
        ),
        (
            &["terminal", "list", "--format", "json"],
            "orchbee.terminal.list",
        ),
        (
            &[
                "terminal",
                "get",
                "00000000000000000000000000000030",
                "--format",
                "json",
            ],
            "orchbee.terminal.get",
        ),
        (&["status", "--format", "json"], "orchbee.status"),
        (&["api", "snapshot"], "orchbee.api.snapshot"),
    ];
    let peer = spawn_peer_n(&scope, queries.len(), |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
    });

    let mut documents = Vec::new();
    for (arguments, schema_id) in queries {
        let output = run(&base, arguments);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            document["schema"],
            serde_json::json!(schema_id),
            "{arguments:?}"
        );
        let schema = compiled_schema(&base, schema_id);
        assert!(
            schema.is_valid(&document),
            "{schema_id} rejected {document}"
        );
        documents.push(document);
    }
    peer.join().expect("peer finished");

    // Content witnesses: every surface produced real fixture rows, not just a
    // valid envelope.
    let projects = documents[0]["result"]["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["projectId"], id(0x10));
    assert_eq!(projects[0]["label"], "proj");
    assert_eq!(projects[0]["directoryCount"], 1);
    assert_eq!(projects[0]["workspaceCount"], 1);

    let project = &documents[1]["result"]["project"];
    assert_eq!(project["projectId"], id(0x10));
    assert_eq!(
        project["directories"],
        serde_json::json!([{"directoryId": id(0x11), "workerId": "6f7263686265652d6c6f63616c2d7631", "path": "/repo"}])
    );
    assert_eq!(
        project["workspaces"],
        serde_json::json!([{"workspaceId": id(0x12), "label": "ws"}])
    );

    let workspaces = documents[2]["result"]["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0]["workspaceId"], id(0x12));
    assert_eq!(workspaces[0]["projectId"], id(0x10));
    assert_eq!(workspaces[0]["primaryDirectoryId"], id(0x11));
    assert_eq!(workspaces[0]["terminalContextId"], id(0x20));

    let workspace = &documents[3]["result"]["workspace"];
    assert_eq!(workspace["workspaceId"], id(0x12));
    assert_eq!(
        workspace["tabs"],
        serde_json::json!([{"tabId": id(0x21), "label": "ws-main"}])
    );

    // Unfiltered tab list: the workspace-context row carries the tightened
    // context shape ({kind:"workspace", workspaceId}) while standalone rows
    // never carry workspaceId.
    let tabs = documents[4]["result"]["tabs"].as_array().unwrap();
    assert_eq!(tabs.len(), 3);
    let ws_row = tabs
        .iter()
        .find(|row| row["tabId"] == id(0x21))
        .expect("workspace tab row");
    assert_eq!(
        ws_row["context"],
        serde_json::json!({
            "kind": "workspace",
            "terminalContextId": id(0x20),
            "workspaceId": id(0x12),
        })
    );
    for row in tabs.iter().filter(|row| row["tabId"] != id(0x21)) {
        assert_eq!(row["context"]["kind"], "standalone", "{row}");
        assert!(
            row["context"].get("workspaceId").is_none(),
            "standalone context carries no workspaceId: {row}"
        );
    }

    // `--workspace` by label resolves to exactly the workspace context's tab.
    let filtered = documents[5]["result"]["tabs"].as_array().unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["tabId"], id(0x21));

    // `tab get` re-encodes the split PanelNode subtree without semantic loss.
    let root = &documents[6]["result"]["tab"]["root"];
    assert_eq!(root["kind"], "split");
    assert_eq!(root["splitId"], id(0x22));
    assert_eq!(root["children"].as_array().unwrap().len(), 2);

    // Panels flatten across contexts; the filter keeps only the resolved tab.
    let panels = documents[7]["result"]["panels"].as_array().unwrap();
    assert_eq!(panels.len(), 4);
    let scoped = documents[8]["result"]["panels"].as_array().unwrap();
    assert_eq!(
        scoped
            .iter()
            .map(|row| row["panelId"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        vec![id(0x24), id(0x27)],
    );
    assert!(scoped.iter().all(|row| row["tabId"] == id(0x21)));

    // `panel get` walks the split: slotPath is the slot ids root→leaf and the
    // context pins the owning workspace.
    let left = &documents[9]["result"]["panel"];
    assert_eq!(left["panelId"], id(0x24));
    assert_eq!(left["slotPath"], serde_json::json!([id(0x23)]));
    assert_eq!(
        left["context"],
        serde_json::json!({
            "kind": "workspace",
            "terminalContextId": id(0x20),
            "workspaceId": id(0x12),
        })
    );
    assert_eq!(left["primaryPresentation"]["presentationId"], id(0x25));
    assert_eq!(
        left["primaryPresentation"]["binding"],
        serde_json::Value::Null
    );
    assert_eq!(left["childPresentations"], serde_json::json!([]));
    let right = &documents[10]["result"]["panel"];
    assert_eq!(right["panelId"], id(0x27));
    assert_eq!(right["slotPath"], serde_json::json!([id(0x26)]));
    assert_eq!(
        right["primaryPresentation"]["binding"]["terminalId"],
        id(0x30)
    );
    assert_eq!(
        right["childPresentations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["presentationId"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        vec![id(0x29)],
    );

    // Terminals: both bound ids are listed; `terminal get` resolves its
    // binding path through the split.
    let terminals = documents[11]["result"]["terminals"].as_array().unwrap();
    assert_eq!(terminals.len(), 2);
    let terminal = &documents[12]["result"]["terminal"];
    assert_eq!(terminal["terminalId"], id(0x30));
    assert_eq!(terminal["workerId"], "6f7263686265652d6c6f63616c2d7631");
    assert_eq!(
        terminal["boundTo"],
        serde_json::json!({
            "presentationId": id(0x28),
            "panelId": id(0x27),
            "tabId": id(0x21),
            "terminalContextId": id(0x20),
        })
    );

    // `status` reports the same snapshot's real counts; `api snapshot`
    // re-encodes the document itself.
    let status = &documents[13]["result"];
    assert_eq!(status["site"]["siteId"], "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(status["site"]["revision"], "7");
    assert_eq!(
        status["counts"],
        serde_json::json!({
            "projects": 1,
            "workspaces": 1,
            "tabs": 3,
            "panels": 4,
            "boundTerminals": 2,
        })
    );
    let snapshot = &documents[14]["result"]["snapshot"];
    assert_eq!(snapshot["kind"], "workbench.snapshot");
    assert_eq!(snapshot["revision"], "7");
    assert_eq!(snapshot["terminalContexts"].as_array().unwrap().len(), 2);

    // Negative mutations prove the two new schema assertions are not vacuous.
    let status_schema = compiled_schema(&base, "orchbee.status");
    let mut unreachable_site = documents[13].clone();
    unreachable_site["result"]["site"]["reachable"] = serde_json::json!(false);
    assert!(!status_schema.is_valid(&unreachable_site));
    let mut extra_count = documents[13].clone();
    extra_count["result"]["counts"]["bogus"] = serde_json::json!(0);
    assert!(!status_schema.is_valid(&extra_count));
    let snapshot_schema = compiled_schema(&base, "orchbee.api.snapshot");
    let mut bad_kind = documents[14].clone();
    bad_kind["result"]["snapshot"]["kind"] = serde_json::json!("bogus");
    assert!(!snapshot_schema.is_valid(&bad_kind));
    let mut bad_revision = documents[14].clone();
    bad_revision["result"]["snapshot"]["revision"] = serde_json::json!("r7");
    assert!(!snapshot_schema.is_valid(&bad_revision));
    Ok(())
}

// ---- local-only surface (no Site required) ---------------------------------

#[test]
fn api_schema_is_local_and_embedded() -> TestResult {
    let base = base_dir("schema");
    // No scope at all: schema answers must not touch the Site.
    let output = run(
        &base,
        &["api", "schema", "orchbee.status", "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["schema"], "orchbee.api.schema");
    assert_eq!(
        document["source"]["kind"], "embedded",
        "schema documents come from the embedded registry"
    );
    Ok(())
}

#[test]
fn unknown_schema_id_is_not_found() -> TestResult {
    let base = base_dir("schema-miss");
    let output = run(
        &base,
        &["api", "schema", "orchbee.nope", "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "not-found");
    Ok(())
}

#[test]
fn usage_failure_with_json_selection_emits_envelope() -> TestResult {
    let base = base_dir("usage");
    let output = run(&base, &["--format=json", "bogus"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "invalid-usage");
    Ok(())
}

#[test]
fn usage_failure_without_selection_stays_silent_on_stdout() -> TestResult {
    let base = base_dir("usage-text");
    let output = run(&base, &["bogus"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "text usage writes no stdout document"
    );
    Ok(())
}

#[test]
fn completion_scripts_are_local_and_parseable() -> TestResult {
    let base = base_dir("completion");
    for shell in ["bash", "zsh", "fish"] {
        let output = run(&base, &["completion", shell]);
        assert_eq!(output.status.code(), Some(0), "{shell}");
        assert!(!output.stdout.is_empty(), "{shell} script is non-empty");
        assert!(output.stderr.is_empty());
    }
    Ok(())
}

#[test]
fn missing_cohort_is_a_check_failure_not_a_crash() -> TestResult {
    // The unadorned build-tree binary has no cohort.json beside it.
    let base = base_dir("nocohort");
    let mut command = Command::new(env!("CARGO_BIN_EXE_orchbee"));
    command
        .args(["status", "--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", &base);
    let output = support::bounded_output(&mut command, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("{timeout}"));
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(failure_kind(&output)["failure"]["kind"], "check-failed");
    assert_eq!(
        failure_kind(&output)["failure"]["details"]["stage"],
        "cohort"
    );
    Ok(())
}

#[test]
fn query_writes_nothing_to_the_scope() -> TestResult {
    let base = base_dir("readonly");
    let scope = stage_scope(&base);
    let peer = spawn_peer_n(&scope, 4, |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
    });
    for arguments in [
        vec!["status"],
        vec!["tab", "list"],
        vec!["terminal", "list"],
        vec!["api", "snapshot"],
    ] {
        let output = run(&base, &arguments);
        assert_eq!(output.status.code(), Some(0), "{arguments:?}");
    }
    peer.join().expect("peer finished");
    let entries: Vec<_> = scope.read_dir()?.collect();
    assert_eq!(
        entries.len(),
        1,
        "only the peer's client.sock may exist afterward"
    );
    Ok(())
}

/// The peer socket being absent at connect time is discovery, not a transport
/// failure — the listen backlog is created at bind, so closing the listener
/// after the CLI already validated still lands on a refused connect.
#[test]
fn refused_connect_is_service_unavailable() -> TestResult {
    let base = base_dir("refused");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    drop(listener);
    // The stale socket file no longer accepts: connect fails immediately.
    let output = run(&base, &["status", "--format", "json"]);
    let code = output.status.code();
    assert!(
        code == Some(4) || code == Some(5),
        "refused connect is unavailable or transport loss, got {code:?}"
    );
    Ok(())
}

// ---- format-envelope ordering (B3) ------------------------------------------
//
// The parser validates the complete argv envelope before the grammar: a
// unique valid `--format json` controls the failure envelope no matter where
// the grammar error sits, while any invalid/duplicate/missing selector — even
// after the error position — forces a silent stdout.

#[test]
fn format_envelope_is_whole_argv() -> TestResult {
    let base = base_dir("envelope");
    // Valid json selection anywhere → typed JSON usage envelope.
    for arguments in [
        vec!["status", "--unknown", "--format", "json"],
        vec!["--bogus", "--format=json"],
        vec!["--format=json", "--", "bogus", "--format=yaml"],
        vec!["workspace", "list", "--workspace", "x", "--format=json"],
    ] {
        let output = run(&base, &arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{arguments:?} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            failure_kind(&output)["failure"]["kind"],
            "invalid-usage",
            "{arguments:?}"
        );
    }
    // Any format defect anywhere → silent stdout.
    for arguments in [
        vec!["status", "--unknown", "--format="],
        vec!["--bogus", "--format", "yaml"],
        vec!["--format", "json", "status", "--format", "text"],
        vec!["status", "extra", "--format"],
        vec!["workspace", "list", "--project", "x", "--format", "yaml"],
    ] {
        let output = run(&base, &arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(
            output.stdout.is_empty(),
            "{arguments:?} must not write a stdout document"
        );
        assert!(
            !output.stderr.is_empty(),
            "{arguments:?} keeps a stderr diagnostic"
        );
    }
    Ok(())
}

// ---- filter scope (B2) -------------------------------------------------------
//
// A supplied filter outside the declared scope is a usage failure, never a
// silently dropped condition. These cases must fail before any Site access.

#[test]
fn out_of_scope_filters_fail_before_runtime_access() -> TestResult {
    // A runtime base that does not even exist proves the rejection precedes
    // discovery: a service-unavailable answer would mean runtime was touched.
    let base = base_dir("filters").join("never-created");
    let cases: &[(&[&str], &str)] = &[
        (&["workspace", "list", "--workspace", "x"], "invalid-usage"),
        (&["workspace", "list", "--tab", "x"], "invalid-usage"),
        (&["workspace", "list", "--standalone"], "invalid-usage"),
        (&["project", "list", "--project", "x"], "invalid-usage"),
        (&["project", "get", "x", "--project", "y"], "invalid-usage"),
        (&["terminal", "list", "--project", "x"], "invalid-usage"),
        (&["terminal", "list", "--standalone"], "invalid-usage"),
        (&["tab", "list", "--project", "x"], "invalid-usage"),
        (&["panel", "list", "--workspace", "x"], "invalid-usage"),
        (&["panel", "list", "--standalone"], "invalid-usage"),
        (
            &["tab", "list", "--workspace", "x", "--standalone"],
            "invalid-usage",
        ),
        (
            &["tab", "list", "--workspace", "x", "--workspace", "y"],
            "invalid-usage",
        ),
        (&["api", "snapshot", "--project", "x"], "invalid-usage"),
        (&["status", "--workspace", "x"], "invalid-usage"),
        (&["config", "check", "--standalone"], "invalid-usage"),
    ];
    for (arguments, kind) in cases {
        let mut argv: Vec<&str> = arguments.to_vec();
        argv.extend(["--format", "json"]);
        let output = run(&base, &argv);
        assert_eq!(output.status.code(), Some(2), "{argv:?}");
        assert_eq!(failure_kind(&output)["failure"]["kind"], *kind, "{argv:?}");
    }
    // The runtime base was never created: rejections happen before discovery.
    assert!(
        !base.exists(),
        "rejected filters must not touch the runtime"
    );
    Ok(())
}

// ---- API failure formats (B1) -------------------------------------------------
//
// `api` success documents are always machine JSON; failures follow the
// caller-selected envelope format instead.

#[test]
fn api_failures_follow_the_caller_format() -> TestResult {
    let base = base_dir("api-fail");
    // Unknown schema id: default/text leaves stdout empty, explicit json
    // emits one typed envelope — both carry a stderr diagnostic.
    let text = run(&base, &["api", "schema", "orchbee.nope"]);
    assert_eq!(text.status.code(), Some(3));
    assert!(text.stdout.is_empty(), "text failure writes no document");
    assert!(!text.stderr.is_empty());

    let json = run(
        &base,
        &["api", "schema", "--format", "json", "orchbee.nope"],
    );
    assert_eq!(json.status.code(), Some(3));
    let envelope = failure_kind(&json);
    assert_eq!(envelope["failure"]["kind"], "not-found");
    assert_eq!(envelope["failure"]["details"]["resource"], "schema");
    assert_eq!(envelope["source"]["kind"], "embedded");
    assert!(!json.stderr.is_empty());

    // `api snapshot` is a Site query: with no Site it fails in the caller's
    // format as well.
    let text = run(&base, &["api", "snapshot"]);
    assert_eq!(text.status.code(), Some(4));
    assert!(text.stdout.is_empty());
    let json = run(&base, &["api", "snapshot", "--format=json"]);
    assert_eq!(json.status.code(), Some(4));
    assert_eq!(
        failure_kind(&json)["failure"]["kind"],
        "service-unavailable"
    );
    Ok(())
}

// ---- schema contrast (R2) -----------------------------------------------------
//
// Every emitted document must validate against the tightened schemas, and
// rejected contract variants — wrong kinds, extra fields, mismatched details
// — must be refused, not just the happy path re-stated.

fn compiled_schema(base: &Path, id: &str) -> jsonschema::Validator {
    let output = run(base, &["api", "schema", id]);
    assert_eq!(output.status.code(), Some(0), "schema {id}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    jsonschema::draft202012::new(&document["result"]["schema"]["document"])
        .expect("schema compiles")
}

#[test]
fn emitted_documents_validate_and_rejected_variants_fail() -> TestResult {
    let base = base_dir("schemas");
    let failure_schema = compiled_schema(&base, "orchbee.cli.failure");
    let config_schema = compiled_schema(&base, "orchbee.config.check");
    let api_schema = compiled_schema(&base, "orchbee.api.schema");

    // Real failure envelopes validate.
    let denied = run(&base, &["project", "get", "x", "--format", "json"]);
    // (no Site → service-unavailable, still a real envelope)
    assert!(
        failure_schema.is_valid(&serde_json::from_slice::<serde_json::Value>(
            &denied.stdout
        )?)
    );

    // Rejected failure variants: wrong kind, unknown field, mismatched or
    // absent details for the kind's pinned shape.
    for variant in [
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"bogus"},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"denied","details":{"unexpected":1}},"source":{"kind":"embedded"},"extra":true}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"not-found","details":{"resource":"bogus","target":"x"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"output-too-large","details":{"limitBytes":4096}},"source":{"kind":"local-runtime"}}),
        // Per-kind pinning: details forbidden on detail-less kinds…
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"denied","details":{}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"invalid-usage","details":{"resource":"tab"}},"source":{"kind":"local-runtime"}}),
        // …required with the right stage domain elsewhere…
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"service-unavailable"},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"service-unavailable","details":{"stage":"socket"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"service-unavailable","details":{"stage":"admission","reason":"unsupported-platform"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"check-failed","details":{"stage":"discovery"}},"source":{"kind":"local-runtime"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"check-failed","details":{"stage":"platform"}},"source":{"kind":"local-runtime"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"check-failed","details":{"stage":"runtime-base","reason":"x"}},"source":{"kind":"local-runtime"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"session-capacity","details":{"stage":"discovery"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"incompatible","details":{"stage":"snapshot"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"protocol-invalid","details":{"stage":"snapshot","reason":"correlation-mismatch","extra":1}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"invalid-identifier","details":{"resource":"schema","target":"x","extra":1}},"source":{"kind":"embedded"}}),
        // …and ambiguity stays in its own domain: resource is the ambiguous
        // kind, candidates are typed ids only, bounded at 8.
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"ambiguous","details":{"resource":"panel","target":"x","candidates":[id(1),id(2)],"totalCount":2}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"ambiguous","details":{"resource":"tab","target":"x","candidates":["label-a","label-b"],"totalCount":2}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"ambiguous","details":{"resource":"tab","target":"x","candidates":[id(1),id(2),id(3),id(4),id(5),id(6),id(7),id(8),id(9)],"totalCount":9}},"source":{"kind":"embedded"}}),
    ] {
        assert!(
            !failure_schema.is_valid(&variant),
            "rejected variant must fail: {variant}"
        );
    }
    // Valid counterparts for each pinned branch validate.
    for variant in [
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"denied"},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"check-failed","details":{"stage":"runtime-base"}},"source":{"kind":"local-runtime"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"check-failed","details":{"stage":"platform","reason":"unsupported-platform"}},"source":{"kind":"local-runtime"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"service-unavailable","details":{"stage":"discovery"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"service-unavailable","details":{"stage":"platform","reason":"unsupported-platform"}},"source":{"kind":"local-runtime"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"session-capacity","details":{"stage":"admission"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"protocol-invalid","details":{"stage":"snapshot","reason":"correlation-mismatch"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"protocol-invalid","details":{"stage":"admission"}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"ambiguous","details":{"resource":"tab","target":"x","candidates":[id(1),id(2)],"totalCount":2}},"source":{"kind":"embedded"}}),
        serde_json::json!({"schema":"orchbee.cli.failure","version":1,"failure":{"kind":"output-too-large","details":{"limitBytes":8388608}},"source":{"kind":"local-runtime"}}),
    ] {
        assert!(
            failure_schema.is_valid(&variant),
            "valid counterpart must pass: {variant}"
        );
    }

    // Real config.check document validates.
    let config = run(&base, &["config", "check", "--format", "json"]);
    let config_document: serde_json::Value = serde_json::from_slice(&config.stdout)?;
    assert!(
        config_schema.is_valid(&config_document),
        "config.check document: {config_document}"
    );

    // Rejected config variants: a sixth check, a bad path fact, an
    // out-of-order check list, reason/exists mismatches, socket-entry shapes
    // outside the four legal combinations, an aggregate status that does not
    // follow its entries, and an `ok` that contradicts the checks.
    let mut extra_check = config_document.clone();
    extra_check["result"]["checks"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"name":"extra","status":"pass","details":{}}));
    assert!(!config_schema.is_valid(&extra_check));
    let mut bad_fact = config_document.clone();
    bad_fact["result"]["checks"][0]["details"] = serde_json::json!({"path": true, "exists": "yes"});
    assert!(!config_schema.is_valid(&bad_fact));
    let mut reordered = config_document.clone();
    reordered["result"]["checks"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(!config_schema.is_valid(&reordered));
    // reason↔exists pinning: io-error must not carry `exists`, absent must
    // carry `exists: false`. `ok` is kept consistent so only the details
    // binding can reject these.
    let mut wrong_exists = config_document.clone();
    wrong_exists["result"]["checks"][0] = serde_json::json!({
        "name": "runtime-base",
        "status": "fail",
        "details": {"path": "/x", "reason": "io-error", "exists": true}
    });
    wrong_exists["result"]["ok"] = serde_json::json!(false);
    assert!(!config_schema.is_valid(&wrong_exists));
    let mut missing_exists = config_document.clone();
    missing_exists["result"]["checks"][0] = serde_json::json!({
        "name": "runtime-base",
        "status": "fail",
        "details": {"path": "/x", "reason": "absent"}
    });
    missing_exists["result"]["ok"] = serde_json::json!(false);
    assert!(!config_schema.is_valid(&missing_exists));
    // The valid counterparts validate — each at its pinned position.
    for (index, check) in [
        (
            0usize,
            serde_json::json!({"name":"runtime-base","status":"fail","details":{"path":"/x","reason":"absent","exists":false}}),
        ),
        (
            0usize,
            serde_json::json!({"name":"runtime-base","status":"fail","details":{"path":"/x","reason":"io-error"}}),
        ),
        (
            0usize,
            serde_json::json!({"name":"runtime-base","status":"fail","details":{"pathBytesHex":"2f78","reason":"unsafe","exists":true}}),
        ),
        (
            1usize,
            serde_json::json!({"name":"runtime-directory","status":"fail","details":{"path":"/x","reason":"unsafe","exists":true}}),
        ),
        (
            1usize,
            serde_json::json!({"name":"runtime-directory","status":"not-applicable","details":{"path":"/x","reason":"absent","exists":false}}),
        ),
    ] {
        let mut document = config_document.clone();
        document["result"]["checks"][index] = check.clone();
        let any_fail = document["result"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["status"] == "fail");
        document["result"]["ok"] = serde_json::json!(!any_fail);
        assert!(
            config_schema.is_valid(&document),
            "valid check variant at {index}: {check}"
        );
    }
    // socketEntry: the four presence combinations plus the address-too-long
    // variant. Both entries and the aggregate status are set together so the
    // aggregate binding stays honest and only the entry shape is under test.
    for (entry, valid) in [
        (
            serde_json::json!({"presence":"present","status":"pass"}),
            true,
        ),
        (
            serde_json::json!({"presence":"present","status":"fail","reason":"unsafe"}),
            true,
        ),
        (
            serde_json::json!({"presence":"absent","status":"not-applicable"}),
            true,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"runtime-unavailable"}),
            true,
        ),
        (
            serde_json::json!({"presence":"absent","status":"fail","reason":"address-too-long","path":"/x","addressBytes":108,"addressCapacityBytes":107}),
            true,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"fail","reason":"address-too-long","pathBytesHex":"2f78","addressBytes":108,"addressCapacityBytes":107}),
            true,
        ),
        (
            serde_json::json!({"presence":"present","status":"pass","reason":"unsafe"}),
            false,
        ),
        (
            serde_json::json!({"presence":"present","status":"fail"}),
            false,
        ),
        (
            serde_json::json!({"presence":"absent","status":"pass"}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable"}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"pass","reason":"runtime-unavailable"}),
            false,
        ),
        // The budget variant pins its own required shape: the path fact, both
        // byte counts, and the closed reason value are all mandatory.
        (
            serde_json::json!({"presence":"absent","status":"fail","reason":"address-too-long","addressBytes":108,"addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"absent","status":"fail","reason":"address-too-long","path":"/x","addressBytes":108}),
            false,
        ),
        (
            serde_json::json!({"presence":"absent","status":"fail","reason":"address-too-long","path":"/x","addressBytes":108,"addressCapacityBytes":107,"pathBytesHex":"2f78"}),
            false,
        ),
        (
            serde_json::json!({"presence":"absent","status":"fail","reason":"unsafe","path":"/x","addressBytes":108,"addressCapacityBytes":107}),
            false,
        ),
        // Budget-entry fields never belong to a presence-inspected socket:
        // `presence-not-inspected` and the byte counts are not socketEntry
        // vocabulary.
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","path":"/x","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
    ] {
        let mut document = config_document.clone();
        document["result"]["checks"][4]["details"]["client"] = entry.clone();
        document["result"]["checks"][4]["details"]["worker"] = entry.clone();
        document["result"]["checks"][4]["status"] = entry["status"].clone();
        let any_fail = document["result"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["status"] == "fail");
        document["result"]["ok"] = serde_json::json!(!any_fail);
        assert_eq!(
            config_schema.is_valid(&document),
            valid,
            "socketEntry {entry}"
        );
    }
    // socketBudgetEntry: `clientPending`/`host` are derived-address entries
    // only — presence is always `unchecked`, a fitting address reports
    // `presence-not-inspected`, and only the budget can fail them. The path
    // fact and both byte counts are always required.
    for (entry, valid) in [
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","path":"/x","addressBytes":10,"addressCapacityBytes":107}),
            true,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","pathBytesHex":"2f78","addressBytes":10,"addressCapacityBytes":107}),
            true,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"fail","reason":"address-too-long","path":"/x","addressBytes":108,"addressCapacityBytes":107}),
            true,
        ),
        // Fake presence: these entries never inspect the endpoint.
        (
            serde_json::json!({"presence":"present","status":"pass","path":"/x","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"absent","status":"not-applicable","reason":"presence-not-inspected","path":"/x","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
        // A fitting budget never produces pass — and a verdict without its
        // reason or measured facts is incomplete.
        (
            serde_json::json!({"presence":"unchecked","status":"pass","reason":"presence-not-inspected","path":"/x","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","path":"/x","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","path":"/x","addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"presence-not-inspected","path":"/x","pathBytesHex":"2f78","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"not-applicable","reason":"io-error","path":"/x","addressBytes":10,"addressCapacityBytes":107}),
            false,
        ),
        (
            serde_json::json!({"presence":"unchecked","status":"fail","reason":"address-too-long","path":"/x","addressBytes":108,"addressCapacityBytes":107,"extra":1}),
            false,
        ),
    ] {
        let mut document = config_document.clone();
        document["result"]["checks"][4]["details"]["clientPending"] = entry.clone();
        document["result"]["checks"][4]["details"]["host"] = entry.clone();
        document["result"]["checks"][4]["status"] = entry["status"].clone();
        let any_fail = document["result"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["status"] == "fail");
        document["result"]["ok"] = serde_json::json!(!any_fail);
        assert_eq!(
            config_schema.is_valid(&document),
            valid,
            "socketBudgetEntry {entry}"
        );
    }
    // Budget entries are required: dropping one leaves the details incomplete.
    let mut missing_budget = config_document.clone();
    missing_budget["result"]["checks"][4]["details"]
        .as_object_mut()
        .unwrap()
        .remove("clientPending");
    assert!(!config_schema.is_valid(&missing_budget));
    // A fitting budget alone never promotes sockets to pass: only a compliant
    // present client/worker can.
    let mut budget_pass = config_document.clone();
    budget_pass["result"]["checks"][4]["status"] = serde_json::json!("pass");
    assert!(!config_schema.is_valid(&budget_pass));
    // A failed budget entry forces the aggregate fail — and `ok` — while a
    // pass verdict over it is rejected.
    let mut budget_fail = config_document.clone();
    budget_fail["result"]["checks"][4]["details"]["host"] = serde_json::json!({
        "presence":"unchecked","status":"fail","reason":"address-too-long",
        "path":"/x","addressBytes":108,"addressCapacityBytes":107
    });
    budget_fail["result"]["checks"][4]["status"] = serde_json::json!("pass");
    assert!(!config_schema.is_valid(&budget_fail));
    budget_fail["result"]["checks"][4]["status"] = serde_json::json!("fail");
    budget_fail["result"]["ok"] = serde_json::json!(false);
    assert!(config_schema.is_valid(&budget_fail));
    // Aggregate binding: a failed entry forces status fail — and `ok` — while
    // keeping status "pass" (or ok true) is rejected; the consistent failed
    // counterpart validates.
    let mut failed_entry = config_document.clone();
    failed_entry["result"]["checks"][4]["details"]["worker"] =
        serde_json::json!({"presence":"present","status":"fail","reason":"io-error"});
    failed_entry["result"]["checks"][4]["status"] = serde_json::json!("pass");
    assert!(!config_schema.is_valid(&failed_entry));
    failed_entry["result"]["checks"][4]["status"] = serde_json::json!("fail");
    failed_entry["result"]["ok"] = serde_json::json!(false);
    assert!(config_schema.is_valid(&failed_entry));
    let mut wrong_ok = config_document.clone();
    wrong_ok["result"]["ok"] = serde_json::json!(
        !config_document["result"]["ok"]
            .as_bool()
            .expect("ok is boolean")
    );
    assert!(!config_schema.is_valid(&wrong_ok));

    // Rejected api.schema variants: unknown id, both result arms, extra key.
    let listed = run(&base, &["api", "schema", "--format", "json"]);
    let listed_document: serde_json::Value = serde_json::from_slice(&listed.stdout)?;
    assert!(api_schema.is_valid(&listed_document));
    let mut both = listed_document.clone();
    both["result"]["schema"] = serde_json::json!({"schemaId":"orchbee.status"});
    assert!(!api_schema.is_valid(&both));
    let mut unknown_id = listed_document.clone();
    unknown_id["result"]["schemas"]["orchbee.bogus"] = serde_json::json!({});
    assert!(!api_schema.is_valid(&unknown_id));
    Ok(())
}

// ---- config path facts (R3) ----------------------------------------------------
//
// `exists` is a filesystem fact: `false` only for an established absence,
// `true` for an observed entry, omitted when inspection could not establish
// either (permission or I/O failure, invalid path).

#[test]
fn config_check_reports_established_path_facts() -> TestResult {
    // Absent base: runtime-base fails with exists:false.
    let base = base_dir("cfg-absent").join("missing");
    let output = run(&base, &["config", "check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let checks = document["result"]["checks"].as_array().unwrap();
    let runtime_base = checks
        .iter()
        .find(|check| check["name"] == "runtime-base")
        .unwrap();
    assert_eq!(runtime_base["status"], "fail");
    assert_eq!(runtime_base["details"]["exists"], false);
    assert_eq!(runtime_base["details"]["reason"], "absent");
    let runtime_directory = checks
        .iter()
        .find(|check| check["name"] == "runtime-directory")
        .unwrap();
    assert_eq!(runtime_directory["status"], "not-applicable");
    assert_eq!(runtime_directory["details"], serde_json::json!({}));

    // Healthy base without a Site scope: runtime-directory is not-applicable
    // with an established absence, not a guess.
    let base = base_dir("cfg-scope");
    let output = run(&base, &["config", "check", "--format", "json"]);
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let checks = document["result"]["checks"].as_array().unwrap();
    let runtime_base = checks
        .iter()
        .find(|check| check["name"] == "runtime-base")
        .unwrap();
    assert_eq!(runtime_base["status"], "pass");
    assert_eq!(runtime_base["details"]["exists"], true);
    let runtime_directory = checks
        .iter()
        .find(|check| check["name"] == "runtime-directory")
        .unwrap();
    assert_eq!(runtime_directory["status"], "not-applicable");
    assert_eq!(runtime_directory["details"]["exists"], false);
    assert_eq!(runtime_directory["details"]["reason"], "absent");
    Ok(())
}

/// The derived Host endpoint under a staged scope — the deepest socket
/// address the Site derives (`<scope>/host/<endpoint-id>.s`).
fn host_endpoint(scope: &Path) -> PathBuf {
    ModeCheckedLocalSiteRendezvous::host_endpoint_path(scope)
}

/// Pad `base` with one extra path component so `measured` — an endpoint path
/// derived under `<base>/<scope>` — reaches `target_len` bytes. Returns `base`
/// unchanged when the derived path already reaches the target.
fn padded_base(base: &Path, measured: &Path, target_len: usize) -> PathBuf {
    let current = measured.as_os_str().as_encoded_bytes().len();
    if current == target_len {
        return base.to_owned();
    }
    let pad = target_len
        .checked_sub(current + 1)
        .unwrap_or_else(|| panic!("base {base:?} already exceeds the {target_len}B socket target"));
    let padded = base.join("x".repeat(pad));
    fs::create_dir(&padded).expect("create padded runtime base");
    fs::set_permissions(&padded, fs::Permissions::from_mode(0o700)).expect("chmod padded base");
    padded
}

#[test]
fn config_check_flags_a_derived_socket_path_that_cannot_fit() -> TestResult {
    let capacity = support::sun_path_limit();

    // Boundary: the deepest derived endpoint — the Host endpoint — exactly at
    // capacity still fits; every entry reports an honest verdict, never a
    // fabricated fault.
    let base = base_dir("sockfit");
    let fitted = padded_base(&base, &host_endpoint(&scope_dir(&base)), capacity);
    stage_scope(&fitted);
    let output = run(&fitted, &["config", "check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["result"]["ok"], serde_json::json!(true));
    let sockets = &document["result"]["checks"][4];
    assert_eq!(sockets["status"], "not-applicable");
    assert_eq!(sockets["details"]["client"]["presence"], "absent");
    for name in ["clientPending", "host"] {
        let entry = &sockets["details"][name];
        assert_eq!(entry["presence"], "unchecked", "{name}: {entry}");
        assert_eq!(entry["status"], "not-applicable", "{name}: {entry}");
        assert_eq!(entry["reason"], "presence-not-inspected", "{name}: {entry}");
        assert_eq!(entry["addressCapacityBytes"], serde_json::json!(capacity));
    }
    assert_eq!(
        sockets["details"]["host"]["addressBytes"],
        serde_json::json!(capacity)
    );

    // One byte over on every derived path with an inspected scope: all four
    // entries fail address-too-long with the measured byte counts — the
    // verdict is about the derived address, independent of presence.
    let base = base_dir("socklong");
    let over = padded_base(&base, &scope_dir(&base).join("client.sock"), capacity + 1);
    stage_scope(&over);
    let output = run(&over, &["config", "check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["result"]["ok"], serde_json::json!(false));
    let sockets = &document["result"]["checks"][4];
    assert_eq!(sockets["status"], "fail");
    for name in ["client", "worker"] {
        let entry = &sockets["details"][name];
        assert_eq!(entry["presence"], "absent", "{name}: {entry}");
        assert_eq!(entry["status"], "fail");
        assert_eq!(entry["reason"], "address-too-long");
        assert_eq!(entry["addressBytes"], serde_json::json!(capacity + 1));
        assert_eq!(entry["addressCapacityBytes"], serde_json::json!(capacity));
        let path = entry["path"].as_str().expect("utf-8 socket path");
        assert!(path.ends_with(&format!("{name}.sock")), "{name}: {path}");
    }
    for (name, derived) in [
        (
            "clientPending",
            ModeCheckedLocalSiteRendezvous::client_pending_socket_path(&scope_dir(&over)),
        ),
        ("host", host_endpoint(&scope_dir(&over))),
    ] {
        let entry = &sockets["details"][name];
        assert_eq!(entry["presence"], "unchecked", "{name}: {entry}");
        assert_eq!(entry["status"], "fail", "{name}: {entry}");
        assert_eq!(entry["reason"], "address-too-long", "{name}: {entry}");
        assert_eq!(
            entry["addressCapacityBytes"],
            serde_json::json!(capacity),
            "{name}: {entry}"
        );
        let path = entry["path"].as_str().expect("utf-8 socket path");
        assert_eq!(Path::new(path), derived, "{name}: {path}");
    }

    // Only the Host endpoint over: inspected client/worker stay honest absent
    // facts and the pending budget still fits — one budget failure fails the
    // aggregate without inventing presence anywhere.
    let base = base_dir("sockhost");
    let over = padded_base(&base, &host_endpoint(&scope_dir(&base)), capacity + 1);
    stage_scope(&over);
    let output = run(&over, &["config", "check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["result"]["ok"], serde_json::json!(false));
    let sockets = &document["result"]["checks"][4];
    assert_eq!(sockets["status"], "fail");
    for name in ["client", "worker"] {
        let entry = &sockets["details"][name];
        assert_eq!(entry["presence"], "absent", "{name}: {entry}");
        assert_eq!(entry["status"], "not-applicable", "{name}: {entry}");
    }
    let pending = &sockets["details"]["clientPending"];
    assert_eq!(pending["presence"], "unchecked");
    assert_eq!(pending["status"], "not-applicable");
    assert_eq!(pending["reason"], "presence-not-inspected");
    let host = &sockets["details"]["host"];
    assert_eq!(host["presence"], "unchecked");
    assert_eq!(host["status"], "fail");
    assert_eq!(host["reason"], "address-too-long");
    assert_eq!(host["addressBytes"], serde_json::json!(capacity + 1));

    // No Site staged at all under an over-budget base: presence stays
    // honestly unchecked while the address verdict is still exact.
    let base = base_dir("socknone");
    let over = padded_base(&base, &scope_dir(&base).join("client.sock"), capacity + 1);
    let output = run(&over, &["config", "check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let sockets = &document["result"]["checks"][4];
    assert_eq!(sockets["status"], "fail");
    for name in ["client", "worker", "clientPending", "host"] {
        let entry = &sockets["details"][name];
        assert_eq!(entry["presence"], "unchecked", "{name}: {entry}");
        assert_eq!(entry["reason"], "address-too-long");
    }

    // A real bound socket inside the budget still passes: live presence is
    // reported without confusing it for the address budget, and the
    // budget-only entries never pretend presence.
    let base = base_dir("socklive");
    let scope = stage_scope(&base);
    let _listener = UnixListener::bind(scope.join("client.sock"))?;
    fs::set_permissions(scope.join("client.sock"), fs::Permissions::from_mode(0o600))?;
    let output = run(&base, &["config", "check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["result"]["ok"], serde_json::json!(true));
    let sockets = &document["result"]["checks"][4];
    assert_eq!(sockets["status"], "pass");
    assert_eq!(sockets["details"]["client"]["status"], "pass");
    for name in ["clientPending", "host"] {
        let entry = &sockets["details"][name];
        assert_eq!(entry["presence"], "unchecked", "{name}: {entry}");
        assert_eq!(entry["status"], "not-applicable", "{name}: {entry}");
        assert_eq!(entry["reason"], "presence-not-inspected", "{name}: {entry}");
    }
    Ok(())
}

// ---- real emit-boundary write failure ------------------------------------------
//
// A controlled writer failure at the real stdout boundary must surface as a
// process failure with a stderr diagnostic, never a truncated silent output.

#[test]
fn stdout_write_failure_is_reported() -> TestResult {
    let base = base_dir("devfull");
    // /dev/full fails every write: the real emit boundary, not an injected
    // earlier stage.
    let dev_full = fs::File::options().write(true).open("/dev/full")?;
    let child = Command::new(cohort().join("orchbee"))
        .args(["api", "schema", "orchbee.status", "--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", &base)
        .env("LANG", "en_US.UTF-8")
        .stdout(std::process::Stdio::from(dev_full))
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let output = support::bounded_wait_with_output(child, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("{timeout}"));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        !output.stderr.is_empty(),
        "a write failure reports a diagnostic"
    );
    Ok(())
}

// ---- generated completion scripts execute in real shells (B4) -------------------
//
// The emitted scripts are the contract surface: run them in real bash and zsh
// (fake compadd harness) against positive and negative contexts. fish is not
// installed on this host, so its script gets structural assertions only.

fn bash_complete(script: &Path, words: &[&str]) -> Vec<String> {
    // words = argv after `orchbee`, last element is the token being completed.
    let harness = format!(
        "source {}\nCOMP_WORDS=(orchbee {})\nCOMP_CWORD={}\n_orchbee\nif ((${{#COMPREPLY[@]}})); then printf '%s\\n' \"${{COMPREPLY[@]}}\"; fi\n",
        script.display(),
        words
            .iter()
            .map(|w| format!("'{}'", w.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" "),
        words.len()
    );
    let mut command = Command::new("bash");
    command.args(["-c", &harness]);
    let output = support::bounded_output(&mut command, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("bash harness: {timeout}"));
    assert_eq!(
        output.status.code(),
        Some(0),
        "bash harness: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    stdout.lines().map(str::to_owned).collect()
}

fn zsh_complete(script: &Path, words: &[&str]) -> Vec<String> {
    // The real completion system filters `compadd` results by the word being
    // completed; the fake harness emulates that prefix filter.
    let harness = format!(
        "compadd() {{ local a; shift; for a in \"$@\"; do [[ $a == ${{words[CURRENT]}}* ]] && print -l -- $a; done }}\ncompdef() {{ : }}\nsource {}\nwords=(orchbee {})\nCURRENT={}\n_orchbee\n",
        script.display(),
        words
            .iter()
            .map(|w| format!("'{}'", w.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" "),
        words.len() + 1
    );
    let mut command = Command::new("zsh");
    command.args(["-f", "-c", &harness]);
    let output = support::bounded_output(&mut command, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("zsh harness: {timeout}"));
    assert_eq!(
        output.status.code(),
        Some(0),
        "zsh harness: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    stdout.lines().map(str::to_owned).collect()
}

/// The same context battery against one engine's completer.
fn assert_completion_shape(complete: impl Fn(&[&str]) -> Vec<String>, engine: &str) {
    let sorted = |candidates: Vec<String>| {
        let mut candidates = candidates;
        candidates.sort();
        candidates
    };
    // Root position: every business word plus root flags and format flags.
    let root = sorted(complete(&[""]));
    for word in [
        "api",
        "completion",
        "config",
        "help",
        "panel",
        "project",
        "status",
        "tab",
        "terminal",
        "tui",
        "workspace",
        "--help",
        "--version",
        "--format",
        "--format=",
    ] {
        assert!(
            root.contains(&word.to_owned()),
            "{engine} root lacks {word}"
        );
    }
    // Prefix filtering is the engine's job; still witness it once.
    assert_eq!(
        sorted(complete(&["w"])),
        vec!["workspace".to_owned()],
        "{engine}"
    );

    // Family verb position: verbs + format flags, nothing else.
    assert_eq!(
        sorted(complete(&["workspace", ""])),
        sorted(
            [
                "accept-directory",
                "create",
                "get",
                "list",
                "rename",
                "--format",
                "--format=",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        ),
        "{engine} workspace verbs"
    );
    // After `workspace list`: format flags + --help + the scoped filter only.
    let after_list = sorted(complete(&["workspace", "list", ""]));
    assert_eq!(
        after_list,
        sorted(
            ["--format", "--format=", "--help", "--project"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} workspace list"
    );
    // A supplied filter consumes its value: no completions inside it.
    assert_eq!(
        complete(&["workspace", "list", "--project", ""]).len(),
        0,
        "{engine} must not invent resource names"
    );
    // A declared filter already supplied is not offered again.
    let again = sorted(complete(&["workspace", "list", "--project", "x", ""]));
    assert!(
        !again.contains(&"--project".to_owned()),
        "{engine} re-offers a consumed filter: {again:?}"
    );
    // tab list mutual exclusion: --standalone consumed → no filters offered.
    let standalone = sorted(complete(&["tab", "list", "--standalone", ""]));
    assert!(
        !standalone.contains(&"--workspace".to_owned())
            && !standalone.contains(&"--standalone".to_owned()),
        "{engine} offers excluded filters: {standalone:?}"
    );
    // get operand position offers no business verbs.
    let operand = sorted(complete(&["workspace", "get", "x", ""]));
    assert!(
        operand.iter().all(|c| c.starts_with("--")),
        "{engine} operand leaks verbs: {operand:?}"
    );
    // Mutation verb flag positions: each command offers exactly its declared
    // options plus the orthogonal format flags and a fresh-path --help.
    assert_eq!(
        sorted(complete(&["workspace", "create", ""])),
        sorted(
            ["--project", "--label", "--format", "--format=", "--help"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} workspace create flags"
    );
    assert_eq!(
        sorted(complete(&["tab", "create", ""])),
        sorted(
            [
                "--workspace",
                "--standalone",
                "--label",
                "--cols",
                "--rows",
                "--cell-width",
                "--cell-height",
                "--format",
                "--format=",
                "--help",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        ),
        "{engine} tab create flags"
    );
    assert_eq!(
        sorted(complete(&["tab", "close", ""])),
        sorted(
            ["--confirm", "--format", "--format=", "--help"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} tab close flags"
    );
    assert_eq!(
        sorted(complete(&["panel", "move", "p", ""])),
        sorted(
            ["--target-panel", "--direction", "--format", "--format=",]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} panel move flags"
    );
    // Direction values are family-scoped enums, not free text.
    assert_eq!(
        sorted(complete(&["tab", "move", "t", "--direction", ""])),
        sorted(
            ["first", "last", "next", "previous"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} tab move direction values"
    );
    assert_eq!(
        sorted(complete(&["panel", "split", "p", "--direction", ""])),
        sorted(
            ["above", "below", "left", "right"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} panel split direction values"
    );
    // Cross-family direction words are dead positions, not fuzzy matches.
    assert_eq!(
        complete(&["tab", "move", "t", "--direction", "above"]).len(),
        0,
        "{engine} offers a panel direction on tab move"
    );
    // Once every required option is consumed only format flags remain.
    assert_eq!(
        sorted(complete(&[
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "l",
            ""
        ])),
        sorted(
            ["--format", "--format="]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} fully supplied create keeps only format flags"
    );
    // Mutation flags on query commands are dead positions.
    assert_eq!(
        complete(&["tab", "list", "--direction", "next", ""]).len(),
        0,
        "{engine} mutation flag on a query"
    );
    assert_eq!(
        complete(&["status", "--confirm", ""]).len(),
        0,
        "{engine} confirm on a query"
    );
    // format value contexts.
    assert_eq!(
        sorted(complete(&["status", "--format", ""])),
        sorted(vec!["json".to_owned(), "text".to_owned()]),
        "{engine} format values"
    );
    assert_eq!(
        sorted(complete(&["status", "--format="])),
        sorted(vec!["--format=json".to_owned(), "--format=text".to_owned()]),
        "{engine} format= values"
    );
    // The `--format=` spelling at root is a format-value context, never a
    // resurrection of root candidates.
    assert_eq!(
        sorted(complete(&["--format="])),
        sorted(vec!["--format=json".to_owned(), "--format=text".to_owned()]),
        "{engine} root --format= context"
    );
    // A consumed `--format` ends the selector: no duplicate `--format`
    // candidate, and `--help` is illegal once any option was consumed.
    assert_eq!(
        complete(&["status", "--format", "json", ""]).len(),
        0,
        "{engine} offers candidates after a consumed selector"
    );
    assert_eq!(
        complete(&["status", "--format=json", ""]).len(),
        0,
        "{engine} offers candidates after a consumed --format="
    );
    // `--help` after a consumed option is usage — never a candidate.
    assert_eq!(
        sorted(complete(&["tab", "list", "--standalone", ""])),
        sorted(
            ["--format", "--format="]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} standalone consumed: only format flags remain"
    );
    // `get` verbs still owe their operand, yet a suffix `--help` is a legal
    // complete-path help (`workspace get --help` describes `workspace get`).
    // All five get paths offer it while no option was consumed.
    for family in ["project", "workspace", "tab", "panel", "terminal"] {
        assert_eq!(
            sorted(complete(&[family, "get", ""])),
            sorted(
                ["--format", "--format=", "--help"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            ),
            "{engine} {family} get operand position"
        );
    }
    // Once any option was consumed — or the operand landed — `--help` is
    // usage again.
    assert_eq!(
        sorted(complete(&["workspace", "get", "--format", "json", ""])),
        Vec::<String>::new(),
        "{engine} get --help suppressed after a consumed option"
    );
    assert_eq!(
        sorted(complete(&["workspace", "get", "x", ""])),
        sorted(
            ["--format", "--format="]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} post-operand position"
    );
    // `api schema` is complete without its id: `--help` is legal there.
    let schema_help = sorted(complete(&["api", "schema", ""]));
    assert!(
        schema_help.contains(&"--help".to_owned()),
        "{engine} api schema lacks --help: {schema_help:?}"
    );
    // help paths.
    let topics = sorted(complete(&["help", ""]));
    for topic in [
        "api",
        "completion",
        "config",
        "panel",
        "project",
        "status",
        "tab",
        "terminal",
        "tui",
        "workspace",
    ] {
        assert!(
            topics.contains(&topic.to_owned()),
            "{engine} help lacks {topic}"
        );
    }
    assert_eq!(
        sorted(complete(&["help", "project", ""])),
        sorted(vec![
            "create".to_owned(),
            "get".to_owned(),
            "list".to_owned()
        ]),
        "{engine} help children"
    );
    assert_eq!(
        complete(&["help", "project", "get", ""]).len(),
        0,
        "{engine} leaf help path must end"
    );
    // completion shell names.
    assert_eq!(
        sorted(complete(&["completion", ""])),
        sorted(
            ["bash", "fish", "zsh", "--help"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} shells"
    );
    // Negative contexts: unknown words and `--` end the grammar.
    assert_eq!(complete(&["bogus", ""]).len(), 0, "{engine} unknown word");
    assert_eq!(
        complete(&["workspace", "bogus", ""]).len(),
        0,
        "{engine} unknown verb"
    );
    assert_eq!(complete(&["--", ""]).len(), 0, "{engine} -- termination");
    assert_eq!(
        complete(&["status", "--", ""]).len(),
        0,
        "{engine} -- after leaf"
    );
    // A failed or terminal parse state must not resurrect `--format=`
    // candidates.
    assert_eq!(
        complete(&["bogus", "--format="]).len(),
        0,
        "{engine} --format= after an invalid command"
    );
    assert_eq!(
        complete(&["tui", "--format="]).len(),
        0,
        "{engine} tui rejects format selectors"
    );
    assert_eq!(
        complete(&["help", "status", "--format="]).len(),
        0,
        "{engine} help rejects options"
    );
    assert_eq!(
        complete(&["completion", "--format="]).len(),
        0,
        "{engine} completion rejects options"
    );
    // A pending filter owns the next token whole: `--format=` typed there is
    // a value, not a selector context.
    assert_eq!(
        complete(&["workspace", "list", "--project", "--format="]).len(),
        0,
        "{engine} --format= inside a filter value"
    );
    // `--` is option termination, never a flag value: the pending filter gets
    // no value and the position is dead.
    assert_eq!(
        complete(&["workspace", "list", "--project", "--", ""]).len(),
        0,
        "{engine} -- is not a filter value"
    );
    assert_eq!(
        complete(&["workspace", "list", "--project", "--"]).len(),
        0,
        "{engine} completing `--` itself yields nothing"
    );
    // Invalid selector values and duplicates are usage, not contexts.
    assert_eq!(
        complete(&["status", "--format", "bogus", ""]).len(),
        0,
        "{engine} invalid format value"
    );
    assert_eq!(
        complete(&["status", "--format", "json", "--format="]).len(),
        0,
        "{engine} duplicate selector"
    );
    // Out-of-scope filters, mutual exclusion, unknown flags, stray operands,
    // and consumed help all kill the position.
    assert_eq!(
        complete(&["workspace", "list", "--workspace", "w", ""]).len(),
        0,
        "{engine} out-of-scope filter"
    );
    assert_eq!(
        complete(&["tab", "list", "--workspace", "w", "--standalone", ""]).len(),
        0,
        "{engine} workspace and standalone are exclusive"
    );
    assert_eq!(
        complete(&["status", "--bogus", ""]).len(),
        0,
        "{engine} unknown flag"
    );
    assert_eq!(
        complete(&["workspace", "list", "x", ""]).len(),
        0,
        "{engine} list takes no operand"
    );
    assert_eq!(
        complete(&["status", "--help", ""]).len(),
        0,
        "{engine} consumed --help is terminal"
    );
    // Doomed consumed prefixes suppress every candidate — including a pending
    // format value, the `--format=` override, and flag offers. Root-scope
    // contradiction: no command accepts both --workspace and --standalone.
    for cur in ["", "--format="] {
        assert_eq!(
            complete(&["--workspace", "x", "--standalone", cur]).len(),
            0,
            "{engine} root contradiction at {cur:?}"
        );
        assert_eq!(
            complete(&["--project", "x", "--tab", "y", cur]).len(),
            0,
            "{engine} disjoint filter scopes at {cur:?}"
        );
    }
    // Known-family contradiction: `--project` leaves `terminal` no legal verb.
    for cur in ["", "--format="] {
        assert_eq!(
            complete(&["--project", "x", "terminal", cur]).len(),
            0,
            "{engine} family contradiction at {cur:?}"
        );
        assert_eq!(
            complete(&["--workspace", "x", "--standalone", "tab", cur]).len(),
            0,
            "{engine} exclusive scopes inside family at {cur:?}"
        );
    }
    // Legal constrained prefixes still complete normally.
    assert_eq!(
        sorted(complete(&["--project", "x", ""])),
        sorted(
            ["workspace", "--format", "--format="]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} valid scoped root prefix"
    );
    assert_eq!(
        sorted(complete(&["--workspace", "x", "tab", ""])),
        sorted(
            ["create", "list", "--format", "--format="]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} valid scoped family prefix"
    );
    assert_eq!(
        sorted(complete(&["--project", "x", "--format="])),
        sorted(
            ["--format=text", "--format=json"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        ),
        "{engine} valid prefix keeps --format= override"
    );
    // A pending format value on a doomed prefix stays dead, too.
    assert_eq!(
        complete(&["--workspace", "x", "--standalone", "--format", ""]).len(),
        0,
        "{engine} pending format value on a dead prefix"
    );
}

#[test]
fn completion_scripts_execute_in_bash_and_zsh() -> TestResult {
    let base = base_dir("completion-exec");
    let bash_script = run(&base, &["completion", "bash"]);
    assert_eq!(bash_script.status.code(), Some(0));
    let zsh_script = run(&base, &["completion", "zsh"]);
    assert_eq!(zsh_script.status.code(), Some(0));

    let dir = base_dir("completion-scripts");
    let bash_path = dir.join("orchbee.bash");
    let zsh_path = dir.join("_orchbee");
    fs::write(&bash_path, &bash_script.stdout)?;
    fs::write(&zsh_path, &zsh_script.stdout)?;

    assert_completion_shape(|words| bash_complete(&bash_path, words), "bash");
    assert_completion_shape(|words| zsh_complete(&zsh_path, words), "zsh");

    // fish is absent on this host: its script still gets structural checks.
    let fish_script = run(&base, &["completion", "fish"]);
    assert_eq!(fish_script.status.code(), Some(0));
    let fish = String::from_utf8(fish_script.stdout)?;
    for needle in [
        "__orchbee_candidates",
        "commandline -opc",
        "--format=text",
        "--format=json",
        "set ended 1",
        "workspace:list",
        "tab:list",
        "panel:list",
        "terminal:read",
    ] {
        assert!(fish.contains(needle), "fish script lacks {needle}");
    }
    Ok(())
}

// ---- terminal read viewer surface -------------------------------------------
//
// `terminal read` keeps the admitted session past the snapshot answer: a
// read-capability viewer attaches to the snapshot-resolved binding, polls
// until one record applies to the projection, detaches, and only then is the
// session dropped for output preparation. These peers speak the real OBCS
// viewer protocol on the same socket, so attach, poll, feedback, detach,
// rejection, and disconnect paths all run against production code.

const OBTF: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin");

const OBTF_ALT: &[u8] = include_bytes!(
    "../../../packages/terminal-protocol/fixtures/ghostty-alternate-surface-v0.0.3.bin"
);

const TERMINAL_ID: &str = "00000000000000000000000000000001";

/// The OBTF snapshot re-wrapped in its transport envelope — the byte payload a
/// `terminal.viewer` `record` outcome carries on the wire.
fn obtf_record() -> Vec<u8> {
    frame_record(OBTF)
}

/// Wrap any terminal-frame fixture in its transport envelope for a `record`
/// outcome; both surface fixtures share the attachment/epoch identities.
fn frame_record(obtf: &[u8]) -> Vec<u8> {
    let route = inspect_terminal_frame_route(obtf).expect("fixture route");
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: if obtf[6] == 1 { 41 } else { 42 },
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: obtf,
    })
    .expect("fixture record")
}

/// A `ProjectionBlocked` control record (kind 49) for the fixture attachment:
/// the host's signal that projection state below `revision` is unusable.
fn blocked_record(revision: u64) -> Vec<u8> {
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    let blocked = encode_projection_demand_control(&ProjectionDemandControl::ProjectionBlocked(
        ProjectionBlocked {
            attachment_id: route.attachment_id,
            reason: ProjectionBlockReason::ProjectionTooLarge,
            revision,
        },
    ))
    .expect("encode blocked");
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: blocked.kind,
        payload_encoding: PayloadEncoding::ControlJson,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: &blocked.payload,
    })
    .expect("blocked record")
}

/// The attach identity the fixture frame carries: the viewer outcome must
/// present the same attachment and epoch or the projection refuses records.
fn attached_identity() -> (ViewerId, HostEpoch) {
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    (
        ViewerId::parse(&route.attachment_id.to_text()).expect("fixture attachment id"),
        HostEpoch::parse(&route.host_epoch.to_text()).expect("fixture host epoch"),
    )
}

fn viewer_request(stream: &UnixStream) -> TerminalViewerRequest {
    match read_client_message(&mut &*stream) {
        Ok(ClientMessage::TerminalViewer(request)) => request,
        other => panic!("peer expected a terminal.viewer request, got {other:?}"),
    }
}

fn viewer_reply(
    stream: &UnixStream,
    request: &TerminalViewerRequest,
    outcome: TerminalViewerOutcome,
) {
    write_server_message(
        &mut &*stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .expect("peer writes viewer result");
}

/// Answer the admission and snapshot stages, then hand the stream to `script`
/// for the viewer stage of a `terminal read` run.
fn read_peer(
    scope: &Path,
    script: impl Fn(&UnixStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    spawn_peer(scope, move |stream| {
        admit(&stream);
        let request_id = read_snapshot_request(&stream);
        snapshot_response(
            &stream,
            request_id,
            WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        );
        script(&stream);
    })
}

/// Assert the request is an `AttachBinding` for the fixture's binding and
/// reply `Attached` with the fixture's own attachment identity.
fn attach_with_fixture(stream: &UnixStream) -> ViewerId {
    let request = viewer_request(stream);
    match request.action() {
        TerminalViewerAction::AttachBinding {
            site_id,
            worker_id,
            terminal_id,
        } => {
            assert_eq!(site_id.as_str(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
            assert_eq!(worker_id.as_str(), "6f7263686265652d6c6f63616c2d7631");
            assert_eq!(terminal_id.as_str(), TERMINAL_ID);
        }
        action => panic!("peer expected attach-binding, got {action:?}"),
    }
    let (attachment, epoch) = attached_identity();
    viewer_reply(
        stream,
        &request,
        TerminalViewerOutcome::Attached {
            attachment_id: attachment.clone(),
            host_epoch: epoch,
        },
    );
    attachment
}

/// Consume the CLI's best-effort `Detach` for `attachment` and answer it.
fn answer_detach(stream: &UnixStream, attachment: &ViewerId) {
    let request = viewer_request(stream);
    match request.action() {
        TerminalViewerAction::Detach { attachment_id } if attachment_id == attachment => {}
        action => panic!("peer expected detach, got {action:?}"),
    }
    viewer_reply(stream, &request, TerminalViewerOutcome::Detached {});
}

/// Consume one request, asserting it is a `Poll` for `attachment`.
fn take_poll(stream: &UnixStream, attachment: &ViewerId) -> TerminalViewerRequest {
    let request = viewer_request(stream);
    match request.action() {
        TerminalViewerAction::Poll { attachment_id } if attachment_id == attachment => {}
        action => panic!("peer expected poll, got {action:?}"),
    }
    request
}

#[test]
fn terminal_read_observes_a_bound_terminal() -> TestResult {
    let base = base_dir("tread");
    let scope = stage_scope(&base);
    let record = obtf_record();
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        // One Empty answer, then the frame record, then the released attach.
        let request = take_poll(stream, &attachment);
        viewer_reply(stream, &request, TerminalViewerOutcome::Empty {});
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        answer_detach(stream, &attachment);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["schema"], "orchbee.terminal.read");
    assert_eq!(document["source"]["kind"], "workbench-snapshot");
    assert_eq!(document["source"]["revision"], "7");
    let route = inspect_terminal_frame_route(OBTF)?;
    let frame = &document["result"]["frame"];
    assert_eq!(document["result"]["terminal"]["terminalId"], TERMINAL_ID);
    assert_eq!(frame["sequence"], route.sequence.get().to_string());
    assert_eq!(frame["generation"], route.generation.get().to_string());
    assert_eq!(frame["revision"], route.revision.to_string());
    let geometry = decode_terminal_text_frame(OBTF)?.geometry;
    assert_eq!(frame["geometry"]["columns"], geometry.columns);
    assert_eq!(frame["geometry"]["rows"], geometry.rows);
    let content = &document["result"]["content"];
    assert_eq!(content["truncated"], false);
    assert_eq!(content["omittedRows"], 0);
    // The fixture grid is known exactly: joined cell text, wide-cell spacer
    // collapse, trailing-padding trim, and the row0↔row1 wrap pair — bit 0 is
    // intoNext on row 0, bit 1 is fromPrevious on row 1.
    let expected_rows: [(u64, &str, bool, bool); 6] = [
        (0, "A中e\u{301}👩🏽\u{200d}💻x", false, true),
        (1, "界", true, false),
        (2, "H", false, false),
        (3, "FBITVKOS", false, false),
        (4, "12345UDE", false, false),
        (5, "^P.", false, false),
    ];
    let emitted = content["rows"].as_array().unwrap();
    assert_eq!(content["rowCount"], emitted.len());
    assert_eq!(emitted.len(), expected_rows.len());
    for (row, (index, text, from_previous, into_next)) in emitted.iter().zip(expected_rows.iter()) {
        assert_eq!(row["index"], *index, "row index");
        assert_eq!(row["text"], *text, "row {index} text");
        assert_eq!(
            row["continuation"]["fromPrevious"], *from_previous,
            "row {index} fromPrevious"
        );
        assert_eq!(
            row["continuation"]["intoNext"], *into_next,
            "row {index} intoNext"
        );
    }
    // The emitted envelope validates against its own advertised schema.
    let schema = compiled_schema(&base, "orchbee.terminal.read");
    assert!(schema.is_valid(&document), "read document: {document}");
    // Nothing but the peer's socket may exist inside the scope afterward.
    assert_eq!(
        scope.read_dir()?.count(),
        1,
        "read must not write the scope"
    );
    Ok(())
}

#[test]
fn terminal_read_text_output_escapes_grid_content() -> TestResult {
    let base = base_dir("tread-text");
    let scope = stage_scope(&base);
    let record = obtf_record();
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        answer_detach(stream, &attachment);
    });
    let output = run(&base, &["terminal", "read", TERMINAL_ID]);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.is_empty(), "grid content is emitted");
    assert!(
        !output.stdout.contains(&0x1b),
        "raw escape bytes never reach stdout"
    );
    Ok(())
}

#[test]
fn terminal_read_unknown_target_is_not_found() -> TestResult {
    let base = base_dir("tread-miss");
    let scope = stage_scope(&base);
    // The peer serves only the snapshot: no binding means no viewer request
    // may ever arrive.
    let peer = read_peer(&scope, |_| {});
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            "ffffffffffffffffffffffffffffffff",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "not-found");
    assert_eq!(failure["failure"]["details"]["resource"], "terminal");
    assert_eq!(
        failure["failure"]["details"]["target"],
        "ffffffffffffffffffffffffffffffff"
    );
    Ok(())
}

#[test]
fn terminal_read_attach_rejected_is_service_unavailable() -> TestResult {
    let base = base_dir("tread-reject");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, |stream| {
        let request = viewer_request(stream);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Rejected {
                reason: ViewerRejectedReason::TerminalUnavailable,
            },
        );
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "service-unavailable");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

#[test]
fn terminal_read_attach_unavailable_is_service_unavailable() -> TestResult {
    let base = base_dir("tread-unavailable");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, |stream| {
        let request = viewer_request(stream);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::ServerBusy,
            },
        );
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "service-unavailable");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

#[test]
fn terminal_read_correlation_mismatch_is_protocol_invalid() -> TestResult {
    let base = base_dir("tread-correlation");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, |stream| {
        let request = viewer_request(stream);
        let wrong =
            orchbee_client_server_protocol::RequestId::parse("0000000000000000000000000000000f")
                .expect("valid request id");
        let (attachment, epoch) = attached_identity();
        write_server_message(
            &mut &*stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                wrong,
                TerminalViewerOutcome::Attached {
                    attachment_id: attachment,
                    host_epoch: epoch,
                },
            )),
        )
        .expect("peer writes mismatched result");
        let _ = request;
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    assert_eq!(
        failure["failure"]["details"]["reason"],
        "correlation-mismatch"
    );
    Ok(())
}

#[test]
fn terminal_read_disconnect_mid_poll_is_transport_loss() -> TestResult {
    let base = base_dir("tread-drop");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, |stream| {
        let _ = attach_with_fixture(stream);
        // Returning here drops the stream mid-poll and retires the session.
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "transport-loss");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

#[test]
fn terminal_read_malformed_record_is_protocol_invalid() -> TestResult {
    let base = base_dir("tread-malformed");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(vec![0xAA; 128]),
        );
        // The failed projection is reported, then the CLI still releases the
        // attachment before preparing output.
        answer_detach(stream, &attachment);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

#[test]
fn terminal_read_recovers_through_need_snapshot_feedback() -> TestResult {
    let base = base_dir("tread-needsnap");
    let scope = stage_scope(&base);
    let blocked = blocked_record(1);
    let stale = obtf_record();
    let recovery = frame_record(OBTF_ALT);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        // A blocked control record is consumed without feedback.
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(blocked.clone()),
        );
        // A frame at/below the blocked revision decodes but cannot apply:
        // the projection must answer with NeedSnapshot recovery feedback.
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(stale.clone()),
        );
        let request = viewer_request(stream);
        match request.action() {
            TerminalViewerAction::NeedSnapshot {
                attachment_id,
                sequence,
                reason,
            } => {
                assert_eq!(attachment_id, &attachment);
                assert_eq!(sequence.get(), 1);
                assert_eq!(*reason, ViewerSnapshotReason::BaseMismatch);
            }
            action => panic!("peer expected need-snapshot feedback, got {action:?}"),
        }
        viewer_reply(stream, &request, TerminalViewerOutcome::FeedbackAccepted {});
        // Recovery: a fresh snapshot at a newer sequence applies cleanly.
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(recovery.clone()),
        );
        answer_detach(stream, &attachment);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    // The applied frame is the recovery snapshot: the alternate fixture's
    // routing counters (sequence 2, generation 2, revision 2).
    let route = inspect_terminal_frame_route(OBTF_ALT)?;
    let frame = &document["result"]["frame"];
    assert_eq!(frame["sequence"], route.sequence.get().to_string());
    assert_eq!(frame["generation"], route.generation.get().to_string());
    assert_eq!(frame["revision"], route.revision.to_string());
    let schema = compiled_schema(&base, "orchbee.terminal.read");
    assert!(schema.is_valid(&document), "read document: {document}");
    Ok(())
}

// ---- bounded frames + rejection witnesses ----------------------------------

/// Assert the CLI released the session carrier after the scripted exchange:
/// any further request — a retry or a leaked viewer action — fails the
/// witness, and a read that stalls instead of hitting EOF fails it too.
fn expect_session_eof(stream: &UnixStream) {
    match read_client_message(&mut &*stream) {
        Ok(message) => panic!("session must be released; CLI sent {message:?}"),
        Err(error) => assert!(
            matches!(
                error,
                orchbee_client_server_protocol::ReadMessageError::Io(ref io)
                    if io.kind() == std::io::ErrorKind::UnexpectedEof
            ),
            "session must close, not stall or corrupt: {error:?}"
        ),
    }
}

/// A decoder-legal OBTF 0.0.3 snapshot built from a shared string table:
/// `rows` carries `(flags, cells)` pairs where each cell entry is a string
/// index and every row must hold exactly `columns` cells — the only shape
/// this builder emits, so no impossible state can be expressed. It reuses
/// the fixture attachment/epoch so the scripted `Attached` answer scopes the
/// record, and the result is re-checked against the real decoder so a broken
/// builder fails inside the test instead of corrupting the peer.
fn budget_obtf(columns: u16, rows: &[(u8, &[u32])], strings: &[&str]) -> Vec<u8> {
    budget_obtf_scrolled(columns, rows, strings, 0)
}

/// `budget_obtf` with an explicit retained-scrollback count at header offset
/// 80 — the fact `terminal read --history` prices its row window from.
fn budget_obtf_scrolled(
    columns: u16,
    rows: &[(u8, &[u32])],
    strings: &[&str],
    scrollback_rows: u64,
) -> Vec<u8> {
    budget_obtf_scrolled_at(columns, rows, strings, scrollback_rows, 1, 1)
}

/// `budget_obtf_scrolled` carrying an explicit record `sequence` and
/// `revision`: a wait run pins each row read to the frame it just applied,
/// so a second applied frame must arrive with genuinely newer coordinates.
fn budget_obtf_scrolled_at(
    columns: u16,
    rows: &[(u8, &[u32])],
    strings: &[&str],
    scrollback_rows: u64,
    sequence: u64,
    revision: u64,
) -> Vec<u8> {
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    let row_count = u16::try_from(rows.len()).expect("row count fits u16");
    let mut frame = Vec::new();
    frame.extend_from_slice(b"OBTF");
    frame.extend_from_slice(&[0, 0]); // format major/minor
    frame.push(1); // snapshot kind: no base revision
    frame.push(0); // routing flags
    frame.extend_from_slice(&[0; 4]); // total length, patched below
    frame.extend_from_slice(&160_u16.to_le_bytes()); // header bytes
    frame.extend_from_slice(&[0; 2]); // reserved
    frame.extend_from_slice(route.host_epoch.as_bytes());
    frame.extend_from_slice(route.attachment_id.as_bytes());
    frame.extend_from_slice(&sequence.to_le_bytes()); // sequence
    frame.extend_from_slice(&u64::MAX.to_le_bytes()); // base revision: none
    frame.extend_from_slice(&revision.to_le_bytes()); // revision
    frame.extend_from_slice(&1_u64.to_le_bytes()); // generation
    frame.extend_from_slice(&scrollback_rows.to_le_bytes()); // captured scrollback
    frame.extend_from_slice(&columns.to_le_bytes());
    frame.extend_from_slice(&row_count.to_le_bytes());
    frame.extend_from_slice(&[0; 2]); // cursor column
    frame.extend_from_slice(&[0; 2]); // cursor row
    frame.push(1); // cursor shape
    frame.push(1); // cursor flags: visible
    frame.push(1); // primary screen
    frame.push(0); // reserved
    frame.extend_from_slice(&0_u32.to_le_bytes()); // interaction
    frame.extend_from_slice(&0_u32.to_le_bytes()); // foreground
    frame.extend_from_slice(&0_u32.to_le_bytes()); // background
    frame.extend_from_slice(&0_u32.to_le_bytes()); // cursor color
    frame.extend_from_slice(&u32::MAX.to_le_bytes()); // title: none
    frame.extend_from_slice(&u32::MAX.to_le_bytes()); // directory: none
    frame.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    frame.extend_from_slice(&1_u16.to_le_bytes()); // one style entry
    frame.extend_from_slice(&row_count.to_le_bytes()); // snapshot: all rows
    frame.extend_from_slice(&0_u16.to_le_bytes()); // no extensions
    frame.extend_from_slice(&0_u16.to_le_bytes()); // reserved tables
    // The 4,096-row bound still fits the 16-bit pixel extent at height 1.
    frame.extend_from_slice(&8_u16.to_le_bytes()); // cell pixel width
    frame.extend_from_slice(&1_u16.to_le_bytes()); // cell pixel height
    frame.extend_from_slice(&(u32::from(columns) * u32::from(row_count)).to_le_bytes());
    frame.push(3); // OBTF version byte checked by the route inspector
    frame.extend_from_slice(&1_u64.to_le_bytes()); // theme revision
    frame.extend_from_slice(&[0; 7]); // trailing reserved header
    for text in strings {
        let bytes = text.as_bytes();
        frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        frame.extend_from_slice(bytes);
    }
    // One style entry: default colors, no link, no flags or underline.
    frame.extend_from_slice(&[0; 12]); // foreground/background/underline
    frame.extend_from_slice(&u32::MAX.to_le_bytes()); // link: none
    frame.extend_from_slice(&[0; 4]); // flags, underline, reserved
    for (ordinal, (flags, cells)) in rows.iter().enumerate() {
        assert_eq!(
            cells.len(),
            usize::from(columns),
            "row {ordinal} cell count"
        );
        frame.extend_from_slice(&(ordinal as u16).to_le_bytes()); // index
        frame.push(*flags);
        frame.push(0); // prompt: none
        frame.extend_from_slice(&columns.to_le_bytes());
        frame.extend_from_slice(&[0; 2]); // reserved
        for &string in *cells {
            frame.extend_from_slice(&string.to_le_bytes());
            frame.extend_from_slice(&0_u16.to_le_bytes()); // style 0
            frame.extend_from_slice(&[0; 2]); // narrow cell, reserved
        }
    }
    let total = u32::try_from(frame.len()).expect("frame length fits u32");
    frame[8..12].copy_from_slice(&total.to_le_bytes());
    decode_terminal_text_frame(&frame).expect("generated frame must decode");
    frame
}

/// The exact-boundary witness: one shared 16 KiB string reused across all
/// 64 cells of a row joins to a 1,048,576-char text whose JSON encoding
/// debits 1,048,578 budget bytes. Three rows debit 3,145,734; row 3 keeps
/// 63 shared cells plus one mixed cell (`中`×4000 → 12,000 encoded bytes,
/// ESC×500 → 3,000, `x`×1376 → 1,376 = 16,376), landing the row at exactly
/// 1,048,570 and the sum on the 4,194,304 budget — `==` must still emit.
/// Mutating the debit check to `>=` or skipping the quote bytes drops row 3.
#[test]
fn terminal_read_emits_rows_at_the_exact_text_budget() -> TestResult {
    let base = base_dir("tread-budget-exact");
    let scope = stage_scope(&base);
    let full = "x".repeat(16384);
    let tail = format!(
        "{}{}{}",
        "中".repeat(4000),
        "\u{1b}".repeat(500),
        "x".repeat(1376)
    );
    let shared = vec![0_u32; 64];
    let mut last = vec![0_u32; 63];
    last.push(1);
    let rows: [(u8, &[u32]); 4] = [(0, &shared), (0, &shared), (0, &shared), (0, &last)];
    let record = frame_record(&budget_obtf(64, &rows, &[full.as_str(), tail.as_str()]));
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        answer_detach(stream, &attachment);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let content = &document["result"]["content"];
    assert_eq!(content["truncated"], false);
    assert_eq!(content["omittedRows"], 0);
    assert_eq!(content["rowCount"], 4);
    let emitted = content["rows"].as_array().unwrap();
    assert_eq!(emitted.len(), 4);
    let full_text = "x".repeat(64 * 16384);
    for (index, row) in emitted[..3].iter().enumerate() {
        assert_eq!(row["index"], index as u64);
        assert_eq!(row["text"].as_str().unwrap(), full_text);
    }
    let mut last_text = "x".repeat(63 * 16384);
    last_text.push_str(&tail);
    assert_eq!(emitted[3]["index"], 3);
    assert_eq!(emitted[3]["text"].as_str().unwrap(), last_text);
    let schema = compiled_schema(&base, "orchbee.terminal.read");
    assert!(schema.is_valid(&document), "read document: {document}");
    Ok(())
}

/// One encoded byte over the remaining budget omits the whole row: the mixed
/// tail's `x` run grows to 1,377, so row 3 encodes to 1,048,571 against
/// 1,048,570 left. The kept rows carry a wrap pair (bit 0 on row 0, bit 1 on
/// row 1) so a continuation-flag swap fails here too.
#[test]
fn terminal_read_omits_the_row_one_byte_over_budget() -> TestResult {
    let base = base_dir("tread-budget-partial");
    let scope = stage_scope(&base);
    let full = "x".repeat(16384);
    let tail = format!(
        "{}{}{}",
        "中".repeat(4000),
        "\u{1b}".repeat(500),
        "x".repeat(1377)
    );
    let shared = vec![0_u32; 64];
    let mut last = vec![0_u32; 63];
    last.push(1);
    let rows: [(u8, &[u32]); 4] = [(1, &shared), (2, &shared), (0, &shared), (0, &last)];
    let record = frame_record(&budget_obtf(64, &rows, &[full.as_str(), tail.as_str()]));
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        answer_detach(stream, &attachment);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let content = &document["result"]["content"];
    assert_eq!(content["truncated"], true);
    assert_eq!(content["omittedRows"], 1);
    assert_eq!(content["rowCount"], 4);
    let emitted = content["rows"].as_array().unwrap();
    assert_eq!(emitted.len(), 3);
    assert_eq!(emitted[0]["continuation"]["intoNext"], true);
    assert_eq!(emitted[0]["continuation"]["fromPrevious"], false);
    assert_eq!(emitted[1]["continuation"]["intoNext"], false);
    assert_eq!(emitted[1]["continuation"]["fromPrevious"], true);
    assert_eq!(emitted[2]["continuation"]["intoNext"], false);
    assert_eq!(emitted[2]["continuation"]["fromPrevious"], false);
    let schema = compiled_schema(&base, "orchbee.terminal.read");
    assert!(schema.is_valid(&document), "read document: {document}");
    Ok(())
}

/// The decoder maximum of 4,096 rows with a first row already over budget:
/// 64 cells sharing a 16 KiB control string join to 1,048,576 chars that
/// JSON-escape to 6,291,458 bytes, so every row is omitted and the count
/// stays honest at the schema's upper bound. Dictionary reuse keeps the wire
/// frame ≈2.1 MiB — no giant fixture. A budget that debited raw text length
/// instead of the escaped encoding, or a miscounted `omittedRows`, fails
/// here; this is also the cell_count = 262,144 decoder boundary.
#[test]
fn terminal_read_omits_all_4096_rows_over_budget() -> TestResult {
    let base = base_dir("tread-budget-omitted");
    let scope = stage_scope(&base);
    let control = "\u{1}".repeat(16384);
    let cells = vec![0_u32; 64];
    let rows: Vec<(u8, &[u32])> = (0..4096).map(|_| (0_u8, cells.as_slice())).collect();
    let record = frame_record(&budget_obtf(64, &rows, &[control.as_str()]));
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        answer_detach(stream, &attachment);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let content = &document["result"]["content"];
    assert_eq!(content["rowCount"], 4096);
    assert_eq!(content["omittedRows"], 4096);
    assert_eq!(content["truncated"], true);
    assert!(content["rows"].as_array().unwrap().is_empty());
    assert_eq!(document["result"]["frame"]["geometry"]["rows"], 4096);
    let schema = compiled_schema(&base, "orchbee.terminal.read");
    assert!(schema.is_valid(&document), "read document: {document}");
    Ok(())
}

/// The all-omitted frame through the text path: no grid rows reach stdout,
/// only the localized truncation marker — and no raw control byte escapes.
#[test]
fn terminal_read_text_output_marks_truncated_rows() -> TestResult {
    let base = base_dir("tread-budget-text");
    let scope = stage_scope(&base);
    let control = "\u{1}".repeat(16384);
    let cells = vec![0_u32; 64];
    let rows: Vec<(u8, &[u32])> = (0..4096).map(|_| (0_u8, cells.as_slice())).collect();
    let record = frame_record(&budget_obtf(64, &rows, &[control.as_str()]));
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        answer_detach(stream, &attachment);
    });
    let output = run(&base, &["terminal", "read", TERMINAL_ID]);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "… 4096 more rows truncated at the read limit\n"
    );
    assert!(
        !output.stdout.contains(&0x01),
        "raw control bytes never reach stdout"
    );
    Ok(())
}

/// `AlreadyAttached` on the attach answer means the binding already has a
/// viewer: contention is unavailability, and the CLI must release the
/// session without a retry or a stray `Detach`.
#[test]
fn terminal_read_attach_already_attached_is_service_unavailable() -> TestResult {
    let base = base_dir("tread-attached");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, |stream| {
        let request = viewer_request(stream);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Rejected {
                reason: ViewerRejectedReason::AlreadyAttached,
            },
        );
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "service-unavailable");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

/// `FeedbackMismatch` rejects the CLI's `NeedSnapshot`: a feedback-state
/// contradiction is a protocol violation, not unavailability. The CLI still
/// sends its best-effort `Detach` before releasing the session.
#[test]
fn terminal_read_feedback_mismatch_is_protocol_invalid() -> TestResult {
    let base = base_dir("tread-fb-mismatch");
    let scope = stage_scope(&base);
    let blocked = blocked_record(1);
    let stale = obtf_record();
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(blocked.clone()),
        );
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(stale.clone()),
        );
        let request = viewer_request(stream);
        match request.action() {
            TerminalViewerAction::NeedSnapshot { .. } => {}
            action => panic!("peer expected need-snapshot feedback, got {action:?}"),
        }
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Rejected {
                reason: ViewerRejectedReason::FeedbackMismatch,
            },
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

/// A `Detached` outcome on `Poll` is wire-incompatible: the transport
/// retires the session itself, so the CLI never retries and the peer sees
/// the carrier close.
#[test]
fn terminal_read_incompatible_poll_result_is_protocol_invalid() -> TestResult {
    let base = base_dir("tread-poll-incompat");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(stream, &request, TerminalViewerOutcome::Detached {});
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

/// A `Record` outcome answering `NeedSnapshot` feedback is wire-incompatible
/// the same way: session retired, no retry, protocol-invalid.
#[test]
fn terminal_read_incompatible_feedback_result_is_protocol_invalid() -> TestResult {
    let base = base_dir("tread-fb-incompat");
    let scope = stage_scope(&base);
    let blocked = blocked_record(1);
    let stale = obtf_record();
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(blocked.clone()),
        );
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(stale.clone()),
        );
        let request = viewer_request(stream);
        match request.action() {
            TerminalViewerAction::NeedSnapshot { .. } => {}
            action => panic!("peer expected need-snapshot feedback, got {action:?}"),
        }
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(vec![0xAA; 64]),
        );
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &["terminal", "read", TERMINAL_ID, "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

// ---- terminal read --history ------------------------------------------------
//
// `--history --rows <n>` reuses the same admitted session: attach, one
// applied frame for its scrollback fact, then exactly one `ReadRows` whose
// absolute window ends at the scrollback boundary. These peers answer with
// hand-built decoder-legal OBRW records, so request shape, window
// validation, typed rejections, and the drop-before-output ordering all run
// against production code.

/// A complete OBRW text-detail record whose header echoes the request the
/// CLI must have sent. `rows` are `(flags, ascii_text)` pairs; the identity
/// mapping flag is added here so every row stays decoder-legal without
/// explicit cell mappings. The record is re-checked against the real decoder
/// so a broken builder fails inside the test.
fn obrw_window(
    start_row: u64,
    total_rows: u64,
    columns: u16,
    sequence: u64,
    revision: u64,
    generation: u64,
    rows: &[(u8, &str)],
) -> Vec<u8> {
    const IDENTITY_ASCII: u8 = 0x04;
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    let mut body = Vec::new();
    let mut entry_count = 0_u32;
    for (flags, text) in rows {
        assert!(text.is_ascii() && text.len() <= usize::from(columns));
        body.push(flags | IDENTITY_ASCII);
        body.push(0); // prompt: none
        body.extend_from_slice(&[0; 2]); // reserved
        body.extend_from_slice(&(text.len() as u32).to_le_bytes());
        body.extend_from_slice(&0_u32.to_le_bytes()); // no explicit mappings
        body.extend_from_slice(text.as_bytes());
        entry_count += text.len() as u32;
    }
    let total = 112 + body.len();
    let mut record = Vec::with_capacity(total);
    record.extend_from_slice(b"OBRW");
    record.extend_from_slice(&[0, 0]); // format major/minor
    record.push(2); // detail: text
    record.push(0); // flags
    record.extend_from_slice(&(total as u32).to_le_bytes());
    record.extend_from_slice(&112_u16.to_le_bytes()); // header bytes
    record.extend_from_slice(&columns.to_le_bytes());
    record.extend_from_slice(route.host_epoch.as_bytes());
    record.extend_from_slice(route.attachment_id.as_bytes());
    record.extend_from_slice(&sequence.to_le_bytes());
    record.extend_from_slice(&revision.to_le_bytes());
    record.extend_from_slice(&generation.to_le_bytes());
    record.extend_from_slice(&total_rows.to_le_bytes());
    record.extend_from_slice(&start_row.to_le_bytes());
    record.extend_from_slice(&(rows.len() as u16).to_le_bytes()); // row count
    record.extend_from_slice(&[0; 2]); // reserved
    record.extend_from_slice(&entry_count.to_le_bytes());
    record.extend_from_slice(&(body.len() as u32).to_le_bytes()); // body bytes
    record.push(3); // format patch
    record.extend_from_slice(&0_u32.to_le_bytes()); // projection bytes: text only
    record.extend_from_slice(&0_u32.to_le_bytes()); // graphics bytes: text only
    record.extend_from_slice(&[0; 3]); // reserved tail
    record.extend_from_slice(&body);
    decode_terminal_text_row_window(&record).expect("generated window must decode");
    record
}

/// Wrap an OBRW record in its OBTP `RowsSnapshot` response envelope — the
/// byte payload a `record` outcome answering `ReadRows` carries.
fn history_record(obrw: &[u8]) -> Vec<u8> {
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    // The OBTP correlation is the Server-issued operation identity: opaque to
    // the CLI and checked by the routing hop, so any valid value suffices.
    let correlation = CorrelationId::from_bytes([0xdd; 16]).expect("correlation id");
    encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind: 46,
        payload_encoding: PayloadEncoding::RowWindow,
        host_epoch: Some(route.host_epoch),
        correlation_id: Some(correlation),
        optional_message: false,
        payload: obrw,
    })
    .expect("history record")
}

/// Consume the CLI's single `ReadRows`, asserting every pinned field, and
/// return the request plus `(start_row, row_count)` for the script's answer.
fn take_read_rows(
    stream: &UnixStream,
    attachment: &ViewerId,
    revision: u64,
    generation: u64,
) -> (TerminalViewerRequest, u64, u16) {
    let request = viewer_request(stream);
    let bounds = match request.action() {
        TerminalViewerAction::ReadRows {
            attachment_id,
            start_row,
            row_count,
            revision: request_revision,
            generation: request_generation,
            request_sequence,
            maximum_bytes,
        } => {
            assert_eq!(attachment_id, attachment);
            assert_eq!(*request_revision, revision);
            assert_eq!(request_generation.get(), generation);
            assert_eq!(request_sequence.get(), 1);
            assert_eq!(*maximum_bytes, MAXIMUM_READ_ROWS_BYTES);
            (*start_row, *row_count)
        }
        action => panic!("peer expected read-rows, got {action:?}"),
    };
    (request, bounds.0, bounds.1)
}

/// Zero retained scrollback answers the history read without any `ReadRows`:
/// the next request after the applied frame must be the release `Detach`.
#[test]
fn terminal_read_history_empty_scrollback_never_reads_rows() -> TestResult {
    let base = base_dir("thist-empty");
    let scope = stage_scope(&base);
    let record = obtf_record();
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "10",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["schema"], "orchbee.terminal.history");
    let history = &document["result"]["history"];
    assert_eq!(history["scrollbackRows"], "0");
    assert_eq!(history["screenRows"], 6);
    assert_eq!(history["totalRows"], "6");
    assert_eq!(history["startRow"], "0");
    assert_eq!(history["requestedRows"], 10);
    assert_eq!(history["windowRows"], 0);
    let content = &document["result"]["content"];
    assert_eq!(content["rowCount"], 0);
    assert_eq!(content["rows"].as_array().unwrap().len(), 0);
    assert_eq!(content["truncated"], false);
    assert_eq!(content["omittedRows"], 0);
    let schema = compiled_schema(&base, "orchbee.terminal.history");
    assert!(schema.is_valid(&document), "history document: {document}");
    Ok(())
}

/// The happy path: `S=3`, `--rows 2` requests the absolute window `[1,3)`;
/// the emitted rows carry their absolute indices and wrap pair.
#[test]
fn terminal_read_history_returns_the_bounded_window() -> TestResult {
    let base = base_dir("thist-window");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells), (0, &cells)], &["s"], 3);
    let record = frame_record(&frame);
    // The window answers start=1, rows 1 and 2 of total 3+2=5; row 1 wraps
    // into row 2 (bit0 set on 1, bit1 set on 2).
    let obrw = obrw_window(1, 5, 8, 1, 1, 1, &[(0x01, "wrap-a"), (0x02, "wrap-b")]);
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        let (request, start_row, row_count) = take_read_rows(stream, &attachment, 1, 1);
        assert_eq!(start_row, 1);
        assert_eq!(row_count, 2);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "2",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["schema"], "orchbee.terminal.history");
    let history = &document["result"]["history"];
    assert_eq!(history["revision"], "1");
    assert_eq!(history["generation"], "1");
    assert_eq!(history["alternateScreen"], false);
    assert_eq!(history["columns"], 8);
    assert_eq!(history["screenRows"], 2);
    assert_eq!(history["scrollbackRows"], "3");
    assert_eq!(history["totalRows"], "5");
    assert_eq!(history["startRow"], "1");
    assert_eq!(history["requestedRows"], 2);
    assert_eq!(history["windowRows"], 2);
    let content = &document["result"]["content"];
    assert_eq!(content["truncated"], false);
    assert_eq!(content["omittedRows"], 0);
    assert_eq!(content["rowCount"], 2);
    assert_eq!(content["limits"]["rows"], 1000);
    let emitted = content["rows"].as_array().unwrap();
    assert_eq!(emitted.len(), 2);
    assert_eq!(emitted[0]["index"], "1");
    assert_eq!(emitted[0]["text"], "wrap-a");
    assert_eq!(emitted[0]["continuation"]["fromPrevious"], false);
    assert_eq!(emitted[0]["continuation"]["intoNext"], true);
    assert_eq!(emitted[1]["index"], "2");
    assert_eq!(emitted[1]["text"], "wrap-b");
    assert_eq!(emitted[1]["continuation"]["fromPrevious"], true);
    assert_eq!(emitted[1]["continuation"]["intoNext"], false);
    let schema = compiled_schema(&base, "orchbee.terminal.history");
    assert!(schema.is_valid(&document), "history document: {document}");
    assert_eq!(
        scope.read_dir()?.count(),
        1,
        "history read must not write the scope"
    );
    Ok(())
}

/// `count = min(requested, scrollback, MAXIMUM_FRAME_CELLS / columns)`: a
/// 500-column frame caps the window at 524 rows, so `--rows 1000` on 600
/// retained rows prices `start = 600 - 524 = 76` — the cap bites the recent
/// end, never the requested edge.
#[test]
fn terminal_read_history_capacity_clamps_the_window() -> TestResult {
    let base = base_dir("thist-cap");
    let scope = stage_scope(&base);
    let cells = [0_u32; 500];
    let frame = budget_obtf_scrolled(500, &[(0, &cells)], &["s"], 600);
    let record = frame_record(&frame);
    // 524 one-cell rows, indices 76..600 of total 601.
    let window_rows: Vec<(u8, String)> = (0..524).map(|_| (0_u8, "x".to_owned())).collect();
    let window_ref: Vec<(u8, &str)> = window_rows
        .iter()
        .map(|(flags, text)| (*flags, text.as_str()))
        .collect();
    let obrw = obrw_window(76, 601, 500, 1, 1, 1, &window_ref);
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        let (request, start_row, row_count) = take_read_rows(stream, &attachment, 1, 1);
        assert_eq!(start_row, 76);
        assert_eq!(row_count, 524);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "1000",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let history = &document["result"]["history"];
    assert_eq!(history["scrollbackRows"], "600");
    assert_eq!(history["totalRows"], "601");
    assert_eq!(history["startRow"], "76");
    assert_eq!(history["requestedRows"], 1000);
    assert_eq!(history["windowRows"], 524);
    let content = &document["result"]["content"];
    // min(requested, scrollback) = 600 rows were entitled; the window cap
    // emitted 524, so 76 count as omitted — honest, not silent.
    assert_eq!(content["truncated"], true);
    assert_eq!(content["omittedRows"], 76);
    assert_eq!(content["rowCount"], 524);
    let emitted = content["rows"].as_array().unwrap();
    assert_eq!(emitted[0]["index"], "76");
    assert_eq!(emitted[523]["index"], "599");
    let schema = compiled_schema(&base, "orchbee.terminal.history");
    assert!(schema.is_valid(&document), "history document: {document}");
    Ok(())
}

/// `requested > scrollback` clamps to the retained count: `--rows 1000` on
/// `S=2` requests `[0,2)`, and nothing past the scrollback boundary is owed.
#[test]
fn terminal_read_history_clamps_to_the_retained_rows() -> TestResult {
    let base = base_dir("thist-clamp");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells), (0, &cells)], &["s"], 2);
    let record = frame_record(&frame);
    let obrw = obrw_window(0, 4, 8, 1, 1, 1, &[(0x00, "oldest"), (0x00, "newer")]);
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        let (request, start_row, row_count) = take_read_rows(stream, &attachment, 1, 1);
        assert_eq!(start_row, 0);
        assert_eq!(row_count, 2);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "1000",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let history = &document["result"]["history"];
    assert_eq!(history["startRow"], "0");
    assert_eq!(history["windowRows"], 2);
    assert_eq!(document["result"]["content"]["omittedRows"], 0);
    assert_eq!(document["result"]["content"]["truncated"], false);
    Ok(())
}

/// `rows-rejected` maps to its fixed failure vocabulary: Host state answers
/// are unavailability with the typed reason, and the local budget overrun is
/// a rejection. No second `ReadRows` is ever sent.
#[test]
fn terminal_read_history_rows_rejected_maps_typed_failures() -> TestResult {
    let cases: &[(&str, ViewerRowsRejectedReason, i32, &str, Option<&str>)] = &[
        (
            "evicted",
            ViewerRowsRejectedReason::HistoryEvicted,
            4,
            "service-unavailable",
            Some("history-evicted"),
        ),
        (
            "stale-rev",
            ViewerRowsRejectedReason::StaleRevision,
            4,
            "service-unavailable",
            Some("stale-revision"),
        ),
        (
            "stale-gen",
            ViewerRowsRejectedReason::StaleGeneration,
            4,
            "service-unavailable",
            Some("stale-generation"),
        ),
        (
            "exhausted",
            ViewerRowsRejectedReason::ResourceExhausted,
            4,
            "service-unavailable",
            Some("resource-exhausted"),
        ),
        (
            "missing",
            ViewerRowsRejectedReason::MissingResource,
            4,
            "service-unavailable",
            Some("missing-resource"),
        ),
        (
            "phase",
            ViewerRowsRejectedReason::InvalidPhase,
            4,
            "service-unavailable",
            Some("invalid-phase"),
        ),
        (
            "limit",
            ViewerRowsRejectedReason::LimitExceeded,
            3,
            "rejected",
            Some("limit-exceeded"),
        ),
        // A reason our fixed request shape cannot produce is a violation.
        (
            "bad-field",
            ViewerRowsRejectedReason::InvalidField,
            5,
            "protocol-invalid",
            None,
        ),
    ];
    for (tag, reason, code, kind, detail_reason) in cases {
        let base = base_dir(&format!("thist-rej-{tag}"));
        let scope = stage_scope(&base);
        let cells = [0_u32; 8];
        let frame = budget_obtf_scrolled(8, &[(0, &cells)], &["s"], 5);
        let record = frame_record(&frame);
        let reason = *reason;
        let peer = read_peer(&scope, move |stream| {
            let attachment = attach_with_fixture(stream);
            let request = take_poll(stream, &attachment);
            viewer_reply(
                stream,
                &request,
                TerminalViewerOutcome::Record(record.clone()),
            );
            let (request, _, _) = take_read_rows(stream, &attachment, 1, 1);
            viewer_reply(
                stream,
                &request,
                TerminalViewerOutcome::RowsRejected { reason },
            );
            // Exactly one read: the release Detach follows, then EOF.
            answer_detach(stream, &attachment);
            expect_session_eof(stream);
        });
        let output = run(
            &base,
            &[
                "terminal",
                "read",
                TERMINAL_ID,
                "--history",
                "--rows",
                "3",
                "--format",
                "json",
            ],
        );
        peer.join().expect("peer finished");
        assert_eq!(
            output.status.code(),
            Some(*code),
            "{tag}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let failure = failure_kind(&output);
        assert_eq!(failure["failure"]["kind"], *kind, "{tag}");
        assert_eq!(failure["failure"]["details"]["stage"], "viewer", "{tag}");
        match detail_reason {
            Some(expected) => {
                assert_eq!(
                    failure["failure"]["details"]["operation"], "read-history",
                    "{tag}"
                );
                assert_eq!(failure["failure"]["details"]["reason"], *expected, "{tag}");
            }
            // A reason outside the read's honest vocabulary reports only the
            // stage — the typed detail would claim more than was seen.
            None => {
                assert!(
                    failure["failure"]["details"].get("operation").is_none(),
                    "{tag}"
                );
                assert!(
                    failure["failure"]["details"].get("reason").is_none(),
                    "{tag}"
                );
            }
        }
        // The emitted envelope validates against the failure schema's new
        // history detail forms.
        let schema = compiled_schema(&base, "orchbee.cli.failure");
        assert!(schema.is_valid(&failure), "{tag} failure: {failure}");
    }
    Ok(())
}

/// A `Record` answer whose OBRW header does not echo the request — wrong
/// start row here — is protocol-invalid; the attachment is still released
/// and the session dropped before the failure envelope is prepared.
#[test]
fn terminal_read_history_window_mismatch_is_protocol_invalid() -> TestResult {
    let base = base_dir("thist-mismatch");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells)], &["s"], 5);
    let record = frame_record(&frame);
    // start_row 0 does not echo the requested start 3.
    let obrw = obrw_window(0, 7, 8, 1, 1, 1, &[(0x00, "aa"), (0x00, "bb")]);
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        let (request, start_row, row_count) = take_read_rows(stream, &attachment, 1, 1);
        assert_eq!(start_row, 3);
        assert_eq!(row_count, 2);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "2",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

/// A success `record` whose OBRW payload exceeds the declared `maximumBytes`
/// budget is rejected as `protocol-invalid` before the row window is
/// materialized — the byte bound is never relaxed into decode or truncation.
#[test]
fn terminal_read_history_oversized_record_is_protocol_invalid() -> TestResult {
    let base = base_dir("thist-oversize");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells)], &["s"], 5);
    let record = frame_record(&frame);
    // Every envelope field is a valid kind-46 response, so the single byte
    // over the declared budget is the only defect this control isolates.
    let route = inspect_terminal_frame_route(OBTF).expect("fixture route");
    let oversized_payload = vec![0_u8; MAXIMUM_READ_ROWS_BYTES as usize + 1];
    let oversized = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind: 46,
        payload_encoding: PayloadEncoding::RowWindow,
        host_epoch: Some(route.host_epoch),
        correlation_id: Some(CorrelationId::from_bytes([0xdd; 16]).expect("correlation id")),
        optional_message: false,
        payload: &oversized_payload,
    })
    .expect("oversized record");
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        let (request, _, _) = take_read_rows(stream, &attachment, 1, 1);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(oversized.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "2",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    assert_eq!(failure["failure"]["details"]["stage"], "viewer");
    Ok(())
}

/// A frame-role or wrong-encoding `record` where the row window belongs is
/// the same violation — the envelope must be a kind-46 response.
#[test]
fn terminal_read_history_frame_record_answer_is_protocol_invalid() -> TestResult {
    let base = base_dir("thist-wrongkind");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells)], &["s"], 5);
    let record = frame_record(&frame);
    let wrong = obtf_record();
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        let (request, _, _) = take_read_rows(stream, &attachment, 1, 1);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(wrong.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "2",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "protocol-invalid");
    Ok(())
}

/// Text format emits one escaped line per row and the localized truncation
/// marker counts rows the window cap never sent.
#[test]
fn terminal_read_history_text_output_marks_omitted_rows() -> TestResult {
    let base = base_dir("thist-text");
    let scope = stage_scope(&base);
    let cells = [0_u32; 500];
    let frame = budget_obtf_scrolled(500, &[(0, &cells)], &["s"], 600);
    let record = frame_record(&frame);
    let window_rows: Vec<(u8, String)> = (0..524).map(|_| (0_u8, "x".to_owned())).collect();
    let window_ref: Vec<(u8, &str)> = window_rows
        .iter()
        .map(|(flags, text)| (*flags, text.as_str()))
        .collect();
    let obrw = obrw_window(76, 601, 500, 1, 1, 1, &window_ref);
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        let (request, _, _) = take_read_rows(stream, &attachment, 1, 1);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "read",
            TERMINAL_ID,
            "--history",
            "--rows",
            "1000",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = String::from_utf8(output.stdout).expect("utf8 output");
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 525, "524 rows plus the marker");
    assert!(lines[..524].iter().all(|line| *line == "x"));
    assert_eq!(lines[524], "… 76 more rows truncated at the read limit");
    Ok(())
}

// ---- terminal wait-output ---------------------------------------------------
//
// `terminal wait-output` holds one read-only viewer through a bounded
// observation loop: every applied frame is acknowledged by a real
// `FrameApplied` whose `FeedbackAccepted` is confirmed, then one absolute
// `ReadRows` pins that exact revision+generation over the unified
// retained-row window. `request_sequence` starts at 1 and strictly increases
// on every attempt — the Host rejects a repeat. These peers script the
// whole exchange, so request shape, feedback ordering, stale re-pinning,
// typed rejections, deadline mapping, and the confirmed-detach cleanup all
// run against production code.

/// Consume one request asserting it is `FrameApplied` for `attachment`, and
/// answer `FeedbackAccepted` — the complete feedback exchange the wait loop
/// runs for every applied frame.
fn take_frame_applied(stream: &UnixStream, attachment: &ViewerId) {
    let request = viewer_request(stream);
    match request.action() {
        TerminalViewerAction::FrameApplied {
            attachment_id,
            sequence,
            generation,
            revision,
        } => {
            assert_eq!(attachment_id, attachment);
            assert_eq!(sequence.get(), 1);
            assert_eq!(generation.get(), 1);
            assert_eq!(*revision, 1);
        }
        action => panic!("peer expected frame-applied, got {action:?}"),
    }
    viewer_reply(stream, &request, TerminalViewerOutcome::FeedbackAccepted {});
}

/// `take_frame_applied` for the second applied frame: the acknowledgement
/// carries the newer record's own coordinates, never the first frame's.
fn take_frame_applied_at(stream: &UnixStream, attachment: &ViewerId, sequence: u64, revision: u64) {
    let request = viewer_request(stream);
    match request.action() {
        TerminalViewerAction::FrameApplied {
            attachment_id,
            sequence: request_sequence,
            generation,
            revision: request_revision,
        } => {
            assert_eq!(attachment_id, attachment);
            assert_eq!(request_sequence.get(), sequence);
            assert_eq!(generation.get(), 1);
            assert_eq!(*request_revision, revision);
        }
        action => panic!("peer expected frame-applied, got {action:?}"),
    }
    viewer_reply(stream, &request, TerminalViewerOutcome::FeedbackAccepted {});
}

/// Consume one `ReadRows` asserting every pinned field plus the expected
/// strictly-increasing `request_sequence`, and return the request plus
/// `(start_row, row_count)` for the script's answer.
fn take_wait_read_rows(
    stream: &UnixStream,
    attachment: &ViewerId,
    revision: u64,
    generation: u64,
    request_sequence: u64,
) -> (TerminalViewerRequest, u64, u16) {
    let request = viewer_request(stream);
    let bounds = match request.action() {
        TerminalViewerAction::ReadRows {
            attachment_id,
            start_row,
            row_count,
            revision: request_revision,
            generation: request_generation,
            request_sequence: sequence,
            maximum_bytes,
        } => {
            assert_eq!(attachment_id, attachment);
            assert_eq!(*request_revision, revision);
            assert_eq!(request_generation.get(), generation);
            assert_eq!(sequence.get(), request_sequence);
            assert_eq!(*maximum_bytes, MAXIMUM_READ_ROWS_BYTES);
            (*start_row, *row_count)
        }
        action => panic!("peer expected read-rows, got {action:?}"),
    };
    (request, bounds.0, bounds.1)
}

/// Drive the raced-detach exchange as the peer: attach, one matched window,
/// then the real `Detach` request — reported over `detach_seen` — whose
/// typed refusal is held back until `release` fires. The two barriers make
/// whatever the main thread does between them provably ordered before the
/// answer reaches the wire, so a signal either preceded the refusal or did
/// not happen at all.
fn peer_detach_hold(
    stream: &UnixStream,
    detach_seen: &mpsc::Sender<()>,
    release: &mpsc::Receiver<()>,
) {
    let attachment = attach_with_fixture(stream);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells)], &["s"], 1);
    let request = take_poll(stream, &attachment);
    viewer_reply(
        stream,
        &request,
        TerminalViewerOutcome::Record(frame_record(&frame)),
    );
    take_frame_applied(stream, &attachment);
    let (request, start_row, row_count) = take_wait_read_rows(stream, &attachment, 1, 1, 1);
    assert_eq!((start_row, row_count), (0, 2));
    let obrw = obrw_window(0, 2, 8, 1, 1, 1, &[(0x00, "hit"), (0x00, "tail")]);
    viewer_reply(
        stream,
        &request,
        TerminalViewerOutcome::Record(history_record(&obrw)),
    );
    // The Detach request on the wire is the real barrier — the CLI is now
    // blocked inside the detach exchange waiting on this answer.
    let request = viewer_request(stream);
    match request.action() {
        TerminalViewerAction::Detach { attachment_id } if attachment_id == &attachment => {}
        action => panic!("peer expected detach, got {action:?}"),
    }
    detach_seen
        .send(())
        .expect("main thread waits at the detach barrier");
    release
        .recv_timeout(PEER_IO_DEADLINE)
        .expect("main thread releases the refused answer");
    viewer_reply(
        stream,
        &request,
        TerminalViewerOutcome::Unavailable {
            reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
        },
    );
    expect_session_eof(stream);
}

/// Keep answering `Empty` to every `Poll` until the CLI drops the session:
/// the deadline-driven timeout path never sends `Detach` once nothing of the
/// budget remains, so the peer only ever sees polls then EOF.
fn answer_empty_polls_until_eof(stream: &UnixStream, attachment: &ViewerId) {
    loop {
        let request = match read_client_message(&mut &*stream) {
            Ok(ClientMessage::TerminalViewer(request)) => request,
            Ok(other) => panic!("peer expected a terminal.viewer request, got {other:?}"),
            Err(_) => return,
        };
        match request.action() {
            TerminalViewerAction::Poll { attachment_id } if attachment_id == attachment => {}
            action => panic!("peer expected poll, got {action:?}"),
        }
        viewer_reply(stream, &request, TerminalViewerOutcome::Empty {});
    }
}

/// The happy path over the unified window: `--rows 5` on `3+2` retained rows
/// prices `[0,5)` in one absolute read, and a soft-wrap chain crossing the
/// history/visible boundary joins before the literal match — no single
/// physical row holds the pattern. The result envelope carries the honest
/// observation record and validates against its published schema.
#[test]
fn terminal_wait_matches_a_wrap_chain_across_the_window() -> TestResult {
    let base = base_dir("twait-match");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells), (0, &cells)], &["s"], 3);
    let record = frame_record(&frame);
    // Indices 0..5 of total 5: history rows 0-2, visible rows 3-4. Row 2
    // wraps into row 3 — the match "cdef" spans the boundary.
    let obrw = obrw_window(
        0,
        5,
        8,
        1,
        1,
        1,
        &[
            (0x00, "line0"),
            (0x00, "line1"),
            (0x01, "abc"),
            (0x02, "def-done"),
            (0x00, "other"),
        ],
    );
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        take_frame_applied(stream, &attachment);
        let (request, start_row, row_count) = take_wait_read_rows(stream, &attachment, 1, 1, 1);
        assert_eq!(start_row, 0);
        assert_eq!(row_count, 5);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "cdef",
            "--rows",
            "5",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["schema"], "orchbee.terminal.wait-output");
    assert_eq!(document["source"]["kind"], "workbench-snapshot");
    let result = &document["result"];
    assert_eq!(result["terminal"]["terminalId"], TERMINAL_ID);
    assert_eq!(result["matched"], true);
    let matched = &result["match"];
    assert_eq!(matched["excerpt"], "abcdef-done");
    assert_eq!(matched["excerptTruncated"], false);
    assert_eq!(matched["matchOffset"], "2");
    assert_eq!(matched["rowStart"], "2");
    assert_eq!(matched["rowEnd"], "3");
    assert_eq!(matched["revision"], "1");
    assert_eq!(matched["generation"], "1");
    let observed = &result["observed"];
    assert_eq!(observed["completedObservations"], 1);
    assert_eq!(observed["windowStart"], "0");
    assert_eq!(observed["windowRows"], "5");
    assert_eq!(observed["windowTruncated"], false);
    assert_eq!(observed["totalRows"], "5");
    assert_eq!(observed["requestedRows"], 5);
    assert_eq!(observed["timeoutMs"], 30_000);
    assert!(observed["elapsedMs"].as_u64().is_some());
    let route = inspect_terminal_frame_route(OBTF)?;
    assert_eq!(observed["attachmentId"], route.attachment_id.to_text());
    assert_eq!(observed["hostEpoch"], route.host_epoch.to_text());
    let schema = compiled_schema(&base, "orchbee.terminal.wait-output");
    assert!(schema.is_valid(&document), "wait document: {document}");
    Ok(())
}

/// A quiet window is not a result: the wait keeps polling, the second applied
/// frame carries strictly newer coordinates, its `ReadRows` uses
/// `request_sequence` 2 — the Host refuses a repeat — and the second
/// consistent window finally matches.
#[test]
fn terminal_wait_polls_until_the_text_appears() -> TestResult {
    let base = base_dir("twait-later");
    let scope = stage_scope(&base);
    let cells = [0_u32; 16];
    let first = budget_obtf_scrolled(16, &[(0, &cells)], &["s"], 2);
    let first_record = frame_record(&first);
    let second = budget_obtf_scrolled_at(16, &[(0, &cells), (0, &cells)], &["s"], 2, 2, 2);
    let second_record = frame_record(&second);
    // First window (total 3): nothing. Second window (total 4): the marker.
    let quiet = history_record(&obrw_window(
        0,
        3,
        16,
        1,
        1,
        1,
        &[(0x00, "line0"), (0x00, "line1"), (0x00, "line2")],
    ));
    let found = history_record(&obrw_window(
        0,
        4,
        16,
        2,
        2,
        1,
        &[
            (0x00, "line0"),
            (0x00, "line1"),
            (0x00, "line2"),
            (0x00, "marker-yes"),
        ],
    ));
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(first_record.clone()),
        );
        take_frame_applied(stream, &attachment);
        let (request, start_row, row_count) = take_wait_read_rows(stream, &attachment, 1, 1, 1);
        assert_eq!(start_row, 0);
        assert_eq!(row_count, 3);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(quiet.clone()),
        );
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(second_record.clone()),
        );
        take_frame_applied_at(stream, &attachment, 2, 2);
        let (request, start_row, row_count) = take_wait_read_rows(stream, &attachment, 2, 1, 2);
        assert_eq!(start_row, 0);
        assert_eq!(row_count, 4);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(found.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "marker-yes",
            "--rows",
            "10",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let matched = &document["result"]["match"];
    assert_eq!(matched["excerpt"], "marker-yes");
    assert_eq!(matched["rowStart"], "3");
    assert_eq!(matched["rowEnd"], "3");
    assert_eq!(matched["revision"], "2");
    let observed = &document["result"]["observed"];
    assert_eq!(observed["completedObservations"], 2);
    assert_eq!(observed["totalRows"], "4");
    let schema = compiled_schema(&base, "orchbee.terminal.wait-output");
    assert!(schema.is_valid(&document), "wait document: {document}");
    Ok(())
}

/// A stale answer to the pinned read is not a failure and never a partial
/// window: `stale-revision` abandons that read, the next applied frame
/// re-pins the coordinates, and the retried read uses `request_sequence` 2.
#[test]
fn terminal_wait_stale_rows_answer_repins_and_retries() -> TestResult {
    let base = base_dir("twait-stale");
    let scope = stage_scope(&base);
    let cells = [0_u32; 16];
    let first = budget_obtf_scrolled(16, &[(0, &cells)], &["s"], 2);
    let first_record = frame_record(&first);
    let second = budget_obtf_scrolled_at(16, &[(0, &cells), (0, &cells)], &["s"], 2, 2, 2);
    let second_record = frame_record(&second);
    let found = history_record(&obrw_window(
        0,
        4,
        16,
        2,
        2,
        1,
        &[
            (0x00, "line0"),
            (0x00, "line1"),
            (0x00, "line2"),
            (0x00, "late-marker"),
        ],
    ));
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(first_record.clone()),
        );
        take_frame_applied(stream, &attachment);
        let (request, _, _) = take_wait_read_rows(stream, &attachment, 1, 1, 1);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::RowsRejected {
                reason: ViewerRowsRejectedReason::StaleRevision,
            },
        );
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(second_record.clone()),
        );
        take_frame_applied_at(stream, &attachment, 2, 2);
        let (request, _, _) = take_wait_read_rows(stream, &attachment, 2, 1, 2);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(found.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "late-marker",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["result"]["match"]["excerpt"], "late-marker");
    // The stale answer completed no observation pass; only the retried one did.
    assert_eq!(document["result"]["observed"]["completedObservations"], 1);
    Ok(())
}

/// A non-stale `rows-rejected` keeps its typed classification — the wait
/// treats it like the one-shot read does — and the viewer is still released
/// through the confirmed detach, so `cleanup` reports `confirmed`.
#[test]
fn terminal_wait_rows_rejection_maps_typed_failure_and_detaches() -> TestResult {
    let base = base_dir("twait-rejected");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells)], &["s"], 2);
    let record = frame_record(&frame);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        take_frame_applied(stream, &attachment);
        let (request, _, _) = take_wait_read_rows(stream, &attachment, 1, 1, 1);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::RowsRejected {
                reason: ViewerRowsRejectedReason::LimitExceeded,
            },
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "never",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "rejected");
    assert_eq!(failure["failure"]["details"]["reason"], "limit-exceeded");
    assert_eq!(failure["failure"]["details"]["cleanup"], "confirmed");
    assert!(failure["failure"]["details"].get("matchObserved").is_none());
    Ok(())
}

/// Deadline mapping: `Empty` polls never produce a match, the total budget
/// is measured from command start, and once nothing remains the viewer
/// release is the session drop alone — the failure honestly says
/// `cleanup: "requested"` with zero completed observations.
#[test]
fn terminal_wait_deadline_is_exit_seven_with_honest_details() -> TestResult {
    let base = base_dir("twait-timeout");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        answer_empty_polls_until_eof(stream, &attachment);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "never-appears",
            "--timeout",
            "3000",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(7));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "timeout");
    let details = &failure["failure"]["details"];
    assert_eq!(details["stage"], "wait");
    assert_eq!(details["timeoutMs"], 3000);
    assert_eq!(details["observed"]["completedObservations"], 0);
    assert_eq!(details["cleanup"], "requested");
    assert!(details.get("matchObserved").is_none());
    Ok(())
}

/// A match whose detach is refused is still a failure, never a clean exit:
/// the answer is the detach's own typed failure honestly carrying
/// `matchObserved: true` and `cleanup: "unconfirmed"`.
#[test]
fn terminal_wait_match_with_refused_detach_reports_match_observed() -> TestResult {
    let base = base_dir("twait-detach-fail");
    let scope = stage_scope(&base);
    let cells = [0_u32; 8];
    let frame = budget_obtf_scrolled(8, &[(0, &cells)], &["s"], 1);
    let record = frame_record(&frame);
    let obrw = obrw_window(0, 2, 8, 1, 1, 1, &[(0x00, "hit"), (0x00, "tail")]);
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        take_frame_applied(stream, &attachment);
        let (request, _, _) = take_wait_read_rows(stream, &attachment, 1, 1, 1);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        // The real Detach request arrives; the answer refuses it.
        let request = viewer_request(stream);
        match request.action() {
            TerminalViewerAction::Detach { attachment_id } if attachment_id == &attachment => {}
            action => panic!("peer expected detach, got {action:?}"),
        }
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Unavailable {
                reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
            },
        );
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "hit",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "service-unavailable");
    let details = &failure["failure"]["details"];
    assert_eq!(details["matchObserved"], true);
    assert_eq!(details["cleanup"], "unconfirmed");
    Ok(())
}

/// A SIGINT that lands while the detach exchange is still open must win the
/// answer even when the detach itself fails: the peer observes the real
/// `Detach` on the wire, only then is this child interrupted, and only then
/// is the typed refusal released — so the cancellation provably preceded
/// the failure answer. The cancellation outranks the refused detach's own
/// classification: exit 130 with `cancelled`, still honestly reporting that
/// the match was observed and the release stays unconfirmed.
#[test]
fn terminal_wait_sigint_during_refused_detach_reports_cancelled() -> TestResult {
    let base = base_dir("twait-detach-int");
    let scope = stage_scope(&base);
    let (detach_seen, detach_noted) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let peer = read_peer(&scope, move |stream| {
        peer_detach_hold(stream, &detach_seen, &release_recv);
    });
    let child = spawn_cli(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "hit",
            "--format",
            "json",
        ],
    );
    detach_noted
        .recv_timeout(PEER_IO_DEADLINE)
        .expect("peer observed the real detach");
    kill(Pid::from_raw(child.id() as i32), Signal::SIGINT)
        .expect("SIGINT to this test's own child");
    release_send.send(()).expect("peer waits for the release");
    let output = support::bounded_wait_with_output(child, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|failure| panic!("{failure}"));
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(130),
        "cancelled wait exits 130, not the refused detach's typed failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "cancelled");
    let details = &failure["failure"]["details"];
    assert_eq!(details["matchObserved"], true);
    assert_eq!(details["cleanup"], "unconfirmed");
    Ok(())
}

/// The same exchange without the signal keeps the refused detach's own
/// typed failure: the raced sibling cannot pass on just any nonzero exit —
/// only the cancellation, never the refusal, may produce 130.
#[test]
fn terminal_wait_refused_detach_without_signal_keeps_typed_failure() -> TestResult {
    let base = base_dir("twait-detach-calm");
    let scope = stage_scope(&base);
    let (detach_seen, detach_noted) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let peer = read_peer(&scope, move |stream| {
        peer_detach_hold(stream, &detach_seen, &release_recv);
    });
    let child = spawn_cli(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "hit",
            "--format",
            "json",
        ],
    );
    detach_noted
        .recv_timeout(PEER_IO_DEADLINE)
        .expect("peer observed the real detach");
    release_send.send(()).expect("peer waits for the release");
    let output = support::bounded_wait_with_output(child, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|failure| panic!("{failure}"));
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(4),
        "an uninterrupted refused detach keeps its typed failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "service-unavailable");
    let details = &failure["failure"]["details"];
    assert_eq!(details["matchObserved"], true);
    assert_eq!(details["cleanup"], "unconfirmed");
    Ok(())
}

/// The default text answer is the matching logical line itself, escaped like
/// `terminal read` content — one line plus the newline, nothing else.
#[test]
fn terminal_wait_text_output_prints_the_matched_line() -> TestResult {
    let base = base_dir("twait-text");
    let scope = stage_scope(&base);
    let cells = [0_u32; 16];
    let frame = budget_obtf_scrolled(16, &[(0, &cells)], &["s"], 1);
    let record = frame_record(&frame);
    let obrw = obrw_window(0, 2, 16, 1, 1, 1, &[(0x00, "pre marker"), (0x00, "post")]);
    let answer = history_record(&obrw);
    let peer = read_peer(&scope, move |stream| {
        let attachment = attach_with_fixture(stream);
        let request = take_poll(stream, &attachment);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(record.clone()),
        );
        take_frame_applied(stream, &attachment);
        let (request, _, _) = take_wait_read_rows(stream, &attachment, 1, 1, 1);
        viewer_reply(
            stream,
            &request,
            TerminalViewerOutcome::Record(answer.clone()),
        );
        answer_detach(stream, &attachment);
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &["terminal", "wait-output", TERMINAL_ID, "--match", "marker"],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = String::from_utf8(output.stdout).expect("utf8 output");
    assert_eq!(body, "pre marker\n");
    Ok(())
}

/// A late `Attached` answer never outranks the total deadline: the peer
/// holds the reply until the wait's own budget is spent, so the failed
/// exchange must settle as `timeout` — not the transport classification a
/// missing settlement would report. The session drop is the whole cleanup,
/// so no cleanup field is claimed.
#[test]
fn terminal_wait_late_attach_answer_settles_as_timeout() -> TestResult {
    let base = base_dir("twait-attach-late");
    let scope = stage_scope(&base);
    let peer = read_peer(&scope, move |stream| {
        let request = viewer_request(stream);
        match request.action() {
            TerminalViewerAction::AttachBinding { .. } => {}
            action => panic!("peer expected attach-binding, got {action:?}"),
        }
        // Hold the reply past the wait's 5000 ms budget. The remaining
        // budget at attach time is always below the 5 s step ceiling
        // (preamble > 0), so the attach exchange dies at the total
        // deadline — never at the step's own budget. The late write and
        // the closed session afterwards are both expected.
        thread::sleep(Duration::from_millis(6000));
        let (attachment, epoch) = attached_identity();
        let _ = write_server_message(
            &mut &*stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                request.request_id().clone(),
                TerminalViewerOutcome::Attached {
                    attachment_id: attachment,
                    host_epoch: epoch,
                },
            )),
        );
        expect_session_eof(stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "wait-output",
            TERMINAL_ID,
            "--match",
            "x",
            "--timeout",
            "5000",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(7),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    peer.join().expect("peer finished");
    let failure = failure_kind(&output);
    assert_eq!(failure["failure"]["kind"], "timeout");
    let details = &failure["failure"]["details"];
    assert_eq!(details["stage"], "wait");
    assert!(details.get("cleanup").is_none());
    Ok(())
}
