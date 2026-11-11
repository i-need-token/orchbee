//! The common preferences `config.toml` carries, admitted by the preference values' own rules.
use super::{Fields, Scalar, launch_profiles, registry::SETTING_KEYS};
use orchbee_workbench_protocol::{PreferenceKey, UserPreferencesValues};
use serde_json::Value;

/// Read every registered preference key; a refused value keeps its default and a diagnostic.
pub(super) fn parse(fields: &mut Fields<'_>) -> UserPreferencesValues {
    let defaults = UserPreferencesValues::default();
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
        let Some(scalar) = fields.scalar(setting.path) else {
            continue;
        };
        let mut candidate = values.clone();
        candidate[key.as_str()] = wire_value(key, scalar);
        if admitted(&candidate).is_some() {
            values = candidate;
        } else {
            fields.push(setting.path, "invalid-value");
        }
    }
    let Some(mut admitted) = admitted(&values) else {
        return defaults;
    };
    admitted.launch_profiles = launch_profiles::parse(fields);
    admitted
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
