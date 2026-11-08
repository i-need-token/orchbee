//! The Session's processes are readable with one `query` request that never enters the Client
//! phase or activates a Session.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_connection_handshake::{
    AcceptorMessage, Build, ConnectionKind, EnvironmentName, Extensions, InitiatorMessage,
    ProcessEntry, ProcessRole, ProcessState, ProductVersion, QueryResult, Rejection,
    RejectionReason, RequestKind, encode_acceptor_frame, read_initiator_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, write_message};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::Cohort;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    num::NonZeroU32,
    process::Output,
    thread,
    time::{Duration, Instant},
};
use support::Fixture;

#[test]
fn tui_measurement_environment_has_no_help_version_or_offline_query_side_effects() {
    let fixture = Fixture::new();
    let measurements = fixture.0.join("tui-measurements.json");
    for (arguments, expected_status) in [
        (vec!["--help"], 0),
        (vec!["--version"], 0),
        (vec!["status", "client", "--format=json"], 0),
        (vec!["config", "check", "--format=json"], 1),
    ] {
        let mut command = fixture.command(&arguments, None, None, &[]);
        command.env_remove("ORCHBEE_TUI_MEASUREMENTS");
        let baseline = fixture.run_command(command, None, Duration::from_secs(45), 8 * 1024 * 1024);
        assert_eq!(
            baseline.status.code(),
            Some(expected_status),
            "{arguments:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&baseline.stdout),
            String::from_utf8_lossy(&baseline.stderr)
        );
        let output = fixture.run_with_env(
            &arguments,
            None,
            None,
            &[("ORCHBEE_TUI_MEASUREMENTS", measurements.as_os_str())],
        );
        assert_eq!(
            output.status.code(),
            baseline.status.code(),
            "{arguments:?}"
        );
        assert_eq!(output.stdout, baseline.stdout, "{arguments:?}");
        assert_eq!(output.stderr, baseline.stderr, "{arguments:?}");
        if expected_status == 1 {
            // Inspection reports the absent base without creating it.
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            let checks = document["result"]["checks"].as_array().unwrap();
            let failed: Vec<_> = checks
                .iter()
                .filter(|check| check["status"] == "fail")
                .map(|check| check["name"].as_str().unwrap())
                .collect();
            assert_eq!(failed, ["runtime-base"]);
            assert_eq!(checks[0]["details"]["reason"], "absent");
        }
        assert!(
            !measurements.exists(),
            "non-TUI command must not create diagnostics: {arguments:?}"
        );
        assert!(!fixture.0.join("runtime").exists());
    }
    let output = fixture.run_with_env(
        &["--version"],
        None,
        None,
        &[(
            "ORCHBEE_TUI_MEASUREMENTS",
            std::ffi::OsStr::new("relative.json"),
        )],
    );
    assert!(
        output.status.success(),
        "non-TUI command must not validate the TUI-only path"
    );
}

#[test]
fn shared_preparation_keeps_mutable_fixtures_and_corrupt_cohorts_isolated() {
    let first = Fixture::new();
    let second = Fixture::new();
    assert_eq!(first.executable(), second.executable());
    let first_root = first.0.clone();
    let private = Fixture::private_cohort();
    let private_root = private.cohort_directory().to_path_buf();
    fs::write(private_root.join("cohort.json"), b"invalid manifest").unwrap();
    fs::write(first.0.join("settings"), b"private state").unwrap();
    assert!(!second.0.join("settings").exists());
    assert!(Cohort::load(&private_root, Instant::now() + Duration::from_secs(10)).is_err());
    drop(private);
    assert!(!private_root.exists());
    drop(first);
    assert!(!first_root.exists());
    // Retiring a sibling and corrupting a private cohort cannot invalidate this reader.
    Cohort::load(
        second.cohort_directory(),
        Instant::now() + Duration::from_secs(10),
    )
    .unwrap();
}

fn document(output: Output, status: i32, schema: &str) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    let source = match schema {
        "orchbee.status.client" => include_str!("../src/cli/schemas/orchbee.status.client.json"),
        "orchbee.status.server" => include_str!("../src/cli/schemas/orchbee.status.server.json"),
        "orchbee.cli.failure" => include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        "orchbee.config.check" => include_str!("../src/cli/schemas/orchbee.config.check.json"),
        _ => panic!("unexpected status schema: {schema}"),
    };
    support::validate(&doc, source);
    doc
}
fn success(output: Output, scope: &str) -> Value {
    document(output, 0, &format!("orchbee.status.{scope}"))
}
fn peer(
    fixture: &Fixture,
    action: impl FnOnce(&mut LocalStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    let session = fixture.rendezvous();
    let listener = LocalListener::bind(session.client_socket_path()).unwrap();
    listener.set_nonblocking(true).unwrap();
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("accept: {e}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        match read_initiator_message(&mut stream).unwrap() {
            InitiatorMessage::Request(request) => {
                assert_eq!(request.kind, RequestKind::Query);
                assert_eq!(
                    request.environment,
                    orchbee_local_runtime::environment_name()
                );
            }
            other => panic!("expected a query request, got {other:?}"),
        }
        action(&mut stream);
        assert_eq!(
            stream.read(&mut [0]).unwrap(),
            0,
            "candidate closes without application traffic"
        );
        assert!(
            matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock),
            "no second connection"
        );
    })
}
fn answer(stream: &mut LocalStream, message: &AcceptorMessage) {
    stream
        .write_all(&encode_acceptor_frame(message).unwrap())
        .unwrap();
}
/// A stopping Server holding three terminals, then the Worker it started.
fn processes() -> AcceptorMessage {
    let entry = |role, pid| ProcessEntry {
        role,
        product_version: ProductVersion::new("9.8.7-test").unwrap(),
        build: Build::default(),
        environment: orchbee_local_runtime::environment_name(),
        pid: NonZeroU32::new(pid).unwrap(),
        state: ProcessState::Stopping,
        terminals: 3,
        connections: Vec::new(),
    };
    AcceptorMessage::QueryResult(
        QueryResult::new(
            vec![
                entry(ProcessRole::Server, 42),
                entry(ProcessRole::Worker, 43),
            ],
            Some(false),
            Extensions::new(),
        )
        .unwrap(),
    )
}
fn rejection(reason: RejectionReason, environment: &str) -> AcceptorMessage {
    AcceptorMessage::Rejection(Rejection {
        reason,
        generations: ConnectionKind::ClientServer.supported_generations(),
        product_version: ProductVersion::new("9.8.7-test").unwrap(),
        environment: EnvironmentName::new(environment).unwrap(),
        extensions: Extensions::new(),
    })
}

#[test]
fn offline_client_and_absent_server_do_not_create_runtime_or_locks() {
    let fixture = Fixture::new();
    let client = success(
        fixture.run(&["status", "client", "--format=json"]),
        "client",
    );
    assert_eq!(client["result"]["cohort"]["state"], "available");
    assert_eq!(client["result"]["platform"], "windows");
    assert!(client["result"]["updateChannel"].is_null());
    let server = success(
        fixture.run(&["status", "server", "--format=json"]),
        "server",
    );
    assert_eq!(
        server["result"]["server"],
        json!({"state":"not-running","reason":"runtime-absent"})
    );
    assert!(!fixture.0.join("runtime").exists());
    // A fresh environment's default runtime base reads as not started, not as a failed check.
    let mut command = fixture.command(&["config", "check", "--format=json"], None, None, &[]);
    command.env_remove("ORCHBEE_RUNTIME_BASE");
    let check = document(
        fixture.run_command(command, None, Duration::from_secs(45), 8 * 1024 * 1024),
        0,
        "orchbee.config.check",
    );
    for path in &check["result"]["checks"].as_array().unwrap()[..2] {
        assert_eq!(path["status"], "not-applicable", "{path}");
        assert_eq!(path["details"]["reason"], "not-started", "{path}");
    }
    let profile = fixture.profile();
    let default_base = orchbee_local_runtime::default_runtime_base(|name| {
        (name == "USERPROFILE").then(|| profile.clone().into_os_string())
    })
    .unwrap();
    assert!(!default_base.exists(), "{}", default_base.display());
    let session = fixture.rendezvous();
    let server = success(
        fixture.run(&["status", "server", "--format=json"]),
        "server",
    );
    assert_eq!(server["result"]["server"]["reason"], "endpoint-absent");
    assert!(
        fs::read_dir(session.runtime_directory())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn every_reported_process_keeps_its_version_without_local_installation_facts() {
    let fixture = Fixture::private_cohort();
    // The Session's facts do not depend on this executable's installation files.
    fs::remove_file(fixture.cohort_directory().join("cohort.json")).unwrap();
    let handle = peer(&fixture, |stream| answer(stream, &processes()));
    let doc = success(
        fixture.run(&["status", "server", "--format=json"]),
        "server",
    );
    handle.join().unwrap();
    let process = json!({"version":"9.8.7-test","state":"stopping","terminals":3});
    let mut server = process.clone();
    server["role"] = json!("server");
    server["processId"] = json!(42);
    let mut worker = process;
    worker["role"] = json!("worker");
    worker["processId"] = json!(43);
    assert_eq!(
        doc["result"]["server"],
        json!({"state":"running","restartKeepsTerminals":false,"processes":[server, worker]})
    );
    fs::write(
        fixture.cohort_directory().join("cohort.json"),
        b"{\"schemaVersion\":1}",
    )
    .unwrap();
    let client = success(
        fixture.run(&["status", "client", "--format=json"]),
        "client",
    );
    assert_eq!(client["result"]["cohort"]["state"], "unavailable");
    assert_eq!(client["result"]["cohort"]["reason"], "invalid");
    assert_eq!(
        client["result"]["cohort"]["diagnostic"]["ioKind"],
        "invalid-data"
    );
    assert!(client["result"]["cohort"]["diagnostic"]["osError"].is_null());
    assert!(
        client["result"]["cohort"]["diagnostic"]
            .get("missingArtifact")
            .is_none()
    );
}

#[test]
fn offline_client_json_identifies_a_sibling_proven_missing_by_open() {
    let fixture = Fixture::private_cohort();
    let server = orchbee_local_runtime::CohortArtifact::Server.file_name();
    fs::remove_file(fixture.cohort_directory().join(&server)).unwrap();
    let json_output = fixture.run(&["status", "client", "--format=json"]);
    let client = success(json_output, "client");
    assert_eq!(client["version"], 1);
    assert_eq!(client["result"]["cohort"]["state"], "unavailable");
    assert_eq!(client["result"]["cohort"]["reason"], "missing");
    assert_eq!(
        client["result"]["cohort"]["diagnostic"]["ioKind"],
        "not-found"
    );
    assert!(client["result"]["cohort"]["diagnostic"]["osError"].is_i64());
    assert_eq!(
        client["result"]["cohort"]["diagnostic"]["missingArtifact"],
        server.as_str()
    );

    let text = fixture.run(&["status", "client", "--format=text"]);
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(!text.contains("NotFound"));
    assert!(!text.contains("missingArtifact"));
    assert!(!text.contains(&server));

    // The localized copy names the missing file and points at the JSON diagnostic.
    let mut texts = Vec::new();
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = fixture.run_with_env(
            &["server", "start"],
            None,
            None,
            &[("LC_ALL", std::ffi::OsStr::new(locale))],
        );
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains(&format!("`{server}`")), "{text}");
        assert!(
            text.contains("orchbee status client --format json"),
            "{text}"
        );
        texts.push(text);
    }
    assert_ne!(texts[0], texts[1]);
    let failure = fixture.run(&["server", "start", "--format=json"]);
    let failure = document(failure, 4, "orchbee.cli.failure");
    assert_eq!(failure["failure"]["details"]["stage"], "cohort");
    assert!(failure["failure"].get("ioKind").is_none());
    assert!(failure["failure"].get("osError").is_none());
}

#[test]
fn old_or_foreign_sessions_and_wrong_or_partial_replies_are_not_absence() {
    let fixture = Fixture::new();
    // A Session that predates the handshake refuses the frame with its old OBLC rejection.
    let handle = peer(&fixture, |stream| {
        write_message(
            stream,
            &LocalClientMessage::Rejected {
                reason: orchbee_local_client_protocol::RejectionReason::MalformedFrame,
            },
        )
        .unwrap()
    });
    let doc = document(
        fixture.run(&["status", "server", "--format=json"]),
        6,
        "orchbee.cli.failure",
    );
    handle.join().unwrap();
    assert_eq!(doc["failure"]["kind"], "incompatible");
    assert_eq!(doc["failure"]["details"]["reason"], "session-too-old");
    assert!(doc["failure"]["details"].get("sessionVersion").is_none());
    // A Session of another environment refuses the request.
    let handle = peer(&fixture, |stream| {
        answer(
            stream,
            &rejection(RejectionReason::EnvironmentMismatch, "orchbee"),
        )
    });
    let doc = document(
        fixture.run(&["status", "server", "--format=json"]),
        6,
        "orchbee.cli.failure",
    );
    handle.join().unwrap();
    assert_eq!(
        doc["failure"]["details"],
        json!({"stage":"admission","reason":"environment-mismatch"})
    );
    // A reply to another request is not a query result.
    let handle = peer(&fixture, |stream| {
        answer(
            stream,
            &AcceptorMessage::ExitResult(orchbee_connection_handshake::ExitResult::default()),
        )
    });
    let doc = document(
        fixture.run(&["status", "server", "--format=json"]),
        5,
        "orchbee.cli.failure",
    );
    handle.join().unwrap();
    assert_eq!(doc["failure"]["kind"], "protocol-invalid");
    let handle = peer(&fixture, |stream| {
        stream.shutdown(std::net::Shutdown::Both).unwrap()
    });
    let doc = document(
        fixture.run(&["status", "server", "--format=json"]),
        5,
        "orchbee.cli.failure",
    );
    handle.join().unwrap();
    assert_eq!(doc["failure"]["kind"], "transport-loss");
    let handle = peer(&fixture, |stream| {
        stream.write_all(b"O").unwrap();
    });
    let started = Instant::now();
    let doc = document(
        fixture.run(&["status", "server", "--format=json"]),
        7,
        "orchbee.cli.failure",
    );
    handle.join().unwrap();
    assert_eq!(doc["failure"]["kind"], "timeout");
    assert!(started.elapsed() < Duration::from_secs(15));
}

#[test]
fn localized_text_lists_each_process_and_json_is_locale_independent() {
    let fixture = Fixture::new();
    let mut outputs = Vec::new();
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = crate::staged_client::orchbee_command(fixture.executable())
            .args(["status", "client", "--format=json"])
            .env("LC_ALL", locale)
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .env("USERPROFILE", fixture.profile())
            .output()
            .unwrap();
        assert!(output.status.success());
        outputs.push(output.stdout);
    }
    assert_eq!(outputs[0], outputs[1]);
    for (locale, label) in [
        ("en_US.UTF-8", "Restart keeps terminals"),
        ("zh_CN.UTF-8", "重启保留终端"),
    ] {
        let handle = peer(&fixture, |stream| answer(stream, &processes()));
        let output = crate::staged_client::orchbee_command(fixture.executable())
            .args(["status", "server"])
            .env("LC_ALL", locale)
            .env("ORCHBEE_RUNTIME_BASE", fixture.0.join("runtime"))
            .env("USERPROFILE", fixture.profile())
            .output()
            .unwrap();
        handle.join().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(label), "{text}");
        assert!(!text.contains(['\x1b', '\x07', '\r']));
        assert_eq!(text.matches("9.8.7-test").count(), 2, "{text}");
    }
}

#[test]
fn grammar_help_and_schema_stay_offline() {
    let fixture = Fixture::new();
    for arguments in [
        vec!["status", "server", "extra"],
        vec!["status", "client", "--current"],
        vec!["status", "server", "--project=p"],
        vec!["status", "client", "--timeout=1"],
        vec!["status", "server", "--format=csv"],
    ] {
        let output = fixture.run(&arguments);
        assert_eq!(output.status.code(), Some(2));
    }
    for scope in ["server", "client"] {
        for arguments in [
            vec!["help", "status", scope],
            vec!["status", scope, "--help"],
        ] {
            let output = fixture.run(&arguments);
            assert!(output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains(&format!("orchbee.status.{scope}"))
            );
        }
    }
    assert!(!fixture.0.join("runtime").exists());
}
