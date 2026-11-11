//! The common preference values derived from `config.toml` keys: the one typed view the reader
//! builds and a Client holding a copy of the file's values builds the same way.
use super::{
    Scalar,
    registry::{SETTING_KEYS, current_platform, find_setting},
};
use orchbee_workbench_protocol::{
    PreferenceKey, SettingChange, SettingEntry, SettingEntryValue, SettingPath, SettingValue,
    SettingsSnapshot, UserLaunchCommand, UserLaunchProfile, UserLaunchProfiles,
    UserPreferencesValues,
};
use serde_json::Value;

/// The preference values a settings snapshot carries, built the way the reader builds them.
pub fn snapshot_preferences(snapshot: &SettingsSnapshot) -> UserPreferencesValues {
    preference_values(
        |path| {
            let key = SettingPath::parse(path).ok()?;
            snapshot
                .values
                .get(&key)?
                .as_ref()
                .map(super::snapshot::scalar)
        },
        snapshot.launch_profiles.clone(),
    )
}

/// The edit that turns `baseline` into `values` on this platform, key by key in the file's
/// spelling. A key in `restored` whose value equals `baseline` is restored to its default instead
/// of being left as the file has it; the launch catalog restores both its profiles and its
/// default profile.
pub fn preference_changes(
    baseline: &UserPreferencesValues,
    values: &UserPreferencesValues,
    restored: &[PreferenceKey],
) -> Vec<SettingChange> {
    let (Ok(before), Ok(after)) = (serde_json::to_value(baseline), serde_json::to_value(values))
    else {
        return Vec::new();
    };
    let mut changes = Vec::new();
    let mut change = |path: &str, value: Option<SettingValue>, restore: bool| {
        let Ok(key) = SettingPath::parse(path) else {
            return;
        };
        match value {
            Some(value) => changes.push(SettingChange::Set { key, value }),
            None if restore => changes.push(SettingChange::Restore { key }),
            None => {}
        }
    };
    for setting in SETTING_KEYS {
        let Some(key) = setting.preference else {
            continue;
        };
        if key == PreferenceKey::LaunchProfiles || setting.path.contains('<') {
            continue;
        }
        let value = (before[key.as_str()] != after[key.as_str()])
            .then(|| file_value(key, &after[key.as_str()]))
            .flatten();
        change(setting.path, value, restored.contains(&key));
    }
    let restore = restored.contains(&PreferenceKey::LaunchProfiles);
    let platform = current_platform();
    let before = &baseline.launch_profiles;
    let after = &values.launch_profiles;
    change(
        &format!("terminal.profiles.{platform}"),
        (before.profiles != after.profiles).then(|| SettingValue::Entries(entries(after))),
        restore,
    );
    change(
        &format!("terminal.default_profile.{platform}"),
        (before.default_profile_id != after.default_profile_id)
            .then(|| SettingValue::Text(after.default_profile_id.clone())),
        restore,
    );
    changes
}

/// A preference value in the file's spelling.
fn file_value(key: PreferenceKey, value: &Value) -> Option<SettingValue> {
    match value {
        Value::Bool(value) => Some(SettingValue::Boolean(*value)),
        Value::Number(value) => value.as_i64().map(SettingValue::Integer),
        Value::String(value) => Some(SettingValue::Text(
            match (key, value.as_str()) {
                (PreferenceKey::AgentNotificationMode, "hidden") => "when-hidden",
                (PreferenceKey::CompletionReminderMode, "unfocused") => "when-unfocused",
                (_, value) => value,
            }
            .to_owned(),
        )),
        _ => None,
    }
}

/// The catalog's profiles in the file's form. A leading built-in profile left as built is not
/// written: the reader puts it there when the table lacks it.
fn entries(catalog: &UserLaunchProfiles) -> Vec<SettingEntry> {
    let built_in = UserLaunchProfiles::default();
    let skip = usize::from(catalog.profiles.first() == built_in.profiles.first());
    catalog.profiles[skip..].iter().map(entry).collect()
}

fn entry(profile: &UserLaunchProfile) -> SettingEntry {
    let text = |value: &str| SettingEntryValue::Text(value.to_owned());
    let mut entry = SettingEntry::new();
    entry.insert("id".into(), text(&profile.id));
    if let Some(name) = &profile.name {
        entry.insert("name".into(), text(name));
    }
    match &profile.command {
        UserLaunchCommand::SystemShell { login } => {
            entry.insert("kind".into(), text("system-shell"));
            entry.insert("login".into(), SettingEntryValue::Boolean(*login));
        }
        UserLaunchCommand::CustomExecutable {
            executable,
            arguments,
        } => {
            entry.insert("kind".into(), text("custom-executable"));
            entry.insert("executable".into(), text(executable));
            entry.insert(
                "arguments".into(),
                SettingEntryValue::List(arguments.clone()),
            );
        }
    }
    entry
}

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
