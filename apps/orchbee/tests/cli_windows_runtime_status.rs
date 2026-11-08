//! Metadata is readable before cohort admission and never activates a Session.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_client_server_protocol::PROTOCOL_VERSION as TERMINAL_PROTOCOL_VERSION;
use orchbee_local_client_protocol::{
    ClientHello, LocalClientMessage, RejectionReason, ServerMetadata, ServerMode, read_message,
    write_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::Cohort;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    num::{NonZeroU8, NonZeroU32},
    process::{Command, Output},
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
        assert_eq!(
            read_message(&mut stream).unwrap(),
            LocalClientMessage::ServerMetadataRequest
        );
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
fn metadata(cohort: &str, protocol: u8) -> LocalClientMessage {
    LocalClientMessage::ServerMetadata(
        ServerMetadata::new(
            "9.8.7-test",
            ClientHello::new(cohort).unwrap(),
            NonZeroU32::new(42).unwrap(),
            NonZeroU8::new(protocol).unwrap(),
            ServerMode::ManagedStopping,
            0x8000_0007,
        )
        .unwrap(),
    )
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
    assert!(server["result"]["compatibility"]["restartRequired"].is_null());
    assert!(!fixture.0.join("runtime").exists());
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
fn observed_server_facts_survive_mismatch_and_missing_local_cohort() {
    let fixture = Fixture::private_cohort();
    let matching = Cohort::load(
        fixture.cohort_directory(),
        Instant::now() + Duration::from_secs(10),
    )
    .unwrap()
    .identity()
    .to_owned();
    for (cohort, protocol, compatible) in [
        (matching.as_str(), TERMINAL_PROTOCOL_VERSION, true),
        ("other-server", TERMINAL_PROTOCOL_VERSION, false),
        (matching.as_str(), 2, false),
    ] {
        let reply = metadata(cohort, protocol);
        let handle = peer(&fixture, move |stream| {
            write_message(stream, &reply).unwrap()
        });
        let doc = success(
            fixture.run(&["status", "server", "--format=json"]),
            "server",
        );
        handle.join().unwrap();
        let result = &doc["result"];
        assert_eq!(result["server"]["version"], "9.8.7-test");
        assert_eq!(result["server"]["processId"], 42);
        assert_eq!(result["server"]["stopping"], true);
        assert_eq!(
            result["server"]["capabilities"]["unknownBits"],
            0x8000_0000u32
        );
        assert_eq!(result["server"]["capabilities"]["liveHandoff"], false);
        assert_eq!(result["compatibility"]["compatible"], compatible);
        assert_eq!(result["compatibility"]["restartRequired"], !compatible);
    }
    fs::remove_file(fixture.cohort_directory().join("cohort.json")).unwrap();
    let reply = metadata(&matching, TERMINAL_PROTOCOL_VERSION);
    let handle = peer(&fixture, move |stream| {
        write_message(stream, &reply).unwrap()
    });
    let doc = success(
        fixture.run(&["status", "server", "--format=json"]),
        "server",
    );
    handle.join().unwrap();
    assert_eq!(
        doc["result"]["localCohort"],
        json!({"state":"unavailable","reason":"missing"})
    );
    assert_eq!(doc["result"]["server"]["version"], "9.8.7-test");
    assert!(doc["result"]["compatibility"]["restartRequired"].is_null());
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

    for (locale, expected) in [
        (
            "en_US.UTF-8",
            format!(
                "Required native sibling `{server}` is missing from the development cohort. Rebuild the development cohort or reinstall the complete application. Run `orchbee status client --format json` for I/O details.\n"
            ),
        ),
        (
            "zh_CN.UTF-8",
            format!(
                "原生开发 cohort 缺少必需的同级文件 `{server}`。请重新构建完整 cohort 或重新安装完整应用。运行 `orchbee status client --format json` 查看 I/O 详情。\n"
            ),
        ),
    ] {
        let output = fixture.run_with_env(
            &["server", "start"],
            None,
            None,
            &[("LC_ALL", std::ffi::OsStr::new(locale))],
        );
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8(output.stderr).unwrap(), expected);
    }
    let failure = fixture.run(&["server", "start", "--format=json"]);
    let failure = document(failure, 4, "orchbee.cli.failure");
    assert_eq!(failure["failure"]["details"]["stage"], "cohort");
    assert!(failure["failure"].get("ioKind").is_none());
    assert!(failure["failure"].get("osError").is_none());
}

#[test]
fn old_rejections_and_wrong_or_partial_replies_are_not_absence() {
    let fixture = Fixture::new();
    for reason in [
        RejectionReason::WrongMessage,
        RejectionReason::MalformedFrame,
    ] {
        let handle = peer(&fixture, move |stream| {
            write_message(stream, &LocalClientMessage::Rejected { reason }).unwrap()
        });
        let doc = document(
            fixture.run(&["status", "server", "--format=json"]),
            6,
            "orchbee.cli.failure",
        );
        handle.join().unwrap();
        assert_eq!(doc["failure"]["kind"], "incompatible");
    }
    let handle = peer(&fixture, |stream| {
        write_message(stream, &LocalClientMessage::ServerMetadataRequest).unwrap()
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
fn localized_text_escapes_peer_control_data_and_json_is_locale_independent() {
    let fixture = Fixture::new();
    let mut outputs = Vec::new();
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = Command::new(fixture.executable())
            .args(["status", "client", "--format=json"])
            .env("LC_ALL", locale)
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .output()
            .unwrap();
        assert!(output.status.success());
        outputs.push(output.stdout);
    }
    assert_eq!(outputs[0], outputs[1]);
    for (locale, label) in [
        ("en_US.UTF-8", "Server version"),
        ("zh_CN.UTF-8", "Server 版本"),
    ] {
        let reply = metadata(
            "unsafe\x1b]52;c;bad\x07\r\ncohort",
            TERMINAL_PROTOCOL_VERSION,
        );
        let handle = peer(&fixture, move |stream| {
            write_message(stream, &reply).unwrap()
        });
        let output = Command::new(fixture.executable())
            .args(["status", "server"])
            .env("LC_ALL", locale)
            .env("ORCHBEE_RUNTIME_BASE", fixture.0.join("runtime"))
            .output()
            .unwrap();
        handle.join().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(label));
        assert!(!text.contains(['\x1b', '\x07', '\r']));
        assert!(text.contains("9.8.7-test"));
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
