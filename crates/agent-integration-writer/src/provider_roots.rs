//! Provider-owned plugin and extension roots that follow each provider's own override variable on
//! every platform: OpenCode's `XDG_CONFIG_HOME`, Kilo's `KILO_CONFIG_DIR` and then
//! `XDG_CONFIG_HOME`, Gemini's `GEMINI_CLI_HOME` and Qwen's `QWEN_HOME`. A variable applies only
//! when it is nonempty, and a relative value admits nothing rather than falling back. Callers
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

/// OpenCode's global plugin folder: `$XDG_CONFIG_HOME/opencode/plugins`, otherwise
/// `~/.config/opencode/plugins`.
pub fn opencode_plugins(home: &Path, variables: Variables) -> Option<PathBuf> {
    match set(variables, "XDG_CONFIG_HOME") {
        Some(root) => root.is_absolute().then(|| root.join("opencode/plugins")),
        None => Some(home.join(".config/opencode/plugins")),
    }
}

/// Kilo's explicit configuration directory supersedes its XDG default.
pub fn kilo_entry(home: &Path, variables: Variables) -> Option<PathBuf> {
    let root = set(variables, "KILO_CONFIG_DIR")
        .or_else(|| set(variables, "XDG_CONFIG_HOME").map(|root| root.join("kilo")))
        .unwrap_or_else(|| home.join(".config/kilo"));
    root.is_absolute()
        .then(|| root.join("plugin/orchbee-agent-observation.js"))
}

/// The two extension providers use different override semantics: Gemini's override replaces the
/// home parent; Qwen's replaces the configuration root.
pub fn extension_directory(id: &str, home: &Path, variables: Variables) -> Option<PathBuf> {
    let (variable, fallback, suffix) = match id {
        "gemini" => (
            "GEMINI_CLI_HOME",
            home.to_path_buf(),
            ".gemini/extensions/orchbee-agent-observation",
        ),
        "qwen-code" => (
            "QWEN_HOME",
            home.join(".qwen"),
            "extensions/orchbee-agent-observation",
        ),
        _ => return None,
    };
    let root = set(variables, variable).unwrap_or(fallback);
    root.is_absolute().then(|| root.join(suffix))
}
