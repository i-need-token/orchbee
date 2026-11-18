//! Provider-owned Windows configuration locations shared by onboarding and
//! offline diagnostics. Callers retain their own filesystem admission guards.
use std::path::{Path, PathBuf};

use crate::configuration_kinds;
use crate::hook_configuration::{HookConfigurationKind, HookConfigurationTarget};

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
    crate::provider_roots::directory(crate::provider_roots::process, variable, home, directory)
        .map(|root| root.join(file))
}

pub struct OpenCodeEntries {
    pub current: PathBuf,
    pub(crate) obsolete: PathBuf,
    /// The OpenCode 2.x plugin directory whose TUI entry reports the Agent's
    /// session from the process that owns its console.
    pub(crate) client: PathBuf,
}

pub fn opencode_entries(
    home: &Path,
    variables: crate::provider_roots::Variables,
) -> Option<OpenCodeEntries> {
    let root = crate::provider_roots::opencode_plugins(home, variables)?;
    Some(OpenCodeEntries {
        current: root.join("orchbee-agent-observation.js"),
        obsolete: root.join("orchbee-agent-observation.mjs"),
        client: root.join("orchbee-agent-observation-tui"),
    })
}

/// Pin the same literal profile root for filesystem admission and provider commands.
pub(crate) fn hermes_home(home: &Path) -> Option<PathBuf> {
    hermes_home_with(home, crate::provider_roots::process)
}

/// Variant of `hermes_home` parameterized over environment lookup.
pub(crate) fn hermes_home_with(
    home: &Path,
    variables: crate::provider_roots::Variables,
) -> Option<PathBuf> {
    let override_root = variables("HERMES_HOME")
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
        let local = variables("LOCALAPPDATA")
            .map(|v| v.into_string())
            .transpose()
            .ok()?;
        let base = local
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"));
        let suffix = variables("HERMES_DATA_DIR_SUFFIX")
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
