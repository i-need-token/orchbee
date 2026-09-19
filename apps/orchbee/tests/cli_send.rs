//! Process-level `terminal send` evidence: the real `orchbee` binary runs
//! against a scripted local peer bound at the Site's client socket. The peer
//! speaks the genuine OBLC admission, OBWP snapshot read, and OBCS
//! `terminal.send`/`terminal.send.result` exchange, so every outcome class —
//! accepted, rejected, unavailable, outcome-unknown — is exercised against
//! the production code path. Every test asserts the wire shape the CLI
//! submitted: exactly one snapshot read, then exactly one `terminal.send`
//! carrying the snapshot's Site/worker/terminal identities under a fresh
//! request identity — never a TUI controller, viewer, or resize message.
#![cfg(target_os = "linux")]

use std::{
    fs,
    io::{ErrorKind, Read},
    num::NonZeroU64,
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::Output,
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, HostTerminalId, SendAcceptedBytes, SendCleanupStatus, SendKey,
    SendRejectedReason, SendTarget, SendUnavailableReason, ServerMessage, TerminalSendOutcome,
    TerminalSendPayload, TerminalSendResult, read_client_message, write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotResult, parse_snapshot, read_workbench_request, write_workbench_response,
};
use sha2::{Digest, Sha256};

// The shared support module also carries helpers only other suites use.
#[allow(dead_code)]
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
                "obcli-send-cohort-{}-{}",
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
            fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600))
                .expect("chmod cohort manifest");
            directory
        })
        .as_path()
}

// ---- Site scope staging ---------------------------------------------------

fn base_dir(tag: &str) -> PathBuf {
    support::runtime_base(tag)
}

fn stage_scope(base: &Path) -> PathBuf {
    support::stage_scope(base)
}

// ---- scripted peer --------------------------------------------------------

fn spawn_peer(
    scope: &Path,
    script: impl Fn(UnixStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    spawn_peer_n(scope, 1, script)
}

const PEER_ACCEPT_DEADLINE: Duration = Duration::from_secs(60);
const PEER_IO_DEADLINE: Duration = Duration::from_secs(30);

fn spawn_peer_n(
    scope: &Path,
    connections: usize,
    script: impl Fn(UnixStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    let socket = scope.join("client.sock");
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    thread::spawn(move || {
        for _ in 0..connections {
            let stream = accept_bounded(&listener);
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

/// OBLC admission: read the hello, answer `ClientAccepted`, return the stream.
fn admit(stream: &UnixStream) {
    match read_message(&mut &*stream) {
        Ok(LocalClientMessage::ClientHello(_)) => {}
        other => panic!("peer expected ClientHello, got {other:?}"),
    }
    write_message(
        &mut &*stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(1).unwrap(),
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

/// Admit the session and answer the snapshot read with the fixture; returns
/// the snapshot request identity so the send's request identity can be
/// proven fresh.
fn admit_and_serve_snapshot(stream: &UnixStream) -> orchbee_workbench_protocol::WorkbenchRequestId {
    admit(stream);
    let request_id = read_snapshot_request(stream);
    write_workbench_response(
        &mut &*stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            outcome: WorkbenchSnapshotOutcome::Available {
                snapshot: parse_snapshot(fixture_document()).expect("fixture"),
            },
        }),
    )
    .expect("peer writes snapshot result");
    request_id
}

/// Read the one OBCS message the send must produce. Any other client message
/// — a TUI viewer, control, open, or a second send — fails the script.
fn read_send(stream: &UnixStream) -> orchbee_client_server_protocol::TerminalSendRequest {
    match read_client_message(&mut &*stream) {
        Ok(ClientMessage::TerminalSend(request)) => request,
        other => panic!("peer expected terminal.send, got {other:?}"),
    }
}

fn send_result(stream: &UnixStream, result: TerminalSendResult) {
    write_server_message(&mut &*stream, &ServerMessage::TerminalSendResult(result))
        .expect("peer writes send result");
}

/// After the exchange the CLI retires the session: a clean EOF proves no
/// retry, no extra attachment message, and no hidden second request.
fn expect_session_closed(stream: &UnixStream) {
    let mut byte = [0_u8; 1];
    match (&*stream).read(&mut byte) {
        Ok(0) => {}
        Ok(_) => panic!("the CLI wrote after the send exchange settled"),
        Err(error) => panic!("the CLI did not retire the send session: {error}"),
    }
}

fn run(base: &Path, arguments: &[&str]) -> Output {
    support::run(&cohort().join("orchbee"), base, arguments)
}

fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("answer is JSON")
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
// One workspace tab "ops" whose panel 31 presents the bound terminal: id 01
// on the fixture worker. `terminal send` resolves that binding's identities
// into the wire request. The standalone context mirrors the mutation suite
// so `standaloneTerminalContextId` never dangles.
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
        }
      ]
    }
  ]
}"#
}

const SITE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TERMINAL: &str = "00000000000000000000000000000001";
const WORKER: &str = "6f7263686265652d6c6f63616c2d7631";
const GENERATION: u64 = 9;
const HOST_EPOCH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HOST_TERMINAL: &str = "cccccccccccccccccccccccccccccccc";

fn target() -> SendTarget {
    SendTarget::new(
        NonZeroU64::new(GENERATION).unwrap(),
        HostEpoch::parse(HOST_EPOCH).unwrap(),
        HostTerminalId::parse(HOST_TERMINAL).unwrap(),
    )
}

/// The wire facts every send must carry: snapshot identities verbatim plus a
/// request identity that is never the snapshot read's.
fn assert_send_base(
    request: &orchbee_client_server_protocol::TerminalSendRequest,
    snapshot_request: &str,
) {
    assert_eq!(
        request.site_id().as_str(),
        SITE,
        "Site identity from snapshot"
    );
    assert_eq!(
        request.worker_id().as_str(),
        WORKER,
        "worker identity from the resolved binding"
    );
    assert_eq!(
        request.terminal_id().as_str(),
        TERMINAL,
        "terminal identity from the resolved binding"
    );
    assert_ne!(
        request.request_id().as_str(),
        snapshot_request,
        "send identity must be fresh, not the snapshot read's"
    );
}

// ---- typed outcomes --------------------------------------------------------

#[test]
fn terminal_send_text_with_enter_lands_one_record() -> TestResult {
    let base = base_dir("send-text");
    let scope = stage_scope(&base);
    let submitted = Arc::new(Mutex::new(String::new()));
    let seen = submitted.clone();
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        *seen.lock().unwrap() = request.request_id().as_str().to_owned();
        let TerminalSendPayload::Text(input) = request.payload() else {
            panic!("expected text payload, got {:?}", request.payload())
        };
        assert_eq!(input.text().as_str(), "cargo test");
        assert!(input.enter(), "--enter folds the CR into the same record");
        assert_eq!(input.record_bytes(), "cargo test".len() + 1);
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::Accepted {
                    accepted_bytes: SendAcceptedBytes::new(input.record_bytes() as u32).unwrap(),
                    cleanup: SendCleanupStatus::Confirmed,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    });
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--text",
            "cargo test",
            "--enter",
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
    assert!(
        output.stderr.is_empty(),
        "accepted+confirmed is quiet: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.terminal.send");
    assert_eq!(document["version"], 1);
    assert_eq!(document["source"]["kind"], "terminal.send");
    assert_eq!(document["source"]["siteId"], SITE);
    assert_eq!(document["source"]["snapshotRevision"], "7");
    assert_eq!(
        document["source"]["requestId"].as_str().unwrap(),
        submitted.lock().unwrap().as_str()
    );
    let result = &document["result"];
    assert_eq!(result["action"], "terminal.send");
    assert_eq!(result["siteId"], SITE);
    assert_eq!(result["workerId"], WORKER);
    assert_eq!(result["terminalId"], TERMINAL);
    assert_eq!(result["payloadKind"], "text");
    assert_eq!(result["recordBytes"], "cargo test".len() + 1);
    assert_eq!(result["inputStatus"], "accepted");
    assert_eq!(result["cleanupStatus"], "confirmed");
    assert_eq!(result["acceptedBytes"], "cargo test".len() + 1);
    // The physical target is the Server-pinned identity verbatim.
    assert_eq!(result["target"]["workerGeneration"], GENERATION.to_string());
    assert_eq!(result["target"]["hostEpoch"], HOST_EPOCH);
    assert_eq!(result["target"]["hostTerminalId"], HOST_TERMINAL);
    // Input contents never leak into the receipt.
    assert!(
        !document.to_string().contains("cargo test"),
        "literal input text must not appear in the envelope"
    );
    assert!(compiled_schema(&base, "orchbee.terminal.send").is_valid(&document));
    Ok(())
}

#[test]
fn terminal_send_key_submits_semantic_key() -> TestResult {
    let base = base_dir("send-key");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        let TerminalSendPayload::Key { key } = request.payload() else {
            panic!("expected key payload, got {:?}", request.payload())
        };
        assert_eq!(*key, SendKey::CtrlC);
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::Accepted {
                    accepted_bytes: SendAcceptedBytes::new(1).unwrap(),
                    cleanup: SendCleanupStatus::Unconfirmed,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    });
    let output = run(
        &base,
        &[
            "terminal", "send", TERMINAL, "--key", "ctrl-c", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    // Accepted input stays accepted even when cleanup is unconfirmed; the
    // nonzero exit plus the do-not-resend warning keep both axes truthful.
    assert_eq!(
        output.status.code(),
        Some(5),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Do not resend"),
        "cleanup-unconfirmed warning: {stderr}"
    );
    let document = envelope(&output);
    let result = &document["result"];
    assert_eq!(result["payloadKind"], "key");
    assert_eq!(result["key"], "ctrl-c");
    assert_eq!(result["inputStatus"], "accepted");
    assert_eq!(result["cleanupStatus"], "unconfirmed");
    assert_eq!(result["acceptedBytes"], 1);
    assert!(result.get("recordBytes").is_none());
    assert!(compiled_schema(&base, "orchbee.terminal.send").is_valid(&document));
    Ok(())
}

#[test]
fn terminal_send_rejected_reports_typed_refusal() -> TestResult {
    let base = base_dir("send-rejected");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::Rejected {
                    reason: SendRejectedReason::HostInputBackpressure,
                    cleanup: SendCleanupStatus::Confirmed,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    });
    let output = run(
        &base,
        &[
            "terminal", "send", TERMINAL, "--text", "x", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    let result = &document["result"];
    assert_eq!(result["inputStatus"], "rejected");
    assert_eq!(result["reason"], "host-input-backpressure");
    assert_eq!(result["cleanupStatus"], "confirmed");
    assert!(compiled_schema(&base, "orchbee.terminal.send").is_valid(&document));
    Ok(())
}

#[test]
fn terminal_send_unavailable_reports_not_submitted() -> TestResult {
    let base = base_dir("send-unavail");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        // A pre-admission refusal: no target was ever pinned.
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                None,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailableReason::WorkerUnavailable,
                    cleanup: SendCleanupStatus::NotNeeded,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    });
    let output = run(
        &base,
        &[
            "terminal", "send", TERMINAL, "--text", "x", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let document = envelope(&output);
    let result = &document["result"];
    assert_eq!(result["inputStatus"], "not-submitted");
    assert_eq!(result["reason"], "worker-unavailable");
    assert_eq!(result["cleanupStatus"], "not-needed");
    assert!(result.get("target").is_none());
    assert!(compiled_schema(&base, "orchbee.terminal.send").is_valid(&document));
    Ok(())
}

/// `HostProtocolFault` is a transport-class defect, not service
/// unavailability: the same not-submitted verdict exits 5 instead of 4.
#[test]
fn terminal_send_host_protocol_fault_exits_transport() -> TestResult {
    let base = base_dir("send-proto-fault");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                None,
                TerminalSendOutcome::Unavailable {
                    reason: SendUnavailableReason::HostProtocolFault,
                    cleanup: SendCleanupStatus::NotNeeded,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    });
    let output = run(
        &base,
        &[
            "terminal", "send", TERMINAL, "--text", "x", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let document = envelope(&output);
    let result = &document["result"];
    assert_eq!(result["inputStatus"], "not-submitted");
    assert_eq!(result["reason"], "host-protocol-fault");
    assert_eq!(result["cleanupStatus"], "not-needed");
    assert!(compiled_schema(&base, "orchbee.terminal.send").is_valid(&document));
    Ok(())
}

#[test]
fn terminal_send_unknown_outcome_never_retries() -> TestResult {
    let base = base_dir("send-unknown");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::OutcomeUnknown {
                    cleanup: SendCleanupStatus::Unconfirmed,
                },
            )
            .unwrap(),
        );
        // The CLI must retire the session — never replay the send.
        expect_session_closed(&stream);
    });
    let output = run(
        &base,
        &[
            "terminal", "send", TERMINAL, "--key", "enter", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(8));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("outcome is unknown") || stderr.contains("unknown"),
        "unknown-outcome guidance: {stderr}"
    );
    assert!(
        stderr.contains("not retried") || stderr.contains("before deciding whether to resend"),
        "do-not-resend guidance: {stderr}"
    );
    let document = envelope(&output);
    let result = &document["result"];
    assert_eq!(result["inputStatus"], "unknown");
    assert_eq!(result["cleanupStatus"], "unconfirmed");
    assert!(compiled_schema(&base, "orchbee.terminal.send").is_valid(&document));
    Ok(())
}

/// A peer that accepts the send and then goes silent: the shared exchange
/// budget retires the session and the CLI reports outcome-unknown once —
/// a silent peer never provokes a second request.
#[test]
fn terminal_send_stalled_answer_exits_unknown_once() -> TestResult {
    let base = base_dir("send-stall");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        // Never answer: the CLI's own deadline must retire the session, then
        // close it rather than replaying the input.
        let mut byte = [0_u8; 1];
        match (&stream).read(&mut byte) {
            Ok(0) => {}
            Ok(_) => panic!("the CLI retried a send after the exchange stalled"),
            Err(error) => panic!("the CLI held the stalled session open: {error}"),
        }
    });
    let output = run(
        &base,
        &[
            "terminal", "send", TERMINAL, "--text", "x", "--format", "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(8),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure");
    assert_eq!(document["failure"]["kind"], "outcome-unknown");
    assert_eq!(document["failure"]["details"]["stage"], "send");
    assert_eq!(document["source"]["kind"], "terminal.send");
    assert!(compiled_schema(&base, "orchbee.cli.failure").is_valid(&document));
    Ok(())
}

#[test]
fn terminal_send_text_format_renders_result_fields() -> TestResult {
    let base = base_dir("send-text-fmt");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::Accepted {
                    accepted_bytes: SendAcceptedBytes::new(2).unwrap(),
                    cleanup: SendCleanupStatus::Confirmed,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    });
    let output = run(&base, &["terminal", "send", TERMINAL, "--text", "hi"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for field in [
        "action",
        "requestId",
        "siteId",
        "workerId",
        "terminalId",
        "payloadKind",
        "recordBytes",
        "inputStatus",
        "cleanupStatus",
        "acceptedBytes",
        "target",
        "workerGeneration",
        "hostEpoch",
        "hostTerminalId",
    ] {
        assert!(
            stdout.contains(field),
            "text output lacks {field}: {stdout}"
        );
    }
    assert!(stdout.contains("accepted"));
    assert!(stdout.contains("confirmed"));
    Ok(())
}

#[test]
fn terminal_send_missing_binding_is_not_found() -> TestResult {
    let base = base_dir("send-missing");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, move |stream| {
        let _snapshot_request = admit_and_serve_snapshot(&stream);
        // Resolution failed from the snapshot: no send may follow. The
        // session closes without a single OBCS write.
        let mut byte = [0_u8; 1];
        match (&stream).read(&mut byte) {
            Ok(0) => {}
            Ok(_) => panic!("the CLI wrote a send for an unbound terminal"),
            Err(error) => panic!("the CLI held the session open: {error}"),
        }
    });
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            "0000000000000000000000000000ffff",
            "--text",
            "x",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure");
    assert_eq!(document["failure"]["kind"], "not-found");
    Ok(())
}

/// Parse-invalid argv exits 2 before any connection is attempted — with no
/// socket staged at all, transport would fail differently if it ever ran.
#[test]
fn terminal_send_invalid_argv_never_connects() -> TestResult {
    let base = base_dir("send-argv");
    for (arguments, name) in [
        (&["terminal", "send", TERMINAL][..], "missing payload"),
        (
            &[
                "terminal", "send", TERMINAL, "--text", "x", "--key", "enter",
            ][..],
            "conflicting payload",
        ),
        (
            &["terminal", "send", TERMINAL, "--enter"][..],
            "enter without text",
        ),
        (
            &["terminal", "send", TERMINAL, "--key", "ctrl-x"][..],
            "unknown key",
        ),
        (
            &["terminal", "send", "not-an-id", "--text", "x"][..],
            "label target",
        ),
    ] {
        let output = run(&base, arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
