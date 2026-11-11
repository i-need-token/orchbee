//! The common setting values derived from `config.toml` keys: the one typed view the reader
//! builds and a Client holding a copy of the file's values builds the same way.
use super::{
    Scalar,
    registry::{SETTING_KEYS, current_platform, find_setting},
};
use orchbee_workbench_protocol::{
    SettingChange, SettingEntry, SettingEntryValue, SettingPath, SettingValue, SettingsSnapshot,
    UserLaunchCommand, UserLaunchProfile, UserLaunchProfiles, UserSettingsValues,
};
use serde_json::Value;

/// One field of [`UserSettingsValues`]. Each scalar field is carried by one `config.toml` key; the
/// launch catalog by the platform's profile table and its default profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UserSettingsField {
    Locale,
    ApplicationTheme,
    LightTheme,
    DarkTheme,
    TuiColorSource,
    TerminalDisplayTheme,
    CustomForeground,
    CustomBackground,
    CopyOnSelect,
    TrimTrailingBlankRowsForAgents,
    ResumeAgentsOnRestore,
    RestoreTerminalOutput,
    AgentNotificationMode,
    CompletionReminderMode,
    CompletionReminderMinimumSeconds,
    SoundEnabled,
    ScrollbackBytes,
    LaunchProfiles,
}

impl UserSettingsField {
    /// Every field, so a caller that walks the values cannot silently omit one.
    pub const ALL: [Self; 18] = [
        Self::Locale,
        Self::ApplicationTheme,
        Self::LightTheme,
        Self::DarkTheme,
        Self::TuiColorSource,
        Self::TerminalDisplayTheme,
        Self::CustomForeground,
        Self::CustomBackground,
        Self::CopyOnSelect,
        Self::TrimTrailingBlankRowsForAgents,
        Self::ResumeAgentsOnRestore,
        Self::RestoreTerminalOutput,
        Self::AgentNotificationMode,
        Self::CompletionReminderMode,
        Self::CompletionReminderMinimumSeconds,
        Self::SoundEnabled,
        Self::ScrollbackBytes,
        Self::LaunchProfiles,
    ];

    /// The `config.toml` key that carries a scalar field; `None` for the launch catalog.
    pub const fn key(self) -> Option<&'static str> {
        Some(match self {
            Self::Locale => "general.language",
            Self::ApplicationTheme => "appearance.mode",
            Self::LightTheme => "appearance.light_theme",
            Self::DarkTheme => "appearance.dark_theme",
            Self::TuiColorSource => "appearance.tui_colors",
            Self::TerminalDisplayTheme => "appearance.terminal_colors",
            Self::CustomForeground => "appearance.custom_foreground",
            Self::CustomBackground => "appearance.custom_background",
            Self::CopyOnSelect => "terminal.copy_on_select",
            Self::TrimTrailingBlankRowsForAgents => "terminal.trim_trailing_blank_rows_for_agents",
            Self::ResumeAgentsOnRestore => "agents.resume_on_restore",
            Self::RestoreTerminalOutput => "terminal.restore_output",
            Self::AgentNotificationMode => "notifications.agent_reminder",
            Self::CompletionReminderMode => "notifications.completion_reminder",
            Self::CompletionReminderMinimumSeconds => "notifications.completion_minimum_seconds",
            Self::SoundEnabled => "notifications.sound",
            Self::ScrollbackBytes => "terminal.scrollback_bytes",
            Self::LaunchProfiles => return None,
        })
    }

    /// The member of serialized [`UserSettingsValues`] this field names.
    pub(super) const fn member(self) -> &'static str {
        match self {
            Self::Locale => "locale",
            Self::ApplicationTheme => "applicationTheme",
            Self::LightTheme => "lightTheme",
            Self::DarkTheme => "darkTheme",
            Self::TuiColorSource => "tuiColorSource",
            Self::TerminalDisplayTheme => "terminalDisplayTheme",
            Self::CustomForeground => "customForeground",
            Self::CustomBackground => "customBackground",
            Self::CopyOnSelect => "copyOnSelect",
            Self::TrimTrailingBlankRowsForAgents => "trimTrailingBlankRowsForAgents",
            Self::ResumeAgentsOnRestore => "resumeAgentsOnRestore",
            Self::RestoreTerminalOutput => "restoreTerminalOutput",
            Self::AgentNotificationMode => "agentNotificationMode",
            Self::CompletionReminderMode => "completionReminderMode",
            Self::CompletionReminderMinimumSeconds => "completionReminderMinimumSeconds",
            Self::SoundEnabled => "soundEnabled",
            Self::ScrollbackBytes => "scrollbackBytes",
            Self::LaunchProfiles => "launchProfiles",
        }
    }

    /// The scalar field a registered key path carries.
    fn carried_by(path: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|field| field.key() == Some(path))
    }
}

/// The common values a settings snapshot carries, built the way the reader builds them.
pub fn snapshot_values(snapshot: &SettingsSnapshot) -> UserSettingsValues {
    settings_values(
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
/// spelling. A field in `restored` whose value equals `baseline` is restored to its default instead
/// of being left as the file has it; the launch catalog restores both its profiles and its
/// default profile.
pub fn value_changes(
    baseline: &UserSettingsValues,
    values: &UserSettingsValues,
    restored: &[UserSettingsField],
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
        let Some(field) = UserSettingsField::carried_by(setting.path) else {
            continue;
        };
        let member = field.member();
        let value = (before[member] != after[member])
            .then(|| file_value(field, &after[member]))
            .flatten();
        change(setting.path, value, restored.contains(&field));
    }
    let restore = restored.contains(&UserSettingsField::LaunchProfiles);
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

/// A field's value in the file's spelling.
fn file_value(field: UserSettingsField, value: &Value) -> Option<SettingValue> {
    match value {
        Value::Bool(value) => Some(SettingValue::Boolean(*value)),
        Value::Number(value) => value.as_i64().map(SettingValue::Integer),
        Value::String(value) => Some(SettingValue::Text(
            match (field, value.as_str()) {
                (UserSettingsField::AgentNotificationMode, "hidden") => "when-hidden",
                (UserSettingsField::CompletionReminderMode, "unfocused") => "when-unfocused",
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

/// The common values that a file's keys give. `lookup` returns the value of a registered key
/// path such as `appearance.mode`, or `None` when the file leaves it unset; a value the key does
/// not admit keeps its default. The launch catalog is a structured value and arrives whole as
/// `launch_profiles`.
pub fn settings_values(
    lookup: impl Fn(&str) -> Option<Scalar>,
    launch_profiles: UserLaunchProfiles,
) -> UserSettingsValues {
    let defaults = UserSettingsValues {
        launch_profiles,
        ..UserSettingsValues::default()
    };
    let Ok(mut values) = serde_json::to_value(&defaults) else {
        return defaults;
    };
    for setting in SETTING_KEYS {
        let Some(field) = UserSettingsField::carried_by(setting.path) else {
            continue;
        };
        let Some(scalar) =
            lookup(setting.path).filter(|scalar| admitted_by_key(setting.path, scalar))
        else {
            continue;
        };
        let mut candidate = values.clone();
        candidate[field.member()] = wire_value(field, scalar);
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

fn admitted(values: &Value) -> Option<UserSettingsValues> {
    serde_json::from_value::<UserSettingsValues>(values.clone())
        .ok()
        .filter(UserSettingsValues::is_valid)
}

/// The file names two modes after when they notify; the typed values name the condition.
fn wire_value(field: UserSettingsField, scalar: Scalar) -> Value {
    match scalar {
        Scalar::Boolean(value) => Value::Bool(value),
        Scalar::Integer(value) => Value::from(value),
        Scalar::Text(value) => Value::String(
            match (field, value.as_str()) {
                (UserSettingsField::AgentNotificationMode, "when-hidden") => "hidden",
                (UserSettingsField::CompletionReminderMode, "when-unfocused") => "unfocused",
                (_, value) => value,
            }
            .to_owned(),
        ),
    }
}
