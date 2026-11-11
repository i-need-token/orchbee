//! The typed view of the common settings both Clients present, and the value types its fields
//! use. `orchbee-local-runtime` builds it from `config.toml` keys or a settings snapshot; this
//! crate owns only the types and their bounds.
use serde::{Deserialize, Serialize};

mod launch;
pub use launch::{UserLaunchCommand, UserLaunchField, UserLaunchProfile, UserLaunchProfiles};

/// Locale selection remains local to this device; System follows the client's declared locale inputs.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SettingsLocale {
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
const MAXIMUM_COLOR_THEME_ID_BYTES: usize = 64;

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

/// One complete set of common setting values. Every field admits only values within its bounds,
/// so a value built from untrusted input passes [`Self::is_valid`] before anything uses it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UserSettingsValues {
    pub locale: SettingsLocale,
    pub application_theme: ApplicationAppearance,
    /// The color-theme id of the light slot. Any well-formed id is admitted; a Client or Worker
    /// resolves one this build does not ship to the Orchbee theme of the slot's polarity.
    #[serde(default = "default_light_theme", deserialize_with = "color_theme_id")]
    pub light_theme: String,
    /// The color-theme id of the dark slot, admitted and resolved like `light_theme`.
    #[serde(default = "default_dark_theme", deserialize_with = "color_theme_id")]
    pub dark_theme: String,
    /// The terminal Client's color source.
    #[serde(default)]
    pub tui_color_source: TuiColorSource,
    pub terminal_display_theme: TerminalDisplayAppearance,
    #[serde(deserialize_with = "color_value")]
    pub custom_foreground: String,
    #[serde(deserialize_with = "color_value")]
    pub custom_background: String,
    pub copy_on_select: bool,
    pub trim_trailing_blank_rows_for_agents: bool,
    /// Whether the Server records supported Agent sessions in its checkpoint and resumes them in
    /// their Panels after a restart.
    #[serde(default = "default_resume_agents_on_restore")]
    pub resume_agents_on_restore: bool,
    /// Whether the Server keeps each terminal's recent output beside its checkpoint and shows it
    /// again after a restart.
    #[serde(default)]
    pub restore_terminal_output: bool,
    pub agent_notification_mode: AgentReminderMode,
    pub completion_reminder_mode: CompletionReminderMode,
    #[serde(deserialize_with = "minimum_seconds")]
    pub completion_reminder_minimum_seconds: u16,
    pub sound_enabled: bool,
    #[serde(deserialize_with = "scrollback_bytes")]
    pub scrollback_bytes: u32,
    pub launch_profiles: UserLaunchProfiles,
}
impl Default for UserSettingsValues {
    fn default() -> Self {
        Self {
            locale: SettingsLocale::System,
            application_theme: ApplicationAppearance::System,
            light_theme: default_light_theme(),
            dark_theme: default_dark_theme(),
            tui_color_source: TuiColorSource::Theme,
            terminal_display_theme: TerminalDisplayAppearance::Theme,
            custom_foreground: "#141414".into(),
            custom_background: "#fcfcfc".into(),
            copy_on_select: false,
            trim_trailing_blank_rows_for_agents: true,
            resume_agents_on_restore: true,
            restore_terminal_output: false,
            agent_notification_mode: AgentReminderMode::Never,
            completion_reminder_mode: CompletionReminderMode::Never,
            completion_reminder_minimum_seconds: 5,
            sound_enabled: false,
            scrollback_bytes: 10_000_000,
            launch_profiles: UserLaunchProfiles::default(),
        }
    }
}
impl UserSettingsValues {
    /// Whether every value is within its bounds; checked wherever values cross a trust boundary.
    pub fn is_valid(&self) -> bool {
        valid_color(&self.custom_foreground)
            && valid_color(&self.custom_background)
            && valid_color_theme_id(&self.light_theme)
            && valid_color_theme_id(&self.dark_theme)
            && (1..=3600).contains(&self.completion_reminder_minimum_seconds)
            && self.scrollback_bytes <= 1_000_000_000
            && self.launch_profiles.is_valid()
    }
}

fn color_value<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if valid_color(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("invalid color"))
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
fn default_resume_agents_on_restore() -> bool {
    true
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
