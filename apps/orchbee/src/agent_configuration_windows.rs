//! Provider-owned Windows configuration locations shared by onboarding and
//! offline diagnostics. Callers retain their own filesystem admission guards.
use std::path::{Path, PathBuf};

/// The actual provider document shape; both Droid origins share one adapter identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HookConfigurationKind {
    Cursor,
    Qoder,
    Antigravity,
    Mastracode,
    DroidStandalone,
    DroidSettings,
    Kimi,
    Letta,
    Devin,
}
impl HookConfigurationKind {
    /// Canonical observation and integration identity independent of file representation.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Kimi => "kimi",
            Self::Letta => "letta",
            Self::Devin => "devin",
            Self::Cursor => "cursor",
            Self::Qoder => "qodercli",
            Self::Antigravity => "antigravity-cli",
            Self::Mastracode => "mastracode",
            Self::DroidStandalone | Self::DroidSettings => "droid",
        }
    }
}
/// One candidate configuration. Callers own reads, budgets and failure handling.
#[derive(Clone)]
pub(crate) struct HookConfigurationTarget {
    pub(crate) path: PathBuf,
    pub(crate) kind: HookConfigurationKind,
}
/// Priority is authoritative; errors at an earlier candidate never admit fallback.
/// When no candidate exists, use the first one for a new declaration.
pub(crate) fn hook_configuration_candidates(
    id: &str,
    home: &Path,
) -> Option<Vec<HookConfigurationTarget>> {
    if id == "droid" {
        if !home.is_absolute() {
            return None;
        }
        return Some(vec![
            HookConfigurationTarget {
                path: home.join(".factory/hooks.json"),
                kind: HookConfigurationKind::DroidStandalone,
            },
            HookConfigurationTarget {
                path: home.join(".factory/hooks/hooks.json"),
                kind: HookConfigurationKind::DroidStandalone,
            },
            HookConfigurationTarget {
                path: home.join(".factory/settings.json"),
                kind: HookConfigurationKind::DroidSettings,
            },
        ]);
    }
    let kind = match id {
        "kimi" => HookConfigurationKind::Kimi,
        "letta" => HookConfigurationKind::Letta,
        "devin" => HookConfigurationKind::Devin,
        "cursor" => HookConfigurationKind::Cursor,
        "qodercli" => HookConfigurationKind::Qoder,
        "antigravity-cli" => HookConfigurationKind::Antigravity,
        "mastracode" => HookConfigurationKind::Mastracode,
        _ => return None,
    };
    Some(vec![HookConfigurationTarget {
        path: single_hook_configuration(id, home)?,
        kind,
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
