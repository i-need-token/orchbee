//! Provider-owned configuration, plugin and extension roots that follow each provider's own override
//! variable on every platform: OpenCode's `XDG_CONFIG_HOME`, Kilo's `KILO_CONFIG_DIR` and then
//! `XDG_CONFIG_HOME`, Gemini's `GEMINI_CLI_HOME`, Qwen's `QWEN_HOME`, and the single-variable
//! directories of the hook-configuration providers. A variable applies only when it is nonempty, and
//! a relative value admits nothing rather than falling back. Hook paths, extension paths and global
//! skill roots all derive from these functions, so each formula has exactly one source. Callers
//! retain their own filesystem admission guards.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Reads one environment variable: the process environment in production, an isolated one in
/// tests, so a variable the test runner's environment sets never moves a test's target.
pub type Variables = fn(&str) -> Option<OsString>;

/// The process environment.
pub(crate) fn process(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

fn set(variables: Variables, name: &str) -> Option<PathBuf> {
    variables(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// `$name` when it is set, otherwise `home/default`, admitted only when absolute.
pub(crate) fn directory(
    variables: Variables,
    name: &str,
    home: &Path,
    default: &str,
) -> Option<PathBuf> {
    let root = set(variables, name).unwrap_or_else(|| home.join(default));
    root.is_absolute().then_some(root)
}

/// OpenCode's configuration directory: `$XDG_CONFIG_HOME/opencode`, otherwise `~/.config/opencode`.
pub(crate) fn opencode_config(home: &Path, variables: Variables) -> Option<PathBuf> {
    match set(variables, "XDG_CONFIG_HOME") {
        Some(root) => root.is_absolute().then(|| root.join("opencode")),
        None => Some(home.join(".config/opencode")),
    }
}

/// OpenCode's global plugin folder: `<opencode configuration>/plugins`.
pub fn opencode_plugins(home: &Path, variables: Variables) -> Option<PathBuf> {
    opencode_config(home, variables).map(|root| root.join("plugins"))
}

/// Kilo's configuration directory: `$KILO_CONFIG_DIR`, otherwise `$XDG_CONFIG_HOME/kilo`, otherwise
/// `~/.config/kilo`.
pub(crate) fn kilo_config(home: &Path, variables: Variables) -> Option<PathBuf> {
    let root = set(variables, "KILO_CONFIG_DIR")
        .or_else(|| set(variables, "XDG_CONFIG_HOME").map(|root| root.join("kilo")))
        .unwrap_or_else(|| home.join(".config/kilo"));
    root.is_absolute().then_some(root)
}

/// Kilo's explicit configuration directory supersedes its XDG default.
pub fn kilo_entry(home: &Path, variables: Variables) -> Option<PathBuf> {
    kilo_config(home, variables).map(|root| root.join("plugin/orchbee-agent-observation.js"))
}

/// Gemini's home: `$GEMINI_CLI_HOME` replaces the user's home directory.
pub(crate) fn gemini_home(home: &Path, variables: Variables) -> Option<PathBuf> {
    let root = set(variables, "GEMINI_CLI_HOME").unwrap_or_else(|| home.to_path_buf());
    root.is_absolute().then_some(root)
}

/// Qwen's home: `$QWEN_HOME` replaces `~/.qwen`.
pub(crate) fn qwen_home(home: &Path, variables: Variables) -> Option<PathBuf> {
    let root = set(variables, "QWEN_HOME").unwrap_or_else(|| home.join(".qwen"));
    root.is_absolute().then_some(root)
}

/// The two extension providers use different override semantics: Gemini's override replaces the
/// home parent; Qwen's replaces the configuration root.
pub fn extension_directory(id: &str, home: &Path, variables: Variables) -> Option<PathBuf> {
    match id {
        "gemini" => gemini_home(home, variables)
            .map(|root| root.join(".gemini/extensions/orchbee-agent-observation")),
        "qwen-code" => {
            qwen_home(home, variables).map(|root| root.join("extensions/orchbee-agent-observation"))
        }
        _ => None,
    }
}
