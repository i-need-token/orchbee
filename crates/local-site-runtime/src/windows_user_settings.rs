//! Shared Windows user settings: fatal source boundaries, field-local admission, and explicit repair.
mod agent_sidebar;
mod diagnostics;
mod herdr_import;
use diagnostics::{DiagnosticsCollector, unknown_field};
pub use herdr_import::{
    PreparedWindowsHerdrImport, WindowsHerdrImportDisposition, WindowsHerdrImportEntry,
    WindowsHerdrImportReport, WindowsHerdrImportWriteFailure, normalize_windows_herdr_file_operand,
    prepare_windows_herdr_import, read_windows_herdr_source, valid_windows_herdr_file_operand,
    write_windows_herdr_import,
};
mod sidebar_tokens;
mod space_sidebar;
pub use sidebar_tokens::WindowsSidebarTokenStyle;
pub use space_sidebar::{
    MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES, WindowsSpaceSidebarSettings, WindowsSpaceSidebarToken,
    WindowsSpaceSidebarTokenKind,
};
mod keys;
pub use agent_sidebar::{
    MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES, WindowsAgentSidebarSettings, WindowsAgentSidebarToken,
    WindowsAgentSidebarTokenKind,
};
mod notifications;
pub use diagnostics::{
    MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS, WindowsSettingsDiagnostic,
    WindowsSettingsDiagnosticDisposition, WindowsSettingsDiagnostics,
};
pub use notifications::{
    MAXIMUM_AGENT_SOUND_OVERRIDES, MAXIMUM_NOTIFICATION_DELAY_SECONDS,
    MAXIMUM_NOTIFICATION_PROFILE_BYTES, WindowsAgentSoundSetting, WindowsNotificationDelivery,
    WindowsNotificationSettings,
};
mod reset_keys;
mod shell;
mod theme;
pub use theme::WindowsThemeName;
mod window_title;
use crate::windows_user_path::{WINDOWS_USER_PATH_LIMIT, normalize_windows_user_path};
pub use keys::{
    WindowsCommandSize, WindowsCustomCommand, WindowsCustomCommandAction,
    WindowsCustomCommandEntry, WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings,
    WindowsKeyTrigger, trigger_label,
};
pub use reset_keys::{WindowsKeysResetOutcome, WindowsKeysResetReport, reset_windows_keybindings};
pub use shell::{WindowsShellMode, WindowsShellSettings};
use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
pub use window_title::{DEFAULT_WINDOW_TITLE, WindowsWindowTitleTemplate};

const LIMIT: u64 = 64 * 1024;
/// One accepted Windows user-config leaf and its stable offline-reference text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsUserConfigOption {
    /// Dotted field path; `*` denotes a dynamic map key.
    pub path: &'static str,
    /// Effective value when the file or leaf is absent.
    pub default: &'static str,
    /// Accepted values or value shape.
    pub choices: &'static str,
    /// Short explanation suitable for a config-file comment.
    pub description: &'static str,
}

/// Accepted leaf options documented by `--default-config` and used by field admission.
pub const WINDOWS_USER_CONFIG_OPTIONS: &[WindowsUserConfigOption] = &[
    WindowsUserConfigOption {
        path: "terminal.new_cwd",
        default: "unset (no explicit override)",
        choices: "empty or follow, home, current, workspace, or a normalized Windows path",
        description: "Starting directory policy for a new terminal.",
    },
    WindowsUserConfigOption {
        path: "terminal.default_shell",
        default: "empty (choose the host default)",
        choices: "a Windows executable name or normalized path",
        description: "Shell executable used for new terminals.",
    },
    WindowsUserConfigOption {
        path: "terminal.shell_mode",
        default: "auto",
        choices: "auto, login, non_login",
        description: "How the selected shell is started.",
    },
    WindowsUserConfigOption {
        path: "worktrees.directory",
        default: "unset",
        choices: "a normalized Windows path",
        description: "Optional root directory for created worktrees.",
    },
    WindowsUserConfigOption {
        path: "session.resume_agents_on_restore",
        default: "true",
        choices: "true, false",
        description: "Resume Agents when restoring a Session.",
    },
    WindowsUserConfigOption {
        path: "update.channel",
        default: "stable",
        choices: "stable, preview",
        description: "Release channel used by explicit update operations.",
    },
    WindowsUserConfigOption {
        path: "update.manifest_check",
        default: "true",
        choices: "true, false",
        description: "Allow the automatic update-manifest check.",
    },
    WindowsUserConfigOption {
        path: "keys.prefix",
        default: "ctrl+b",
        choices: "one supported key chord",
        description: "Prefix chord for registered Client actions.",
    },
    WindowsUserConfigOption {
        path: "keys.command",
        default: "no commands",
        choices: "up to 64 command tables; see custom-command rules below",
        description: "Optional user-defined key commands; commands run only after their key is invoked.",
    },
    WindowsUserConfigOption {
        path: "keys.command[].command",
        default: "required per command",
        choices: "a non-empty string up to 4096 bytes",
        description: "Command text executed only after its configured key is invoked.",
    },
    WindowsUserConfigOption {
        path: "keys.command[].type",
        default: "shell",
        choices: "shell, pane, popup, plugin_action",
        description: "Command action kind.",
    },
    WindowsUserConfigOption {
        path: "keys.command[].key",
        default: "unbound",
        choices: "one chord string or an array of up to 16 chord strings",
        description: "Optional trigger keys; omitted means the command is unbound.",
    },
    WindowsUserConfigOption {
        path: "keys.command[].description",
        default: "command text",
        choices: "an optional string up to 512 bytes",
        description: "Label shown for the command.",
    },
    WindowsUserConfigOption {
        path: "keys.command[].width",
        default: "unset",
        choices: "integer cells from 1 through 4096 or percentage string from 1% through 100%",
        description: "Popup-only width.",
    },
    WindowsUserConfigOption {
        path: "keys.command[].height",
        default: "unset",
        choices: "integer cells from 1 through 4096 or percentage string from 1% through 100%",
        description: "Popup-only height.",
    },
    WindowsUserConfigOption {
        path: "keys.fullscreen",
        default: "legacy alias for keys.zoom",
        choices: "same as keys.zoom",
        description: "Compatibility alias; prefer the registered `zoom` action.",
    },
    WindowsUserConfigOption {
        path: "theme.name",
        default: "terminal-native",
        choices: "terminal-native, one-light, one-dark",
        description: "Chrome palette; no automatic host-appearance selection.",
    },
    WindowsUserConfigOption {
        path: "ui.window_title",
        default: "{hostname}: {workspace}",
        choices: "up to 4096 bytes; tokens: hostname, workspace, tab, pane, terminal_title; doubled braces are literal",
        description: "Template for the native terminal window title.",
    },
    WindowsUserConfigOption {
        path: "ui.sidebar.agents.rows",
        default: "inherited native layout",
        choices: "up to 16 rows of 16 tokens; state_icon, state_text, machine, workspace, tab, pane, agent, terminal_title, terminal_title_stripped, or custom token keys",
        description: "Rows shown for Agent sidebar entries.",
    },
    WindowsUserConfigOption {
        path: "ui.sidebar.agents.rows_by_agent",
        default: "empty map",
        choices: "agent ID to the same row layout accepted by `rows`",
        description: "Optional per-Agent row overrides.",
    },
    WindowsUserConfigOption {
        path: "ui.sidebar.agents.row_gap",
        default: "0",
        choices: "an unsigned 16-bit integer",
        description: "Vertical gap between Agent sidebar rows.",
    },
    WindowsUserConfigOption {
        path: "ui.sidebar.spaces.rows",
        default: "[['state_icon', 'workspace'], ['branch', 'git_status']]",
        choices: "up to 16 rows of 16 tokens; state_icon, state_text, workspace, branch, git_status, or custom token keys",
        description: "Rows shown for Space sidebar entries.",
    },
    WindowsUserConfigOption {
        path: "ui.sidebar.spaces.row_gap",
        default: "0",
        choices: "an unsigned 16-bit integer",
        description: "Vertical gap between Space sidebar rows.",
    },
    WindowsUserConfigOption {
        path: "ui.toast.delivery",
        default: "off",
        choices: "off, orchbee, terminal, system (legacy alias: herdr)",
        description: "Delivery for explicit CLI notifications.",
    },
    WindowsUserConfigOption {
        path: "ui.toast.enabled",
        default: "unset",
        choices: "true, false",
        description: "Legacy boolean alias for `delivery`; used only when `delivery` is absent.",
    },
    WindowsUserConfigOption {
        path: "ui.toast.agent_delivery",
        default: "orchbee",
        choices: "off, orchbee, terminal, system (legacy alias: herdr)",
        description: "Delivery for automatic Agent notifications.",
    },
    WindowsUserConfigOption {
        path: "ui.toast.delay_seconds",
        default: "1",
        choices: "an integer from 0 through 3600",
        description: "Quiet period before an automatic Agent notification.",
    },
    WindowsUserConfigOption {
        path: "ui.toast.herdr.position",
        default: "bottom-right",
        choices: "top-left, top-right, bottom-left, bottom-right",
        description: "Corner used by the compatibility notification surface.",
    },
    WindowsUserConfigOption {
        path: "ui.sound.enabled",
        default: "false",
        choices: "true, false",
        description: "Enable automatic built-in notification sounds.",
    },
    WindowsUserConfigOption {
        path: "ui.sound.agents.*",
        default: "no overrides",
        choices: "default, on, off; up to 64 Agent IDs of 1-64 lowercase ASCII letters, digits, dots, or hyphens, beginning and ending with a letter or digit",
        description: "Per-Agent sound override; an absent entry inherits the global policy.",
    },
];

/// Whether `key` is an accepted direct child of `table_path` in the field registry.
fn windows_user_config_has_child(table_path: &str, key: &str) -> bool {
    let candidate = format!("{table_path}.{key}");
    let descendant_prefix = format!("{candidate}.");
    WINDOWS_USER_CONFIG_OPTIONS.iter().any(|option| {
        registry_path_matches(option.path, &candidate)
            || option.path.starts_with(descendant_prefix.as_str())
    })
}

fn registry_path_matches(pattern: &str, candidate: &str) -> bool {
    let mut pattern = pattern.split('.');
    let mut candidate = candidate.split('.');
    loop {
        match (pattern.next(), candidate.next()) {
            (None, None) => return true,
            (Some("*"), Some(_)) => {}
            (Some(left), Some(right)) if left == right => {}
            _ => return false,
        }
    }
}

/// One action and its current Client-registry defaults, supplied by the application owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsDefaultActionBindings {
    pub action: &'static str,
    /// Empty means the action has no default key binding.
    pub bindings: Vec<String>,
}

const HERDR_ONLY_UI_FIELDS: &[&str] = &[
    "sidebar_width",
    "sidebar_min_width",
    "sidebar_max_width",
    "sidebar_start_collapsed",
    "sidebar_collapsed_mode",
    "mobile_width_threshold",
    "mouse_capture",
    "copy_on_select",
    "host_cursor",
    "right_click_passthrough_modifier",
    "redraw_on_focus_gained",
    "mouse_scroll_lines",
    "confirm_close",
    "prompt_new_tab_name",
    "prompt_new_workspace_name",
    "pane_borders",
    "pane_outer_borders",
    "pane_scrollbars",
    "pane_gaps",
    "show_agent_labels_on_pane_borders",
    "hide_tab_bar_when_single_tab",
    "tab_bar_position",
    "tab_bar_right",
    "tab_bar_right_separator",
    "agent_panel_sort",
    "agent_panel_scope",
    "status_indicators",
    "accent",
];
const DEFAULT_CONFIG: &str = r#"[terminal]
default_shell = ""
shell_mode = "auto"

[session]
resume_agents_on_restore = true

[update]
channel = "stable"
manifest_check = true

[keys]
prefix = "ctrl+b"
# <default-action-bindings>

[theme]
name = "terminal-native"

[ui]
window_title = "{hostname}: {workspace}"

[ui.toast]
delivery = "off"
agent_delivery = "orchbee"
delay_seconds = 1

[ui.toast.herdr]
position = "bottom-right"

[ui.sound]
enabled = false
"#;
mod channel;
pub use channel::set_windows_update_channel;

/// Return the deterministic offline Windows configuration template without Client action rows.
pub fn default_windows_user_settings_toml() -> String {
    default_windows_user_settings_toml_with_actions(&[])
}

/// Render setting help and current Client-registry actions into one offline TOML template.
pub fn default_windows_user_settings_toml_with_actions(
    actions: &[WindowsDefaultActionBindings],
) -> String {
    let mut output = String::from(
        "# Orchbee Windows user configuration. Comments are fixed English so this offline reference is deterministic across locales.\n# Edit or remove settings as needed; action rows below stay comments until explicitly enabled.\n# Setting reference (default; accepted values; purpose):\n",
    );
    for option in WINDOWS_USER_CONFIG_OPTIONS {
        use std::fmt::Write as _;
        writeln!(output, "# {}", option.path).expect("writing to a String cannot fail");
        push_wrapped_comment(&mut output, "  Default:", option.default);
        push_wrapped_comment(&mut output, "  Values:", option.choices);
        push_wrapped_comment(&mut output, "  Purpose:", option.description);
    }
    output.push('\n');
    let action_comments = render_action_comments(actions);
    output.push_str(&DEFAULT_CONFIG.replace("# <default-action-bindings>", &action_comments));
    output
}

fn render_action_comments(actions: &[WindowsDefaultActionBindings]) -> String {
    let mut output = String::from(
        "# Registered Client actions and their current defaults; uncomment one line to override it.\n",
    );
    for action in actions {
        let value = if action.bindings.is_empty() {
            "[]".to_owned()
        } else {
            let values = action
                .bindings
                .iter()
                .map(|binding| toml::Value::String(binding.clone()).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        };
        use std::fmt::Write as _;
        writeln!(output, "# {} = {}", action.action, value)
            .expect("writing to a String cannot fail");
    }
    output.push_str(
        "# Custom-command rules: each [[keys.command]] requires a non-empty command (max 4096 bytes).\n# Type is shell (default), pane, popup, or plugin_action; the last accepts up to 241 ASCII characters from letters, digits, . _ : -.\n# Key is optional and accepts one chord or up to 16; description is optional (max 512 bytes).\n# Width and height are popup-only (1..4096 cells or 1%..100%). The config reader never executes a command.\n# Example (uncomment to add a popup command):\n# [[keys.command]]\n# command = \"pwsh.exe -NoProfile\"\n# type = \"popup\"\n# key = \"prefix+f12\"\n# description = \"Open a PowerShell popup\"\n# width = \"60%\"\n# height = 20\n",
    );
    output
}

fn push_wrapped_comment(output: &mut String, label: &str, value: &str) {
    let mut line = format!("# {label}");
    let continuation = "#     ";
    let mut line_has_value = false;
    for word in value.split_whitespace() {
        if line_has_value && line.len() + 1 + word.len() > 100 {
            output.push_str(&line);
            output.push('\n');
            line = continuation.to_owned();
            line.push_str(word);
        } else {
            line.push(' ');
            line.push_str(word);
        }
        line_has_value = true;
    }
    output.push_str(&line);
    output.push('\n');
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsUserSettings {
    pub theme: WindowsThemeName,
    pub window_title: String,
    pub agent_sidebar: WindowsAgentSidebarSettings,
    pub space_sidebar: WindowsSpaceSidebarSettings,
    pub notifications: WindowsNotificationSettings,
    pub keys: WindowsKeySettings,
    pub shell: WindowsShellSettings,
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub explicit: bool,
    pub update_channel: WindowsUpdateChannel,
    pub update_channel_source: &'static str,
    pub manifest_check: bool,
    pub manifest_check_source: &'static str,
    pub cwd: Option<(WindowsDirectoryPolicy, Option<String>)>,
    pub worktree_root: Option<String>,
    pub resume_agents_on_restore: bool,
    pub resume_agents_on_restore_source: &'static str,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// One complete effective settings candidate and its bounded admission diagnostics.
pub struct WindowsUserSettingsLoad {
    /// Effective settings after field-local fallback.
    pub settings: WindowsUserSettings,
    /// Diagnostics describing every visible fallback, ignored field, or unsupported field.
    pub diagnostics: WindowsSettingsDiagnostics,
}

impl std::ops::Deref for WindowsUserSettingsLoad {
    type Target = WindowsUserSettings;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowsUpdateChannel {
    #[default]
    Stable,
    Preview,
}
impl WindowsUpdateChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
        }
    }
}

impl Default for WindowsUserSettings {
    fn default() -> Self {
        Self {
            theme: WindowsThemeName::default(),
            window_title: DEFAULT_WINDOW_TITLE.into(),
            agent_sidebar: WindowsAgentSidebarSettings::default(),
            space_sidebar: WindowsSpaceSidebarSettings::default(),
            notifications: WindowsNotificationSettings::default(),
            keys: WindowsKeySettings::default(),
            shell: WindowsShellSettings::default(),
            path: None,
            exists: false,
            explicit: false,
            update_channel: WindowsUpdateChannel::Stable,
            update_channel_source: "default",
            manifest_check: true,
            manifest_check_source: "default",
            cwd: None,
            worktree_root: None,
            resume_agents_on_restore: true,
            resume_agents_on_restore_source: "default",
        }
    }
}

/// Resolve once; an environment override never bypasses full-file validation.
pub fn read_windows_user_settings(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    let mut load = read_file(&env)?;
    apply_resume_environment(&mut load, &env)?;
    Ok(load)
}

fn apply_resume_environment(
    load: &mut WindowsUserSettingsLoad,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<(), WindowsSettingsError> {
    if let Some(value) = env("ORCHBEE_RESUME_AGENTS_ON_RESTORE") {
        load.settings.resume_agents_on_restore = match value.to_str() {
            Some("true") => true,
            Some("false") => false,
            _ => {
                return Err(WindowsSettingsError::new(
                    "ORCHBEE_RESUME_AGENTS_ON_RESTORE",
                    "invalid-boolean",
                ));
            }
        };
        load.settings.resume_agents_on_restore_source = "environment";
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
pub struct WindowsSettingsError {
    pub field: &'static str,
    pub reason: &'static str,
}
impl WindowsSettingsError {
    pub const fn new(field: &'static str, reason: &'static str) -> Self {
        Self { field, reason }
    }
}

impl std::fmt::Display for WindowsSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid Windows configuration: {} ({})",
            self.field, self.reason
        )
    }
}
impl std::error::Error for WindowsSettingsError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsDirectoryPolicy {
    Follow,
    Home,
    Current,
    Workspace,
    Explicit,
}

fn location(
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<Option<(PathBuf, bool)>, WindowsSettingsError> {
    let (value, field, explicit) = if let Some(value) = env("ORCHBEE_CONFIG_FILE") {
        (value, "ORCHBEE_CONFIG_FILE", true)
    } else if let Some(value) = env("APPDATA") {
        (value, "APPDATA", false)
    } else {
        return Ok(None);
    };
    let raw = value
        .to_str()
        .ok_or(WindowsSettingsError::new(field, "invalid-path"))?;
    let path = Path::new(raw);
    if !path.is_absolute() || raw.is_empty() {
        return Err(WindowsSettingsError::new(field, "invalid-path"));
    }
    let checked =
        normalize_windows_user_path(raw, env, || Err(std::io::ErrorKind::NotFound.into()))
            .map_err(|_| WindowsSettingsError::new(field, "invalid-path"))?;
    let path = PathBuf::from(checked.as_str());
    let path = if explicit {
        path
    } else {
        path.join("Orchbee/config.toml")
    };
    if path
        .to_str()
        .is_none_or(|value| value.len() > WINDOWS_USER_PATH_LIMIT)
    {
        return Err(WindowsSettingsError::new(field, "invalid-path"));
    }
    Ok(Some((path, explicit)))
}

/// The shared settings location without opening the file or applying runtime settings.
/// Companion user assets use this parent directory; absence remains explicit.
pub fn windows_user_settings_path(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<Option<PathBuf>, WindowsSettingsError> {
    location(&env).map(|location| location.map(|(path, _)| path))
}

fn read_file(
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    let location = location(&env)?;
    read_file_at(
        location.as_ref().map(|(path, _)| path.as_path()),
        location.as_ref().is_some_and(|(_, explicit)| *explicit),
        env,
    )
}
/// Explicit reload keeps the admitted startup path, including default-file absence semantics.
pub fn read_windows_user_settings_at(
    path: Option<&Path>,
    explicit: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    let mut load = read_file_at(path, explicit, &env)?;
    apply_resume_environment(&mut load, &env)?;
    Ok(load)
}

fn read_file_at(
    path: Option<&Path>,
    explicit: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    let Some(path) = path else {
        if explicit {
            return Err(WindowsSettingsError::new("file", "invalid-path"));
        }
        return Ok(WindowsUserSettingsLoad::default());
    };
    let path = path.to_owned();
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !explicit => {
            return Ok(WindowsUserSettingsLoad {
                settings: WindowsUserSettings {
                    path: Some(path),
                    ..WindowsUserSettings::default()
                },
                diagnostics: WindowsSettingsDiagnostics::default(),
            });
        }
        Err(error) => {
            return Err(WindowsSettingsError::new(
                "file",
                if error.kind() == std::io::ErrorKind::NotFound {
                    "missing"
                } else if std::fs::metadata(&path).is_ok_and(|metadata| !metadata.is_file()) {
                    "not-file"
                } else {
                    "io-error"
                },
            ));
        }
    };
    let metadata = file
        .metadata()
        .map_err(|_| WindowsSettingsError::new("file", "io-error"))?;
    if !metadata.is_file() {
        return Err(WindowsSettingsError::new("file", "not-file"));
    }
    if metadata.len() > LIMIT {
        return Err(WindowsSettingsError::new("file", "too-large"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WindowsSettingsError::new("file", "io-error"))?;
    if bytes.len() as u64 > LIMIT {
        return Err(WindowsSettingsError::new("file", "too-large"));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| WindowsSettingsError::new("file", "invalid-utf8"))?;
    let mut load = parse(text, &path, &env)?;
    load.settings.explicit = explicit;
    Ok(load)
}

fn parse(
    text: &str,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsSettingsError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|_| WindowsSettingsError::new("file", "invalid-toml"))?;
    let mut settings = WindowsUserSettings {
        path: Some(path.to_owned()),
        exists: true,
        ..WindowsUserSettings::default()
    };
    let mut diagnostics = DiagnosticsCollector::default();
    for (section, value) in table {
        if section == "ui" {
            let Some(table) = value.as_table() else {
                diagnostics.push(
                    "ui",
                    "wrong-type",
                    WindowsSettingsDiagnosticDisposition::Defaulted,
                );
                continue;
            };
            for key in table
                .keys()
                .filter(|key| !windows_user_config_has_child("ui", key))
            {
                diagnostics.push(
                    unknown_field("ui", key),
                    if HERDR_ONLY_UI_FIELDS.contains(&key.as_str()) {
                        "unsupported-herdr"
                    } else {
                        "unknown-key"
                    },
                    if HERDR_ONLY_UI_FIELDS.contains(&key.as_str()) {
                        WindowsSettingsDiagnosticDisposition::Unsupported
                    } else {
                        WindowsSettingsDiagnosticDisposition::Ignored
                    },
                );
            }
            if table.contains_key("toast") || table.contains_key("sound") {
                settings.notifications = WindowsNotificationSettings::parse_lenient(
                    table.get("toast"),
                    table.get("sound"),
                    &mut diagnostics,
                );
            }
            if let Some(sidebar) = table.get("sidebar") {
                if let Some(sidebar) = sidebar.as_table() {
                    for key in sidebar
                        .keys()
                        .filter(|key| !windows_user_config_has_child("ui.sidebar", key))
                    {
                        diagnostics.push(
                            unknown_field("ui.sidebar", key),
                            "unknown-key",
                            WindowsSettingsDiagnosticDisposition::Ignored,
                        );
                    }
                    if let Some(spaces) = sidebar.get("spaces") {
                        match parse_space_sidebar(spaces, &mut diagnostics) {
                            Ok(profile) => settings.space_sidebar = profile,
                            Err(error) => diagnostics.push_error(
                                &error,
                                WindowsSettingsDiagnosticDisposition::Defaulted,
                            ),
                        }
                    }
                    if let Some(agents) = sidebar.get("agents") {
                        match parse_agent_sidebar(agents, &mut diagnostics) {
                            Ok(profile) => settings.agent_sidebar = profile,
                            Err(error) => diagnostics.push_error(
                                &error,
                                WindowsSettingsDiagnosticDisposition::Defaulted,
                            ),
                        }
                    }
                } else {
                    diagnostics.push(
                        "ui.sidebar",
                        "wrong-type",
                        WindowsSettingsDiagnosticDisposition::Defaulted,
                    );
                }
            }
            if let Some(value) = table.get("window_title") {
                if let Some(value) = value.as_str() {
                    match WindowsWindowTitleTemplate::parse(value) {
                        Ok(_) => settings.window_title = value.into(),
                        Err(reason) => diagnostics.push(
                            "ui.window_title",
                            reason,
                            WindowsSettingsDiagnosticDisposition::Defaulted,
                        ),
                    }
                } else {
                    diagnostics.push(
                        "ui.window_title",
                        "wrong-type",
                        WindowsSettingsDiagnosticDisposition::Defaulted,
                    );
                }
            }
            continue;
        }
        if section == "keys" {
            settings.keys = WindowsKeySettings::parse_lenient(&value, &mut diagnostics);
            continue;
        }
        if section == "theme" {
            settings.theme = theme::admit(&value, &mut diagnostics);
            continue;
        }
        if matches!(
            section.as_str(),
            "onboarding" | "server" | "advanced" | "experimental" | "remote"
        ) {
            record_unsupported_fields(&section, &value, &mut diagnostics);
            continue;
        }
        let Some(table) = value.as_table() else {
            let field = match section.as_str() {
                "terminal" => "terminal",
                "update" => "update",
                "worktrees" => "worktrees",
                "session" => "session",
                _ => {
                    diagnostics.push(
                        unknown_field("", &section),
                        "unknown-key",
                        WindowsSettingsDiagnosticDisposition::Ignored,
                    );
                    continue;
                }
            };
            diagnostics.push(
                field,
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            );
            continue;
        };
        match section.as_str() {
            "terminal" => parse_terminal(table, &mut settings, &mut diagnostics, path, env),
            "update" => parse_update(table, &mut settings, &mut diagnostics),
            "worktrees" => parse_worktrees(table, &mut settings, &mut diagnostics, path, env),
            "session" => parse_session(table, &mut settings, &mut diagnostics),
            _ => diagnostics.push(
                unknown_field("", &section),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            ),
        }
    }
    Ok(WindowsUserSettingsLoad {
        settings,
        diagnostics: diagnostics.finish(),
    })
}

fn record_unsupported_fields(
    section: &str,
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) {
    let Some(table) = value.as_table() else {
        diagnostics.push(
            section,
            "unsupported-herdr",
            WindowsSettingsDiagnosticDisposition::Unsupported,
        );
        return;
    };
    if table.is_empty() {
        diagnostics.push(
            section,
            "unsupported-herdr",
            WindowsSettingsDiagnosticDisposition::Unsupported,
        );
    }
    for key in table.keys() {
        diagnostics.push(
            unknown_field(section, key),
            "unsupported-herdr",
            WindowsSettingsDiagnosticDisposition::Unsupported,
        );
    }
}

fn parse_agent_sidebar(
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) -> Result<WindowsAgentSidebarSettings, WindowsSettingsError> {
    let Some(table) = value.as_table() else {
        return WindowsAgentSidebarSettings::parse(value);
    };
    let mut admitted = table.clone();
    admitted.retain(|key, _| {
        let known = windows_user_config_has_child("ui.sidebar.agents", key);
        if !known {
            diagnostics.push(
                unknown_field("ui.sidebar.agents", key),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        known
    });
    WindowsAgentSidebarSettings::parse(&toml::Value::Table(admitted))
}

fn parse_space_sidebar(
    value: &toml::Value,
    diagnostics: &mut DiagnosticsCollector,
) -> Result<WindowsSpaceSidebarSettings, WindowsSettingsError> {
    let Some(table) = value.as_table() else {
        return WindowsSpaceSidebarSettings::parse(value);
    };
    let mut admitted = table.clone();
    admitted.retain(|key, _| {
        let known = windows_user_config_has_child("ui.sidebar.spaces", key);
        if !known {
            diagnostics.push(
                unknown_field("ui.sidebar.spaces", key),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        known
    });
    WindowsSpaceSidebarSettings::parse(&toml::Value::Table(admitted))
}

fn parse_terminal(
    table: &toml::Table,
    settings: &mut WindowsUserSettings,
    diagnostics: &mut DiagnosticsCollector,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) {
    for key in table
        .keys()
        .filter(|key| !windows_user_config_has_child("terminal", key))
    {
        let unsupported = key == "kitty_graphics";
        diagnostics.push(
            unknown_field("terminal", key),
            if unsupported {
                "unsupported-herdr"
            } else {
                "unknown-key"
            },
            if unsupported {
                WindowsSettingsDiagnosticDisposition::Unsupported
            } else {
                WindowsSettingsDiagnosticDisposition::Ignored
            },
        );
    }
    let mut executable = String::new();
    if let Some(value) = table.get("default_shell") {
        match value.as_str() {
            Some(value) => match WindowsShellSettings::new(value, "auto") {
                Ok(shell) => executable = shell.executable().to_owned(),
                Err(error) => {
                    diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
                }
            },
            None => diagnostics.push(
                "terminal.default_shell",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    let mut mode = "auto";
    if let Some(value) = table.get("shell_mode") {
        match value.as_str() {
            Some(value) => match WindowsShellSettings::new("", value) {
                Ok(_) => mode = value,
                Err(error) => {
                    diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
                }
            },
            None => diagnostics.push(
                "terminal.shell_mode",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    settings.shell = WindowsShellSettings::new(&executable, mode)
        .expect("independently admitted shell leaves compose");
    let Some(value) = table.get("new_cwd") else {
        return;
    };
    let Some(value) = value.as_str() else {
        diagnostics.push(
            "terminal.new_cwd",
            "wrong-type",
            WindowsSettingsDiagnosticDisposition::Defaulted,
        );
        return;
    };
    let candidate = match value.trim() {
        "" | "follow" => Ok((WindowsDirectoryPolicy::Follow, None)),
        "home" => Ok((WindowsDirectoryPolicy::Home, None)),
        "current" => Ok((WindowsDirectoryPolicy::Current, None)),
        "workspace" => Ok((WindowsDirectoryPolicy::Workspace, None)),
        _ => normalize(value, path, env, "terminal.new_cwd")
            .map(|value| (WindowsDirectoryPolicy::Explicit, Some(value))),
    };
    match candidate {
        Ok(candidate) => settings.cwd = Some(candidate),
        Err(error) => {
            diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
        }
    }
}

fn parse_update(
    table: &toml::Table,
    settings: &mut WindowsUserSettings,
    diagnostics: &mut DiagnosticsCollector,
) {
    for key in table
        .keys()
        .filter(|key| !windows_user_config_has_child("update", key))
    {
        let unsupported = key == "version_check";
        diagnostics.push(
            unknown_field("update", key),
            if unsupported {
                "unsupported-herdr"
            } else {
                "unknown-key"
            },
            if unsupported {
                WindowsSettingsDiagnosticDisposition::Unsupported
            } else {
                WindowsSettingsDiagnosticDisposition::Ignored
            },
        );
    }
    if let Some(value) = table.get("channel") {
        match value.as_str() {
            Some("stable") => {
                settings.update_channel = WindowsUpdateChannel::Stable;
                settings.update_channel_source = "file";
            }
            Some("preview") => {
                settings.update_channel = WindowsUpdateChannel::Preview;
                settings.update_channel_source = "file";
            }
            Some(_) => diagnostics.push(
                "update.channel",
                "invalid-channel",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
            None => diagnostics.push(
                "update.channel",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
    if let Some(value) = table.get("manifest_check") {
        match value.as_bool() {
            Some(value) => {
                settings.manifest_check = value;
                settings.manifest_check_source = "file";
            }
            None => diagnostics.push(
                "update.manifest_check",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
}

fn parse_worktrees(
    table: &toml::Table,
    settings: &mut WindowsUserSettings,
    diagnostics: &mut DiagnosticsCollector,
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) {
    for key in table
        .keys()
        .filter(|key| !windows_user_config_has_child("worktrees", key))
    {
        diagnostics.push(
            unknown_field("worktrees", key),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
    }
    let Some(value) = table.get("directory") else {
        return;
    };
    let candidate = value
        .as_str()
        .ok_or(WindowsSettingsError::new(
            "worktrees.directory",
            "wrong-type",
        ))
        .and_then(|value| normalize(value, path, env, "worktrees.directory"));
    match candidate {
        Ok(value) => settings.worktree_root = Some(value),
        Err(error) => {
            diagnostics.push_error(&error, WindowsSettingsDiagnosticDisposition::Defaulted)
        }
    }
}

fn parse_session(
    table: &toml::Table,
    settings: &mut WindowsUserSettings,
    diagnostics: &mut DiagnosticsCollector,
) {
    for key in table
        .keys()
        .filter(|key| !windows_user_config_has_child("session", key))
    {
        diagnostics.push(
            unknown_field("session", key),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
    }
    if let Some(value) = table.get("resume_agents_on_restore") {
        match value.as_bool() {
            Some(value) => {
                settings.resume_agents_on_restore = value;
                settings.resume_agents_on_restore_source = "file";
            }
            None => diagnostics.push(
                "session.resume_agents_on_restore",
                "wrong-type",
                WindowsSettingsDiagnosticDisposition::Defaulted,
            ),
        }
    }
}

fn normalize(
    value: &str,
    file: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
    field: &'static str,
) -> Result<String, WindowsSettingsError> {
    if value.trim().is_empty() {
        return Err(WindowsSettingsError::new(field, "invalid-path"));
    }
    normalize_windows_user_path(value, env, || {
        file.parent()
            .map(Path::to_owned)
            .ok_or_else(|| std::io::ErrorKind::NotFound.into())
    })
    .map_err(|_| WindowsSettingsError::new(field, "invalid-path"))
}

#[cfg(test)]
mod tests;
