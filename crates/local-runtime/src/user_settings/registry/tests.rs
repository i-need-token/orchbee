// The registry and the typed values are separate authorities for the same defaults; these checks
// keep them from drifting apart and keep each file key declared once.
use super::*;
use crate::user_settings::UserSettingsField;
use orchbee_workbench_protocol::UserSettingsValues;

mod key_file;

#[test]
fn every_scalar_field_has_one_file_key_with_the_same_default() {
    let defaults = serde_json::to_value(UserSettingsValues::default()).unwrap();
    for field in UserSettingsField::ALL {
        let Some(path) = field.key() else {
            continue;
        };
        let carriers: Vec<_> = SETTING_KEYS
            .iter()
            .filter(|setting| setting.path == path)
            .collect();
        assert_eq!(carriers.len(), 1, "{field:?}");
        let expected = &defaults[field.member()];
        let declared = match carriers[0].default {
            SettingDefault::Boolean(value) => serde_json::json!(value),
            SettingDefault::Integer(value) => serde_json::json!(value),
            SettingDefault::Text(value) => serde_json::json!(value),
            other => panic!("{field:?} has default {other:?}"),
        };
        let wire_spelling = match declared.as_str() {
            Some("when-hidden") => serde_json::json!("hidden"),
            Some("when-unfocused") => serde_json::json!("unfocused"),
            _ => declared,
        };
        assert_eq!(&wire_spelling, expected, "{path}");
    }
}

#[test]
fn no_two_keys_claim_the_same_path() {
    for (index, setting) in SETTING_KEYS.iter().enumerate() {
        let concrete: Vec<String> = setting
            .path
            .split('.')
            .map(|segment| {
                segment
                    .replace("<platform>", "windows")
                    .replace("<action>", ACTIONS[0])
                    .replace('*', "codex")
            })
            .collect();
        let concrete: Vec<&str> = concrete.iter().map(String::as_str).collect();
        let first = SETTING_KEYS
            .iter()
            .position(|other| path_matches(other.path, concrete.iter().copied()));
        assert_eq!(first, Some(index), "{}", setting.path);
    }
}
