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
    io::{ErrorKind, Read, Seek, SeekFrom},
    num::NonZeroU64,
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use orchbee_client_server_protocol::{
    ClientMessage, HostEpoch, HostTerminalId, SendCleanupStatus, SendKey, SendProgress, SendRecord,
    SendRejectedReason, SendTarget, SendUnavailableReason, ServerMessage, TerminalSendOutcome,
    TerminalSendResult, read_client_message, write_server_message,
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
            // A controlled dummy digest stands in for the adapters payload —
            // this fixture stages no sidecar; admission stays lazy.
            artifacts.insert(
                "agent-observation-adapters".to_owned(),
                format!("{:x}", Sha256::digest(b"staged stand-in adapters\n")).into(),
            );
            let manifest = directory.join("cohort.json");
            fs::write(
                &manifest,
                serde_json::json!({"schemaVersion": 2, "artifacts": artifacts}).to_string(),
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
        let [SendRecord::Text(input)] = request.records().as_slice() else {
            panic!("expected a single text record, got {:?}", request.records())
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
                    progress: SendProgress::complete(1, input.record_bytes() as u64),
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
    assert_eq!(result["acceptedEncodedBytes"], "cargo test".len() + 1);
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
        let [SendRecord::Key { key }] = request.records().as_slice() else {
            panic!("expected a single key record, got {:?}", request.records())
        };
        assert_eq!(*key, SendKey::CtrlC);
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::Accepted {
                    progress: SendProgress::complete(1, 1),
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
    assert_eq!(result["records"], 1);
    assert_eq!(result["inputStatus"], "accepted");
    assert_eq!(result["cleanupStatus"], "unconfirmed");
    assert_eq!(result["acceptedEncodedBytes"], 1);
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
                    progress: SendProgress::none(),
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
                    progress: SendProgress::none(),
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
                    progress: SendProgress::none(),
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
                    progress: SendProgress::none(),
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
                    progress: SendProgress::complete(1, 2),
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
        "acceptedEncodedBytes",
        "records",
        "recordsAccepted",
        "firstStoppedRecord",
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
            &[
                "terminal",
                "send",
                TERMINAL,
                "--text",
                "x",
                "--file",
                "/etc/hostname",
            ][..],
            "text-file conflict",
        ),
        (
            &[
                "terminal",
                "send",
                TERMINAL,
                "--file",
                "/etc/hostname",
                "--stdin",
            ][..],
            "file-stdin conflict",
        ),
        (
            &["terminal", "send", TERMINAL, "--key", "enter", "--stdin"][..],
            "key-stdin conflict",
        ),
        (
            &["terminal", "send", TERMINAL, "--enter"][..],
            "enter without text",
        ),
        (
            &["terminal", "send", TERMINAL, "--key", "enter", "--enter"][..],
            "enter on key source",
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

// ---- declared input sources -------------------------------------------------
//
// `--file` and `--stdin` materialize one bounded text record before any
// runtime effect; a source failure therefore proves zero Server contact —
// the scripted listener below never accepts — while a valid source produces
// exactly the same single `terminal.send` wire request `--text` does.

/// A spawn with caller-owned stdin (the shared `run` always closes it).
fn run_with_stdin(base: &Path, arguments: &[&str], stdin: Stdio) -> std::process::Child {
    Command::new(cohort().join("orchbee"))
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cli child")
}

fn wait(child: std::process::Child) -> Output {
    support::bounded_wait_with_output(child, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("{timeout}"))
}

/// A staged file under the test base, returning its path.
fn staged_file(base: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = base.join(name);
    fs::write(&path, bytes).expect("write staged source file");
    path
}

/// The scripted exchange a materialized source must produce: the same one
/// snapshot read plus one `terminal.send` carrying the folded text.
fn serve_send(
    scope: &Path,
    expected_text: String,
    expected_bytes: usize,
    enter: bool,
) -> thread::JoinHandle<()> {
    spawn_peer(scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        let [SendRecord::Text(input)] = request.records().as_slice() else {
            panic!("expected a single text record, got {:?}", request.records())
        };
        assert_eq!(input.text().as_str(), expected_text);
        assert_eq!(input.enter(), enter);
        assert_eq!(input.record_bytes(), expected_bytes);
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::Accepted {
                    progress: SendProgress::complete(1, input.record_bytes() as u64),
                    cleanup: SendCleanupStatus::Confirmed,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    })
}

/// The same scripted exchange for content routed to an opaque `bytes`
/// record: the wire payload is the literal bytes plus any folded CR.
fn serve_bytes(scope: &Path, expected: Vec<u8>, enter: bool) -> thread::JoinHandle<()> {
    spawn_peer(scope, move |stream| {
        let snapshot_request = admit_and_serve_snapshot(&stream);
        let request = read_send(&stream);
        assert_send_base(&request, snapshot_request.as_str());
        let [SendRecord::Bytes { bytes }] = request.records().as_slice() else {
            panic!(
                "expected a single bytes record, got {:?}",
                request.records()
            )
        };
        let mut expected_wire = expected.clone();
        if enter {
            expected_wire.push(b'\r');
        }
        assert_eq!(bytes.as_bytes(), expected_wire.as_slice());
        send_result(
            &stream,
            TerminalSendResult::new(
                request.request_id().clone(),
                Some(target()),
                TerminalSendOutcome::Accepted {
                    progress: SendProgress::complete(1, bytes.as_bytes().len() as u64),
                    cleanup: SendCleanupStatus::Confirmed,
                },
            )
            .unwrap(),
        );
        expect_session_closed(&stream);
    })
}

#[test]
fn terminal_send_file_lands_one_record() -> TestResult {
    let base = base_dir("send-file");
    let scope = stage_scope(&base);
    let path = staged_file(&base, "input.txt", b"deploy the release\n");
    let peer = serve_send(&scope, "deploy the release\n".to_owned(), 19, false);
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--file",
            path.to_str().unwrap(),
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
    assert_eq!(document["schema"], "orchbee.terminal.send");
    assert_eq!(document["result"]["payloadKind"], "text");
    assert_eq!(document["result"]["recordBytes"], 19);
    assert_eq!(document["result"]["inputStatus"], "accepted");
    Ok(())
}

#[test]
fn terminal_send_file_with_enter_folds_inside_limit() -> TestResult {
    let base = base_dir("send-file-enter");
    let scope = stage_scope(&base);
    let path = staged_file(&base, "max.txt", &vec![b'a'; 4095]);
    // --enter survives materialization as the wire fold flag: the wire text
    // is the file bytes verbatim and the record counts text + folded CR.
    let peer = serve_send(&scope, "a".repeat(4095), 4096, true);
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--file",
            path.to_str().unwrap(),
            "--enter",
            "--format",
            "json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(envelope(&output)["result"]["recordBytes"], 4096);
    Ok(())
}

#[test]
fn terminal_send_stdin_pipe_lands_one_record() -> TestResult {
    let base = base_dir("send-stdin");
    let scope = stage_scope(&base);
    let peer = serve_send(&scope, "echo from stdin".to_owned(), 15, false);
    let mut child = run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::piped(),
    );
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin
            .write_all(b"echo from stdin")
            .expect("write stdin bytes");
    } // dropping the write end delivers EOF
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(envelope(&output)["result"]["recordBytes"], 15);
    Ok(())
}

#[test]
fn terminal_send_stdin_regular_file_inherits_offset() -> TestResult {
    let base = base_dir("send-stdin-offset");
    let scope = stage_scope(&base);
    // The inherited file position is honored in place: seeking the shared
    // description past "SKIP" leaves only the tail for the record.
    let path = staged_file(&base, "stdin-file", b"SKIPlanded from offset\n");
    let mut file = fs::File::open(&path).expect("open stdin file");
    file.seek(SeekFrom::Start(4)).expect("seek shared offset");
    let peer = serve_send(&scope, "landed from offset\n".to_owned(), 19, false);
    let child = run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::from(file),
    );
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(envelope(&output)["result"]["recordBytes"], 19);
    Ok(())
}

/// A source-stage validation/local failure exits 2 on the standard
/// `invalid-usage` envelope and provably never reaches the runtime: the
/// staged listener observes no connection.
#[test]
fn terminal_send_source_failures_have_zero_runtime_effect() -> TestResult {
    let base = base_dir("send-source-usage");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let oversized = staged_file(&base, "big.txt", &vec![b'b'; 1024 * 1024 + 1]);
    let empty = staged_file(&base, "empty.txt", b"");
    let cases: [(Vec<String>, &'static str); 4] = [
        (
            vec!["--file".into(), base.join("missing").display().to_string()],
            "missing file",
        ),
        (
            vec!["--file".into(), oversized.display().to_string()],
            "oversize file",
        ),
        (
            vec!["--file".into(), empty.display().to_string()],
            "empty file",
        ),
        (
            vec!["--file".into(), scope.display().to_string()],
            "directory path",
        ),
    ];
    for (source_args, name) in cases {
        let mut arguments = vec!["terminal", "send", TERMINAL];
        arguments.extend(source_args.iter().map(String::as_str));
        arguments.extend(["--format", "json"]);
        let output = run(&base, &arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document = envelope(&output);
        assert_eq!(document["schema"], "orchbee.cli.failure", "{name}");
        assert_eq!(document["failure"]["kind"], "invalid-usage", "{name}");
    }
    // No staged connection may ever have been attempted.
    let deadline = Instant::now() + Duration::from_millis(400);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok(_) => panic!("a source-stage failure must never reach the runtime"),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    }
    Ok(())
}

/// A hung stdin (held-open pipe, no data) fails at the shared 5s deadline
/// with the typed source envelope — and never touches the runtime.
#[test]
fn terminal_send_stdin_timeout_is_typed_and_effectless() -> TestResult {
    let base = base_dir("send-stdin-timeout");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let child = run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::piped(), // held open, never written
    );
    let started = Instant::now();
    let output = wait(child);
    let elapsed = started.elapsed();
    assert_eq!(
        output.status.code(),
        Some(7),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        elapsed >= Duration::from_secs(5),
        "the deadline must not fire early: {elapsed:?}"
    );
    let document = envelope(&output);
    compiled_schema(&base, "orchbee.cli.failure.source")
        .validate(&document)
        .expect("source failure validates against its schema");
    assert_eq!(document["schema"], "orchbee.cli.failure.source");
    assert_eq!(document["failure"]["kind"], "timeout");
    assert_eq!(document["failure"]["details"]["stage"], "input");
    assert_eq!(document["failure"]["details"]["inputSource"], "stdin");
    assert_eq!(document["failure"]["details"]["timeoutMs"], 5000);
    // The human diagnosis names the input read — never a wait or a match.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not be read within the time limit"),
        "input-stage timeout diagnoses the read: {stderr}"
    );
    assert!(
        !stderr.contains("wait") && !stderr.contains("match"),
        "no wait or match claim survives into the input stage: {stderr}"
    );
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        _ => panic!("a timed-out source must never reach the runtime"),
    }
    // The text projection routes the same diagnosis with empty stdout.
    let child = run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "text"],
        Stdio::piped(),
    );
    let output = wait(child);
    assert_eq!(output.status.code(), Some(7));
    assert!(output.stdout.is_empty(), "text source failure is silent");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("could not be read within the time limit")
    );
    Ok(())
}

/// SIGINT inside the source interval is a typed cancellation: exit 130 with
/// the source envelope, zero runtime contact.
#[test]
fn terminal_send_stdin_sigint_is_typed_cancellation() -> TestResult {
    let base = base_dir("send-stdin-int");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let child = run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::piped(),
    );
    thread::sleep(Duration::from_millis(1500));
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGINT,
    )
    .expect("signal child");
    let output = wait(child);
    // 130 is an *exit status*: a raw signal death would report signal, not
    // code — this proves the typed cancellation path emitted the envelope.
    assert_eq!(
        output.status.code(),
        Some(130),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    compiled_schema(&base, "orchbee.cli.failure.source")
        .validate(&document)
        .expect("source failure validates against its schema");
    assert_eq!(document["failure"]["kind"], "cancelled");
    assert_eq!(document["failure"]["details"]["stage"], "input");
    assert_eq!(document["failure"]["details"]["inputSource"], "stdin");
    assert!(
        document["failure"]["details"].get("timeoutMs").is_none(),
        "a cancellation carries no timeout detail"
    );
    // The human diagnosis names the cancelled read — never a wait's match.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Reading the declared input was cancelled"),
        "input-stage cancellation diagnoses the read: {stderr}"
    );
    assert!(
        !stderr.contains("wait") && !stderr.contains("match"),
        "no wait or match claim survives into the input stage: {stderr}"
    );
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        _ => panic!("a cancelled source must never reach the runtime"),
    }
    Ok(())
}

/// A signal the process inherited as ignored is excluded from capture:
/// SIGINT does nothing to a held-open source, while the still-eligible
/// SIGTERM cancels it at 143.
#[test]
fn terminal_send_stdin_inherited_ignore_excluded() -> TestResult {
    let base = base_dir("send-stdin-ign");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let _listener = UnixListener::bind(&socket).expect("bind client socket");
    let binary = cohort().join("orchbee");
    // `trap '' INT` installs SIG_IGN for SIGINT, which `exec` preserves —
    // the CLI inherits an ignored SIGINT, exactly the eligibility input the
    // guard must respect.
    let mut command = Command::new("sh");
    command
        .args(["-c", "trap '' INT; exec \"$@\"", "sh"])
        .arg(&binary)
        .args(["terminal", "send", TERMINAL, "--stdin", "--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", &base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().expect("spawn sh wrapper");
    let pid = nix::unistd::Pid::from_raw(child.id() as i32);
    thread::sleep(Duration::from_millis(1500));
    nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGINT)
        .expect("send inherited-ignored SIGINT");
    thread::sleep(Duration::from_millis(400));
    // A null-signal probe: the child must still be running — an ignored
    // SIGINT is excluded from capture and ends nothing.
    assert!(
        nix::sys::signal::kill(pid, None::<nix::sys::signal::Signal>).is_ok(),
        "an inherited-ignored SIGINT must not cancel the source stage"
    );
    nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM).expect("send SIGTERM");
    let output = wait(child);
    assert_eq!(
        output.status.code(),
        Some(143),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["failure"]["kind"], "cancelled");
    assert_eq!(document["failure"]["details"]["inputSource"], "stdin");
    Ok(())
}

/// stdin declared on an unsupported descriptor type is a source-stage
/// rejection: a socket and a character device both exit 2 on the standard
/// `invalid-usage` envelope without a single read, and provably never
/// reach the runtime.
#[test]
fn terminal_send_stdin_unsupported_streams() -> TestResult {
    let base = base_dir("send-stdin-unsupported");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    // A unix socket end as the inherited stdin — S_IFSOCK is rejected by
    // the exact S_IFMT classification.
    let (socket_end, _peer_end) = UnixStream::pair().expect("socketpair");
    let output = wait(run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::from(std::os::fd::OwnedFd::from(socket_end)),
    ));
    assert_eq!(
        output.status.code(),
        Some(2),
        "socket stdin: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure");
    assert_eq!(document["failure"]["kind"], "invalid-usage");
    // A character device as the inherited stdin — S_IFCHR is rejected the
    // same way.
    let null = fs::File::open("/dev/null").expect("open /dev/null");
    let output = wait(run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::from(null),
    ));
    assert_eq!(
        output.status.code(),
        Some(2),
        "char-device stdin: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(envelope(&output)["failure"]["kind"], "invalid-usage");
    // No staged connection may ever have been attempted.
    let deadline = Instant::now() + Duration::from_millis(400);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok(_) => panic!("an unsupported stream must never reach the runtime"),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    }
    Ok(())
}

/// An oversized regular-file stdin is bounded by real consumption: the
/// shared file position advances by at most the cap+1 probe bytes from the
/// inherited offset, and the descriptor's status flags are untouched.
#[test]
fn terminal_send_stdin_oversize_bounds_consumption() -> TestResult {
    let base = base_dir("send-stdin-cap");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    // 1 MiB + 4 bytes remain past the inherited offset: a buffered or
    // unbounded reader would consume far beyond the 1 MiB + 1 probe.
    let path = staged_file(&base, "big-stdin", &vec![b'z'; 1024 * 1024 + 8]);
    let mut file = fs::File::open(&path).expect("open stdin file");
    file.seek(SeekFrom::Start(4)).expect("seek shared offset");
    let mut observer = file.try_clone().expect("same-description observer");
    let flags_before =
        nix::fcntl::fcntl(&observer, nix::fcntl::FcntlArg::F_GETFL).expect("flags before spawn");
    let output = wait(run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::from(file),
    ));
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(envelope(&output)["failure"]["kind"], "invalid-usage");
    let advanced = observer.stream_position().expect("shared offset") - 4;
    assert!(
        advanced <= 1024 * 1024 + 1,
        "consumption must stop at the cap probe, advanced {advanced} bytes"
    );
    let flags_after =
        nix::fcntl::fcntl(&observer, nix::fcntl::FcntlArg::F_GETFL).expect("flags after wait");
    assert_eq!(
        flags_before, flags_after,
        "the inherited descriptor's status flags must be preserved"
    );
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        _ => panic!("an oversized source must never reach the runtime"),
    }
    Ok(())
}

/// Multibyte UTF-8 counts bytes, not scalars: `中` is three bytes, so the
/// 4,096 record bound lands mid-character for some lengths. Below the
/// bound the bytes arrive verbatim; at LIMIT+1 the source is refused even
/// when the whole content is valid UTF-8.
#[test]
fn terminal_send_file_utf8_multibyte_boundaries() -> TestResult {
    let base = base_dir("send-file-utf8");
    let scope = stage_scope(&base);
    // 1,365 × "中" = 4,095 bytes: --enter's folded CR completes a 4,096
    // record; the wire text is the multibyte content verbatim.
    let folded = "中".repeat(1365);
    let path = staged_file(&base, "m4095", folded.as_bytes());
    let peer = serve_send(&scope, folded, 4096, true);
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--file",
            path.to_str().unwrap(),
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
    assert_eq!(envelope(&output)["result"]["recordBytes"], 4096);
    // 4,094 ASCII bytes + one "é" = exactly 4,096 bytes: the multibyte
    // scalar's last byte lands on the bound and is kept verbatim.
    let exact = format!("{}é", "a".repeat(4094));
    assert_eq!(exact.len(), 4096);
    let path = staged_file(&base, "m4096", exact.as_bytes());
    let peer = serve_send(&scope, exact, 4096, false);
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--file",
            path.to_str().unwrap(),
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
    assert_eq!(envelope(&output)["result"]["recordBytes"], 4096);
    // 4,095 ASCII bytes + one "é" = 4,097 bytes: valid UTF-8 that crosses
    // the text-record bound — routed to a literal bytes record, kept whole.
    let over = format!("{}é", "a".repeat(4095));
    assert_eq!(over.len(), 4097);
    let path = staged_file(&base, "m4097", over.as_bytes());
    let bytes_bytes = over.len();
    let peer = serve_bytes(&scope, over.clone().into_bytes(), false);
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--file",
            path.to_str().unwrap(),
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
    assert_eq!(document["result"]["payloadKind"], "bytes");
    assert_eq!(document["result"]["recordBytes"], bytes_bytes);
    Ok(())
}

/// An inherited terminal as stdin is refused before a single read or any
/// runtime contact: `script -e` seats the CLI's stdio on a real pty slave
/// and propagates its exit status.
#[test]
fn terminal_send_stdin_tty_is_refused() -> TestResult {
    let base = base_dir("send-stdin-tty");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let binary = cohort().join("orchbee");
    let output = Command::new("script")
        .args([
            "-qec",
            &format!(
                "exec '{}' terminal send {} --stdin --format json",
                binary.display(),
                TERMINAL
            ),
            "/dev/null",
        ])
        .env("ORCHBEE_RUNTIME_BASE", &base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn script pty wrapper");
    let output = wait(output);
    assert_eq!(
        output.status.code(),
        Some(2),
        "pty wrapper: out={}",
        String::from_utf8_lossy(&output.stdout)
    );
    // The pty merges the child's stderr into the wrapper's stdout: the
    // localized refusal must name the terminal cause.
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("is a terminal"),
        "pty output must carry the terminal refusal"
    );
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        _ => panic!("a terminal stdin must never reach the runtime"),
    }
    Ok(())
}

/// `--file -` is a literal filesystem path, never an alias for standard
/// input: a file actually named `-` in the child's working directory is
/// the source that gets read.
#[test]
fn terminal_send_file_dash_is_a_literal_path() -> TestResult {
    let base = base_dir("send-file-dash");
    let scope = stage_scope(&base);
    let workdir = base.join("cwd");
    fs::create_dir(&workdir).expect("create literal cwd");
    staged_file(&workdir, "-", b"from the dash file\n");
    let peer = serve_send(&scope, "from the dash file\n".to_owned(), 19, false);
    let child = Command::new(cohort().join("orchbee"))
        .args([
            "terminal", "send", TERMINAL, "--file", "-", "--format", "json",
        ])
        .current_dir(&workdir)
        .env("ORCHBEE_RUNTIME_BASE", &base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cli child");
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(envelope(&output)["result"]["recordBytes"], 19);
    Ok(())
}

/// A `--file` symlink is resolved to its target's real type: a link to a
/// regular file reads fine, while a link to a writer-less FIFO is refused
/// promptly — the nonblocking open must not hang on the FIFO.
#[test]
fn terminal_send_file_symlink_dispatch() -> TestResult {
    let base = base_dir("send-file-symlink");
    let scope = stage_scope(&base);
    // symlink → regular file: accepted, content verbatim.
    let real = staged_file(&base, "real.txt", b"via symlink\n");
    let link = base.join("link.txt");
    std::os::unix::fs::symlink(&real, &link).expect("symlink to regular file");
    let peer = serve_send(&scope, "via symlink\n".to_owned(), 12, false);
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--file",
            link.to_str().unwrap(),
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
    assert_eq!(envelope(&output)["result"]["recordBytes"], 12);
    // symlink → FIFO with no writer: a blocking open would hang forever;
    // the nonblocking open reports the target's true type and refuses.
    let fifo = base.join("fifo-target");
    nix::unistd::mkfifo(
        &fifo,
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )
    .expect("mkfifo target");
    let fifo_link = base.join("fifo-link");
    std::os::unix::fs::symlink(&fifo, &fifo_link).expect("symlink to fifo");
    let started = Instant::now();
    let output = run(
        &base,
        &[
            "terminal",
            "send",
            TERMINAL,
            "--file",
            fifo_link.to_str().unwrap(),
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(envelope(&output)["failure"]["kind"], "invalid-usage");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the FIFO open must refuse promptly, not block"
    );
    Ok(())
}

/// The signal the spawned CLI will evaluate for eligibility: if the test
/// process itself inherited it ignored or blocked, the child cannot
/// capture it — report that environment honestly instead of claiming
/// evidence the run never produced.
fn require_eligible(signal: nix::sys::signal::Signal) {
    let bit = 1_u64 << (signal as u32 - 1);
    let status = fs::read_to_string("/proc/self/status").expect("read own SigIgn");
    let ignored = status
        .lines()
        .find_map(|line| {
            line.strip_prefix("SigIgn:")
                .and_then(|value| u64::from_str_radix(value.trim(), 16).ok())
        })
        .expect("SigIgn field");
    assert_eq!(
        ignored & bit,
        0,
        "test environment inherited {signal:?} as ignored — the capture precondition is unmet"
    );
    let mask = nix::sys::signal::SigSet::thread_get_mask().expect("thread mask");
    assert!(
        !mask.contains(signal),
        "test environment blocks {signal:?} — the capture precondition is unmet"
    );
}

/// SIGHUP inside the source interval is the same typed cancellation as
/// SIGINT: exit 129 with the source envelope, zero runtime contact.
#[test]
fn terminal_send_stdin_sighup_is_typed_cancellation() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGHUP);
    let base = base_dir("send-stdin-hup");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let child = run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::piped(),
    );
    thread::sleep(Duration::from_millis(1500));
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGHUP,
    )
    .expect("signal child");
    let output = wait(child);
    // A raw SIGHUP death would report a signal, never an exit code — 129
    // proves the typed cancellation path emitted the envelope itself.
    assert_eq!(
        output.status.code(),
        Some(129),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    compiled_schema(&base, "orchbee.cli.failure.source")
        .validate(&document)
        .expect("source failure validates against its schema");
    assert_eq!(document["failure"]["kind"], "cancelled");
    assert_eq!(document["failure"]["details"]["stage"], "input");
    assert_eq!(document["failure"]["details"]["inputSource"], "stdin");
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        _ => panic!("a cancelled source must never reach the runtime"),
    }
    Ok(())
}

/// SIGQUIT inside the source interval cancels at 131: the guarded set is
/// wider than the wait stage's INT/TERM pair, and every member produces
/// the conventional `128 + signo` status.
#[test]
fn terminal_send_stdin_sigquit_is_typed_cancellation() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGQUIT);
    let base = base_dir("send-stdin-quit");
    let scope = stage_scope(&base);
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    listener
        .set_nonblocking(true)
        .expect("listener nonblocking");
    let child = run_with_stdin(
        &base,
        &["terminal", "send", TERMINAL, "--stdin", "--format", "json"],
        Stdio::piped(),
    );
    thread::sleep(Duration::from_millis(1500));
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id() as i32),
        nix::sys::signal::Signal::SIGQUIT,
    )
    .expect("signal child");
    let output = wait(child);
    assert_eq!(
        output.status.code(),
        Some(131),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = envelope(&output);
    compiled_schema(&base, "orchbee.cli.failure.source")
        .validate(&document)
        .expect("source failure validates against its schema");
    assert_eq!(document["failure"]["kind"], "cancelled");
    assert_eq!(document["failure"]["details"]["stage"], "input");
    assert_eq!(document["failure"]["details"]["inputSource"], "stdin");
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        _ => panic!("a cancelled source must never reach the runtime"),
    }
    Ok(())
}
