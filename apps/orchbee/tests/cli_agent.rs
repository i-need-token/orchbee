//! Process-level Agent read evidence: the real `orchbee` binary runs against
//! a scripted local peer bound at the Site's client socket. The peer speaks
//! the genuine OBLC admission and the OBAP record protocol, so every admitted
//! outcome class — listed/found, semantic refusal, unavailable, malformed,
//! correlation mismatch — is exercised against the production code path.
#![cfg(target_os = "linux")]

use std::{
    fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener, UnixStream},
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{OnceLock, mpsc::Receiver, mpsc::sync_channel},
    thread,
    time::{Duration, Instant},
};

use orchbee_agent_observation_protocol::{
    AgentReadRequest, AgentReadResult, read_agent_request, write_agent_result,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
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
                "obcli-agent-cohort-{}-{}",
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

fn base_dir(tag: &str) -> PathBuf {
    support::runtime_base(tag)
}

fn stage_scope(base: &Path) -> PathBuf {
    support::stage_scope(base)
}

// ---- scripted peer --------------------------------------------------------

/// Bind `client.sock` inside the scope and run `script` on each of
/// `connections` sequential admitted sessions.
fn spawn_peer(
    scope: &Path,
    connections: usize,
    script: impl Fn(UnixStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    let socket = scope.join("client.sock");
    let listener = UnixListener::bind(&socket).expect("bind client socket");
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
        .set_nonblocking(true)
        .expect("peer listener nonblocking");
    thread::spawn(move || {
        for _ in 0..connections {
            let deadline = Instant::now() + Duration::from_secs(30);
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "peer accept timed out waiting for the CLI to connect"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("peer accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .expect("peer read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(30)))
                .expect("peer write timeout");
            script(stream);
        }
    })
}

/// OBLC admission: read the hello, answer `ClientAccepted`.
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

/// Read the OBAP request record off the admitted session.
fn read_request(stream: &UnixStream) -> AgentReadRequest {
    read_agent_request(&mut &*stream).expect("peer reads one OBAP request")
}

/// Build a typed result document from its wire JSON and frame it back; the
/// fixture itself passes through the same strict decoder the CLI trusts, so
/// a malformed fixture fails in the test rather than on the wire.
fn answer(stream: &UnixStream, document: serde_json::Value) {
    let result: AgentReadResult =
        serde_json::from_value(document).expect("fixture decodes as an AgentReadResult");
    write_agent_result(&mut &*stream, &result).expect("peer writes OBAP result");
}

fn run(base: &Path, arguments: &[&str]) -> Output {
    support::run(&cohort().join("orchbee"), base, arguments)
}

fn document(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("output is a JSON envelope")
}

// ---- fixtures -------------------------------------------------------------

/// A wire-complete `listed` outcome with one live session row.
fn listed_document(
    request_id: &str,
    items: serde_json::Value,
    next_cursor: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "agent.session.list.result",
        "requestId": request_id,
        "result": {
            "outcome": {
                "kind": "listed",
                "items": items,
                "nextCursor": next_cursor,
                "revision": 7
            }
        }
    })
}

fn session_summary(session_id: &str) -> serde_json::Value {
    serde_json::json!({
        "sessionId": session_id,
        "providerId": "acme.agent",
        "lifecycle": "live",
        "executionLocation": {"executionLocationId": "host:7"},
        "observation": {
            "fidelity": "rich",
            "freshness": "fresh",
            "generation": 3,
            "sequence": 11,
            "source": "native"
        },
        "parentSessionId": null,
        "pendingRequestCount": 0,
        "presentationAttachments": [],
        "status": {"kind": "idle"}
    })
}

fn session_detail(session_id: &str) -> serde_json::Value {
    serde_json::json!({
        "snapshot": {
            "sessionId": session_id,
            "providerId": "acme.agent",
            "lifecycle": "live",
            "executionLocation": {"executionLocationId": "host:7"},
            "observation": {
                "fidelity": "rich",
                "freshness": "fresh",
                "generation": 3,
                "sequence": 11,
                "source": "native"
            },
            "parentSessionId": null,
            "activityHint": null,
            "capabilities": ["questions", "activity"],
            "lastOutcome": null,
            "pendingRequests": [{
                "actionPath": "navigate-only",
                "kind": "question",
                "requestId": "req-42",
                "turnId": "turn-9"
            }],
            "presentationAttachments": [{
                "generation": "gen-2",
                "navigable": true,
                "presentationId": "11111111111111111111111111111111",
                "visibility": "visible"
            }],
            "turn": {"kind": "active", "activity": "tool", "phase": "working", "turnId": "turn-9"}
        },
        "status": {"kind": "needs-attention", "count": 1}
    })
}

fn explanation() -> serde_json::Value {
    serde_json::json!({
        "actionPath": "navigate-only",
        "capabilityCeiling": ["questions", "activity"],
        "contentAvailable": false,
        "fidelity": "rich",
        "freshness": "fresh",
        "freshnessMeaning": "current-observation",
        "generation": 3,
        "sequence": 11,
        "source": "native",
        "status": {"kind": "needs-attention", "count": 1},
        "statusBasis": "attention-request"
    })
}

fn found_document(
    kind: &str,
    request_id: &str,
    session: serde_json::Value,
    explanation: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut outcome = serde_json::json!({
        "kind": "found",
        "revision": 7,
        "session": session,
    });
    if let Some(explanation) = explanation {
        outcome["explanation"] = explanation;
    }
    serde_json::json!({
        "kind": kind,
        "requestId": request_id,
        "result": {"outcome": outcome}
    })
}

/// A shape-valid `v1.<59>.<43>` cursor literal for `--cursor` arguments.
const VALID_CURSOR: &str = "v1.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ";

// ---- scripted outcome matrix ----------------------------------------------

#[test]
fn list_defaults_emit_verbatim_listed_outcome() -> TestResult {
    let base = base_dir("alist");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::List {
            request_id,
            include_ended,
            limit,
            cursor,
        } = request
        else {
            panic!("peer expected a list request");
        };
        assert!(
            !include_ended,
            "default list request must not include ended"
        );
        assert_eq!(limit.value(), 50, "default page size");
        assert!(cursor.is_none(), "first page omits the cursor");
        answer(
            &stream,
            listed_document(
                request_id.as_str(),
                serde_json::json!([]),
                serde_json::Value::Null,
            ),
        );
    });
    let output = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.agent.list");
    assert_eq!(document["source"]["kind"], "agent-read");
    assert_eq!(
        document["source"]["requestId"].as_str().unwrap().len(),
        32,
        "source carries the correlated request identity"
    );
    assert_eq!(document["result"]["kind"], "listed");
    assert_eq!(document["result"]["items"], serde_json::json!([]));
    assert!(
        document["result"]["nextCursor"].is_null(),
        "required-nullable nextCursor stays an explicit null"
    );
    assert_eq!(document["result"]["revision"], 7);
    Ok(())
}

#[test]
fn list_options_reach_the_wire_verbatim() -> TestResult {
    let base = base_dir("alistopt");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::List {
            request_id,
            include_ended,
            limit,
            cursor,
        } = request
        else {
            panic!("peer expected a list request");
        };
        assert!(include_ended);
        assert_eq!(limit.value(), 25);
        assert_eq!(
            cursor.expect("cursor present").as_str(),
            VALID_CURSOR,
            "the minted cursor text reaches the wire unchanged"
        );
        answer(
            &stream,
            listed_document(
                request_id.as_str(),
                serde_json::json!([]),
                serde_json::Value::Null,
            ),
        );
    });
    let output = run(
        &base,
        &[
            "agent",
            "list",
            "--include-ended",
            "--limit",
            "25",
            "--cursor",
            VALID_CURSOR,
            "--format=json",
        ],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    Ok(())
}

#[test]
fn list_items_and_next_cursor_render_text() -> TestResult {
    let base = base_dir("alisttxt");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::List { request_id, .. } = request else {
            panic!("peer expected a list request");
        };
        answer(
            &stream,
            listed_document(
                request_id.as_str(),
                serde_json::json!([session_summary("sess-alpha")]),
                serde_json::json!(VALID_CURSOR),
            ),
        );
    });
    let output = run(&base, &["agent", "list"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).expect("text output is UTF-8");
    assert!(text.contains("sess-alpha"), "row identity renders: {text}");
    assert!(text.contains("acme.agent"), "provider renders: {text}");
    assert!(
        text.contains(VALID_CURSOR),
        "a non-null nextCursor surfaces the continuation hint: {text}"
    );
    Ok(())
}

#[test]
fn list_empty_page_text_notice() -> TestResult {
    let base = base_dir("alistempty");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::List { request_id, .. } = request else {
            panic!("peer expected a list request");
        };
        answer(
            &stream,
            listed_document(
                request_id.as_str(),
                serde_json::json!([]),
                serde_json::Value::Null,
            ),
        );
    });
    let output = run(&base, &["agent", "list"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).expect("text output is UTF-8");
    assert!(
        text.contains("No agent sessions"),
        "empty page answers the localized notice, not an empty table: {text}"
    );
    Ok(())
}

#[test]
fn list_read_denied_is_agent_failure_document() -> TestResult {
    let base = base_dir("adenied");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let request_id = request.request_id().as_str().to_owned();
        answer(
            &stream,
            serde_json::json!({
                "kind": "agent.session.list.result",
                "requestId": request_id,
                "result": {"outcome": {"kind": "rejected", "reason": "read-denied"}}
            }),
        );
    });
    let output = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure.agent");
    assert_eq!(document["failure"]["kind"], "denied");
    assert_eq!(document["failure"]["details"]["stage"], "agent-read");
    assert_eq!(document["failure"]["details"]["operation"], "list");
    assert_eq!(document["failure"]["details"]["reason"], "read-denied");
    assert_eq!(
        document["source"]["kind"], "agent-read",
        "a correlated semantic refusal keeps its request identity"
    );
    Ok(())
}

/// The peer completes admission promptly and then never answers the OBAP
/// request: the command reports its Agent timeout (~5s elapsed) with the
/// published detail set and no source, inside one exchange ceiling. This
/// proves the post-admission exchange expires against the shared deadline
/// and the timeout document is real — it does NOT distinguish a
/// post-cohort budget refresh (the earlier two-budget shape produced the
/// same ~5s here); preamble accounting is pinned by the
/// `agent_session_until`/`agent_settle` seams and the single threaded
/// `Instant`. The peer blocks on the session so it retires as soon as the
/// CLI drops the connection.
#[test]
fn held_peer_timeout_reports_agent_failure() -> TestResult {
    let base = base_dir("aheld");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let _request = read_request(&stream);
        // Hold without answering: the CLI's shared total must expire the
        // wait; the session close ends this read and lets the peer retire.
        let mut sink = [0_u8; 64];
        let _ = (&stream).read(&mut sink);
    });
    let started = Instant::now();
    let output = run(&base, &["agent", "list", "--format", "json"]);
    let elapsed = started.elapsed();
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(7));
    assert!(
        elapsed >= Duration::from_secs(4) && elapsed < Duration::from_secs(8),
        "one shared ~5s total bounds admission plus exchange, not a renewed budget: {elapsed:?}"
    );
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure.agent");
    assert_eq!(document["failure"]["kind"], "timeout");
    assert_eq!(document["failure"]["details"]["stage"], "agent-read");
    assert_eq!(document["failure"]["details"]["operation"], "list");
    assert_eq!(document["failure"]["details"]["timeoutMs"], 5000);
    assert!(
        document.get("source").is_none() || document["source"].is_null(),
        "a timeout after admission carries no source"
    );
    Ok(())
}

// ---- signal ownership: SIGINT/SIGTERM only ----------------------------------
//
// The Agent read commands own exactly SIGINT and SIGTERM for their armed
// span — never the source stage's four-signal set. A captured member settles
// the invocation as the typed `cancelled` agent failure at exit *status* 130,
// while SIGHUP and SIGQUIT keep their inherited disposition entirely: a raw
// signal death, not a typed 130. A signal inherited ignored is excluded from
// eligibility. Every signal is sent only after the peer confirms the exact
// phase, so the captured signal — never the shared five-second deadline —
// provably ends the wait.

/// A spawn with the shared deterministic-locale environment whose `Child`
/// stays caller-owned — signal tests must hold it to signal and reap it.
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

fn wait(child: Child) -> Output {
    support::bounded_wait_with_output(child, support::RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("{timeout}"))
}

/// A spawn that zeroes `RLIMIT_CORE` through a shell wrapper — the raw-death
/// probes must die by signal without leaving a core file behind (SIGQUIT
/// dumps by default), and the crate forbids `unsafe` so `pre_exec` is out.
fn spawn_cli_no_core(base: &Path, arguments: &[&str]) -> Child {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg("ulimit -c 0; exec \"$@\"")
        .arg("sh")
        .arg(cohort().join("orchbee"))
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn no-core cli child")
}

/// The live child's `/proc/<pid>/status` `SigPnd`/`ShdPnd`/`SigBlk`/`SigIgn`
/// bitmasks — the eligibility readback an inherited-disposition test asserts
/// while the armed span is parked in its exchange wait. A signal `kill`ed at
/// the process lands in `ShdPnd`, not the per-thread `SigPnd`.
fn proc_signal_masks(pid: u32) -> (u64, u64, u64, u64) {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).expect("read child status");
    let field = |name: &str| -> u64 {
        status
            .lines()
            .find_map(|line| {
                line.strip_prefix(name)
                    .and_then(|value| u64::from_str_radix(value.trim(), 16).ok())
            })
            .unwrap_or_else(|| panic!("{name} field in child status"))
    };
    (
        field("SigPnd:"),
        field("ShdPnd:"),
        field("SigBlk:"),
        field("SigIgn:"),
    )
}

fn signal_bit(signo: i32) -> u64 {
    1_u64 << (signo as u64 - 1)
}

fn signal_child(child: &Child, signal: nix::sys::signal::Signal) {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(child.id() as i32), signal)
        .expect("signal child");
}

/// The signal the spawned CLI will evaluate for eligibility: if the test
/// process itself inherited it ignored or blocked, the child cannot capture
/// it — report that environment honestly instead of claiming evidence the
/// run never produced.
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

/// One admitted session that reads the OBAP request, reports `ready` once the
/// request is on the wire, then holds the exchange unanswered until the CLI's
/// own arbitration closes the session.
fn held_exchange_peer(scope: &Path) -> (thread::JoinHandle<()>, Receiver<()>) {
    let (ready, rx) = sync_channel::<()>(0);
    let peer = spawn_peer(scope, 1, move |stream| {
        admit(&stream);
        let _request = read_request(&stream);
        ready.send(()).expect("report the held exchange");
        let mut sink = [0_u8; 64];
        let _ = (&stream).read(&mut sink);
    });
    (peer, rx)
}

/// The typed-cancellation assertion family shared by every captured-signal
/// test: a real exit status of 130 — a raw signal death would report
/// `signal()`, not `code()` — plus the agent failure envelope carrying the
/// `agent-read` stage, this operation, and no correlated source.
fn assert_typed_cancellation(output: &Output, base: &Path, operation: &str) {
    assert_eq!(
        output.status.signal(),
        None,
        "typed cancellation is an exit, not a signal death"
    );
    assert_eq!(
        output.status.code(),
        Some(130),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = document(output);
    compiled_schema(base, "orchbee.cli.failure.agent")
        .validate(&document)
        .expect("agent failure validates against its schema");
    assert_eq!(document["failure"]["kind"], "cancelled");
    assert_eq!(document["failure"]["details"]["stage"], "agent-read");
    assert_eq!(document["failure"]["details"]["operation"], operation);
    assert!(
        document.get("source").is_none() || document["source"].is_null(),
        "a cancelled read carries no correlated source"
    );
}

/// SIGINT parked inside the held OBAP exchange is the typed Agent
/// cancellation: exit status 130 with the agent envelope.
#[test]
fn list_sigint_during_held_exchange_is_typed_cancellation() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGINT);
    let base = base_dir("alist-int");
    let scope = stage_scope(&base);
    let (peer, ready) = held_exchange_peer(&scope);
    let child = spawn_cli(&base, &["agent", "list", "--format", "json"]);
    ready
        .recv_timeout(Duration::from_secs(15))
        .expect("the request reached the held peer");
    signal_child(&child, nix::sys::signal::Signal::SIGINT);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_typed_cancellation(&output, &base, "list");
    Ok(())
}

/// SIGTERM is the same typed cancellation — status 130, never the raw
/// `signal() == 15` death an unwatched TERM would produce.
#[test]
fn list_sigterm_during_held_exchange_is_typed_cancellation() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGTERM);
    let base = base_dir("alist-term");
    let scope = stage_scope(&base);
    let (peer, ready) = held_exchange_peer(&scope);
    let child = spawn_cli(&base, &["agent", "list", "--format", "json"]);
    ready
        .recv_timeout(Duration::from_secs(15))
        .expect("the request reached the held peer");
    signal_child(&child, nix::sys::signal::Signal::SIGTERM);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_typed_cancellation(&output, &base, "list");
    Ok(())
}

/// The operation detail follows the command: `agent get` under SIGINT names
/// `get`, not `list`.
#[test]
fn get_sigint_reports_its_own_operation() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGINT);
    let base = base_dir("aget-int");
    let scope = stage_scope(&base);
    let (peer, ready) = held_exchange_peer(&scope);
    let child = spawn_cli(&base, &["agent", "get", "sess-alpha", "--format", "json"]);
    ready
        .recv_timeout(Duration::from_secs(15))
        .expect("the request reached the held peer");
    signal_child(&child, nix::sys::signal::Signal::SIGINT);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_typed_cancellation(&output, &base, "get");
    Ok(())
}

/// SIGINT while admission is still unanswered cancels the same way: the guard
/// arms before any runtime contact, so a parked admission is still the typed
/// `agent-read` cancellation — not a generic pre-admission failure.
#[test]
fn list_sigint_before_admission_is_typed_cancellation() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGINT);
    let base = base_dir("alist-int-pre");
    let scope = stage_scope(&base);
    let (ready, rx) = sync_channel::<()>(0);
    let peer = spawn_peer(&scope, 1, move |stream| {
        // Read the hello but never answer `ClientAccepted`: the CLI is parked
        // inside the admission exchange when the signal lands.
        match read_message(&mut &stream) {
            Ok(LocalClientMessage::ClientHello(_)) => {}
            other => panic!("peer expected ClientHello, got {other:?}"),
        }
        ready.send(()).expect("report the held admission");
        let mut sink = [0_u8; 64];
        let _ = (&stream).read(&mut sink);
    });
    let child = spawn_cli(&base, &["agent", "list", "--format", "json"]);
    rx.recv_timeout(Duration::from_secs(15))
        .expect("the admission reached the held peer");
    signal_child(&child, nix::sys::signal::Signal::SIGINT);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_typed_cancellation(&output, &base, "list");
    Ok(())
}

/// SIGHUP is not in the Agent watch set: it keeps its inherited disposition —
/// a raw signal death that emits no typed cancellation and no exit code.
#[test]
fn list_sighup_is_raw_signal_death() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGHUP);
    let base = base_dir("alist-hup");
    let scope = stage_scope(&base);
    let (peer, ready) = held_exchange_peer(&scope);
    let child = spawn_cli_no_core(&base, &["agent", "list", "--format", "json"]);
    ready
        .recv_timeout(Duration::from_secs(15))
        .expect("the request reached the held peer");
    signal_child(&child, nix::sys::signal::Signal::SIGHUP);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.signal(),
        Some(nix::sys::signal::Signal::SIGHUP as i32),
        "SIGHUP keeps its raw disposition — no typed 130"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("\"cancelled\""),
        "no typed cancellation envelope survives a raw death"
    );
    Ok(())
}

/// SIGQUIT is equally outside the Agent watch set: raw signal death, never a
/// normalized 131 or 130.
#[test]
fn list_sigquit_is_raw_signal_death() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGQUIT);
    let base = base_dir("alist-quit");
    let scope = stage_scope(&base);
    let (peer, ready) = held_exchange_peer(&scope);
    let child = spawn_cli_no_core(&base, &["agent", "list", "--format", "json"]);
    ready
        .recv_timeout(Duration::from_secs(15))
        .expect("the request reached the held peer");
    signal_child(&child, nix::sys::signal::Signal::SIGQUIT);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_eq!(
        output.status.signal(),
        Some(nix::sys::signal::Signal::SIGQUIT as i32),
        "SIGQUIT keeps its raw disposition — no typed cancellation"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("\"cancelled\""),
        "no typed cancellation envelope survives a raw death"
    );
    Ok(())
}

/// A SIGINT inherited as ignored is excluded from the Agent eligible set:
/// it cannot cancel the held exchange — the process provably stays alive —
/// while the still-eligible SIGTERM then cancels at the typed 130.
#[test]
fn list_inherited_ignored_sigint_is_not_captured() -> TestResult {
    let base = base_dir("alist-ign");
    let scope = stage_scope(&base);
    let (peer, ready) = held_exchange_peer(&scope);
    let binary = cohort().join("orchbee");
    // `trap '' INT` installs SIG_IGN for SIGINT, which `exec` preserves —
    // the CLI inherits an ignored SIGINT, exactly the eligibility input the
    // guard must respect.
    let child = Command::new("sh")
        .args(["-c", "trap '' INT; exec \"$@\"", "sh"])
        .arg(&binary)
        .args(["agent", "list", "--format", "json"])
        .env("ORCHBEE_RUNTIME_BASE", &base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sh wrapper");
    ready
        .recv_timeout(Duration::from_secs(15))
        .expect("the request reached the held peer");
    // While parked in the armed exchange, the /proc readback shows the
    // inherited disposition respected end to end: SIGINT still ignored, and
    // the guard never added it to the block mask — the still-eligible
    // SIGTERM is the only Agent member it blocks.
    let (_pnd, _shd, blk, ign) = proc_signal_masks(child.id());
    assert_ne!(
        ign & signal_bit(nix::libc::SIGINT),
        0,
        "SIGINT stays ignored"
    );
    assert_eq!(
        blk & signal_bit(nix::libc::SIGINT),
        0,
        "an ignored member is never masked for capture"
    );
    assert_ne!(
        blk & signal_bit(nix::libc::SIGTERM),
        0,
        "the eligible member is armed"
    );
    signal_child(&child, nix::sys::signal::Signal::SIGINT);
    thread::sleep(Duration::from_millis(400));
    // A null-signal probe: the child must still be running — an ignored
    // SIGINT is excluded from capture and ends nothing.
    assert!(
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(child.id() as i32),
            None::<nix::sys::signal::Signal>
        )
        .is_ok(),
        "an inherited-ignored SIGINT must not cancel the agent read"
    );
    signal_child(&child, nix::sys::signal::Signal::SIGTERM);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_typed_cancellation(&output, &base, "list");
    Ok(())
}

/// A SIGTERM inherited as *blocked* is equally ineligible: it stays pending
/// under the mask the exec preserved — the guard never unblocks to capture —
/// while the eligible SIGINT still cancels at the typed 130.
#[test]
fn list_inherited_blocked_sigterm_is_not_captured() -> TestResult {
    require_eligible(nix::sys::signal::Signal::SIGINT);
    let base = base_dir("alist-blk");
    let scope = stage_scope(&base);
    let (peer, ready) = held_exchange_peer(&scope);
    // Block SIGTERM on this test thread around the spawn: the mask survives
    // fork and exec, so the child exec's with SIGTERM already blocked — the
    // armed guard evaluates an inherited-blocked member without any unsafe.
    let mut inherited = nix::sys::signal::SigSet::empty();
    inherited.add(nix::sys::signal::Signal::SIGTERM);
    nix::sys::signal::sigprocmask(
        nix::sys::signal::SigmaskHow::SIG_BLOCK,
        Some(&inherited),
        None,
    )
    .expect("block SIGTERM for the spawn");
    let child = spawn_cli(&base, &["agent", "list", "--format", "json"]);
    nix::sys::signal::sigprocmask(
        nix::sys::signal::SigmaskHow::SIG_UNBLOCK,
        Some(&inherited),
        None,
    )
    .expect("restore the test thread mask");
    ready
        .recv_timeout(Duration::from_secs(15))
        .expect("the request reached the held peer");
    // Parked in the armed exchange: SigBlk still carries the inherited TERM
    // bit (set before exec — never unblocked for capture) alongside the
    // arm-added INT bit.
    let (_pnd, _shd, blk, _ign) = proc_signal_masks(child.id());
    assert_ne!(
        blk & signal_bit(nix::libc::SIGTERM),
        0,
        "the inherited TERM block survives arming"
    );
    assert_ne!(
        blk & signal_bit(nix::libc::SIGINT),
        0,
        "the eligible INT member is armed"
    );
    signal_child(&child, nix::sys::signal::Signal::SIGTERM);
    thread::sleep(Duration::from_millis(400));
    // Still running: the blocked TERM was neither delivered nor captured —
    // it sits pending.
    assert!(
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(child.id() as i32),
            None::<nix::sys::signal::Signal>
        )
        .is_ok(),
        "an inherited-blocked SIGTERM must not cancel the agent read"
    );
    let (_pnd, shd, _blk, _ign) = proc_signal_masks(child.id());
    assert_ne!(
        shd & signal_bit(nix::libc::SIGTERM),
        0,
        "the blocked TERM stays pending"
    );
    signal_child(&child, nix::sys::signal::Signal::SIGINT);
    let output = wait(child);
    peer.join().expect("peer finished");
    assert_typed_cancellation(&output, &base, "list");
    Ok(())
}

/// A post-admission transport fault is not a cancellation: the held peer
/// drops the session mid-response and the CLI answers the `transport-loss`
/// agent failure at exit 5 — a real exit, never a signal death or a 130.
#[test]
fn list_peer_drop_mid_response_is_transport_loss() -> TestResult {
    let base = base_dir("alist-rst");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, move |stream| {
        admit(&stream);
        let _request = read_request(&stream);
        // The admitted session drops without an answer — the client's read
        // fails as a transport fault.
    });
    let output = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.signal(), None, "a transport fault exits");
    assert_eq!(output.status.code(), Some(5));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure.agent");
    assert_eq!(document["failure"]["kind"], "transport-loss");
    assert_eq!(document["failure"]["details"]["stage"], "agent-read");
    assert_eq!(document["failure"]["details"]["operation"], "list");
    Ok(())
}

/// A peer that accepts the socket but closes before answering admission is
/// the pre-admission failure family: the generic `orchbee.cli.failure`
/// transport loss — never the typed 130 an unwatched signal would not
/// produce either.
#[test]
fn list_peer_drop_before_admission_is_admission_failure() -> TestResult {
    let base = base_dir("alist-rst-pre");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        match read_message(&mut &stream) {
            Ok(LocalClientMessage::ClientHello(_)) => {}
            other => panic!("peer expected ClientHello, got {other:?}"),
        }
        drop(stream);
    });
    let output = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.signal(), None);
    assert_eq!(output.status.code(), Some(5));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure");
    assert_eq!(document["failure"]["kind"], "transport-loss");
    assert_eq!(document["failure"]["details"]["stage"], "admission");
    Ok(())
}

#[test]
fn list_runtime_unavailable_is_agent_failure_document() -> TestResult {
    let base = base_dir("aunavail");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let request_id = request.request_id().as_str().to_owned();
        answer(
            &stream,
            serde_json::json!({
                "kind": "agent.session.list.result",
                "requestId": request_id,
                "result": {"outcome": {"kind": "unavailable", "reason": "runtime-unavailable"}}
            }),
        );
    });
    let output = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(4));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure.agent");
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    assert_eq!(document["failure"]["details"]["operation"], "list");
    assert_eq!(
        document["failure"]["details"]["reason"],
        "runtime-unavailable"
    );
    Ok(())
}

#[test]
fn get_found_emits_verbatim_outcome() -> TestResult {
    let base = base_dir("aget");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::Get {
            request_id,
            session_id,
        } = request
        else {
            panic!("peer expected a get request");
        };
        assert_eq!(session_id.as_str(), "sess-alpha");
        answer(
            &stream,
            found_document(
                "agent.session.get.result",
                request_id.as_str(),
                session_detail("sess-alpha"),
                None,
            ),
        );
    });
    let output = run(&base, &["agent", "get", "sess-alpha", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.agent.get");
    assert_eq!(document["result"]["kind"], "found");
    assert_eq!(
        document["result"]["session"]["snapshot"]["sessionId"],
        "sess-alpha"
    );
    assert!(
        document["result"]["session"]["snapshot"]["lastOutcome"].is_null(),
        "required-nullable fields stay explicit on the wire document"
    );
    Ok(())
}

#[test]
fn get_text_renders_session_fields() -> TestResult {
    let base = base_dir("agettxt");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::Get { request_id, .. } = request else {
            panic!("peer expected a get request");
        };
        answer(
            &stream,
            found_document(
                "agent.session.get.result",
                request_id.as_str(),
                session_detail("sess-alpha"),
                None,
            ),
        );
    });
    let output = run(&base, &["agent", "get", "sess-alpha"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).expect("text output is UTF-8");
    assert!(text.contains("sess-alpha"), "{text}");
    assert!(text.contains("acme.agent"), "{text}");
    assert!(
        text.contains("req-42"),
        "pending request row renders: {text}"
    );
    assert!(
        text.contains("11111111111111111111111111111111"),
        "attachment row renders: {text}"
    );
    Ok(())
}

#[test]
fn get_not_found_maps_to_agent_failure_document() -> TestResult {
    let base = base_dir("anotfound");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let request_id = request.request_id().as_str().to_owned();
        answer(
            &stream,
            serde_json::json!({
                "kind": "agent.session.get.result",
                "requestId": request_id,
                "result": {"outcome": {"kind": "rejected", "reason": "not-found"}}
            }),
        );
    });
    let output = run(&base, &["agent", "get", "sess-gone", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(3));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure.agent");
    assert_eq!(document["failure"]["kind"], "not-found");
    assert_eq!(document["failure"]["details"]["operation"], "get");
    assert_eq!(document["failure"]["details"]["sessionId"], "sess-gone");
    Ok(())
}

#[test]
fn explain_found_emits_verbatim_outcome() -> TestResult {
    let base = base_dir("aexplain");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::Explain {
            request_id,
            session_id,
        } = request
        else {
            panic!("peer expected an explain request");
        };
        assert_eq!(session_id.as_str(), "sess-alpha");
        answer(
            &stream,
            found_document(
                "agent.session.explain.result",
                request_id.as_str(),
                session_detail("sess-alpha"),
                Some(explanation()),
            ),
        );
    });
    let output = run(
        &base,
        &["agent", "explain", "sess-alpha", "--format", "json"],
    );
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.agent.explain");
    assert_eq!(document["result"]["kind"], "found");
    assert_eq!(
        document["result"]["explanation"]["statusBasis"],
        "attention-request"
    );
    assert_eq!(
        document["result"]["explanation"]["actionPath"],
        "navigate-only"
    );
    Ok(())
}

#[test]
fn explain_text_renders_derived_fields() -> TestResult {
    let base = base_dir("aexplaintxt");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let AgentReadRequest::Explain { request_id, .. } = request else {
            panic!("peer expected an explain request");
        };
        answer(
            &stream,
            found_document(
                "agent.session.explain.result",
                request_id.as_str(),
                session_detail("sess-alpha"),
                Some(explanation()),
            ),
        );
    });
    let output = run(&base, &["agent", "explain", "sess-alpha"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).expect("text output is UTF-8");
    assert!(text.contains("sess-alpha"), "{text}");
    assert!(
        text.contains("navigate-only") || text.to_lowercase().contains("navigate"),
        "the action boundary renders: {text}"
    );
    Ok(())
}

#[test]
fn correlation_mismatch_is_protocol_invalid() -> TestResult {
    let base = base_dir("acorrel");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let _ = read_request(&stream);
        answer(
            &stream,
            listed_document(
                "0000000000000000000000000000000f",
                serde_json::json!([]),
                serde_json::Value::Null,
            ),
        );
    });
    let output = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure.agent");
    assert_eq!(document["failure"]["kind"], "protocol-invalid");
    assert!(
        document["source"].is_null(),
        "an uncorrelated answer carries no claimed source"
    );
    Ok(())
}

#[test]
fn wrong_result_kind_is_protocol_invalid() -> TestResult {
    let base = base_dir("awrongkind");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let request = read_request(&stream);
        // A get request answered by a list result with the right identity:
        // requestId correlates but the operation does not.
        let request_id = request.request_id().as_str().to_owned();
        answer(
            &stream,
            listed_document(&request_id, serde_json::json!([]), serde_json::Value::Null),
        );
    });
    let output = run(&base, &["agent", "get", "sess-alpha", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let document = document(&output);
    assert_eq!(document["failure"]["kind"], "protocol-invalid");
    assert_eq!(document["failure"]["details"]["operation"], "get");
    Ok(())
}

#[test]
fn malformed_frame_is_protocol_invalid() -> TestResult {
    let base = base_dir("amalform");
    let scope = stage_scope(&base);
    let peer = spawn_peer(&scope, 1, |stream| {
        admit(&stream);
        let _ = read_request(&stream);
        use std::io::Write;
        (&mut &stream)
            .write_all(b"NOT-AN-OBAP-FRAME")
            .expect("peer writes garbage");
    });
    let output = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    assert_eq!(output.status.code(), Some(5));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure.agent");
    assert_eq!(document["failure"]["kind"], "protocol-invalid");
    Ok(())
}

#[test]
fn invalid_session_id_fails_before_any_session() -> TestResult {
    let base = base_dir("abadid");
    let scope = stage_scope(&base);
    let output = run(&base, &["agent", "get", "bad id!", "--format", "json"]);
    assert_eq!(output.status.code(), Some(2));
    let document = document(&output);
    assert_eq!(document["schema"], "orchbee.cli.failure");
    assert_eq!(document["failure"]["kind"], "invalid-usage");
    assert!(
        scope.read_dir()?.count() == 0,
        "a parse failure never creates a socket peer"
    );
    Ok(())
}

#[test]
fn missing_scope_is_generic_failure() -> TestResult {
    let base = base_dir("aabsent");
    let output = run(&base, &["agent", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(4));
    let document = document(&output);
    // Pre-admission discovery failures keep the generic document — the
    // agent-specific schema only covers post-admission outcomes.
    assert_eq!(document["schema"], "orchbee.cli.failure");
    assert_eq!(document["failure"]["kind"], "service-unavailable");
    Ok(())
}

// ---- schema contrast --------------------------------------------------------
//
// The published CLI success schemas mirror the wire contract exactly: emitted
// envelopes validate, while contract-looser variants — an out-of-range
// pendingRequestCount or duplicate capability entries the wire binding would
// never produce — must be refused, not just the happy path re-stated.

fn compiled_schema(base: &Path, id: &str) -> jsonschema::Validator {
    let output = run(base, &["api", "schema", id]);
    assert_eq!(output.status.code(), Some(0), "schema {id}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    jsonschema::draft202012::new(&document["result"]["schema"]["document"])
        .expect("schema compiles")
}

#[test]
fn agent_envelopes_validate_and_loose_variants_fail() -> TestResult {
    let base = base_dir("aschema");
    let scope = stage_scope(&base);
    let list_schema = compiled_schema(&base, "orchbee.agent.list");
    let get_schema = compiled_schema(&base, "orchbee.agent.get");
    let explain_schema = compiled_schema(&base, "orchbee.agent.explain");
    let failure_schema = compiled_schema(&base, "orchbee.cli.failure.agent");

    // Real emitted envelopes validate against their published schemas: one
    // peer serves all four sequential sessions — the second `list` carries the
    // semantic refusal whose failure envelope must also validate.
    let lists = std::sync::atomic::AtomicU32::new(0);
    let peer = spawn_peer(&scope, 4, move |stream| {
        admit(&stream);
        let request = read_request(&stream);
        let request_id = request.request_id().as_str().to_owned();
        match &request {
            AgentReadRequest::List { .. }
                if lists.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 =>
            {
                answer(
                    &stream,
                    listed_document(
                        &request_id,
                        serde_json::json!([session_summary("sess-alpha")]),
                        serde_json::Value::Null,
                    ),
                );
            }
            AgentReadRequest::List { .. } => answer(
                &stream,
                serde_json::json!({
                    "kind": "agent.session.list.result",
                    "requestId": request_id,
                    "result": {"outcome": {"kind": "rejected", "reason": "read-denied"}}
                }),
            ),
            AgentReadRequest::Get { .. } => answer(
                &stream,
                found_document(
                    "agent.session.get.result",
                    &request_id,
                    session_detail("sess-alpha"),
                    None,
                ),
            ),
            AgentReadRequest::Explain { .. } => answer(
                &stream,
                found_document(
                    "agent.session.explain.result",
                    &request_id,
                    session_detail("sess-alpha"),
                    Some(explanation()),
                ),
            ),
        }
    });
    let list_out = run(&base, &["agent", "list", "--format", "json"]);
    let get_out = run(&base, &["agent", "get", "sess-alpha", "--format", "json"]);
    let explain_out = run(
        &base,
        &["agent", "explain", "sess-alpha", "--format", "json"],
    );
    let denied = run(&base, &["agent", "list", "--format", "json"]);
    peer.join().expect("peer finished");
    for (output, validator, name) in [
        (&list_out, &list_schema, "list"),
        (&get_out, &get_schema, "get"),
        (&explain_out, &explain_schema, "explain"),
    ] {
        assert_eq!(output.status.code(), Some(0), "{name}");
        let document = document(output);
        assert!(
            validator.is_valid(&document),
            "emitted {name} envelope must validate: {document}"
        );
    }
    assert_eq!(denied.status.code(), Some(3));
    let denied_document = document(&denied);
    assert!(
        failure_schema.is_valid(&denied_document),
        "emitted failure envelope must validate: {denied_document}"
    );

    // Wire-contract bounds restated in the CLI schemas: the count bound is
    // inclusive at 128, and capability lists reject duplicates — both are
    // wire-impossible variants that must not validate under the published
    // contract.
    let mut summary = session_summary("sess-alpha");
    let valid_list = |summary: serde_json::Value| {
        serde_json::json!({
            "schema": "orchbee.agent.list",
            "version": 1,
            "source": {"kind": "agent-read", "requestId": "11111111111111111111111111111111"},
            "result": {
                "kind": "listed",
                "items": [summary],
                "nextCursor": null,
                "revision": 7
            }
        })
    };
    summary["pendingRequestCount"] = serde_json::json!(129);
    assert!(
        !list_schema.is_valid(&valid_list(summary.clone())),
        "pendingRequestCount beyond the wire bound must fail"
    );
    summary["pendingRequestCount"] = serde_json::json!(128);
    assert!(
        list_schema.is_valid(&valid_list(summary.clone())),
        "the inclusive wire bound itself must pass"
    );

    let valid_get = |detail: serde_json::Value| {
        serde_json::json!({
            "schema": "orchbee.agent.get",
            "version": 1,
            "source": {"kind": "agent-read", "requestId": "11111111111111111111111111111111"},
            "result": {"kind": "found", "revision": 7, "session": detail}
        })
    };
    let mut detail = session_detail("sess-alpha");
    detail["snapshot"]["capabilities"] = serde_json::json!(["activity", "activity"]);
    assert!(
        !get_schema.is_valid(&valid_get(detail.clone())),
        "duplicate capabilities must fail"
    );
    detail["snapshot"]["capabilities"] = serde_json::json!(["activity"]);
    assert!(
        get_schema.is_valid(&valid_get(detail)),
        "unique capabilities must pass"
    );

    let valid_explain = |explanation: serde_json::Value| {
        serde_json::json!({
            "schema": "orchbee.agent.explain",
            "version": 1,
            "source": {"kind": "agent-read", "requestId": "11111111111111111111111111111111"},
            "result": {
                "kind": "found",
                "revision": 7,
                "session": session_detail("sess-alpha"),
                "explanation": explanation
            }
        })
    };
    let mut explanation = explanation();
    explanation["capabilityCeiling"] = serde_json::json!(["questions", "questions"]);
    assert!(
        !explain_schema.is_valid(&valid_explain(explanation.clone())),
        "duplicate capabilityCeiling entries must fail"
    );
    explanation["capabilityCeiling"] = serde_json::json!(["questions"]);
    assert!(
        explain_schema.is_valid(&valid_explain(explanation)),
        "unique capabilityCeiling entries must pass"
    );
    Ok(())
}
