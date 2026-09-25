//! Provider-owned Windows configuration locations shared by onboarding and
//! offline diagnostics. Callers retain their own filesystem admission guards.
use std::path::{Path, PathBuf};

pub(crate) struct OpenCodeEntries {
    pub(crate) current: PathBuf,
    pub(crate) obsolete: PathBuf,
}

pub(crate) fn opencode_entries(home: &Path) -> Option<OpenCodeEntries> {
    let configured = std::env::var_os("XDG_CONFIG_HOME");
    let root = match configured.filter(|value| !value.is_empty()) {
        Some(value) => {
            let root = PathBuf::from(value);
            if !root.is_absolute() {
                return None;
            }
            root.join("opencode/plugins")
        }
        None => home.join(".config/opencode/plugins"),
    };
    Some(OpenCodeEntries {
        current: root.join("orchbee-agent-observation.js"),
        obsolete: root.join("orchbee-agent-observation.mjs"),
    })
}

/// Kilo's explicit configuration directory supersedes its XDG default.
pub(crate) fn kilo_entry(home: &Path) -> Option<PathBuf> {
    let root = if let Some(value) = std::env::var_os("KILO_CONFIG_DIR").filter(|v| !v.is_empty()) {
        PathBuf::from(value)
    } else if let Some(value) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        PathBuf::from(value).join("kilo")
    } else {
        home.join(".config/kilo")
    };
    root.is_absolute()
        .then(|| root.join("plugin/orchbee-agent-observation.js"))
}

/// The two extension providers use different override semantics: Gemini's
/// override replaces the home parent; Qwen's replaces the configuration root.
pub(crate) fn extension_directory(id: &str, home: &Path) -> Option<PathBuf> {
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
    let root = std::env::var_os(variable)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or(fallback);
    root.is_absolute().then(|| root.join(suffix))
}
