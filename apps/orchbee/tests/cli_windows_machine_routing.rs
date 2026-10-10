//! Actual native relay boundary; owned local fixtures, no SSH configuration changes.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use orchbee_local_site_runtime::{NativeCohort, WindowsPrivateCache, run_windows_command_exchange};
use serde_json::{Value, json};
use std::{
    process::Command,
    time::{Duration, Instant},
};
use support::{Fixture, validate};

fn packet(fixture: &Fixture, args: &[&str]) -> Value {
    let cohort = NativeCohort::load(
        fixture.cohort_directory(),
        Instant::now() + Duration::from_secs(15),
    )
    .unwrap();
    json!({"version":1,"cohort":cohort.identity(),"site":"route-test","arguments":args,"input":null,"locale":"en_US.UTF-8"})
}
fn relay(fixture: &Fixture, packet: &Value, shell: bool) -> Value {
    let executable = fixture.executable();
    let mut command = if shell {
        let mut c = Command::new(
            std::path::PathBuf::from(winsafe::GetSystemDirectory().unwrap())
                .join("WindowsPowerShell/v1.0/powershell.exe"),
        );
        let script = format!(
            "$ErrorActionPreference='Stop';try{{& '{}' '--machine-relay-v1';exit $LASTEXITCODE}}catch{{exit 125}}",
            executable.to_str().unwrap().replace('\'', "''")
        );
        c.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &STANDARD.encode(
                script
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            ),
        ]);
        c
    } else {
        let mut c = Command::new(executable);
        c.arg("--machine-relay-v1");
        c
    };
    command
        .env("ORCHBEE_RUNTIME_BASE", fixture.0.join("runtime"))
        .env("APPDATA", fixture.0.join("appdata"))
        .env_remove("ORCHBEE_CONFIG_FILE")
        .env("ORCHBEE_TERMINAL_ID", "invalid-caller")
        .env("ORCHBEE_WORKER_ID", "invalid-caller");
    let result = run_windows_command_exchange(
        command,
        Some(&serde_json::to_vec(packet).unwrap()),
        24 * 1024 * 1024,
        Some(Instant::now() + Duration::from_secs(30)),
        || Ok(false),
    );
    assert_eq!(result.failure, None, "{result:?}");
    assert_eq!(result.exit_code, Some(0));
    serde_json::from_slice(&result.stdout).unwrap()
}
#[test]
fn relay_round_trip_runs_exact_cohort_and_keeps_success_and_failure_receipts() {
    let fixture = Fixture::new();
    for shell in [false, true] {
        let reply = relay(
            &fixture,
            &packet(&fixture, &["status", "server", "--format=json"]),
            shell,
        );
        assert_eq!(reply["outcome"], "completed", "{reply}");
        assert_eq!(reply["status"], 0);
        let result: Value =
            serde_json::from_slice(&STANDARD.decode(reply["stdout"].as_str().unwrap()).unwrap())
                .unwrap();
        assert_eq!(result["schema"], "orchbee.status.server");
        validate(
            &result,
            include_str!("../src/cli/schemas/orchbee.status.server.json"),
        );
        let reply = relay(
            &fixture,
            &packet(&fixture, &["workspace", "list", "--format=json"]),
            shell,
        );
        assert_eq!(reply["outcome"], "completed", "{reply}");
        assert_ne!(reply["status"], 0);
        assert!(
            !STANDARD
                .decode(reply["stderr"].as_str().unwrap())
                .unwrap()
                .is_empty()
        );
    }
    assert!(!fixture.0.join("appdata").exists());
}
#[test]
fn relay_refuses_recursion_wrong_cohort_and_local_file_authority() {
    let fixture = Fixture::new();
    let mut value = packet(&fixture, &["status", "server"]);
    value["cohort"] = json!("0".repeat(64));
    assert_eq!(relay(&fixture, &value, false)["outcome"], "not-submitted");
    for args in [
        vec!["--machine=x", "status"],
        vec!["--machine-relay-v1"],
        vec!["machine", "remove", "11111111111111111111111111111111"],
        vec!["terminal", "send", "@terminal:22", "--file=secret"],
        vec!["tui"],
    ] {
        assert_eq!(
            relay(&fixture, &packet(&fixture, &args), false)["outcome"],
            "not-submitted"
        );
    }
    assert!(!fixture.0.join("runtime").exists());
}
#[test]
fn routing_help_and_selection_refuse_before_ssh_or_site_effects() {
    let fixture = Fixture::new();
    assert!(
        fixture
            .run(&["--machine=missing", "workspace", "--help"])
            .status
            .success()
    );
    let root = fixture.0.join("appdata/Orchbee/machines");
    let mut cache = WindowsPrivateCache::acquire(&root).unwrap();
    cache.replace("catalog.json",&serde_json::to_vec(&json!({"schemaVersion":1,"revision":1,"profiles":[
        {"id":"11111111111111111111111111111111","label":"same","target":"example.invalid","session":"work","enabled":false},
        {"id":"22222222222222222222222222222222","label":"same","target":"example.invalid","session":"work","enabled":true}
    ]})).unwrap()).unwrap();
    drop(cache);
    for (selector, reason) in [
        ("same", "ambiguous-machine"),
        ("11111111111111111111111111111111", "disabled-machine"),
        ("missing", "not-found"),
    ] {
        let output = fixture.run(&["--machine", selector, "workspace", "list", "--format=json"]);
        assert_eq!(output.status.code(), Some(3));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &value,
            include_str!("../src/cli/schemas/orchbee.cli.failure.machine-route.json"),
        );
        assert_eq!(value["failure"]["details"]["reason"], reason);
        assert_eq!(
            value["failure"]["details"]["remoteOutcome"],
            "not-submitted"
        );
    }
    assert!(!fixture.0.join("runtime").exists());
}
