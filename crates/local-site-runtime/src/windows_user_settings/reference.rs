//! Registry-backed offline reference rendering for Windows user configuration.
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
        description: "Chrome palette used by the native terminal.",
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
pub(super) fn windows_user_config_has_child(table_path: &str, key: &str) -> bool {
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
