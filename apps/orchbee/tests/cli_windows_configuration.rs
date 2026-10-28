//! Persistent configuration through real Windows CLI processes and scripted IPC.
#![cfg(windows)]
use crate::peer_support as support;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
use support::{Fixture, assert_closed, configuration_reply, peer, validate};

#[test]
fn channel_set_persists_before_failed_update_and_never_activates_a_session() {
    use std::ffi::OsStr;
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    let install = fixture.0.join("installation");
    let schema = include_str!("../src/cli/schemas/orchbee.update.json");
    let env = [
        ("ORCHBEE_UPDATE_MANIFEST", OsStr::new("")),
        ("ORCHBEE_INSTALL_DIR", install.as_os_str()),
    ];
    let first = fixture.run_with_env(&["update", "--format=json"], None, None, &env);
    assert_eq!(first.status.code(), Some(1));
    let first = document(first);
    validate(&first, schema);
    assert_eq!(first["result"]["error"], "source-unconfigured");
    assert_eq!(first["result"]["channelSaved"], false);
    assert!(!config.exists());
    assert!(!install.exists());
    let changed = fixture.run_with_env(
        &["channel", "set", "preview", "--format=json"],
        None,
        None,
        &env,
    );
    assert_eq!(changed.status.code(), Some(1));
    let changed = document(changed);
    validate(&changed, schema);
    assert_eq!(changed["operation"], "channel-set");
    assert_eq!(changed["result"]["channelSaved"], true);
    assert_eq!(changed["result"]["channel"], "preview");
    assert_eq!(changed["result"]["error"], "source-unconfigured");
    assert_eq!(
        document(fixture.run(&["channel", "show", "--format=json"]))["result"]["channel"],
        "preview"
    );
    let original = "# keep comment\n[update]\nmanifest_check=false # keep policy\nchannel='preview' # retained channel note\n[terminal]\nnew_cwd='home'\n";
    std::fs::write(&config, original).unwrap();
    let changed = document(fixture.run_with_env(
        &["channel", "set", "stable", "--format=json"],
        None,
        None,
        &env,
    ));
    validate(&changed, schema);
    assert_eq!(changed["result"]["channelSaved"], true);
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(
        text.contains("# keep comment")
            && text.contains("# keep policy")
            && text.contains("# retained channel note")
    );
    assert!(text.contains("manifest_check=false") && text.contains("new_cwd='home'"));
    for (warning, retained) in [
        ("[unknown]\nsecret='kept'", Some("secret='kept'")),
        ("[update]\nchannel=true", None),
    ] {
        std::fs::write(&config, warning).unwrap();
        let output = fixture.run_with_env(
            &["channel", "set", "preview", "--format=json"],
            None,
            None,
            &env,
        );
        assert_eq!(output.status.code(), Some(1));
        let doc = document(output);
        validate(&doc, schema);
        assert_eq!(doc["result"]["channelSaved"], true);
        assert_eq!(doc["result"]["channel"], "preview");
        assert_eq!(doc["result"]["error"], "source-unconfigured");
        let saved = std::fs::read_to_string(&config).unwrap();
        if let Some(retained) = retained {
            assert!(saved.contains(retained));
        }
        assert_eq!(
            document(fixture.run(&["channel", "show", "--format=json"]))["result"]["channel"],
            "preview"
        );
    }
    let invalid = "not TOML";
    std::fs::write(&config, invalid).unwrap();
    let output = fixture.run_with_env(
        &["channel", "set", "preview", "--format=json"],
        None,
        None,
        &env,
    );
    assert_eq!(output.status.code(), Some(1));
    let doc = document(output);
    validate(&doc, schema);
    assert_eq!(doc["result"]["channelSaved"], false);
    assert_eq!(doc["result"]["error"], "configuration");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), invalid);
    for args in [
        vec!["channel", "set"],
        vec!["channel", "set", "nightly"],
        vec!["channel", "set", "stable", "extra"],
        vec!["update", "extra"],
        vec!["update", "--client", "bad"],
    ] {
        assert_eq!(
            fixture.run_with_env(&args, None, None, &env).status.code(),
            Some(2),
            "{args:?}"
        );
    }
    assert!(!install.exists());
    assert!(!fixture.0.join("runtime").exists());
}

#[test]
fn channel_show_uses_the_effective_channel_and_preserves_warning_input() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    let run = || fixture.run(&["channel", "show", "--format=json"]);
    let output = run();
    assert!(output.status.success());
    let doc = document(output);
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.channel.show.json"),
    );
    assert_eq!(doc["result"]["channel"], "stable");
    assert_eq!(doc["result"]["source"], "default");
    assert_eq!(doc["result"]["exists"], false);
    assert!(!config.parent().unwrap().exists());
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let bytes = b"# retained comment\n[update]\nchannel='preview'\nmanifest_check=false\n";
    std::fs::write(&config, bytes).unwrap();
    let output = run();
    assert!(output.status.success());
    let doc = document(output);
    validate(
        &doc,
        include_str!("../src/cli/schemas/orchbee.channel.show.json"),
    );
    assert_eq!(doc["result"]["channel"], "preview");
    assert_eq!(doc["result"]["source"], "file");
    let text = fixture.run(&["channel", "show"]);
    assert!(text.status.success());
    assert_eq!(text.stdout, b"preview\n");
    assert_eq!(std::fs::read(&config).unwrap(), bytes);
    for (content, reason) in [
        ("[update]\nchannel='DO-NOT-ECHO'", "invalid-channel"),
        ("[update]\nchannel=true", "wrong-type"),
    ] {
        std::fs::write(&config, content).unwrap();
        let output = run();
        assert!(output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("DO-NOT-ECHO"));
        let doc = document(output);
        validate(
            &doc,
            include_str!("../src/cli/schemas/orchbee.channel.show.json"),
        );
        assert_eq!(doc["result"]["channel"], "stable");
        assert_eq!(doc["result"]["source"], "default");
        assert_eq!(doc["result"]["exists"], true);
        let checked = config_check_document(&fixture);
        assert_warning_diagnostic(&checked, "update.channel", reason, "defaulted");
        assert_eq!(std::fs::read_to_string(&config).unwrap(), content);
    }
    let missing = fixture.0.join("explicit-missing.toml");
    let output = fixture.run_with_env(
        &["channel", "show", "--format=json"],
        None,
        None,
        &[("ORCHBEE_CONFIG_FILE", missing.as_os_str())],
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(document(output)["failure"]["details"]["reason"], "missing");
    let panel = id(7);
    for invalid in [
        vec!["channel"],
        vec!["channel", "unknown"],
        vec!["channel", "show", "extra"],
        vec!["channel", "show", "--client", &panel],
    ] {
        assert_eq!(fixture.run(&invalid).status.code(), Some(2), "{invalid:?}");
    }
    assert!(!fixture.0.join("runtime").exists());
}

fn id(n: u64) -> String {
    format!("{n:032x}")
}

#[test]
fn sidebar_configuration_check_projects_profiles_and_defaults_invalid_profiles() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let content = "[ui.sidebar.agents]\nrows=[['state_icon','state_text','terminal_title','terminal_title_stripped'],[{token='$issue',fg='#abc',bold=true,rules=[{starts_with='ORCH-',dim=false}]}]]\nrow_gap=1\n[ui.sidebar.agents.rows_by_agent]\nclaude=[['tab','pane']]\n";
    std::fs::write(&config, content).unwrap();
    let doc = document(fixture.run(&["config", "check", "--format=json"]));
    let schema = include_str!("../src/cli/schemas/orchbee.config.check.json");
    validate(&doc, schema);
    assert_eq!(doc["version"], 1);
    let profile = orchbee_local_runtime::AgentSidebarSettings::from_profile(
        doc["result"]["checks"][5]["details"]["agentSidebarProfile"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(profile.row_gap(), 1);
    assert_eq!(
        profile.rows_for_agent("claude", None).unwrap()[0][0].name(),
        "tab"
    );
    assert_eq!(std::fs::read_to_string(&config).unwrap(), content);
    for (token, reason) in [
        ("terminal_title_typo", "invalid-token"),
        ("unknown", "invalid-token"),
    ] {
        let invalid = format!("[ui.sidebar.agents]\nrows=[['{token}']]");
        std::fs::write(&config, &invalid).unwrap();
        let checked = config_check_document(&fixture);
        assert_warning_diagnostic(&checked, "ui.sidebar.agents", reason, "defaulted");
        let profile = orchbee_local_runtime::AgentSidebarSettings::from_profile(
            checked["result"]["checks"][5]["details"]["agentSidebarProfile"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert!(profile.rows_for_agent("codex", None).is_none());
        assert_creation_reaches_missing_session(&fixture);
        assert_eq!(std::fs::read_to_string(&config).unwrap(), invalid);
    }
}
fn document(output: std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn config_check_document(fixture: &Fixture) -> Value {
    let runtime = fixture.0.join("runtime");
    let remove_runtime = !runtime.exists();
    if remove_runtime {
        std::fs::create_dir(&runtime).unwrap();
    }
    let output = fixture.run(&["config", "check", "--format=json"]);
    if remove_runtime {
        std::fs::remove_dir(&runtime).unwrap();
    }
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = document(output);
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.config.check.json"),
    );
    assert_eq!(document["version"], 1);
    document
}

#[test]
fn generated_default_config_passes_the_offline_config_check() {
    let fixture = Fixture::new();
    let generated = fixture.run(&["--default-config"]);
    assert_eq!(generated.status.code(), Some(0));
    assert!(generated.stderr.is_empty());
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, generated.stdout).unwrap();

    let document = config_check_document(&fixture);
    let user_config = &document["result"]["checks"][5];
    assert_eq!(user_config["name"], "user-config");
    assert_eq!(user_config["status"], "pass");
    assert_eq!(user_config["details"]["diagnostics"], json!([]));
    assert_eq!(user_config["details"]["keyOverrides"], 0);

    let generated = std::fs::read_to_string(&config).unwrap();
    let actions = orchbee_local_runtime::KeySettings::registered_actions();
    let mut uncommented = String::new();
    for line in generated.lines() {
        let action_line = line.strip_prefix("# ").filter(|candidate| {
            actions.iter().any(|action| {
                let prefix = format!("{action} = ");
                candidate.starts_with(prefix.as_str())
            })
        });
        uncommented.push_str(action_line.unwrap_or(line));
        uncommented.push('\n');
    }
    std::fs::write(&config, uncommented).unwrap();
    let document = config_check_document(&fixture);
    let user_config = &document["result"]["checks"][5];
    assert_eq!(actions.len(), 37);
    assert_eq!(user_config["status"], "warning");
    assert_eq!(user_config["details"]["keyOverrides"], json!(actions.len()));
    assert_eq!(user_config["details"]["diagnosticsOmitted"], 0);
    assert_eq!(
        user_config["details"]["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    for action in [
        "focus_agent",
        "resize_pane_down",
        "resize_pane_left",
        "resize_pane_right",
        "resize_pane_up",
    ] {
        assert_warning_diagnostic(
            &document,
            &format!("keys.{action}"),
            "invalid-binding",
            "disabled",
        );
    }
}

fn assert_warning_diagnostic(document: &Value, field: &str, reason: &str, disposition: &str) {
    assert_eq!(document["result"]["ok"], true);
    let check = &document["result"]["checks"][5];
    assert_eq!(check["name"], "user-config");
    assert_eq!(check["status"], "warning");
    assert!(
        check["details"]["diagnostics"]
            .as_array()
            .unwrap()
            .contains(&json!({
                "field": field,
                "reason": reason,
                "disposition": disposition,
            })),
        "{check}"
    );
}

fn assert_creation_reaches_missing_session(fixture: &Fixture) {
    std::fs::create_dir_all(fixture.0.join("runtime")).unwrap();
    let output = fixture.run(&["workspace", "create", "--cwd=C:/explicit", "--format=json"]);
    assert_eq!(
        output.status.code(),
        Some(4),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let failure = document(output);
    validate(
        &failure,
        include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
    );
    assert_eq!(failure["failure"]["kind"], "service-unavailable");
}
fn snapshot(stream: &mut LocalStream, project: bool) {
    let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot");
    };
    let snapshot: Snapshot = serde_json::from_value(json!({"protocolVersion":1,"sessionId":id(2),"revision":"12","navigation":{"projects":if project {vec![json!({"projectId":id(3),"label":"project"})]} else {vec![]},"directories":if project {vec![json!({"directoryId":id(11),"workerId":id(9),"path":"C:/repo"})]} else {vec![]},"workspaces":if project {vec![json!({"workspaceId":id(4),"projectId":id(3),"label":"source","primaryDirectoryId":id(11),"terminalContextId":id(5)})]} else {vec![]}},"terminalContexts":if project {vec![json!({"terminalContextId":id(5),"groups":[],"tabs":[{"tabId":id(6),"label":"shell","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":id(7),"primaryPresentation":{"customLabel":null,"presentationId":id(8),"label":"shell","binding":null},"childPresentations":[]}}]})]} else {vec![]}})).unwrap();
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    )
    .unwrap();
}

#[test]
fn file_level_configuration_errors_fail_the_offline_check_but_never_creation() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.0.join("runtime")).unwrap();
    let config = fixture.0.join("config.toml");
    let check_schema = include_str!("../src/cli/schemas/orchbee.config.check.json");
    let check = |reason: &str| {
        let environment = [("ORCHBEE_CONFIG_FILE", config.as_os_str())];
        let checked = fixture.run_with_env(
            &["config", "check", "--format=json"],
            None,
            None,
            &environment,
        );
        assert_eq!(checked.status.code(), Some(1));
        let checked = document(checked);
        validate(&checked, check_schema);
        assert_eq!(checked["version"], 1);
        assert_eq!(checked["result"]["ok"], false);
        assert_eq!(checked["result"]["checks"][5]["status"], "fail");
        assert_eq!(
            checked["result"]["checks"][5]["details"],
            json!({"field": "file", "reason": reason})
        );
        // Creation never reads the file: it proceeds to the Session, which is absent here.
        for args in [
            vec!["workspace", "create", "--format=json"],
            vec!["tab", "create", "--workspace", "Workspace", "--format=json"],
            vec![
                "panel",
                "split",
                "--direction=right",
                "00000000000000000000000000000007",
                "--format=json",
            ],
            vec!["worktree", "create", "--cwd=C:/repo", "--format=json"],
        ] {
            let output = fixture.run_with_env(&args, None, None, &environment);
            assert_eq!(
                output.status.code(),
                Some(4),
                "{args:?}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let doc = document(output);
            assert_eq!(
                doc["failure"]["kind"], "service-unavailable",
                "{args:?}: {doc}"
            );
        }
    };
    for (bytes, reason) in [
        (vec![b'a'; 65537], "too-large"),
        (vec![0xff], "invalid-utf8"),
        (
            b"[terminal]\nnew_cwd='home'\nnew_cwd='follow'".to_vec(),
            "invalid-toml",
        ),
    ] {
        std::fs::write(&config, bytes).unwrap();
        check(reason);
    }
    std::fs::remove_file(&config).unwrap();
    check("missing");
    assert!(!config.exists());
    std::fs::create_dir(&config).unwrap();
    check("not-file");
    std::fs::remove_dir(&config).unwrap();
}

#[test]
fn field_errors_warn_default_or_disable_only_the_affected_setting() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    for (text, field, reason, disposition) in [
        (
            "unknown = 'DO-NOT-ECHO'",
            "unknown",
            "unknown-key",
            "ignored",
        ),
        (
            "[ui.toast]\nenabled=true",
            "ui.toast.enabled",
            "unknown-key",
            "ignored",
        ),
        (
            "[ui.toast]\ndelivery='invalid-provider'",
            "ui.toast.delivery",
            "invalid-mode",
            "defaulted",
        ),
        (
            "[terminal]\nnew_cwd=1",
            "terminal.new_cwd",
            "wrong-type",
            "defaulted",
        ),
        (
            "[terminal]\ndefault_shell='pwsh.exe'",
            "terminal.default_shell",
            "unknown-key",
            "ignored",
        ),
        (
            "[keys]\nhelp='ctrl+d'",
            "keys.help",
            "unsafe-binding",
            "disabled",
        ),
        (
            "[keys]\nhelp='prefix+Q'\nnew_tab='prefix+shift+q'",
            "keys.new_tab",
            "duplicate-binding",
            "disabled",
        ),
    ] {
        std::fs::write(&config, text).unwrap();
        let checked = config_check_document(&fixture);
        assert_warning_diagnostic(&checked, field, reason, disposition);
        assert_eq!(
            checked["result"]["checks"][5]["details"]["diagnosticsOmitted"],
            0
        );
        assert!(!checked.to_string().contains("DO-NOT-ECHO"));
        assert_creation_reaches_missing_session(&fixture);
        assert_eq!(std::fs::read_to_string(&config).unwrap(), text);
    }
}

#[test]
fn config_check_bounds_diagnostics_and_reports_the_exact_omitted_count() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let text = (0..130)
        .map(|index| format!("unknown_{index:03}=true"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&config, text).unwrap();
    let checked = config_check_document(&fixture);
    let details = &checked["result"]["checks"][5]["details"];
    assert_eq!(checked["result"]["checks"][5]["status"], "warning");
    assert_eq!(details["diagnostics"].as_array().unwrap().len(), 128);
    assert_eq!(details["diagnosticsOmitted"], 2);
    assert_eq!(
        details["diagnostics"][0],
        json!({
            "field": "unknown_000",
            "reason": "unknown-key",
            "disposition": "ignored",
        })
    );
    assert_eq!(details["diagnostics"][127]["field"], "unknown_127");
}

#[test]
fn config_check_reports_the_actual_file_without_creating_it() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    let schema = include_str!("../src/cli/schemas/orchbee.config.check.json");
    let doc = document(fixture.run(&["config", "check", "--format=json"]));
    validate(&doc, schema);
    let check = &doc["result"]["checks"][5];
    assert_eq!(check["details"]["manifestCheck"], true);
    assert_eq!(check["details"]["manifestCheckSource"], "default");
    assert_eq!(check["details"]["exists"], false);
    assert!(!config.exists());
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "[terminal]\nnew_cwd='home'\n[worktrees]\ndirectory='../trees'\n[update]\nmanifest_check=false",
    )
    .unwrap();
    let doc = document(fixture.run(&["config", "check", "--format=json"]));
    validate(&doc, schema);
    let check = &doc["result"]["checks"][5];
    assert_eq!(check["status"], "pass");
    assert_eq!(check["details"]["newCwd"], "home");
    assert_eq!(check["details"]["manifestCheck"], false);
    assert_eq!(check["details"]["manifestCheckSource"], "file");
    assert_eq!(
        std::path::Path::new(check["details"]["worktreeDirectory"].as_str().unwrap()),
        fixture.0.join(r"appdata\trees")
    );
    std::fs::write(&config, "[worktrees]\ndirectory=3").unwrap();
    let doc = config_check_document(&fixture);
    assert_warning_diagnostic(&doc, "worktrees.directory", "wrong-type", "defaulted");
    assert_eq!(
        doc["result"]["checks"][5]["details"]["worktreeDirectory"],
        Value::Null
    );
}

#[test]
fn config_check_validates_current_key_policy() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "[keys]\nprefix='ctrl+a'\nhelp=['prefix+?', 'f1']\nswitch_tab='alt+1..9'\nnew_tab=[]",
    )
    .unwrap();
    let doc = config_check_document(&fixture);
    assert_eq!(doc["result"]["checks"][5]["status"], "warning");
    assert_eq!(doc["result"]["checks"][5]["details"]["keyPrefix"], "Ctrl+A");
    assert_eq!(doc["result"]["checks"][5]["details"]["keyOverrides"], 3);
    assert_warning_diagnostic(&doc, "keys.new_tab", "invalid-binding", "disabled");
    std::fs::write(&config, "[keys]\nhelp='ctrl+c'").unwrap();
    let doc = config_check_document(&fixture);
    assert_warning_diagnostic(&doc, "keys.help", "unsafe-binding", "disabled");
    assert_eq!(doc["result"]["checks"][5]["details"]["keyOverrides"], 1);
}

#[test]
fn workspace_tab_and_panel_creation_apply_the_session_cwd_and_allow_override() {
    for (args, expected) in [
        (
            vec!["workspace", "create", "--format=json"],
            "C:/configured",
        ),
        (
            vec!["tab", "create", "--workspace=source", "--format=json"],
            "C:/configured",
        ),
        (
            vec![
                "panel",
                "split",
                "--direction=right",
                "00000000000000000000000000000007",
                "--format=json",
            ],
            "C:/configured",
        ),
        (
            vec!["workspace", "create", "--cwd=C:/explicit", "--format=json"],
            "C:/explicit",
        ),
    ] {
        let fixture = Fixture::new();
        let config = fixture.0.join("config.toml");
        std::fs::write(&config, "[terminal]\nnew_cwd='C:/file-only'").unwrap();
        let rendezvous = fixture.rendezvous();
        let handler = peer(
            &fixture,
            LocalListener::bind(rendezvous.client_socket_path()).unwrap(),
            move |mut stream| {
                snapshot(&mut stream, true);
                if expected == "C:/configured" {
                    configuration_reply(&mut stream, "C:/configured", None);
                }
                let WorkbenchRequest::Command(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("one canonical creation, no cwd query");
                };
                let action = json!(request.action);
                assert_eq!(
                    std::path::Path::new(action["options"]["cwd"].as_str().unwrap()),
                    std::path::Path::new(expected)
                );
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::Command(WorkbenchCommandResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        outcome: WorkbenchCommandOutcome::Rejected {
                            revision: WorkbenchRevision::new(12),
                            reason: WorkbenchRejectionReason::RevisionMismatch,
                        },
                    }),
                )
                .unwrap();
                assert_closed(&mut stream);
            },
        );
        let output = fixture.run_with_env(
            &args,
            None,
            None,
            &[("ORCHBEE_CONFIG_FILE", config.as_os_str())],
        );
        assert!(
            handler.join().is_ok(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(3));
    }
}

#[test]
fn configured_checkout_query_precedes_setup_and_explicit_path_skips_it() {
    for mode in ["configured", "generated", "explicit", "query-failed"] {
        let fixture = Fixture::new();
        let config = fixture.0.join("config.toml");
        std::fs::write(&config, "[worktrees]\ndirectory='C:/file-only'").unwrap();
        let rendezvous = fixture.rendezvous();
        let handler = peer(
            &fixture,
            LocalListener::bind(rendezvous.client_socket_path()).unwrap(),
            move |mut stream| {
                snapshot(&mut stream, mode != "query-failed");
                if mode != "explicit" {
                    configuration_reply(&mut stream, "follow", Some("C:/configured"));
                    let WorkbenchRequest::WorktreeQuery(request) =
                        read_workbench_request(&mut stream).unwrap()
                    else {
                        panic!("repository query before any Project/Git mutation");
                    };
                    assert_eq!(
                        json!(request.source),
                        json!({"kind":"directory","path":"C:/repo"})
                    );
                    assert!(request.trust_repository);
                    let outcome = if mode == "query-failed" {
                        json!({"kind":"failed","reason":{"kind":"not-found"}})
                    } else {
                        json!({"kind":"found","sessionId":id(2),"workbenchRevision":"12","workerId":id(9),"source":{"requestedPath":"C:/repo","repositoryRoot":"C:/repo","commonDirectory":"C:/repo/.git","checkoutPath":"C:/repo","workspaceId":null},"entries":[{"checkout":{"worktreePath":"C:/repo","head":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","branch":"refs/heads/main","bare":false,"detached":false,"locked":false,"prunable":false},"linked":false,"workspaces":[]}]})
                    };
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::WorktreeQuery(WorkbenchWorktreeQueryResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            outcome: serde_json::from_value(outcome).unwrap(),
                        }),
                    )
                    .unwrap();
                    if mode == "query-failed" {
                        assert_closed(&mut stream);
                        return;
                    }
                }
                let WorkbenchRequest::WorktreeOperation(request) =
                    read_workbench_request(&mut stream).unwrap()
                else {
                    panic!("single worktree operation");
                };
                let WorkbenchWorktreeAction::Start {
                    operation_id,
                    intent,
                    ..
                } = request.action
                else {
                    panic!("Start");
                };
                let WorktreeIntent::Create {
                    path: Some(path),
                    branch: Some(branch),
                    ..
                } = intent
                else {
                    panic!("fixed path and branch");
                };
                let expected = match mode {
                    "explicit" => "C:/explicit".to_owned(),
                    "generated" => format!("C:/configured/repo/worktree-{operation_id}"),
                    _ => "C:/configured/repo/feature-one".to_owned(),
                };
                assert_eq!(
                    std::path::Path::new(path.as_str()),
                    std::path::Path::new(&expected)
                );
                assert_eq!(
                    branch.as_str(),
                    if mode == "generated" {
                        format!("worktree/{operation_id}")
                    } else {
                        "Feature/One".to_owned()
                    }
                );
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        operation_id,
                        outcome: WorkbenchWorktreeOutcome::Unavailable {
                            reason: WorktreeAdmissionFailure::Busy,
                        },
                    }),
                )
                .unwrap();
                assert_closed(&mut stream);
            },
        );
        let mut args = vec![
            "worktree",
            "create",
            "--cwd=C:/repo",
            "--trust-repository",
            "--format=json",
        ];
        if mode != "generated" {
            args.push("--branch=Feature/One");
        }
        if mode == "explicit" {
            args.push("--path=C:/explicit");
        }
        let output = fixture.run_with_env(
            &args,
            None,
            None,
            &[("ORCHBEE_CONFIG_FILE", config.as_os_str())],
        );
        assert!(
            handler.join().is_ok(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let doc = document(output);
        if mode == "query-failed" {
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.cli.failure.configuration.json"),
            );
            assert_eq!(doc["failure"]["details"]["reason"], "source-query-failed");
        } else {
            validate(
                &doc,
                include_str!("../src/cli/schemas/orchbee.worktree.operation.json"),
            );
        }
    }
}

#[test]
fn reset_keys_real_windows_cli_backs_up_and_preserves_configuration_without_runtime() {
    let fixture = Fixture::new();
    let config = fixture.0.join("config.toml");
    let schema = include_str!("../src/cli/schemas/orchbee.config.reset-keys.json");
    let args = ["config", "reset-keys", "--format=json"];
    let missing = document(fixture.run(&args));
    validate(&missing, schema);
    assert_eq!(missing["result"]["state"], "missing");
    assert!(!fixture.0.join("appdata").exists());
    let env = [("ORCHBEE_CONFIG_FILE", config.as_os_str())];
    let missing = document(fixture.run_with_env(&args, None, None, &env));
    validate(&missing, schema);
    assert_eq!(missing["result"]["state"], "missing");
    let original = b"# retained\r\n[terminal]\r\nnew_cwd='home'\r\n[keys]\r\nprefix='ctrl+a'\r\n[keys.indexed]\r\nswitch_tab='alt+1..9'\r\n[[keys.command]]\r\ncommand='DO_NOT_ECHO'\r\n";
    std::fs::write(&config, original).unwrap();
    let output = fixture.run_with_env(&args, None, None, &env);
    assert_eq!(output.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("DO_NOT_ECHO"));
    let reset = document(output);
    validate(&reset, schema);
    assert_eq!(reset["result"]["state"], "reset");
    let backup = std::path::PathBuf::from(reset["result"]["backupPath"].as_str().unwrap());
    assert_eq!(std::fs::read(backup).unwrap(), original);
    assert_eq!(
        std::fs::read(&config).unwrap(),
        b"# retained\r\n[terminal]\r\nnew_cwd='home'\r\n"
    );
    let before = std::fs::read_dir(&fixture.0).unwrap().count();
    let repeated = document(fixture.run_with_env(&args, None, None, &env));
    validate(&repeated, schema);
    assert_eq!(repeated["result"]["state"], "no-keys");
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), before);
    let text = fixture.run_with_env(
        &["config", "reset-keys"],
        None,
        None,
        &[
            ("ORCHBEE_CONFIG_FILE", config.as_os_str()),
            ("LC_ALL", std::ffi::OsStr::new("zh_CN.UTF-8")),
        ],
    );
    assert_eq!(text.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&text.stdout).contains("没有自定义键位"));
    assert!(!fixture.0.join("runtime").exists());
}

#[test]
fn reset_keys_real_windows_cli_refuses_bad_input_before_writing() {
    let fixture = Fixture::new();
    let config = fixture.0.join("config.toml");
    let schema = include_str!("../src/cli/schemas/orchbee.config.reset-keys.json");
    for bytes in [
        b"[keys\nDO_NOT_ECHO".to_vec(),
        vec![0xff],
        vec![b'a'; 65537],
        b"keys = [\n'ctrl+a',\n]\n".to_vec(),
    ] {
        std::fs::write(&config, &bytes).unwrap();
        let before = std::fs::read_dir(&fixture.0).unwrap().count();
        let output = fixture.run_with_env(
            &["config", "reset-keys", "--format=json"],
            None,
            None,
            &[("ORCHBEE_CONFIG_FILE", config.as_os_str())],
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("DO_NOT_ECHO"));
        let rejected = document(output);
        validate(&rejected, schema);
        assert_eq!(rejected["result"]["state"], "failed");
        assert_eq!(rejected["result"]["backupPath"], Value::Null);
        assert_eq!(rejected["result"]["recoveryRequired"], false);
        assert_eq!(std::fs::read(&config).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), before);
    }
    let original = b"[keys]\nprefix='f12'\n";
    std::fs::write(&config, original).unwrap();
    let output = fixture.run_with_env(
        &["config", "reset-keys", "extra", "--format=json"],
        None,
        None,
        &[("ORCHBEE_CONFIG_FILE", config.as_os_str())],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(std::fs::read(&config).unwrap(), original);
    let held = std::fs::File::open(&config).unwrap();
    let output = fixture.run_with_env(
        &["config", "reset-keys", "--format=json"],
        None,
        None,
        &[("ORCHBEE_CONFIG_FILE", config.as_os_str())],
    );
    assert_eq!(output.status.code(), Some(1));
    let rejected = document(output);
    validate(&rejected, schema);
    drop(held);
    assert_eq!(std::fs::read(&config).unwrap(), original);
    assert!(!fixture.0.join("runtime").exists());
}

#[test]
fn config_check_window_title_diagnostics() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let default = orchbee_local_runtime::DEFAULT_WINDOW_TITLE;
    for (title, effective, diagnostic) in [
        ("", "", None),
        ("{workspace}/{tab}", "{workspace}/{tab}", None),
        ("{unknown}", default, Some("unknown-token")),
        ("{tab", default, Some("unclosed-brace")),
    ] {
        std::fs::write(&config, format!("[ui]\nwindow_title='{title}'")).unwrap();
        let doc = config_check_document(&fixture);
        let check = &doc["result"]["checks"][5];
        assert_eq!(check["details"]["windowTitle"], effective);
        assert_eq!(check["details"]["windowTitleDiagnostic"], Value::Null);
        if let Some(diagnostic) = diagnostic {
            assert_warning_diagnostic(&doc, "ui.window_title", diagnostic, "defaulted");
        } else {
            assert_eq!(check["status"], "pass");
        }
    }
    std::fs::write(&config, "[ui]\nwindow_title=false").unwrap();
    let checked = config_check_document(&fixture);
    assert_warning_diagnostic(&checked, "ui.window_title", "wrong-type", "defaulted");
    assert_eq!(
        checked["result"]["checks"][5]["details"]["windowTitle"],
        default
    );
    assert_creation_reaches_missing_session(&fixture);
}

#[test]
fn config_check_notification_policy_validates_actual_file_and_closed_versions() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "[ui.toast]\ndelivery='orchbee'\ndelay_seconds=7\n[ui.toast.overlay]\nposition='top-left'\n[ui.sound.agents]\ncodex='off'\npi='on'",
    )
    .unwrap();
    let doc = document(fixture.run(&["config", "check", "--format=json"]));
    let schema = include_str!("../src/cli/schemas/orchbee.config.check.json");
    validate(&doc, schema);
    assert_eq!(
        doc["result"]["checks"][5]["details"]["notificationDelivery"],
        "orchbee"
    );
    assert_eq!(
        doc["result"]["checks"][5]["details"]["notificationPosition"],
        "top-left"
    );
    assert_eq!(
        doc["result"]["checks"][5]["details"]["notificationAgentDelivery"],
        "orchbee"
    );
    assert_eq!(
        doc["result"]["checks"][5]["details"]["notificationDelaySeconds"],
        7
    );
    assert_eq!(
        doc["result"]["checks"][5]["details"]["notificationSoundEnabled"],
        false
    );
    assert_eq!(
        doc["result"]["checks"][5]["details"]["notificationAgentSounds"],
        json!({"codex":"off", "pi":"on"})
    );
    for (text, field, reason, detail, expected) in [
        (
            "[ui.toast]\ndelivery='bad'",
            "ui.toast.delivery",
            "invalid-mode",
            "notificationDelivery",
            json!("off"),
        ),
        (
            "[ui.toast]\ndelay_seconds=3601",
            "ui.toast.delay_seconds",
            "invalid-delay",
            "notificationDelaySeconds",
            json!(1),
        ),
    ] {
        std::fs::write(&config, text).unwrap();
        let checked = config_check_document(&fixture);
        assert_warning_diagnostic(&checked, field, reason, "defaulted");
        assert_eq!(checked["result"]["checks"][5]["details"][detail], expected);
        assert_creation_reaches_missing_session(&fixture);
    }
}

#[test]
fn workspace_sidebar_configuration_has_separate_profiles_and_defaults_invalid_profiles() {
    let fixture = Fixture::new();
    let config = fixture.0.join("appdata/Orchbee/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let text = "[ui.sidebar.spaces]\nrows=[['state_icon','workspace'],[{token='branch',fg='#abc',rules=[{starts_with='worktree/',bold=true}]},'git_status','$branch']]\nrow_gap=2\n[ui.sidebar.agents]\nrows=[['agent']]";
    std::fs::write(&config, text).unwrap();
    let doc = document(fixture.run(&["config", "check", "--format=json"]));
    let schema = include_str!("../src/cli/schemas/orchbee.config.check.json");
    validate(&doc, schema);
    assert_eq!(doc["version"], 1);
    let details = &doc["result"]["checks"][5]["details"];
    let profile = orchbee_local_runtime::SpaceSidebarSettings::from_profile(
        details["spacesSidebarProfile"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(profile.row_gap(), 2);
    assert!(profile.needs_git());
    assert_eq!(profile.rows()[1][2].name(), "$branch");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), text);
    for (invalid, field, reason, disposition) in [
        (
            "rows=[['agent']]",
            "ui.sidebar.spaces",
            "invalid-token",
            "defaulted",
        ),
        (
            "rows=[[{token='git_status',rules=[{gt=0}]}]]",
            "ui.sidebar.spaces",
            "invalid-rules",
            "defaulted",
        ),
        (
            "rows_by_agent={codex=[]}",
            "ui.sidebar.spaces.rows_by_agent",
            "unknown-key",
            "ignored",
        ),
    ] {
        std::fs::write(&config, format!("[ui.sidebar.spaces]\n{invalid}")).unwrap();
        let checked = config_check_document(&fixture);
        assert_warning_diagnostic(&checked, field, reason, disposition);
        let profile = orchbee_local_runtime::SpaceSidebarSettings::from_profile(
            checked["result"]["checks"][5]["details"]["spacesSidebarProfile"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(profile.rows().len(), 2);
        assert_creation_reaches_missing_session(&fixture);
    }
}
