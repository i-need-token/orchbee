//! The near side of a routed command before anything reaches a hub; owned local fixtures, no SSH
//! configuration changes.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_local_runtime::PrivateCache;
use serde_json::{Value, json};
use support::{Fixture, validate};

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
