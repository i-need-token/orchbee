use super::*;

#[test]
fn config_check_projects_file_creation_defaults() {
    let settings = project(UserSettingsLoad {
        settings: UserSettings {
            cwd: Some((DirectoryPolicy::Explicit, Some(r"C:\项目".into()))),
            worktree_root: Some(r"C:\worktrees".into()),
            ..UserSettings::default()
        },
        diagnostics: SettingsDiagnostics::default(),
    })
    .unwrap();
    let details = settings.check_details();
    assert_eq!(details["newCwd"], r"C:\项目");
    assert_eq!(details["worktreeDirectory"], r"C:\worktrees");
}

#[test]
fn shared_path_budget_matches_wire_and_cli_failure_preserves_field() {
    assert_eq!(
        orchbee_local_runtime::USER_PATH_LIMIT,
        orchbee_workbench_protocol::MAXIMUM_DIRECTORY_PATH_BYTES
    );
    let error = Error::from(SettingsError::new("update.manifest_check", "wrong-type"));
    assert_eq!(error.field, "update.manifest_check");
    assert_eq!(error.reason, "wrong-type");
}

#[test]
fn config_check_details_satisfy_the_closed_schema_without_retired_fields() {
    use serde_json::{Value, json};
    let mut schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.config.check.json")).unwrap();
    schema["$ref"] = json!("#/$defs/userConfigCheck");
    for key in ["required", "properties", "additionalProperties", "allOf"] {
        schema.as_object_mut().unwrap().remove(key);
    }
    let validator = jsonschema::validator_for(&schema).unwrap();
    let load = orchbee_local_runtime::UserSettingsLoad {
        settings: orchbee_local_runtime::UserSettings::default(),
        diagnostics: Default::default(),
    };
    let details = super::project(load).unwrap().check_details();
    let check = json!({"name":"user-config","status":"pass","details":details});
    assert!(validator.is_valid(&check));
    for (field, value) in [
        ("themeName", json!("terminal-native")),
        ("defaultShell", json!("")),
        ("shellMode", json!("auto")),
    ] {
        let mut retired = check.clone();
        retired["details"][field] = value;
        assert!(!validator.is_valid(&retired), "retired {field}");
    }
}
