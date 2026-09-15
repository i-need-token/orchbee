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
    process::{Command, Output},
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};

use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, ServerMessage, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, TerminalViewerResult, ViewerId, ViewerRejectedReason,
    ViewerSnapshotReason, ViewerUnavailableReason, read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{
    LocalClientMessage, RejectionReason, read_message, write_message,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, ProjectionBlockReason,
    ProjectionBlocked, ProjectionDemandControl, decode_terminal_text_frame,
    encode_projection_demand_control, encode_transport_envelope, inspect_terminal_frame_route,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, WorkbenchSnapshotUnavailableReason, parse_snapshot,
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
            // NativeCohort rejects a group/other-accessible manifest; the
            // process umask is not guaranteed to be 022.
            fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600))
                .expect("chmod cohort manifest");
            directory
        })
        .as_path()
}

// ---- Site scope staging ---------------------------------------------------

fn base_dir(tag: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "obcli-query-base-{}-{tag}-{}",
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

fn run(base: &Path, arguments: &[&str]) -> Output {
    Command::new(cohort().join("orchbee"))
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .output()
        .expect("spawn staged orchbee")
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
    let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args(["status", "--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", &base)
        .output()?;
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
    // socketEntry: exactly the four (presence,status,reason) combinations.
    // Both entries and the aggregate status are set together so the aggregate
    // binding stays honest and only the entry shape is under test.
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
    let output = Command::new(cohort().join("orchbee"))
        .args(["api", "schema", "orchbee.status", "--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", &base)
        .env("LANG", "en_US.UTF-8")
        .stdout(std::process::Stdio::from(dev_full))
        .stderr(std::process::Stdio::piped())
        .spawn()?
        .wait_with_output()?;
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
    let output = Command::new("bash")
        .args(["-c", &harness])
        .output()
        .expect("bash runs");
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
    let output = Command::new("zsh")
        .args(["-f", "-c", &harness])
        .output()
        .expect("zsh runs");
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
            ["get", "list", "--format", "--format="]
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
        sorted(vec!["get".to_owned(), "list".to_owned()]),
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
            ["list", "--format", "--format="]
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
