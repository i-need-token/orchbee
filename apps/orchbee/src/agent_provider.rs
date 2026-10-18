//! Recognized interactive Agent identities and their Windows command names.

pub(crate) const CANONICAL_IDS: &[&str] = &[
    "pi",
    "claude-code",
    "codex",
    "gemini",
    "cursor",
    "devin",
    "antigravity",
    "cline",
    "omp",
    "mastracode",
    "opencode",
    "github-copilot",
    "kimi",
    "kiro",
    "droid",
    "amp",
    "grok",
    "hermes",
    "kilo",
    "qodercli",
    "qwen-code",
    "letta",
    "maki",
    "muse",
];

#[cfg(windows)]
pub(crate) use orchbee_agent_observation_protocol::native_agent_basename as basename;
pub(crate) use orchbee_agent_observation_protocol::{
    native_agent_provider as provider, normalize_agent_report_label as reported_label,
};

/// Only canonical identities select a command; aliases remain read-only recognition.
pub(crate) fn executable(canonical: &str) -> Option<&'static str> {
    let id = provider(canonical).filter(|id| *id == canonical)?;
    Some(match id {
        "claude-code" => "claude",
        "cursor" => "agent",
        "qodercli" => "qoder",
        "antigravity" => "agy",
        "github-copilot" => "copilot",
        "kiro" => "kiro-cli",
        "qwen-code" => "qwen",
        _ => id,
    })
}

/// Canonical startup kinds whose mapped command is an actual regular file on
/// the inherited Windows PATH. The search is deliberately finite and admits
/// only local absolute disk paths. Presence is a catalog fact; the startup
/// workflow still proves idle-shell binding and canonical hook readiness.
#[cfg(all(windows, test))]
pub(crate) fn installed_start_providers(path_env: Option<&std::ffi::OsStr>) -> Vec<&'static str> {
    installed_start_providers_while(path_env, || true)
}

/// The owned background catalog uses this variant so Session teardown can
/// stop the finite PATH walk between filesystem probes before joining it.
#[cfg(windows)]
pub(crate) fn installed_start_providers_while(
    path_env: Option<&std::ffi::OsStr>,
    mut keep_going: impl FnMut() -> bool,
) -> Vec<&'static str> {
    use std::path::{Component, Prefix};

    let directories: Vec<_> = path_env
        .into_iter()
        .flat_map(std::env::split_paths)
        .take(256)
        .filter(|path| {
            let Some(text) = path.to_str() else {
                return false;
            };
            text.len() <= 4096
                && !text.chars().any(char::is_control)
                && path.is_absolute()
                && !text
                    .split(['\\', '/'])
                    .any(|part| matches!(part, "." | ".."))
                && matches!(path.components().next(), Some(Component::Prefix(prefix))
                    if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        })
        .collect();
    let mut found = Vec::new();
    for provider in CANONICAL_IDS.iter().copied() {
        if !keep_going() {
            return found;
        }
        let Some(command) = executable(provider) else {
            continue;
        };
        'directories: for directory in &directories {
            for extension in ["exe", "com", "cmd", "bat", "ps1"] {
                if !keep_going() {
                    return found;
                }
                if directory
                    .join(format!("{command}.{extension}"))
                    .metadata()
                    .is_ok_and(|metadata| metadata.is_file())
                {
                    found.push(provider);
                    break 'directories;
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests;
