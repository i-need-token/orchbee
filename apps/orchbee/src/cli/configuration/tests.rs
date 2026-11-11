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
        syntax: None,
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

/// Every edit refusal reaches the failure document with its own reason, exits 1 like every other
/// configuration failure, and its text comes from the same details.
#[test]
fn each_edit_refusal_keeps_its_own_reason_and_exit_class() {
    use crate::{cli_text::CliText, invocation::OutputFormat, localization::Locale};
    use serde_json::Value;
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/orchbee.cli.failure.configuration.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let syntax = SettingsSyntaxError {
        reason: "invalid-toml",
        line: 4,
        column: 9,
    };
    let refusals = [
        (
            SettingsEditError::UnknownKey {
                key: "update.colour".into(),
            },
            "update.colour",
            "unknown-key",
        ),
        (
            SettingsEditError::InvalidValue {
                key: "update.channel".into(),
                field: "update.channel".into(),
                reason: "invalid-channel",
            },
            "update.channel",
            "invalid-channel",
        ),
        (SettingsEditError::Syntax(syntax), "file", "invalid-toml"),
        (
            SettingsEditError::Source(SettingsError::new("ORCHBEE_HOME", "invalid-path")),
            "ORCHBEE_HOME",
            "invalid-path",
        ),
        (SettingsEditError::ReadOnly, "file", "read-only"),
        (SettingsEditError::InUse, "file", "file-in-use"),
        (SettingsEditError::Locked, "file", "locked"),
        (SettingsEditError::Changed, "file", "changed"),
        (SettingsEditError::TooLarge, "file", "too-large"),
        (
            SettingsEditError::Io(std::io::ErrorKind::Other),
            "file",
            "io-error",
        ),
    ];
    let mut reasons = std::collections::BTreeSet::new();
    for (refusal, field, reason) in refusals {
        assert!(reasons.insert(reason), "{reason} is shared");
        let error = Error::from(refusal);
        assert_eq!((error.field.as_str(), error.reason), (field, reason));
        assert_eq!(error.position, (reason == "invalid-toml").then_some((4, 9)));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let copy = Error {
                field: error.field.clone(),
                reason: error.reason,
                position: error.position,
                path: None,
            };
            let output =
                crate::cli::failure_output(copy.failure(), Some(OutputFormat::Json), locale);
            assert_eq!(output.status, 1, "{reason}");
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(validator.is_valid(&document), "{document}");
            assert_eq!(document["failure"]["kind"], "check-failed");
            let details = &document["failure"]["details"];
            assert_eq!(details, &Value::Object(error.details()));
            if let Some(expected) = CliText::new(locale).cli_configuration_refusal(
                reason,
                details["line"].as_u64().zip(details["column"].as_u64()),
            ) {
                assert_eq!(output.stderr, expected, "{reason}");
            }
        }
    }
    for reason in ["read-only", "file-in-use", "locked", "changed", "too-large"] {
        let en = CliText::new(Locale::EnUs).cli_configuration_refusal(reason, None);
        let zh = CliText::new(Locale::ZhCn).cli_configuration_refusal(reason, None);
        assert!(en.is_some() && zh.is_some() && en != zh, "{reason}");
    }
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
        syntax: None,
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
