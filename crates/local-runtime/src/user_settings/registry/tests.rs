// The registry and the preference values are separate authorities for the same defaults; these
// checks keep them from drifting apart and keep each file key declared once.
use super::*;
use orchbee_workbench_protocol::UserPreferencesValues;

mod key_file;

#[test]
fn every_preference_has_one_file_key_with_the_same_default() {
    let defaults = serde_json::to_value(UserPreferencesValues::default()).unwrap();
    for preference in PreferenceKey::ALL {
        let carriers: Vec<_> = SETTING_KEYS
            .iter()
            .filter(|setting| setting.preference == Some(preference))
            .collect();
        assert!(!carriers.is_empty(), "{preference:?} has no file key");
        if preference == PreferenceKey::LaunchProfiles {
            continue;
        }
        assert_eq!(carriers.len(), 1, "{preference:?}");
        let expected = &defaults[preference.as_str()];
        let declared = match carriers[0].default {
            SettingDefault::Boolean(value) => serde_json::json!(value),
            SettingDefault::Integer(value) => serde_json::json!(value),
            SettingDefault::Text(value) => serde_json::json!(value),
            other => panic!("{preference:?} has default {other:?}"),
        };
        let wire_spelling = match declared.as_str() {
            Some("when-hidden") => serde_json::json!("hidden"),
            Some("when-unfocused") => serde_json::json!("unfocused"),
            _ => declared,
        };
        assert_eq!(&wire_spelling, expected, "{}", carriers[0].path);
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
