//! Device-scoped preferences shared by graphical and terminal clients.
//! Storage, migration, notification delivery and terminal provisioning stay with their owners.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

mod launch;
pub use launch::{UserLaunchCommand, UserLaunchField, UserLaunchProfile, UserLaunchProfiles};

/// Includes the existing 1 MiB launch-profile catalog and the bounded preference envelope.
pub const MAXIMUM_USER_PREFERENCES_BYTES: usize = 2 * 1024 * 1024;

/// Locale selection remains local to this device; System follows the client's declared locale inputs.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum PreferencesLocale {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

/// System follows host appearance without pretending the client can change its host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationAppearance {
    System,
    Light,
    Dark,
}

/// These change default colors only; explicit application colors retain their terminal authority.
/// Theme takes the active theme's canvas colors, Host the host's own defaults where a Client has
/// a host to follow, and Custom the custom pair.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalDisplayAppearance {
    Theme,
    Host,
    Custom,
}

/// Where the terminal Client takes its interface colors. Theme uses the active color-theme slot
/// wherever the host can show it; Terminal keeps the host terminal's own colors. The graphical
/// Client has no host terminal and ignores it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TuiColorSource {
    #[default]
    Theme,
    Terminal,
}

/// The longest admitted color-theme id, in bytes.
pub const MAXIMUM_COLOR_THEME_ID_BYTES: usize = 64;

/// Hidden means no attachment currently presents the Agent's terminal in a focused client.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentReminderMode {
    Never,
    Hidden,
}

/// Completion reminders use the same duration threshold in both clients.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompletionReminderMode {
    Never,
    Unfocused,
    Always,
}

/// Stable keys identify preferences for reads, updates, and resets; they are never translated labels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreferenceKey {
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
    AgentNotificationMode,
    CompletionReminderMode,
    CompletionReminderMinimumSeconds,
    SoundEnabled,
    ScrollbackBytes,
    LaunchProfiles,
}
impl PreferenceKey {
    /// Complete closed inventory; adapters cannot silently omit an admitted common value.
    pub const ALL: [Self; 16] = [
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
        Self::AgentNotificationMode,
        Self::CompletionReminderMode,
        Self::CompletionReminderMinimumSeconds,
        Self::SoundEnabled,
        Self::ScrollbackBytes,
        Self::LaunchProfiles,
    ];
    /// The serialized field name, shared with patch admission.
    pub const fn as_str(self) -> &'static str {
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
            Self::AgentNotificationMode => "agentNotificationMode",
            Self::CompletionReminderMode => "completionReminderMode",
            Self::CompletionReminderMinimumSeconds => "completionReminderMinimumSeconds",
            Self::SoundEnabled => "soundEnabled",
            Self::ScrollbackBytes => "scrollbackBytes",
            Self::LaunchProfiles => "launchProfiles",
        }
    }
}

/// One complete candidate. Admission checks the scalar and collection bounds before publication.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UserPreferencesValues {
    pub locale: PreferencesLocale,
    pub application_theme: ApplicationAppearance,
    /// The color-theme id of the light slot. Any well-formed id is admitted; a Client or Worker
    /// resolves one this build does not ship to the Orchbee theme of the slot's polarity. An
    /// omitted slot reads as Orchbee Light.
    #[serde(default = "default_light_theme", deserialize_with = "color_theme_id")]
    pub light_theme: String,
    /// The color-theme id of the dark slot, admitted and resolved like `light_theme`. An omitted
    /// slot reads as Orchbee Dark.
    #[serde(default = "default_dark_theme", deserialize_with = "color_theme_id")]
    pub dark_theme: String,
    /// The terminal Client's color source. An omitted source reads as Theme.
    #[serde(default)]
    pub tui_color_source: TuiColorSource,
    pub terminal_display_theme: TerminalDisplayAppearance,
    #[serde(deserialize_with = "color_value")]
    pub custom_foreground: String,
    #[serde(deserialize_with = "color_value")]
    pub custom_background: String,
    pub copy_on_select: bool,
    pub trim_trailing_blank_rows_for_agents: bool,
    pub agent_notification_mode: AgentReminderMode,
    pub completion_reminder_mode: CompletionReminderMode,
    #[serde(deserialize_with = "minimum_seconds")]
    pub completion_reminder_minimum_seconds: u16,
    pub sound_enabled: bool,
    #[serde(deserialize_with = "scrollback_bytes")]
    pub scrollback_bytes: u32,
    pub launch_profiles: UserLaunchProfiles,
}
impl Default for UserPreferencesValues {
    fn default() -> Self {
        Self {
            locale: PreferencesLocale::System,
            application_theme: ApplicationAppearance::System,
            light_theme: default_light_theme(),
            dark_theme: default_dark_theme(),
            tui_color_source: TuiColorSource::Theme,
            terminal_display_theme: TerminalDisplayAppearance::Theme,
            custom_foreground: "#141414".into(),
            custom_background: "#fcfcfc".into(),
            copy_on_select: true,
            trim_trailing_blank_rows_for_agents: true,
            agent_notification_mode: AgentReminderMode::Never,
            completion_reminder_mode: CompletionReminderMode::Never,
            completion_reminder_minimum_seconds: 5,
            sound_enabled: false,
            scrollback_bytes: 10_000_000,
            launch_profiles: UserLaunchProfiles::default(),
        }
    }
}
impl UserPreferencesValues {
    /// Validate complete values at every untrusted or persistence boundary.
    pub fn validate(&self) -> Result<(), PreferencesFailure> {
        if !valid_color(&self.custom_foreground)
            || !valid_color(&self.custom_background)
            || !valid_color_theme_id(&self.light_theme)
            || !valid_color_theme_id(&self.dark_theme)
            || !(1..=3600).contains(&self.completion_reminder_minimum_seconds)
            || self.scrollback_bytes > 1_000_000_000
        {
            return Err(PreferencesFailure::InvalidChanges);
        }
        self.launch_profiles.validate()
    }
}

/// A null leaf resets that key to the common default; omitted keys are never written.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct UserPreferencesPatch(BTreeMap<PreferenceKey, Value>);
impl<'de> Deserialize<'de> for UserPreferencesPatch {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let patch = Self(BTreeMap::deserialize(deserializer)?);
        patch
            .apply(&UserPreferencesValues::default())
            .map_err(|_| serde::de::Error::custom("invalid preference changes"))?;
        Ok(patch)
    }
}
impl UserPreferencesPatch {
    /// Compute only changed leaves from a complete validated editor draft.
    pub fn between(before: &UserPreferencesValues, after: &UserPreferencesValues) -> Self {
        let before = serde_json::to_value(before).expect("preference values serialize");
        let after = serde_json::to_value(after).expect("preference values serialize");
        Self(
            PreferenceKey::ALL
                .into_iter()
                .filter(|key| before[key.as_str()] != after[key.as_str()])
                .map(|key| (key, after[key.as_str()].clone()))
                .collect(),
        )
    }
    /// Reset explicitly claims these keys and restores their common defaults.
    pub fn reset(keys: impl IntoIterator<Item = PreferenceKey>) -> Self {
        Self(keys.into_iter().map(|key| (key, Value::Null)).collect())
    }
    /// Compose explicit editor intents; later leaves replace earlier resets or edits.
    pub fn with_overrides(mut self, later: Self) -> Self {
        self.0.extend(later.0);
        self
    }
    /// One validated scalar or collection replacement.
    pub fn set(key: PreferenceKey, value: Value) -> Result<Self, PreferencesFailure> {
        let patch = Self(BTreeMap::from([(key, value)]));
        patch.apply(&UserPreferencesValues::default())?;
        Ok(patch)
    }
    /// Empty edits do not authorize persistence.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Only an explicit reset of every key may replace a refused record.
    pub fn is_reset_all(&self) -> bool {
        self.0.len() == PreferenceKey::ALL.len() && self.0.values().all(Value::is_null)
    }
    /// Exact affected identities used by the update and reset owner.
    pub fn keys(&self) -> impl Iterator<Item = PreferenceKey> + '_ {
        self.0.keys().copied()
    }
    /// Validate and construct the whole candidate before any externally observable effect.
    pub fn apply(
        &self,
        current: &UserPreferencesValues,
    ) -> Result<UserPreferencesValues, PreferencesFailure> {
        let mut candidate =
            serde_json::to_value(current).map_err(|_| PreferencesFailure::InvalidChanges)?;
        let defaults = serde_json::to_value(UserPreferencesValues::default())
            .map_err(|_| PreferencesFailure::InvalidChanges)?;
        for (key, value) in &self.0 {
            candidate[key.as_str()] = if value.is_null() {
                defaults[key.as_str()].clone()
            } else {
                value.clone()
            };
        }
        let candidate: UserPreferencesValues =
            serde_json::from_value(candidate).map_err(|_| PreferencesFailure::InvalidChanges)?;
        candidate.validate()?;
        Ok(candidate)
    }
}

/// No paths or arbitrary file operations cross the helper's request boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "operation", rename_all = "kebab-case")]
pub enum UserPreferencesRequest {
    Read {},
    Update {
        #[serde(deserialize_with = "source_value")]
        source: String,
        #[serde(deserialize_with = "nonempty_patch")]
        changes: UserPreferencesPatch,
    },
}

/// A digest binds exact durable bytes; `missing` denotes absence. `unreadable` is a recovery
/// basis of built-in defaults, never confirmed storage, and only permits a complete reset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UserPreferencesSnapshot {
    #[serde(deserialize_with = "source_value")]
    pub source: String,
    pub values: UserPreferencesValues,
    #[serde(deserialize_with = "claimed_keys")]
    pub claimed: BTreeSet<PreferenceKey>,
}

/// Expected failures remain machine identities; the UI owns localized recovery copy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreferencesFailure {
    InvalidRequest,
    InvalidChanges,
    InvalidSource,
    Unavailable,
    Busy,
    SourceChanged,
    WriteFailed,
}

/// Success confirms persistence (or a read/no-op), never that another client's host has applied it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "kebab-case")]
pub enum UserPreferencesResponse {
    Snapshot {
        // Boxed so the refusal and unknown-outcome variants stay small.
        snapshot: Box<UserPreferencesSnapshot>,
        changed: bool,
    },
    Refused {
        reason: PreferencesFailure,
    },
    /// A publication may have committed but its durability or transport could not be confirmed.
    OutcomeUnknown {},
}

/// Bound bytes, closed keys, source syntax and complete candidate validity before opening storage.
pub fn parse_user_preferences_request(
    bytes: &[u8],
) -> Result<UserPreferencesRequest, PreferencesFailure> {
    if bytes.len() > MAXIMUM_USER_PREFERENCES_BYTES {
        return Err(PreferencesFailure::InvalidRequest);
    }
    let request: UserPreferencesRequest =
        serde_json::from_slice(bytes).map_err(|_| PreferencesFailure::InvalidRequest)?;
    let changes = match &request {
        UserPreferencesRequest::Read {} => return Ok(request),
        UserPreferencesRequest::Update { source, changes } => {
            if !(matches!(source.as_str(), "missing" | "unreadable")
                || source.len() == 64
                    && source
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
            {
                return Err(PreferencesFailure::InvalidRequest);
            }
            if source == "unreadable" && !changes.is_reset_all() {
                return Err(PreferencesFailure::InvalidChanges);
            }
            if changes.is_empty() {
                return Err(PreferencesFailure::InvalidChanges);
            }
            changes
        }
    };
    changes.apply(&UserPreferencesValues::default())?;
    Ok(request)
}

fn source_value<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if matches!(value.as_str(), "missing" | "unreadable")
        || value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("invalid preference source"))
    }
}
fn color_value<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if valid_color(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("invalid preference color"))
    }
}
fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
fn default_light_theme() -> String {
    "orchbee-light".into()
}
fn default_dark_theme() -> String {
    "orchbee-dark".into()
}
fn color_theme_id<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if valid_color_theme_id(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("invalid color theme id"))
    }
}
/// Lowercase kebab-case, as theme files spell their ids, within the byte bound.
fn valid_color_theme_id(value: &str) -> bool {
    value.len() <= MAXIMUM_COLOR_THEME_ID_BYTES
        && !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}
fn minimum_seconds<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    let value = u16::deserialize(deserializer)?;
    if (1..=3600).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("invalid reminder duration"))
    }
}
fn scrollback_bytes<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = u32::deserialize(deserializer)?;
    if value <= 1_000_000_000 {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("invalid scrollback budget"))
    }
}
fn nonempty_patch<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<UserPreferencesPatch, D::Error> {
    let value = UserPreferencesPatch::deserialize(deserializer)?;
    if value.is_empty() {
        return Err(serde::de::Error::custom("empty preference update"));
    }
    value
        .apply(&UserPreferencesValues::default())
        .map_err(|_| serde::de::Error::custom("invalid preference changes"))?;
    Ok(value)
}
fn claimed_keys<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeSet<PreferenceKey>, D::Error> {
    let keys = Vec::<PreferenceKey>::deserialize(deserializer)?;
    let unique: BTreeSet<_> = keys.iter().copied().collect();
    if keys.len() == unique.len() {
        Ok(unique)
    } else {
        Err(serde::de::Error::custom("duplicate claimed preference"))
    }
}
