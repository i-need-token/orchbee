//! Global Orchbee discovery `SKILL.md` management for supported Agent harnesses. Each supported
//! provider installs `<skills_root>/orchbee/SKILL.md` stamped with its environment's capsule marker.
//! Ownership follows the recorded root: a user-authored or cross-environment skill is never
//! overwritten or deleted, and it never blocks the integration it accompanies.

use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use base64::Engine as _;

use crate::{
    configuration, files, hook_configuration,
    inspect::{self, ResourceOptions, ResourceOwnership},
    model::{IntegrationId, State},
    provider_roots::{self, Variables},
};

#[cfg(test)]
mod tests;

const MAX_SKILL_BYTES: u64 = 64 * 1024;
const SKILL_DIR_NAME: &str = "orchbee";
const SKILL_FILE_NAME: &str = "SKILL.md";
const MARKER_PREFIX: &str = "<!-- orchbee-agent-integration-skill:v1:";
const MARKER_SUFFIX: &str = " -->";

/// Classification of the global `<skills_root>/orchbee/SKILL.md` entry for one provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SkillState {
    NotSupported,
    Missing,
    Current,
    Outdated,
    Elsewhere,
    Foreign,
    Unknown,
}

/// Whether `id` manages an established global `SKILL.md` discovery path.
pub(crate) const fn supported(id: IntegrationId) -> bool {
    matches!(
        id,
        IntegrationId::ClaudeCode
            | IntegrationId::Codex
            | IntegrationId::AntigravityCli
            | IntegrationId::Gemini
            | IntegrationId::QwenCode
            | IntegrationId::Opencode
            | IntegrationId::Kilo
            | IntegrationId::Cursor
            | IntegrationId::GitHubCopilot
            | IntegrationId::QoderCli
            | IntegrationId::Droid
            | IntegrationId::Pi
            | IntegrationId::Omp
            | IntegrationId::Kimi
            | IntegrationId::Hermes
    )
}

/// Resolves `<skills_root>/orchbee/SKILL.md` for `id`, refusing invalid overrides without falling
/// back to defaults.
pub(crate) fn skill_target(
    id: IntegrationId,
    home: &Path,
    variables: Variables,
) -> Option<PathBuf> {
    let skills_root = skills_root(id, home, variables)?;
    let target = skills_root.join(SKILL_DIR_NAME).join(SKILL_FILE_NAME);
    (target.is_absolute() && !has_parent_dir(&target) && files::path_admitted(&target))
        .then_some(target)
}

/// Each provider's skill root derives from the same configuration root as its hook or extension
/// path, so one override moves both and no formula exists twice.
fn skills_root(id: IntegrationId, home: &Path, variables: Variables) -> Option<PathBuf> {
    let directory_skills = |name: &str, default: &str| {
        provider_roots::directory(variables, name, home, default).map(|root| root.join("skills"))
    };
    match id {
        IntegrationId::ClaudeCode => directory_skills("CLAUDE_CONFIG_DIR", ".claude"),
        IntegrationId::Codex => directory_skills("CODEX_HOME", ".codex"),
        IntegrationId::AntigravityCli => Some(home.join(".gemini/config/skills")),
        IntegrationId::Gemini => {
            provider_roots::gemini_home(home, variables).map(|root| root.join(".gemini/skills"))
        }
        IntegrationId::QwenCode => {
            provider_roots::qwen_home(home, variables).map(|root| root.join("skills"))
        }
        IntegrationId::Opencode => {
            provider_roots::opencode_config(home, variables).map(|root| root.join("skills"))
        }
        IntegrationId::Kilo => {
            provider_roots::kilo_config(home, variables).map(|root| root.join("skills"))
        }
        IntegrationId::Cursor => directory_skills("CURSOR_CONFIG_DIR", ".cursor"),
        IntegrationId::GitHubCopilot => directory_skills("COPILOT_HOME", ".copilot"),
        IntegrationId::QoderCli => directory_skills("QODER_CONFIG_DIR", ".qoder"),
        IntegrationId::Droid => Some(home.join(".factory/skills")),
        IntegrationId::Pi => directory_skills("PI_CODING_AGENT_DIR", ".pi/agent"),
        IntegrationId::Omp => Some(home.join(".omp/agent/skills")),
        IntegrationId::Kimi => directory_skills("KIMI_CODE_HOME", ".kimi-code"),
        IntegrationId::Hermes => {
            configuration::hermes_home_with(home, variables).map(|profile| profile.join("skills"))
        }
        IntegrationId::Grok
        | IntegrationId::Devin
        | IntegrationId::Letta
        | IntegrationId::Mastracode
        | IntegrationId::Cline => None,
    }
}

fn has_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

/// Canonical `SKILL.md` bytes stamped for `capsule_root`.
pub(crate) fn skill_bytes(capsule_root: &Path) -> Option<Vec<u8>> {
    let root_text = capsule_root.to_str()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(root_text.as_bytes());
    let body = format!(
        "---\n\
         name: orchbee\n\
         description: Use when running inside an Orchbee terminal (ORCHBEE_TERMINAL_ID is set) to discover current Orchbee CLI orchestration commands and safe cursor rules.\n\
         ---\n\
         {MARKER_PREFIX}{encoded}{MARKER_SUFFIX}\n\
         \n\
         When `ORCHBEE_TERMINAL_ID` is set in your environment, you are running inside an Orchbee-managed terminal.\n\
         \n\
         Before orchestrating panels, terminals, or nested agents, run `orchbee --skill` to read the current quickstart recipes and safety rules.\n"
    );
    Some(body.into_bytes())
}

/// The capsule root that a `v1` Orchbee skill file records in its marker. The body is not
/// compared, so a later template change still recognizes every earlier owned file.
pub(crate) fn skill_root(bytes: &[u8]) -> Option<PathBuf> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut matches = text.lines().filter_map(|line| {
        line.strip_prefix(MARKER_PREFIX)?
            .strip_suffix(MARKER_SUFFIX)
    });
    let encoded = matches.next()?;
    if matches.next().is_some() || encoded.is_empty() {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let root_text = String::from_utf8(decoded).ok()?;
    let root = PathBuf::from(&root_text);
    (root.is_absolute() && !has_parent_dir(&root)).then_some(root)
}

/// Inspects the existing skill entry for `id` without modifying the filesystem.
pub(crate) fn inspect(
    id: IntegrationId,
    home: &Path,
    variables: Variables,
    options: &ResourceOptions,
) -> SkillState {
    if !supported(id) {
        return SkillState::NotSupported;
    }
    let Some(target) = skill_target(id, home, variables) else {
        return SkillState::Unknown;
    };
    let Some(orchbee_dir) = target.parent() else {
        return SkillState::Unknown;
    };
    let Some(skills_dir) = orchbee_dir.parent() else {
        return SkillState::Unknown;
    };

    for dir in [skills_dir, orchbee_dir] {
        match fs::symlink_metadata(dir) {
            Err(error) if error.kind() == ErrorKind::NotFound => return SkillState::Missing,
            Err(_) => return SkillState::Unknown,
            Ok(metadata) if is_reparse_or_symlink(&metadata) || !metadata.is_dir() => {
                return SkillState::Foreign;
            }
            Ok(_) => {}
        }
    }

    match fs::symlink_metadata(&target) {
        Err(error) if error.kind() == ErrorKind::NotFound => return SkillState::Missing,
        Err(_) => return SkillState::Unknown,
        Ok(metadata) => {
            if is_reparse_or_symlink(&metadata) || !metadata.is_file() {
                return SkillState::Foreign;
            }
            if metadata.len() > MAX_SKILL_BYTES {
                return SkillState::Foreign;
            }
        }
    }

    let bytes = match files::read_bounded(&target, MAX_SKILL_BYTES) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return SkillState::Missing,
        Err(error) if error.kind() == ErrorKind::InvalidData => return SkillState::Foreign,
        Err(_) => return SkillState::Unknown,
    };

    classify_bytes(&bytes, options)
}

/// Ownership by the recorded capsule root, the rule owned Agent entries follow. This environment's
/// root is ours: `Current` only with the exact current template, and otherwise `Outdated`, so enable
/// rewrites it. A sibling capsule of this environment is `Outdated`, another environment's root is
/// `Elsewhere`, and any other file is `Foreign`.
pub(crate) fn classify_bytes(bytes: &[u8], options: &ResourceOptions) -> SkillState {
    let Some(root) = skill_root(bytes) else {
        return SkillState::Foreign;
    };
    let Some(root_str) = root.to_str() else {
        return SkillState::Foreign;
    };
    match inspect::classify_resource_root(root_str, options) {
        ResourceOwnership::Current if skill_bytes(&root).as_deref() == Some(bytes) => {
            SkillState::Current
        }
        ResourceOwnership::Current | ResourceOwnership::Outdated => SkillState::Outdated,
        ResourceOwnership::Foreign => {
            if hook_configuration::owned_by_another_environment(
                &root,
                options.resource_root.parent(),
            ) {
                SkillState::Elsewhere
            } else {
                SkillState::Foreign
            }
        }
    }
}

fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Folds primary snapshot `state` with `skill` for the user-visible integration status. A skill this
/// environment does not own is an accessory that is skipped: it neither changes nor blocks the
/// integration.
pub(crate) fn combine_state(primary: State, skill: SkillState) -> State {
    match skill {
        SkillState::NotSupported | SkillState::Foreign | SkillState::Elsewhere => primary,
        SkillState::Unknown => State::Unknown,
        SkillState::Missing => match primary {
            State::Enabled => State::UpdateRequired,
            other => other,
        },
        SkillState::Current => match primary {
            State::NotInstalled | State::UpdateRequired => State::UpdateRequired,
            State::Disabled => State::Disabled,
            State::Enabled => State::Enabled,
            State::Conflict => State::Conflict,
            State::Unknown => State::Unknown,
        },
        SkillState::Outdated => match primary {
            State::NotInstalled | State::Disabled | State::Enabled | State::UpdateRequired => {
                State::UpdateRequired
            }
            State::Conflict => State::Conflict,
            State::Unknown => State::Unknown,
        },
    }
}

/// Folds files-only `list`/`list_owned` `(row, own)` with `skill` without starting Agent binaries.
pub(crate) fn combine_unread(row: State, own: State, skill: SkillState) -> (State, State) {
    match skill {
        SkillState::NotSupported
        | SkillState::Missing
        | SkillState::Foreign
        | SkillState::Elsewhere => (row, own),
        SkillState::Unknown => (State::Unknown, State::Unknown),
        SkillState::Current | SkillState::Outdated => {
            let folded_own = match own {
                State::Unknown => State::UpdateRequired,
                other => combine_state(other, skill),
            };
            (row, folded_own)
        }
    }
}
