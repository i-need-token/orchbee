//! Provider-owned Windows configuration locations shared by onboarding and
//! offline diagnostics. Callers retain their own filesystem admission guards.
use std::path::{Path, PathBuf};

pub(crate) use crate::agent_integrations::HookConfigurationKind;
use crate::agent_integrations::configuration_kinds;

/// One candidate configuration. Callers own reads, budgets and failure handling.
#[derive(Clone)]
pub struct HookConfigurationTarget {
    pub path: PathBuf,
    pub kind: HookConfigurationKind,
}
/// Priority is authoritative; errors at an earlier candidate never admit fallback.
/// When no candidate exists, use the first one for a new declaration.
pub(crate) fn hook_configuration_candidates(
    id: &str,
    home: &Path,
) -> Option<Vec<HookConfigurationTarget>> {
    hook_configuration_candidates_for(id, home, configuration_kinds(id)?)
}

/// Resolves the shared ordered syntax plan with the existing Windows location rules.
pub fn hook_configuration_candidates_for(
    id: &str,
    home: &Path,
    kinds: &[HookConfigurationKind],
) -> Option<Vec<HookConfigurationTarget>> {
    if id == "droid" {
        if !home.is_absolute() {
            return None;
        }
        let [first, second, third] = kinds else {
            return None;
        };
        return Some(vec![
            HookConfigurationTarget {
                path: home.join(".factory/hooks.json"),
                kind: *first,
            },
            HookConfigurationTarget {
                path: home.join(".factory/hooks/hooks.json"),
                kind: *second,
            },
            HookConfigurationTarget {
                path: home.join(".factory/settings.json"),
                kind: *third,
            },
        ]);
    }
    let [kind] = kinds else {
        return None;
    };
    Some(vec![HookConfigurationTarget {
        path: single_hook_configuration(id, home)?,
        kind: *kind,
    }])
}

fn single_hook_configuration(id: &str, home: &Path) -> Option<PathBuf> {
    if id == "devin" {
        let root = PathBuf::from(std::env::var_os("APPDATA").filter(|v| !v.is_empty())?);
        return root.is_absolute().then(|| root.join("devin/config.json"));
    }
    if id == "letta" {
        let root = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.to_path_buf());
        return root
            .is_absolute()
            .then(|| root.join(".letta/settings.json"));
    }
    if id == "mastracode" {
        return home
            .is_absolute()
            .then(|| home.join(".mastracode/hooks.json"));
    }
    if id == "antigravity-cli" {
        return home
            .is_absolute()
            .then(|| home.join(".gemini/config/hooks.json"));
    }
    let (variable, directory, file) = match id {
        "kimi" => ("KIMI_CODE_HOME", ".kimi-code", "config.toml"),
        "cursor" => ("CURSOR_CONFIG_DIR", ".cursor", "hooks.json"),
        "qodercli" => ("QODER_CONFIG_DIR", ".qoder", "settings.json"),
        _ => return None,
    };
    let root = std::env::var_os(variable)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(directory));
    root.is_absolute().then(|| root.join(file))
}

pub struct OpenCodeEntries {
    pub current: PathBuf,
    pub(crate) obsolete: PathBuf,
}

pub fn opencode_entries(home: &Path) -> Option<OpenCodeEntries> {
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
pub fn kilo_entry(home: &Path) -> Option<PathBuf> {
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
pub fn extension_directory(id: &str, home: &Path) -> Option<PathBuf> {
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

/// Pin the same literal profile root for filesystem admission and provider commands.
pub(crate) fn hermes_home(home: &Path) -> Option<PathBuf> {
    let override_root = std::env::var_os("HERMES_HOME")
        .map(|v| v.into_string())
        .transpose()
        .ok()?;
    let explicit = override_root
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let root = if let Some(value) = explicit {
        PathBuf::from(value)
    } else {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(|v| v.into_string())
            .transpose()
            .ok()?;
        let base = local
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"));
        let suffix = std::env::var_os("HERMES_DATA_DIR_SUFFIX")
            .unwrap_or_default()
            .into_string()
            .ok()?;
        if !suffix
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'.' | b'_' | b'-'))
        {
            return None;
        }
        base.join(format!("hermes{suffix}"))
    };
    let text = root.to_str()?;
    let bytes = text.as_bytes();
    let expands = bytes
        .windows(2)
        .any(|s| s[0] == b'$' && (s[1].is_ascii_alphanumeric() || matches!(s[1], b'_' | b'{')))
        || text.matches('%').count() >= 2
        || text.starts_with('~');
    (root.is_absolute() && !expands).then_some(root)
}
