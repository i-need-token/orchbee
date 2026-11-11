//! The common preference values derived from `config.toml` keys: the one typed view the reader
//! builds and a Client holding a copy of the file's values builds the same way.
use super::{
    Scalar,
    registry::{SETTING_KEYS, find_setting},
};
use orchbee_workbench_protocol::{PreferenceKey, UserLaunchProfiles, UserPreferencesValues};
use serde_json::Value;

/// The preference values that a file's keys give. `lookup` returns the value of a registered key
/// path such as `appearance.mode`, or `None` when the file leaves it unset; a value the key does
/// not admit keeps its default. The launch catalog is a structured value and arrives whole as
/// `launch_profiles`.
pub fn preference_values(
    lookup: impl Fn(&str) -> Option<Scalar>,
    launch_profiles: UserLaunchProfiles,
) -> UserPreferencesValues {
    let defaults = UserPreferencesValues {
        launch_profiles,
        ..UserPreferencesValues::default()
    };
    let Ok(mut values) = serde_json::to_value(&defaults) else {
        return defaults;
    };
    for setting in SETTING_KEYS {
        let Some(key) = setting.preference else {
            continue;
        };
        if key == PreferenceKey::LaunchProfiles {
            continue;
        }
        let Some(scalar) =
            lookup(setting.path).filter(|scalar| admitted_by_key(setting.path, scalar))
        else {
            continue;
        };
        let mut candidate = values.clone();
        candidate[key.as_str()] = wire_value(key, scalar);
        if admitted(&candidate).is_some() {
            values = candidate;
        }
    }
    admitted(&values).unwrap_or(defaults)
}

/// A looked-up value passes its key's admission again, so values that did not come through the
/// reader meet the same rule.
fn admitted_by_key(path: &str, scalar: &Scalar) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    find_setting(&segments).is_some_and(|setting| setting.admit(&scalar.to_toml()).is_ok())
}

fn admitted(values: &Value) -> Option<UserPreferencesValues> {
    serde_json::from_value::<UserPreferencesValues>(values.clone())
        .ok()
        .filter(|values| values.validate().is_ok())
}

/// The file names two modes after when they notify; the preference values name the condition.
fn wire_value(key: PreferenceKey, scalar: Scalar) -> Value {
    match scalar {
        Scalar::Boolean(value) => Value::Bool(value),
        Scalar::Integer(value) => Value::from(value),
        Scalar::Text(value) => Value::String(
            match (key, value.as_str()) {
                (PreferenceKey::AgentNotificationMode, "when-hidden") => "hidden",
                (PreferenceKey::CompletionReminderMode, "when-unfocused") => "unfocused",
                (_, value) => value,
            }
            .to_owned(),
        ),
    }
}
