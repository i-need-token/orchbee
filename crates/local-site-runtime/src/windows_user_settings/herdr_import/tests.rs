//! Source-derived import cases protect explicitness, admission and relocation semantics.
use super::*;
#[test]
fn worktree_entries_are_imported_as_native_overrides_and_invalid_binding_disables_only_its_action()
{
    for action in ["new_worktree", "open_worktree"] {
        for (binding, enabled) in [("'f2'", true), ("[]", false), ("'ctrl+'", false)] {
            let plan = prepare(&format!("[keys]\n{action}={binding}\nnew_tab='prefix+c'\n"));
            let loaded = parse(
                plan.candidate_toml(),
                Path::new("D:/Orchbee/candidate.toml"),
                &|_| None,
            )
            .unwrap();
            let override_entry = loaded
                .settings
                .keys
                .overrides
                .iter()
                .find(|item| item.action == action)
                .unwrap();
            assert_eq!(!override_entry.triggers.is_empty(), enabled);
            assert!(
                loaded
                    .settings
                    .keys
                    .overrides
                    .iter()
                    .any(|item| item.action == "new_tab" && !item.triggers.is_empty())
            );
            assert_ne!(
                plan.report()
                    .items()
                    .iter()
                    .find(|item| item.field == format!("keys.{action}"))
                    .unwrap()
                    .disposition,
                WindowsHerdrImportDisposition::Unsupported
            );
        }
    }
}

fn prepare(text: &str) -> PreparedWindowsHerdrImport {
    prepare_windows_herdr_import(
        text,
        Path::new("C:/Herdr/config.toml"),
        Path::new("D:/Orchbee/candidate.toml"),
        |name| (name == "USERPROFILE").then(|| OsString::from("C:/Users/test")),
    )
    .unwrap()
}

fn candidate(plan: &PreparedWindowsHerdrImport) -> toml::Table {
    toml::from_str(plan.candidate_toml()).unwrap()
}

#[test]
fn copy_mode_is_mapped_readmitted_and_invalid_binding_disables_only_that_action() {
    for (binding, enabled) in [
        ("'prefix+U'", true),
        ("'f2'", true),
        ("[]", false),
        ("'ctrl+'", false),
    ] {
        let plan = prepare(&format!(
            "[keys]\ncopy_mode={binding}\nnew_tab='prefix+c'\n"
        ));
        let table = candidate(&plan);
        let item = plan
            .report()
            .items()
            .iter()
            .find(|item| item.field == "keys.copy_mode")
            .unwrap();
        assert_ne!(item.disposition, WindowsHerdrImportDisposition::Unsupported);
        assert_eq!(item.target_fields, ["keys.copy_mode"]);
        assert_eq!(table["keys"]["new_tab"].as_str(), Some("prefix+c"));
        let loaded = parse(
            plan.candidate_toml(),
            Path::new("D:/Orchbee/candidate.toml"),
            &|_| None,
        )
        .unwrap();
        let action = loaded
            .settings
            .keys
            .overrides
            .iter()
            .find(|item| item.action == "copy_mode")
            .unwrap();
        assert_eq!(!action.triggers.is_empty(), enabled);
        assert_eq!(
            prepare(&format!(
                "[keys]\ncopy_mode={binding}\nnew_tab='prefix+c'\n"
            )),
            plan
        );
    }
}

#[test]
fn absent_fields_never_freeze_default_keys_notifications_or_sidebar_rows() {
    let plan =
        prepare("onboarding=false\n[theme]\nname='one-light'\n[update]\nchannel='preview'\n");
    assert!(candidate(&plan).is_empty());
    assert_eq!(plan.candidate_toml(), "\n");
    assert_eq!(plan.report().items().len(), 3);
    let plan = prepare("[ui.sidebar.spaces]\nrow_gap=2\n");
    let table = candidate(&plan);
    assert_eq!(
        table["ui"]["sidebar"]["spaces"]["row_gap"].as_integer(),
        Some(2)
    );
    assert!(table["ui"]["sidebar"]["spaces"].get("rows").is_none());
    assert!(table.get("keys").is_none());
    let explicit = prepare("[ui.sidebar.spaces]\nrows=[]\n");
    assert_eq!(
        candidate(&explicit)["ui"]["sidebar"]["spaces"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn notification_precedence_preserves_the_two_existing_safe_defaults() {
    for (source, expected) in [
        ("delivery='herdr'", Some("orchbee")),
        ("enabled=false", Some("off")),
        ("enabled=true", Some("orchbee")),
        ("delivery='terminal'\nenabled=false", Some("terminal")),
        ("delivery='invalid'\nenabled=true", None),
    ] {
        let plan = prepare(&format!("[ui.toast]\n{source}\n"));
        let table = candidate(&plan);
        if let Some(expected) = expected {
            assert_eq!(table["ui"]["toast"]["delivery"].as_str(), Some(expected));
            assert_eq!(
                table["ui"]["toast"]["agent_delivery"].as_str(),
                Some(expected)
            );
        } else {
            assert!(table.get("ui").is_none());
        }
    }
}

#[test]
fn relocation_cannot_change_source_relative_directories_and_sound_identities() {
    let plan = prepare(
        "[terminal]\nnew_cwd='project'\n[worktrees]\ndirectory='~/trees'\n[ui.sound.agents]\nopen_code='off'\ngithub_copilot='on'\nagy='default'\n",
    );
    let table = candidate(&plan);
    assert_eq!(
        table["terminal"]["new_cwd"].as_str(),
        Some("C:/Herdr\\project")
    );
    assert_eq!(
        table["worktrees"]["directory"].as_str(),
        Some("C:/Users/test\\trees")
    );
    assert_eq!(
        table["ui"]["sound"]["agents"]["opencode"].as_str(),
        Some("off")
    );
    assert_eq!(
        table["ui"]["sound"]["agents"]["copilot"].as_str(),
        Some("on")
    );
    assert_eq!(
        table["ui"]["sound"]["agents"]["agy"].as_str(),
        Some("default")
    );
}

#[test]
fn invalid_action_is_disabled_without_activating_commands_or_freezing_prefix() {
    let plan = prepare(
        "[keys]\nprefix='ctrl+'\nnew_tab='prefix+t'\nclose_pane='ctrl+'\n[[keys.command]]\nkey='prefix+g'\ncommand='do-not-execute-secret'\n",
    );
    let table = candidate(&plan);
    assert!(table["keys"].get("prefix").is_none());
    assert_eq!(table["keys"]["new_tab"].as_str(), Some("prefix+t"));
    assert!(table["keys"]["close_pane"].as_array().unwrap().is_empty());
    assert!(table["keys"].get("command").is_none());
    assert!(!plan.candidate_toml().contains("secret"));
    assert!(
        plan.report()
            .items()
            .iter()
            .all(|item| !item.field.contains("secret"))
    );
}

#[test]
fn reports_are_stable_bounded_and_do_not_hide_omitted_fields() {
    let source: String = (0..160)
        .map(|i| format!("unknown_{i:03}='secret'\n"))
        .collect();
    let one = prepare(&source);
    let two = prepare(&source);
    assert_eq!(one, two);
    assert_eq!(one.report().items().len(), 128);
    assert_eq!(one.report().omitted_count(), 32);
    assert!(candidate(&one).is_empty());
}

#[test]
fn file_operands_reject_streams_devices_aliases_and_stdin_before_io() {
    for raw in [
        "-",
        "C:relative",
        "C:/file:stream",
        "C:/NUL.toml",
        "C:/COM1",
        "C:/x.",
        "C:/x ",
        "C:/./x",
        "C:/../x",
        "//server/share/x",
        "\\\\?\\C:\\x",
        "C:/dir/",
        "C:/x\n",
    ] {
        assert!(!valid_windows_herdr_file_operand(raw), "{raw:?}");
    }
    for raw in [
        "source.toml",
        "C:/test/config.toml",
        "~/config.toml",
        "relative/配置.toml",
    ] {
        assert!(valid_windows_herdr_file_operand(raw), "{raw:?}");
    }
}

#[test]
fn generated_import_partitions_remain_sparse_and_readmit_the_supported_subset() {
    // Fixed deterministic partitions combine valid/invalid fields with distinct home/source contexts.
    for mask in 0..128 {
        let enabled = mask & 1 != 0;
        let cwd = if mask & 2 != 0 { "follow" } else { "project" };
        let prefix = if mask & 4 != 0 { "ctrl+a" } else { "ctrl+" };
        let close = if mask & 8 != 0 { "prefix+x" } else { "ctrl+" };
        let legacy = if mask & 16 != 0 {
            "enabled=false"
        } else {
            "delivery='herdr'"
        };
        let rows = if mask & 32 != 0 {
            "rows=[]"
        } else {
            "row_gap=2"
        };
        let zoom = if mask & 64 != 0 {
            "zoom='prefix+z'\nfullscreen='prefix+f'"
        } else {
            "fullscreen='prefix+f'"
        };
        let source = format!(
            "[session]\nresume_agents_on_restore={enabled}\n[terminal]\nnew_cwd='{cwd}'\n[keys]\nprefix='{prefix}'\nclose_pane='{close}'\n{zoom}\n[ui.toast]\n{legacy}\n[ui.sidebar.spaces]\n{rows}\n[update]\nmanifest_check=false\n"
        );
        let plan = prepare(&source);
        let table = candidate(&plan);
        let loaded = parse(
            plan.candidate_toml(),
            Path::new("D:/different/config.toml"),
            &|_| None,
        )
        .unwrap();
        assert_eq!(loaded.settings.resume_agents_on_restore, enabled);
        assert!(table.get("update").is_none());
        assert_eq!(table["keys"].get("prefix").is_some(), mask & 4 != 0);
        assert_eq!(
            table["ui"]["sidebar"]["spaces"].get("rows").is_some(),
            mask & 32 != 0
        );
        assert_eq!(
            table["keys"]["zoom"].as_str(),
            Some(if mask & 64 != 0 {
                "prefix+z"
            } else {
                "prefix+f"
            })
        );
        assert!(plan.candidate_toml().ends_with('\n'));
        assert_eq!(prepare(&source), plan);
    }
}

#[test]
fn unknown_nested_fields_do_not_materialize_profiles_or_claim_herdr_support() {
    let plan = prepare(
        "[terminal]\nkitty_graphics=true\nfuture=1\n[worktrees]\nfuture=1\n[session]\nfuture=1\n[ui]\nsidebar_width=20\nfuture=1\n[ui.sidebar.agents]\nfuture=1\n[ui.sidebar.spaces]\n[ui.toast]\nfuture=1\n[ui.sound]\npath='do-not-migrate'\nfuture=1\n[keys]\ncommand=[]\nfuture='prefix+x'\n",
    );
    assert!(candidate(&plan).is_empty());
    for item in plan.report().items() {
        if item.field.ends_with("future") {
            assert_eq!(item.disposition, WindowsHerdrImportDisposition::Ignored);
            assert_eq!(item.reason, "unknown-key");
        } else {
            assert_eq!(item.disposition, WindowsHerdrImportDisposition::Unsupported);
        }
        assert!(item.target_fields.is_empty());
    }
}
