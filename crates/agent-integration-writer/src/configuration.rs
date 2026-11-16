//! Provider-owned POSIX configuration locations of the hook-configuration providers and Hermes,
//! taken from each provider's macOS documentation or source. Linux compiles the same rules
//! unverified; those integrations stay manual-only there, so nothing reads them. Callers retain
//! their own filesystem admission guards.
use std::path::{Path, PathBuf};

use crate::configuration_kinds;
use crate::hook_configuration::HookConfigurationTarget;

/// Priority is authoritative; errors at an earlier candidate never admit fallback.
/// When no candidate exists, use the first one for a new declaration.
pub(crate) fn hook_configuration_candidates(
    id: &str,
    home: &Path,
) -> Option<Vec<HookConfigurationTarget>> {
    let kinds = configuration_kinds(id)?;
    if !home.is_absolute() {
        return None;
    }
    if id == "droid" {
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
    // Letta reads `$HOME`, which is `home` here; Devin's documented POSIX location ignores
    // XDG_CONFIG_HOME.
    let fixed = match id {
        "letta" => Some(".letta/settings.json"),
        "devin" => Some(".config/devin/config.json"),
        "mastracode" => Some(".mastracode/hooks.json"),
        "antigravity-cli" => Some(".gemini/config/hooks.json"),
        _ => None,
    };
    if let Some(relative) = fixed {
        return Some(home.join(relative));
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

/// Pin the same literal profile root for filesystem admission and provider commands: a trimmed
/// absolute HERMES_HOME, otherwise `~/.hermes` with a safe HERMES_DATA_DIR_SUFFIX. Hermes expands
/// `~` and `$NAME` in HERMES_HOME itself; such a value refuses rather than being guessed.
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
        home.join(format!(".hermes{suffix}"))
    };
    let text = root.to_str()?;
    let expands = text
        .as_bytes()
        .windows(2)
        .any(|s| s[0] == b'$' && (s[1].is_ascii_alphanumeric() || matches!(s[1], b'_' | b'{')))
        || text.starts_with('~');
    (root.is_absolute() && !expands).then_some(root)
}
