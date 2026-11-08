//! Real process receipts and offline effects for the native plugin catalog.
#![cfg(windows)]
use crate::peer_support as support;
use serde_json::Value;
use support::{Fixture, validate};
const SCHEMA: &str = include_str!("../src/cli/schemas/orchbee.plugin.registry.json");
fn run(fixture: &Fixture, args: &[&str], status: i32) -> Value {
    let mut args = args.to_vec();
    args.push("--format=json");
    let output = fixture.run(&args);
    assert_eq!(
        output.status.code(),
        Some(status),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(&document, SCHEMA);
    document
}

#[test]
fn plugin_link_resolves_home_and_relative_paths_in_the_executing_process() {
    for home_operand in [true, false] {
        let fixture = Fixture::new();
        let context = fixture.0.join("execution-context");
        let plugin = context.join("remote");
        std::fs::create_dir_all(&plugin).unwrap();
        let manifest = plugin.join("orchbee-plugin.toml");
        std::fs::write(&manifest, "id='paths'\nname='Path test'\nversion='1'\n[[actions]]\nid='run'\ntitle='Declared action'\ncommand=['cmd.exe','/c','exit 99']\n").unwrap();
        // The catalog follows explicit configuration, so only the operand reads the profile.
        let settings = fixture.settings_file();
        let mut command = fixture.command(
            &[
                "plugin",
                "link",
                if home_operand { r"~\remote" } else { "remote" },
                "--disabled",
                "--format=json",
            ],
            None,
            None,
            &[
                (
                    "USERPROFILE",
                    if home_operand {
                        context.as_os_str()
                    } else {
                        std::ffi::OsStr::new("private-invalid-home")
                    },
                ),
                ("ORCHBEE_CONFIG_FILE", settings.as_os_str()),
            ],
        );
        command.current_dir(if home_operand { &fixture.0 } else { &context });
        let output = fixture.run_command(
            command,
            None,
            std::time::Duration::from_secs(45),
            8 * 1024 * 1024,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(&document, SCHEMA);
        let entry = &document["result"]["catalog"]["plugins"][0];
        assert_eq!(
            std::path::Path::new(entry["manifestPath"].as_str().unwrap()),
            manifest.canonicalize().unwrap()
        );
        assert_eq!(
            std::path::Path::new(entry["pluginRoot"].as_str().unwrap()),
            plugin.canonicalize().unwrap()
        );
        assert_eq!(entry["enabled"], false);
        assert!(output.stderr.is_empty());
        assert!(!fixture.0.join("runtime").exists());
    }
}

#[test]
fn plugin_link_context_failure_precedes_catalog_access_in_both_output_formats() {
    let fixture = Fixture::new();
    for format in ["json", "text"] {
        let output = fixture.run_with_env(
            &["plugin", "link", "~", "--format", format],
            None,
            None,
            &[("USERPROFILE", std::ffi::OsStr::new("private-invalid-home"))],
        );
        assert_eq!(output.status.code(), Some(2));
        if format == "json" {
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            validate(
                &document,
                include_str!("../src/cli/schemas/orchbee.cli.failure.json"),
            );
            assert_eq!(document["failure"]["kind"], "invalid-usage");
        } else {
            assert!(output.stdout.is_empty());
        }
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("<path>"), "{diagnostic}");
        assert!(!diagnostic.contains("private-invalid-home"));
        assert!(!fixture.0.join("appdata").exists());
        assert!(!fixture.0.join("runtime").exists());
    }
}

#[test]
fn plugin_catalog_real_cli_round_trip_without_session_or_command_execution() {
    let fixture = Fixture::new();
    let empty = run(&fixture, &["plugin", "list"], 0);
    assert_eq!(empty["result"]["catalog"]["revision"], 0);
    assert!(!fixture.0.join("appdata").exists());
    let source = fixture.0.join("orchbee-plugin.toml");
    std::fs::write(&source,"id='tools'\nname='插件工具'\nversion='1'\n[[actions]]\nid='fail'\ntitle='Declared action'\ncommand=['cmd.exe','/c','exit 99']\n").unwrap();
    let source_text = source.to_str().unwrap();
    let link = run(&fixture, &["plugin", "link", source_text, "--disabled"], 0);
    assert_eq!(link["result"]["catalog"]["revision"], 1);
    assert_eq!(link["result"]["catalog"]["plugins"][0]["enabled"], false);
    assert_eq!(
        run(&fixture, &["plugin", "action", "list"], 0)["result"]["actions"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    run(&fixture, &["plugin", "enable", "tools"], 0);
    let actions = run(&fixture, &["plugin", "action", "list", "tools"], 0);
    assert_eq!(actions["result"]["actions"][0]["action"]["id"], "fail");
    let config = run(&fixture, &["plugin", "config-dir", "tools"], 0);
    let config = std::path::PathBuf::from(config["result"]["path"].as_str().unwrap());
    std::fs::write(config.join("user.txt"), b"keep").unwrap();
    run(&fixture, &["plugin", "disable", "tools"], 0);
    std::fs::write(&source, "bad = true").unwrap();
    let list = run(&fixture, &["plugin", "list", "tools"], 0);
    assert_eq!(
        list["result"]["catalog"]["plugins"][0]["warning"],
        "invalid-manifest"
    );
    assert_eq!(
        run(&fixture, &["plugin", "enable", "tools"], 1)["result"]["reason"],
        "invalid-manifest"
    );
    assert!(
        run(&fixture, &["plugin", "unlink", "tools"], 0)["result"]["catalog"]["plugins"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        run(&fixture, &["plugin", "unlink", "tools"], 0)["result"]["catalog"]["changed"],
        false
    );
    assert_eq!(
        run(&fixture, &["plugin", "enable", "tools"], 1)["result"]["reason"],
        "plugin-not-found"
    );
    assert!(source.exists());
    assert_eq!(std::fs::read(config.join("user.txt")).unwrap(), b"keep");
    assert!(!fixture.0.join("runtime").exists());
}
#[test]
fn catalog_failures_separate_localized_diagnostics_from_locale_neutral_documents() {
    let fixture = Fixture::new();
    let source = fixture.0.join("orchbee-plugin.toml");
    std::fs::write(&source, "bad = true\n").unwrap();
    let source_text = source.to_str().unwrap();
    let mut machine_output = None;
    for (locale, diagnostic) in [
        ("en_US.UTF-8", "Invalid plugin manifest.\n"),
        ("zh_CN.UTF-8", "插件清单无效。\n"),
    ] {
        for format in ["text", "json"] {
            let output = fixture.run_with_env(
                &["plugin", "link", source_text, "--format", format],
                None,
                None,
                &[("LC_ALL", std::ffi::OsStr::new(locale))],
            );
            assert_eq!(output.status.code(), Some(1), "{locale} {format}");
            assert_eq!(output.stderr, diagnostic.as_bytes(), "{locale} {format}");
            if format == "json" {
                let document: Value = serde_json::from_slice(&output.stdout).unwrap();
                validate(&document, SCHEMA);
                assert_eq!(document["result"]["kind"], "failed");
                assert_eq!(document["result"]["reason"], "invalid-manifest");
                if let Some(previous) = &machine_output {
                    assert_eq!(&output.stdout, previous);
                }
                machine_output = Some(output.stdout);
            } else {
                assert!(output.stdout.is_empty());
            }
        }
    }
    assert!(!fixture.0.join("appdata").exists());
    assert!(!fixture.0.join("runtime").exists());
    assert_eq!(std::fs::read(&source).unwrap(), b"bad = true\n");
}

#[test]
fn plugin_cli_rejects_malformed_scope_before_filesystem_effects_and_help_is_offline() {
    let fixture = Fixture::new();
    for args in [
        vec!["plugin", "link", "missing", "--enabled", "--disabled"],
        vec!["plugin", "list", "--disabled"],
        vec!["plugin", "enable", "../bad"],
        vec!["plugin", "config-dir"],
        vec!["plugin", "action", "list", "a", "extra"],
        vec!["plugin", "link", "missing", "--enabled", "--enabled"],
        vec!["workspace", "list", "--enabled"],
        vec!["plugin", "list", "tools", "--plugin", "other"],
        vec!["plugin", "log", "list", "--limit", "201"],
        vec!["plugin", "action", "list", "--context", "{}"],
        vec!["plugin", "action", "invoke", "hello", "--context", "{"],
        vec![
            "plugin",
            "action",
            "invoke",
            "hello",
            "--context",
            "{\"unknown\":1}",
        ],
        vec!["plugin", "action", "invoke", "bad/path"],
        vec![
            "plugin", "action", "invoke", "hello", "--plugin", "tools", "--plugin", "tools",
        ],
        vec!["workspace", "list", "--plugin", "tools"],
    ] {
        assert_eq!(fixture.run(&args).status.code(), Some(2), "{args:?}");
    }
    for args in [
        vec!["plugin", "--help"],
        vec!["help", "plugin", "action", "list"],
        vec!["plugin", "link", "--help"],
        vec!["plugin", "action", "invoke", "--help"],
        vec!["help", "plugin", "log", "list"],
    ] {
        let result = fixture.run(&args);
        assert!(result.status.success());
        let path = if args[0] == "help" {
            &args[1..]
        } else {
            &args[..args.len() - 1]
        };
        assert!(
            String::from_utf8_lossy(&result.stdout)
                .contains(&format!("orchbee {}", path.join(" ")))
        );
        assert!(result.stderr.is_empty());
    }
    assert!(!fixture.0.join("appdata").exists());
    assert!(!fixture.0.join("runtime").exists());
}
