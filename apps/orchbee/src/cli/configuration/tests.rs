use super::*;

#[test]
fn explicit_creation_wins_over_shared_file_policy() {
    let settings = project(WindowsUserSettingsLoad {
        settings: WindowsUserSettings {
            cwd: Some((WindowsDirectoryPolicy::Explicit, Some(r"C:\项目".into()))),
            worktree_root: Some(r"C:\worktrees".into()),
            resume_agents_on_restore: false,
            resume_agents_on_restore_source: "file",
            ..WindowsUserSettings::default()
        },
        diagnostics: WindowsSettingsDiagnostics::default(),
    })
    .unwrap();
    let mut directory = CreationDirectory::Default;
    let mut options = LaunchOptions::default();
    settings.apply_directory(&mut directory, &mut options);
    assert_eq!(directory, CreationDirectory::Explicit);
    assert_eq!(options.cwd.as_ref().unwrap().as_str(), r"C:\项目");
    directory = CreationDirectory::Current;
    options.cwd = None;
    settings.apply_directory(&mut directory, &mut options);
    assert_eq!(directory, CreationDirectory::Current);
    assert!(options.cwd.is_none());
    let details = settings.check_details();
    assert_eq!(details["resumeAgentsOnRestore"], false);
    assert_eq!(details["resumeAgentsOnRestoreSource"], "file");
    assert_eq!(details["worktreeDirectory"], r"C:\worktrees");
}

#[test]
fn shared_path_budget_matches_wire_and_cli_failure_preserves_field() {
    assert_eq!(
        orchbee_local_site_runtime::WINDOWS_USER_PATH_LIMIT,
        orchbee_workbench_protocol::MAXIMUM_DIRECTORY_PATH_BYTES
    );
    let error = Error::from(WindowsSettingsError::new(
        "session.resume_agents_on_restore",
        "wrong-type",
    ));
    assert_eq!(error.field, "session.resume_agents_on_restore");
    assert_eq!(error.reason, "wrong-type");
}

#[test]
fn named_theme_config_check_is_required_and_closed() {
    use serde_json::{Value, json};
    let mut schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.config.check.json")).unwrap();
    schema["$ref"] = json!("#/$defs/userConfigCheck");
    for key in ["required", "properties", "additionalProperties", "allOf"] {
        schema.as_object_mut().unwrap().remove(key);
    }
    let validator = jsonschema::validator_for(&schema).unwrap();
    let load = orchbee_local_site_runtime::WindowsUserSettingsLoad {
        settings: orchbee_local_site_runtime::WindowsUserSettings::default(),
        diagnostics: Default::default(),
    };
    let details = super::project(load).unwrap().check_details();
    let mut check = json!({"name":"user-config","status":"pass","details":details});
    assert!(validator.is_valid(&check));
    for name in ["terminal-native", "one-light", "one-dark"] {
        check["details"]["themeName"] = json!(name);
        assert!(validator.is_valid(&check));
    }
    for invalid in [json!("custom"), json!("One-Light"), json!(null), json!(1)] {
        check["details"]["themeName"] = invalid;
        assert!(!validator.is_valid(&check));
    }
    check["details"]
        .as_object_mut()
        .unwrap()
        .remove("themeName");
    assert!(!validator.is_valid(&check));
}
