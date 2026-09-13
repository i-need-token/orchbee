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

use orchbee_local_client_protocol::{
    LocalClientMessage, RejectionReason, read_message, write_message,
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
            fs::write(
                directory.join("cohort.json"),
                serde_json::json!({"schemaVersion": 1, "artifacts": artifacts}).to_string(),
            )
            .expect("write cohort manifest");
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
    thread::spawn(move || {
        for _ in 0..connections {
            let (stream, _) = listener.accept().expect("peer accept");
            script(stream);
        }
    })
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

/// One context, two tabs sharing the label "dup", one bound terminal.
fn fixture_document() -> &'static [u8] {
    br#"{
  "kind": "workbench.snapshot",
  "protocolVersion": 1,
  "revision": "7",
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
    assert!(text.contains("bound terminals 1"), "{text}");

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
