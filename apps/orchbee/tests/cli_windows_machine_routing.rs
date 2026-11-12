//! Actual native relay boundary; owned local fixtures, no SSH configuration changes.
#![cfg(windows)]
use crate::peer_support as support;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use orchbee_local_runtime::{PrivateCache, run_windows_command_exchange};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::{Fixture, validate};

fn packet(args: &[&str]) -> Value {
    json!({"version":1,"session":"route-test","arguments":args,"input":null,"locale":"en_US.UTF-8"})
}
fn relay(fixture: &Fixture, packet: &Value, shell: bool) -> Value {
    relay_with_environment(fixture, packet, shell, &[])
}
fn relay_with_environment(
    fixture: &Fixture,
    packet: &Value,
    shell: bool,
    environment: &[(&str, &std::ffi::OsStr)],
) -> Value {
    let executable = fixture.executable();
    let mut command = if shell {
        let mut c = crate::staged_client::orchbee_command(
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
        let mut c = crate::staged_client::orchbee_command(executable);
        c.arg("--machine-relay-v1");
        c
    };
    command
        .env("ORCHBEE_RUNTIME_BASE", fixture.0.join("runtime"))
        .env("APPDATA", fixture.profile())
        .env("USERPROFILE", fixture.profile())
        .env_remove("ORCHBEE_CONFIG_FILE")
        .env("ORCHBEE_TERMINAL_ID", "invalid-caller")
        .env("ORCHBEE_WORKER_ID", "invalid-caller")
        .envs(environment.iter().copied());
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

/// An invalid profile that only home expansion sees. The data root holds configuration and the
/// machine catalog, so naming one outside the profile keeps them valid.
fn invalid_home(data_root: &std::path::Path) -> [(&'static str, &std::ffi::OsStr); 2] {
    [
        ("USERPROFILE", std::ffi::OsStr::new("private-invalid-home")),
        ("ORCHBEE_HOME", data_root.as_os_str()),
    ]
}

const HOME_PATH_COMMANDS: &[(&[&str], &str)] = &[
    (&["plugin", "link", r"~\remote"], "<path>"),
    (
        &["workspace", "create", "--cwd", r"~\remote"],
        "USERPROFILE",
    ),
    (&["worktree", "list", "--cwd", r"~\remote"], "USERPROFILE"),
    (&["worktree", "create", "--cwd", r"~\remote"], "USERPROFILE"),
    (
        &["worktree", "create", "--path", r"~\remote"],
        "USERPROFILE",
    ),
    (&["worktree", "open", "--path", r"~\remote"], "USERPROFILE"),
];

#[test]
fn home_path_route_conflict_precedes_execution_context_resolution() {
    for (command, _) in HOME_PATH_COMMANDS {
        let fixture = Fixture::new();
        let mut arguments = vec!["--machine", "work", "--session", "local"];
        arguments.extend_from_slice(command);
        arguments.push("--format=json");
        let data_root = fixture.separate_data_root();
        let output = fixture.run_with_env(&arguments, None, None, &invalid_home(&data_root));
        assert_eq!(output.status.code(), Some(2));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(document["failure"]["kind"], "invalid-usage");
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(
            diagnostic.contains("Options '--machine' and '--session' cannot be combined."),
            "{command:?}: {diagnostic}"
        );
        assert!(!diagnostic.contains("private-invalid-home"));
        assert!(!fixture.0.join("appdata").exists());
        assert!(!fixture.0.join("runtime").exists());
    }
}

#[test]
fn home_path_sender_selects_the_machine_without_resolving_its_operand() {
    for (command, _) in HOME_PATH_COMMANDS {
        let fixture = Fixture::new();
        let mut arguments = vec!["--machine", "missing"];
        arguments.extend_from_slice(command);
        arguments.push("--format=json");
        let data_root = fixture.separate_data_root();
        let output = fixture.run_with_env(&arguments, None, None, &invalid_home(&data_root));
        assert_eq!(
            output.status.code(),
            Some(3),
            "{command:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.machine-route.json"),
        );
        assert_eq!(document["failure"]["details"]["reason"], "not-found");
        assert_eq!(
            document["failure"]["details"]["remoteOutcome"],
            "not-submitted"
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-invalid-home"));
        assert!(!fixture.0.join("appdata").exists());
        assert!(!fixture.0.join("runtime").exists());
    }
}

#[test]
fn home_path_relay_leaves_resolution_to_the_business_process() {
    for (command, expected_option) in HOME_PATH_COMMANDS {
        let fixture = Fixture::new();
        let mut arguments = command.to_vec();
        arguments.push("--format=json");
        let data_root = fixture.separate_data_root();
        let reply = relay_with_environment(
            &fixture,
            &packet(&arguments),
            false,
            &invalid_home(&data_root),
        );
        assert_eq!(reply["outcome"], "completed", "{command:?}: {reply}");
        assert_eq!(reply["status"], 2);
        let document: Value =
            serde_json::from_slice(&STANDARD.decode(reply["stdout"].as_str().unwrap()).unwrap())
                .unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(document["failure"]["kind"], "invalid-usage");
        let diagnostic =
            String::from_utf8(STANDARD.decode(reply["stderr"].as_str().unwrap()).unwrap()).unwrap();
        assert!(
            diagnostic.contains(expected_option),
            "{command:?}: {diagnostic}"
        );
        assert!(!diagnostic.contains("private-invalid-home"));
        assert!(!fixture.0.join("appdata").exists());
        assert!(!fixture.0.join("runtime").exists());
    }
}
#[test]
fn relay_round_trip_runs_the_installed_client_and_keeps_success_and_failure_receipts() {
    let fixture = Fixture::new();
    for shell in [false, true] {
        let reply = relay(
            &fixture,
            &packet(&["status", "server", "--format=json"]),
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
            &packet(&["workspace", "list", "--format=json"]),
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
    // Windows PowerShell creates folders under the profile it receives; Orchbee state stays absent.
    assert!(!fixture.data_root().path().exists());
}
#[test]
fn relay_refuses_recursion_unread_generations_and_local_file_authority() {
    let fixture = Fixture::new();
    for (member, value) in [("cohort", json!("0".repeat(64))), ("version", json!(2))] {
        let mut refused = packet(&["status", "server"]);
        refused[member] = value;
        assert_eq!(relay(&fixture, &refused, false)["outcome"], "not-submitted");
    }
    for args in [
        vec!["--machine=x", "status"],
        vec!["--machine-relay-v1"],
        vec!["machine", "remove", "11111111111111111111111111111111"],
        vec!["terminal", "send", "@terminal:0x22", "--file=secret"],
        vec!["tui"],
    ] {
        assert_eq!(
            relay(&fixture, &packet(&args), false)["outcome"],
            "not-submitted",
            "{args:?}"
        );
    }
    assert!(!fixture.0.join("runtime").exists());
}
#[test]
fn routing_help_and_selection_refuse_before_ssh_or_session_effects() {
    let fixture = Fixture::new();
    assert!(
        fixture
            .run(&["--machine=missing", "workspace", "--help"])
            .status
            .success()
    );
    let root = fixture.data_root().state();
    let mut cache = PrivateCache::acquire(&root, "machines").unwrap();
    cache.replace("machines.json",&serde_json::to_vec(&json!({"version":1,"revision":1,"profiles":[
        {"id":"11111111111111111111111111111111","label":"same","connection":{"kind":"ssh","target":"example.invalid"},"session":"work","enabled":false,"remoteInstall":"manual"},
        {"id":"22222222222222222222222222222222","label":"same","connection":{"kind":"ssh","target":"example.invalid"},"session":"work","enabled":true,"remoteInstall":"manual"}
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

#[test]
fn home_path_worktree_grammar_precedes_execution_context_resolution() {
    for (arguments, expected) in [
        (
            vec![
                "worktree",
                "open",
                "--path",
                r"~\remote",
                "--branch",
                "main",
                "--format=json",
            ],
            "Options '--path' and '--branch' cannot be combined.",
        ),
        (
            vec![
                "worktree",
                "create",
                "--cwd",
                r"~\remote",
                "--timeout",
                "0",
                "--format=json",
            ],
            "--timeout",
        ),
    ] {
        let fixture = Fixture::new();
        let output = fixture.run_with_env(
            &arguments,
            None,
            None,
            &[("USERPROFILE", std::ffi::OsStr::new("private-invalid-home"))],
        );
        assert_eq!(output.status.code(), Some(2));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
        );
        assert_eq!(document["failure"]["kind"], "invalid-usage");
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains(expected), "{arguments:?}: {diagnostic}");
        assert!(!diagnostic.contains("private-invalid-home"));
        assert!(!fixture.0.join("appdata").exists());
        assert!(!fixture.0.join("runtime").exists());
    }
}
