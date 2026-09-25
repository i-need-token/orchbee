//! Metadata is readable before cohort admission and never activates a Site.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use orchbee_local_client_protocol::{
    ClientHello, LocalClientMessage, RejectionReason, ServerMetadata, ServerMode, read_message,
    write_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_site_runtime::NativeCohort;
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

fn document(output: Output, status: i32, schema: &str) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    let exported = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args(["api", "schema", schema])
        .output()
        .unwrap();
    assert!(exported.status.success());
    let catalog: Value = serde_json::from_slice(&exported.stdout).unwrap();
    let validator = jsonschema::draft202012::new(&catalog["result"]["schema"]["document"]).unwrap();
    validator.validate(&doc).unwrap();
    doc
}
fn success(output: Output, scope: &str) -> Value {
    document(output, 0, &format!("orchbee.status.{scope}"))
}
fn peer(
    fixture: &Fixture,
    action: impl FnOnce(&mut LocalStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    let site = fixture.rendezvous();
    let listener = LocalListener::bind(site.client_socket_path()).unwrap();
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
    let site = fixture.rendezvous();
    let server = success(
        fixture.run(&["status", "server", "--format=json"]),
        "server",
    );
    assert_eq!(server["result"]["server"]["reason"], "endpoint-absent");
    assert!(
        fs::read_dir(site.runtime_directory())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn observed_server_facts_survive_mismatch_and_missing_local_cohort() {
    let fixture = Fixture::new();
    let matching = NativeCohort::load(&fixture.0, Instant::now() + Duration::from_secs(10))
        .unwrap()
        .identity()
        .to_owned();
    for (cohort, protocol, compatible) in [
        (matching.as_str(), 2, true),
        ("other-server", 2, false),
        (matching.as_str(), 3, false),
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
    fs::remove_file(fixture.0.join("cohort.json")).unwrap();
    let reply = metadata(&matching, 2);
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
    fs::write(fixture.0.join("cohort.json"), b"{\"schemaVersion\":1}").unwrap();
    let client = success(
        fixture.run(&["status", "client", "--format=json"]),
        "client",
    );
    assert_eq!(
        client["result"]["cohort"],
        json!({"state":"unavailable","reason":"invalid"})
    );
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
        let output = Command::new(fixture.0.join("orchbee.exe"))
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
        let reply = metadata("unsafe\x1b]52;c;bad\x07\r\ncohort", 2);
        let handle = peer(&fixture, move |stream| {
            write_message(stream, &reply).unwrap()
        });
        let output = Command::new(fixture.0.join("orchbee.exe"))
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
