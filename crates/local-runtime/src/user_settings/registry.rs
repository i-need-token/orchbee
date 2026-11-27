//! The one declaration of every `config.toml` key, and admission of scalar values by their type.
//!
//! The reader, the offline reference, unknown-key detection, diagnostic bundles and the editor all
//! derive their key knowledge from [`SETTING_KEYS`]; no second list of keys exists.
use super::{keys::ACTIONS, notifications::MAXIMUM_NOTIFICATION_DELAY_SECONDS};
use orchbee_workbench_protocol::{
    AgentStartSelector, DEFAULT_QUOTA_RESUME_PROMPT, MAXIMUM_AGENT_ARGUMENT_INPUT_BYTES,
    MAXIMUM_QUOTA_RESUME_PROMPT_BYTES, parse_agent_arguments, valid_quota_resume_prompt,
};

pub(super) const AGENT_ARGUMENTS_PATH: &str = "agents.launch.<agent>.arguments";
pub(super) const AGENT_YOLO_PATH: &str = "agents.launch.<yolo-agent>.yolo";
const QUOTA_RESUME_PROMPT_PATH: &str = "agents.quota_resume.prompt";

/// Longest color-theme id, the bound the typed values admit.
const THEME_ID_BYTES: usize = 64;

/// Settings-page category, in page order. Display names belong to each client's localization.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettingCategory {
    General,
    Appearance,
    /// Holds the light and dark theme choices, whose keys stay in the `appearance` table.
    Themes,
    Terminal,
    /// The File Bar's placement and what its views confirm; only a window that offers the File Bar
    /// shows it.
    FileBar,
    Notifications,
    Keyboard,
    Agents,
    /// The version and update control, beside the update settings.
    About,
}

impl SettingCategory {
    /// Stable identifier shared by both clients.
    pub const fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Themes => "themes",
            Self::Terminal => "terminal",
            Self::FileBar => "file-bar",
            Self::Notifications => "notifications",
            Self::Keyboard => "keyboard",
            Self::Agents => "agents",
            Self::About => "about",
        }
    }
}

/// Group inside a category, in page order. Display names belong to each client's localization.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettingGroup {
    Basics,
    Restore,
    Confirmations,
    QuotaResume,
    AgentLaunch,
    Updates,
    Theme,
    TerminalColors,
    TerminalText,
    Sidebar,
    Light,
    Dark,
    Input,
    Output,
    Panels,
    Profiles,
    When,
    How,
    Prefix,
    Commands,
    /// The File Bar's side of the terminal area.
    Placement,
    /// What the File Bar's Explorer confirms before it changes files.
    Explorer,
    /// How the File Bar's Source Control view works with Git.
    SourceControl,
}

impl SettingGroup {
    /// Stable identifier shared by both clients.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Basics => "basics",
            Self::Restore => "restore",
            Self::Confirmations => "confirmations",
            Self::QuotaResume => "quota-resume",
            Self::AgentLaunch => "agent-launch",
            Self::Updates => "updates",
            Self::Theme => "theme",
            Self::TerminalColors => "tcolors",
            Self::TerminalText => "text",
            Self::Sidebar => "sidebar",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Input => "input",
            Self::Output => "output",
            Self::Panels => "panels",
            Self::Profiles => "profiles",
            Self::When => "when",
            Self::How => "how",
            Self::Prefix => "prefix",
            Self::Commands => "commands",
            Self::Placement => "placement",
            Self::Explorer => "explorer",
            Self::SourceControl => "source-control",
        }
    }
}

/// The clients whose behaviour a key changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingClients {
    Both,
    Tui,
    Desktop,
}

impl SettingClients {
    /// Stable identifier shared by both clients.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Tui => "tui",
            Self::Desktop => "desktop",
        }
    }
}

/// Whose file a key is read from and written to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingScope {
    /// The file on the Client's own machine: how that Client looks and handles input.
    Client,
    /// The file on the attached Server's machine: what the Server does with terminals, Agents
    /// and updates.
    Server,
}

impl SettingScope {
    /// Stable identifier shared by both clients.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
        }
    }
}

/// When a changed value takes effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingEffect {
    /// Running clients apply it as soon as the Server publishes the file.
    Immediate,
    /// Terminals opened after the change use it; open terminals keep their value.
    NewTerminals,
    /// It takes effect after Orchbee restarts.
    Restart,
}

impl SettingEffect {
    /// Stable identifier shared by both clients.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::NewTerminals => "new-terminals",
            Self::Restart => "restart",
        }
    }
}

/// The value a key admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingValue {
    Boolean,
    /// An integer from `minimum` through `maximum` that is a multiple of `step`.
    Integer {
        minimum: i64,
        maximum: i64,
        step: i64,
    },
    /// One of these strings.
    Choice(&'static [&'static str]),
    /// A string of at most `maximum_bytes` UTF-8 bytes whose shape `shape` states.
    Text {
        maximum_bytes: usize,
        shape: &'static str,
    },
    /// `#` followed by six lowercase hexadecimal digits.
    Color,
    /// A color-theme id: lowercase letters, digits and single hyphens, at most 64 characters.
    Theme,
    /// An absolute folder, a folder under `~`, or one relative to the settings file.
    Path,
    /// A table or array with its own admission rules, which `shape` states.
    Structured(&'static str),
}

/// The effective value when a key is absent from the file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingDefault {
    Boolean(bool),
    Integer(i64),
    Text(&'static str),
    /// A structured value written as TOML.
    Toml(&'static str),
    /// No value: `meaning` names the behaviour. `example` is a TOML value the reference shows, or
    /// a whole `key = value` line for a key under a map of names.
    Unset {
        meaning: &'static str,
        example: &'static str,
    },
}

/// One `config.toml` key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingKey {
    /// Dotted path. `*` stands for any name, `<action>` for a registered Client action,
    /// `<platform>` for `windows`, `macos` or `linux`, and a `[]` suffix for an array of tables.
    pub path: &'static str,
    pub value: SettingValue,
    pub default: SettingDefault,
    pub category: SettingCategory,
    pub group: SettingGroup,
    pub scope: SettingScope,
    pub clients: SettingClients,
    pub effect: SettingEffect,
    /// Whether the settings page shows the key; other keys live only in the file.
    pub page: bool,
    /// Whether `config default` lists the key.
    pub reference: bool,
    /// Fixed English purpose for the offline reference.
    pub description: &'static str,
}

/// Platforms with their own launch-profile tables.
pub const SETTING_PLATFORMS: [&str; 3] = ["windows", "macos", "linux"];

/// The current platform's launch-profile table name.
pub const fn current_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

const BOTH: SettingClients = SettingClients::Both;
const CLIENT: SettingScope = SettingScope::Client;
const SERVER: SettingScope = SettingScope::Server;

const fn key(
    path: &'static str,
    value: SettingValue,
    default: SettingDefault,
    placement: (SettingCategory, SettingGroup),
    scope: SettingScope,
    description: &'static str,
) -> SettingKey {
    SettingKey {
        path,
        value,
        default,
        category: placement.0,
        group: placement.1,
        scope,
        clients: BOTH,
        effect: SettingEffect::Immediate,
        page: false,
        reference: true,
        description,
    }
}

impl SettingKey {
    const fn page(mut self) -> Self {
        self.page = true;
        self
    }
    const fn clients(mut self, clients: SettingClients) -> Self {
        self.clients = clients;
        self
    }
    const fn effect(mut self, effect: SettingEffect) -> Self {
        self.effect = effect;
        self
    }
    const fn unreferenced(mut self) -> Self {
        self.reference = false;
        self
    }
}

use SettingCategory as C;
use SettingDefault as D;
use SettingGroup as G;
use SettingValue as V;

const fn integer(minimum: i64, maximum: i64) -> SettingValue {
    V::Integer {
        minimum,
        maximum,
        step: 1,
    }
}
const fn unset(meaning: &'static str, example: &'static str) -> SettingDefault {
    D::Unset { meaning, example }
}
const SIDEBAR_ROWS: &str = "up to 16 rows of up to 16 tokens";
const DESKTOP: SettingClients = SettingClients::Desktop;
const TUI: SettingClients = SettingClients::Tui;

/// Every `config.toml` key, in reference order.
pub const SETTING_KEYS: &[SettingKey] = &[
    key(
        "general.language",
        V::Choice(&["system", "en-US", "zh-CN"]),
        D::Text("system"),
        (C::General, G::Basics),
        CLIENT,
        "Display language; system follows the operating system.",
    )
    .page(),
    key(
        "appearance.mode",
        V::Choice(&["system", "light", "dark"]),
        D::Text("system"),
        (C::Appearance, G::Theme),
        CLIENT,
        "Light or dark appearance; system follows the operating system.",
    )
    .page(),
    key(
        "appearance.light_theme",
        V::Theme,
        D::Text("orchbee-light"),
        (C::Themes, G::Light),
        CLIENT,
        "Color theme in light appearance; an id this build does not ship reads as Orchbee Light.",
    )
    .page(),
    key(
        "appearance.dark_theme",
        V::Theme,
        D::Text("orchbee-dark"),
        (C::Themes, G::Dark),
        CLIENT,
        "Color theme in dark appearance; an id this build does not ship reads as Orchbee Dark.",
    )
    .page(),
    key(
        "appearance.terminal_colors",
        V::Choice(&["theme", "host", "custom"]),
        D::Text("theme"),
        (C::Appearance, G::TerminalColors),
        CLIENT,
        "Colors of terminal output: the theme, the host terminal, or the two custom colors.",
    )
    .page(),
    key(
        "appearance.custom_foreground",
        V::Color,
        D::Text("#141414"),
        (C::Appearance, G::TerminalColors),
        CLIENT,
        "Text color when terminal_colors is custom.",
    )
    .page(),
    key(
        "appearance.custom_background",
        V::Color,
        D::Text("#fcfcfc"),
        (C::Appearance, G::TerminalColors),
        CLIENT,
        "Background color when terminal_colors is custom.",
    )
    .page(),
    key(
        "appearance.window_title",
        V::Text {
            maximum_bytes: 4096,
            shape: "a template; tokens {hostname}, {workspace}, {tab}, {panel}, {terminal_title}; doubled braces are literal",
        },
        D::Text("{hostname}: {workspace}"),
        (C::General, G::Basics),
        CLIENT,
        "Template for the terminal window title.",
    ),
    key(
        "appearance.sidebar.workspaces.rows",
        V::Structured(SIDEBAR_ROWS),
        D::Toml("[[\"workspace\", \"git_status\"], [\"location\"]]"),
        (C::Appearance, G::Sidebar),
        CLIENT,
        "Rows shown for each Workspace in the sidebar; tokens: state_icon, state_text, workspace, branch, location, git_status, or custom token keys.",
    ),
    key(
        "appearance.sidebar.workspaces.row_gap",
        integer(0, u16::MAX as i64),
        D::Integer(0),
        (C::Appearance, G::Sidebar),
        CLIENT,
        "Blank space between Workspace rows.",
    ),
    // The File Bar's keys, here and in the `explorer` and `git` tables, which keep VS Code's names. They
    // stay out of the reference while releases withhold the File Bar.
    key(
        "appearance.file_bar.position",
        V::Choice(&["right", "left"]),
        D::Text("right"),
        (C::FileBar, G::Placement),
        CLIENT,
        "Docking side of the File Bar: right or left of the terminal area.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "terminal.new_cwd",
        V::Text {
            maximum_bytes: crate::USER_PATH_LIMIT,
            shape: "follow, home, current, workspace, or a folder: absolute, under ~, or relative to this file",
        },
        unset("no override", "\"home\""),
        (C::General, G::Basics),
        SERVER,
        "Starting folder for a terminal an orchbee command creates without --cwd.",
    )
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.copy_on_select",
        V::Boolean,
        D::Boolean(false),
        (C::Terminal, G::Input),
        CLIENT,
        "Copy selected terminal text without a separate copy step.",
    )
    .page(),
    key(
        "terminal.macos_option_as_alt",
        V::Choice(&["off", "left", "right", "both"]),
        D::Text("off"),
        (C::Terminal, G::Input),
        CLIENT,
        "Which Option keys act as Alt in the desktop app on macOS.",
    )
    .page()
    .clients(DESKTOP),
    key(
        "terminal.scrollback_bytes",
        integer(0, 1_000_000_000),
        D::Integer(10_000_000),
        (C::Terminal, G::Output),
        SERVER,
        "Terminal history kept per terminal, in bytes.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.restore_after_restart",
        V::Boolean,
        D::Boolean(true),
        (C::Terminal, G::Restore),
        SERVER,
        "After a restart, resume supported Agent conversations in their panels and show every other terminal's recent output again.",
    )
    .page()
    .effect(SettingEffect::Restart),
    key(
        "terminal.font_family",
        V::Text {
            maximum_bytes: 256,
            shape: "a CSS font-family list",
        },
        unset("the built-in terminal font stack", "\"Cascadia Mono\""),
        (C::Appearance, G::TerminalText),
        CLIENT,
        "Terminal font in the desktop app.",
    )
    .page()
    .clients(DESKTOP),
    key(
        "terminal.font_size",
        integer(8, 48),
        D::Integer(13),
        (C::Appearance, G::TerminalText),
        CLIENT,
        "Terminal font size in the desktop app, in pixels.",
    )
    .page()
    .clients(DESKTOP),
    key(
        "terminal.font_weight",
        V::Integer {
            minimum: 100,
            maximum: 900,
            step: 100,
        },
        D::Integer(400),
        (C::Appearance, G::TerminalText),
        CLIENT,
        "Weight of normal terminal text in the desktop app.",
    )
    .clients(DESKTOP),
    key(
        "terminal.font_weight_bold",
        V::Integer {
            minimum: 100,
            maximum: 900,
            step: 100,
        },
        D::Integer(700),
        (C::Appearance, G::TerminalText),
        CLIENT,
        "Weight of bold terminal text in the desktop app.",
    )
    .clients(DESKTOP),
    key(
        "terminal.font_ligatures",
        V::Boolean,
        D::Boolean(true),
        (C::Appearance, G::TerminalText),
        CLIENT,
        "Join character pairs such as => when the font supports it, in the desktop app.",
    )
    .clients(DESKTOP),
    key(
        "terminal.padding",
        integer(0, 32),
        D::Integer(0),
        (C::Appearance, G::TerminalText),
        CLIENT,
        "Space around terminal text in the desktop app, in pixels.",
    )
    .clients(DESKTOP),
    key(
        "terminal.background_opacity",
        integer(10, 100),
        D::Integer(100),
        (C::Appearance, G::TerminalColors),
        CLIENT,
        "Terminal background opacity in the desktop app, in percent.",
    )
    .clients(DESKTOP),
    key(
        "terminal.default_profile.<platform>",
        V::Text {
            maximum_bytes: 128,
            shape: "the id of the built-in system-default profile or of a profile in this platform's table",
        },
        D::Text("system-default"),
        (C::Terminal, G::Profiles),
        SERVER,
        "Launch profile a new terminal uses on this platform.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.profiles.<platform>",
        V::Structured("up to 63 profile tables; a profile with id system-default replaces the built-in one"),
        unset("only the built-in system-default profile", ""),
        (C::Terminal, G::Profiles),
        SERVER,
        "Launch profiles for this platform; each platform reads only its own table.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.profiles.<platform>[].id",
        V::Text {
            maximum_bytes: 128,
            shape: "letters, digits, . _ : - beginning with a letter or digit; unique in the table",
        },
        unset("required", "\"pwsh\""),
        (C::Terminal, G::Profiles),
        SERVER,
        "Profile id that default_profile names.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.profiles.<platform>[].name",
        V::Text {
            maximum_bytes: 512,
            shape: "non-empty text without surrounding spaces",
        },
        unset("the id", "\"PowerShell 7\""),
        (C::Terminal, G::Profiles),
        SERVER,
        "Name shown for the profile.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.profiles.<platform>[].kind",
        V::Choice(&["system-shell", "custom-executable"]),
        unset("required", "\"custom-executable\""),
        (C::Terminal, G::Profiles),
        SERVER,
        "What the profile starts: the system shell or a program.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.profiles.<platform>[].login",
        V::Boolean,
        D::Boolean(false),
        (C::Terminal, G::Profiles),
        SERVER,
        "Start the system shell as a login shell; system-shell profiles only.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.profiles.<platform>[].executable",
        V::Text {
            maximum_bytes: 32 * 1024,
            shape: "a program name or path",
        },
        unset("required for custom-executable", "\"pwsh.exe\""),
        (C::Terminal, G::Profiles),
        SERVER,
        "Program a custom-executable profile starts.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "terminal.profiles.<platform>[].arguments",
        V::Structured("an array of up to 128 strings, each passed as one literal argument"),
        D::Toml("[]"),
        (C::Terminal, G::Profiles),
        SERVER,
        "Arguments for a custom-executable profile.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "worktrees.directory",
        V::Path,
        unset("worktrees go beside their repository", "\"~/worktrees\""),
        (C::Terminal, G::Panels),
        SERVER,
        "Folder that holds created worktrees.",
    ),
    key(
        "notifications.agent_reminder",
        V::Choice(&["never", "when-hidden"]),
        D::Text("never"),
        (C::Notifications, G::When),
        CLIENT,
        "Notify when an Agent needs attention while its panel is hidden.",
    )
    .page(),
    key(
        "notifications.completion_reminder",
        V::Choice(&["never", "when-unfocused", "always"]),
        D::Text("never"),
        (C::Notifications, G::When),
        CLIENT,
        "Notify when a long command finishes.",
    )
    .page(),
    key(
        "notifications.completion_minimum_seconds",
        integer(1, 3600),
        D::Integer(5),
        (C::Notifications, G::When),
        CLIENT,
        "Shortest command, in seconds, whose completion is notified.",
    ),
    key(
        "notifications.sound",
        V::Boolean,
        D::Boolean(false),
        (C::Notifications, G::How),
        CLIENT,
        "Play a sound with notifications.",
    )
    .page(),
    key(
        "notifications.cli_delivery",
        V::Choice(&["off", "orchbee", "terminal", "system"]),
        D::Text("off"),
        (C::Notifications, G::How),
        CLIENT,
        "Where notifications sent with orchbee notify appear.",
    ),
    key(
        "notifications.delay_seconds",
        integer(0, MAXIMUM_NOTIFICATION_DELAY_SECONDS as i64),
        D::Integer(1),
        (C::Notifications, G::How),
        CLIENT,
        "Quiet period, in seconds, before an automatic Agent notification.",
    ),
    key(
        "notifications.position",
        V::Choice(&["top-left", "top-right", "bottom-left", "bottom-right"]),
        D::Text("bottom-right"),
        (C::Notifications, G::How),
        CLIENT,
        "Corner where in-terminal notifications appear.",
    ),
    key(
        "notifications.sound_by_agent.*",
        V::Choice(&["default", "on", "off"]),
        unset("no overrides", "codex = \"off\""),
        (C::Notifications, G::How),
        CLIENT,
        "Sound for one Agent; default follows notifications.sound.",
    ),
    key(
        "keys.prefix",
        V::Text {
            maximum_bytes: 64,
            shape: "one key chord such as ctrl+b",
        },
        D::Text("ctrl+b"),
        (C::Keyboard, G::Prefix),
        CLIENT,
        "Prefix chord before a Client action key.",
    )
    .page()
    .clients(TUI),
    key(
        "keys.<action>",
        V::Structured("one chord, an array of up to 16 chords, or [] for no key"),
        unset("the action's built-in keys", "[\"prefix+%\"]"),
        (C::Keyboard, G::Prefix),
        CLIENT,
        "Keys for one registered Client action.",
    )
    .page()
    .clients(TUI),
    key(
        "keys.command",
        V::Structured("up to 64 command tables"),
        unset("no commands", ""),
        (C::Keyboard, G::Commands),
        CLIENT,
        "User-defined commands; a command runs only after its key is pressed.",
    ),
    key(
        "keys.command[].command",
        V::Text {
            maximum_bytes: 4096,
            shape: "non-empty text",
        },
        unset("required", "\"pwsh.exe -NoProfile\""),
        (C::Keyboard, G::Commands),
        CLIENT,
        "Command text, run only after its key is pressed.",
    ),
    key(
        "keys.command[].type",
        V::Choice(&["shell", "panel", "popup", "plugin-action"]),
        D::Text("shell"),
        (C::Keyboard, G::Commands),
        CLIENT,
        "How the command runs; plugin-action takes up to 241 letters, digits or . _ : -.",
    ),
    key(
        "keys.command[].key",
        V::Structured("one chord or an array of up to 16 chords"),
        unset("no key", "\"prefix+f12\""),
        (C::Keyboard, G::Commands),
        CLIENT,
        "Keys that run the command.",
    ),
    key(
        "keys.command[].description",
        V::Text {
            maximum_bytes: 512,
            shape: "text",
        },
        unset("the command text", "\"Open a PowerShell popup\""),
        (C::Keyboard, G::Commands),
        CLIENT,
        "Label shown for the command.",
    ),
    key(
        "keys.command[].width",
        V::Structured("cells from 1 through 4096, or a percentage string from 1% through 100%"),
        unset("the default popup width", "\"60%\""),
        (C::Keyboard, G::Commands),
        CLIENT,
        "Popup width; popup commands only.",
    ),
    key(
        "keys.command[].height",
        V::Structured("cells from 1 through 4096, or a percentage string from 1% through 100%"),
        unset("the default popup height", "20"),
        (C::Keyboard, G::Commands),
        CLIENT,
        "Popup height; popup commands only.",
    ),
    key(
        AGENT_ARGUMENTS_PATH,
        V::Text {
            maximum_bytes: MAXIMUM_AGENT_ARGUMENT_INPUT_BYTES,
            shape: "literal arguments separated by spaces, with single or double quotes for grouping",
        },
        D::Text(""),
        (C::Agents, G::AgentLaunch),
        SERVER,
        "Extra arguments for a new Agent start. Use its provider ID with underscores in the table name.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        AGENT_YOLO_PATH,
        V::Boolean,
        D::Boolean(false),
        (C::Agents, G::AgentLaunch),
        SERVER,
        "Start a supported Agent with its native YOLO mode. An explicit permission option in arguments takes precedence.",
    )
    .page()
    .effect(SettingEffect::NewTerminals),
    key(
        "agents.auto_allow.claude",
        V::Boolean,
        D::Boolean(false),
        (C::Agents, G::Confirmations),
        SERVER,
        "Let Orchbee press Claude Code's one-time Yes on routine confirmations; commands are not checked, so one that deletes or uploads files runs too.",
    )
    .page(),
    key(
        "agents.auto_allow.codex",
        V::Boolean,
        D::Boolean(false),
        (C::Agents, G::Confirmations),
        SERVER,
        "Let Orchbee press Codex's one-time Yes on routine confirmations; commands are not checked, so one that deletes or uploads files runs too.",
    )
    .page(),
    key(
        "agents.auto_allow.gemini",
        V::Boolean,
        D::Boolean(false),
        (C::Agents, G::Confirmations),
        SERVER,
        "Let Orchbee press Gemini CLI's one-time Yes on routine confirmations; commands are not checked, so one that deletes or uploads files runs too.",
    )
    .page(),
    key(
        "agents.auto_allow.opencode",
        V::Boolean,
        D::Boolean(false),
        (C::Agents, G::Confirmations),
        SERVER,
        "Let Orchbee press OpenCode's one-time Allow on routine confirmations; commands are not checked, so one that deletes or uploads files runs too.",
    )
    .page(),
    key(
        "agents.auto_allow.offer",
        V::Boolean,
        D::Boolean(true),
        (C::Agents, G::Confirmations),
        SERVER,
        "Offer to turn on automatic confirmation when a supported Agent waits on a routine confirmation.",
    ),
    key(
        "agents.quota_resume.enabled",
        V::Boolean,
        D::Boolean(false),
        (C::Agents, G::QuotaResume),
        SERVER,
        "Submit the resume prompt to a live Codex session after its reported usage limit resets.",
    )
    .page(),
    key(
        QUOTA_RESUME_PROMPT_PATH,
        V::Text {
            maximum_bytes: MAXIMUM_QUOTA_RESUME_PROMPT_BYTES,
            shape: "One nonblank line of prompt text without control characters or a leading / or !.",
        },
        D::Text(DEFAULT_QUOTA_RESUME_PROMPT),
        (C::Agents, G::QuotaResume),
        SERVER,
        "Prompt to submit when Orchbee resumes an Agent after a quota reset.",
    )
    .page(),
    key(
        "explorer.confirm_delete",
        V::Boolean,
        D::Boolean(true),
        (C::FileBar, G::Explorer),
        CLIENT,
        "Explorer asks before it deletes files and folders.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "explorer.confirm_drag_and_drop",
        V::Boolean,
        D::Boolean(true),
        (C::FileBar, G::Explorer),
        CLIENT,
        "Explorer asks before it moves files and folders by drag and drop.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "explorer.confirm_paste_native",
        V::Boolean,
        D::Boolean(true),
        (C::FileBar, G::Explorer),
        CLIENT,
        "Explorer asks before it pastes files copied outside Orchbee.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.open_repository_in_parent_folders",
        V::Choice(&["always", "never", "prompt"]),
        D::Text("prompt"),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git repositories in a parent folder of a Workspace's directory: always opened, never, or after asking.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.enable_smart_commit",
        V::Boolean,
        D::Boolean(false),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git commits every change when nothing is staged.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.suggest_smart_commit",
        V::Boolean,
        D::Boolean(true),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git suggests smart commit when a commit finds nothing staged.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.confirm_sync",
        V::Boolean,
        D::Boolean(true),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git asks before it syncs a branch with its remote.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.confirm_force_push",
        V::Boolean,
        D::Boolean(true),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git asks before it force pushes.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.autofetch",
        V::Boolean,
        D::Boolean(false),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git fetches each repository's remotes periodically.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.ignore_limit_warning",
        V::Boolean,
        D::Boolean(false),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git stays quiet about a repository with too many changes.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "git.ignore_missing_git_warning",
        V::Boolean,
        D::Boolean(false),
        (C::FileBar, G::SourceControl),
        CLIENT,
        "Git missing from the machine raises no warning.",
    )
    .clients(DESKTOP)
    .unreferenced(),
    key(
        "update.channel",
        V::Choice(&["stable", "preview"]),
        D::Text("stable"),
        (C::About, G::Updates),
        SERVER,
        "Release channel for explicit update operations.",
    )
    .unreferenced(),
    key(
        "update.automatic",
        V::Boolean,
        D::Boolean(true),
        (C::About, G::Updates),
        SERVER,
        "Download and install new versions in the background; development builds ignore it.",
    )
    .page(),
];

fn segment_matches(pattern: &str, segment: &str) -> bool {
    let (pattern, segment) = match (pattern.strip_suffix("[]"), segment.strip_suffix("[]")) {
        (Some(pattern), Some(segment)) => (pattern, segment),
        (None, None) => (pattern, segment),
        _ => return false,
    };
    match pattern {
        "*" => true,
        "<action>" => ACTIONS.contains(&segment),
        "<platform>" => SETTING_PLATFORMS.contains(&segment),
        "<agent>" => settings_agent(segment).is_some(),
        "<yolo-agent>" => settings_agent(segment).is_some_and(|agent| agent.yolo_flag().is_some()),
        _ => pattern == segment,
    }
}

fn settings_agent(segment: &str) -> Option<AgentStartSelector> {
    AgentStartSelector::parse(&segment.replace('_', "-"))
        .filter(|agent| agent.settings_id() == segment)
}

fn path_matches<'a>(pattern: &str, mut segments: impl Iterator<Item = &'a str>) -> bool {
    let mut pattern = pattern.split('.');
    loop {
        match (pattern.next(), segments.next()) {
            (None, None) => return true,
            (Some(left), Some(right)) if segment_matches(left, right) => {}
            _ => return false,
        }
    }
}

/// The key that `segments` names exactly. Array elements are named with a `[]` suffix.
pub fn find_setting(segments: &[&str]) -> Option<&'static SettingKey> {
    SETTING_KEYS
        .iter()
        .find(|setting| path_matches(setting.path, segments.iter().copied()))
}

/// Whether `key` is a registered key under `table_path`, or a table that holds registered keys.
/// The root table is the empty path.
pub(super) fn user_config_has_child(table_path: &str, key: &str) -> bool {
    let candidate: Vec<&str> = table_path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .chain([key])
        .collect();
    SETTING_KEYS.iter().any(|setting| {
        let pattern: Vec<&str> = setting.path.split('.').collect();
        pattern.len() >= candidate.len()
            && pattern
                .iter()
                .zip(&candidate)
                .all(|(pattern, segment)| segment_matches(pattern, segment))
    })
}

/// Whether a dotted path, with `[]` marking array elements, is a registered key or lies under one.
pub(crate) fn recognized_path(path: &str) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    SETTING_KEYS.iter().any(|setting| {
        let pattern: Vec<&str> = setting.path.split('.').collect();
        pattern.len() <= segments.len()
            && pattern.iter().zip(&segments).all(|(pattern, segment)| {
                segment_matches(pattern, segment)
                    || (!pattern.ends_with("[]")
                        && segment
                            .strip_suffix("[]")
                            .is_some_and(|segment| segment_matches(pattern, segment)))
            })
    })
}

/// One scalar setting value: what a key other than a structured one holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scalar {
    Boolean(bool),
    Integer(i64),
    Text(String),
}

impl Scalar {
    /// The value as the file writes it.
    pub fn to_toml(&self) -> toml::Value {
        match self {
            Self::Boolean(value) => toml::Value::Boolean(*value),
            Self::Integer(value) => toml::Value::Integer(*value),
            Self::Text(value) => toml::Value::String(value.clone()),
        }
    }
}

impl SettingKey {
    /// Admit `value` by this key's declared type, or return the diagnostic reason: `wrong-type`,
    /// `invalid-value`, `invalid-mode` for a string outside the choices, or `value-too-long`.
    pub fn admit(&self, value: &toml::Value) -> Result<Scalar, &'static str> {
        match self.value {
            V::Boolean => value.as_bool().map(Scalar::Boolean).ok_or("wrong-type"),
            V::Integer {
                minimum,
                maximum,
                step,
            } => {
                let value = value.as_integer().ok_or("wrong-type")?;
                if (minimum..=maximum).contains(&value) && value % step == 0 {
                    Ok(Scalar::Integer(value))
                } else {
                    Err("invalid-value")
                }
            }
            V::Choice(choices) => {
                let value = value.as_str().ok_or("wrong-type")?;
                if choices.contains(&value) {
                    Ok(Scalar::Text(value.to_owned()))
                } else {
                    Err("invalid-mode")
                }
            }
            V::Text { maximum_bytes, .. } => {
                let value = value.as_str().ok_or("wrong-type")?;
                if self.path == AGENT_ARGUMENTS_PATH {
                    parse_agent_arguments(value)?;
                }
                if value.len() > maximum_bytes || value.contains('\0') {
                    return Err("value-too-long");
                }
                if self.path == QUOTA_RESUME_PROMPT_PATH && !valid_quota_resume_prompt(value) {
                    return Err("invalid-value");
                }
                Ok(Scalar::Text(value.to_owned()))
            }
            V::Color => {
                let value = value.as_str().ok_or("wrong-type")?;
                let digits = value.strip_prefix('#').unwrap_or_default();
                if digits.len() == 6
                    && digits
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                {
                    Ok(Scalar::Text(value.to_owned()))
                } else {
                    Err("invalid-value")
                }
            }
            V::Theme => {
                let value = value.as_str().ok_or("wrong-type")?;
                if value.len() <= THEME_ID_BYTES
                    && value.split('-').all(|part| {
                        !part.is_empty()
                            && part
                                .bytes()
                                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                    })
                {
                    Ok(Scalar::Text(value.to_owned()))
                } else {
                    Err("invalid-value")
                }
            }
            V::Path => value
                .as_str()
                .map(|value| Scalar::Text(value.to_owned()))
                .ok_or("wrong-type"),
            V::Structured(_) => Err("wrong-type"),
        }
    }
}

#[cfg(test)]
mod tests;
