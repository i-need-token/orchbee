//! Per-provider observation — the canonical state classifiers for managed
//! plugins, Codex, Cline, and the guarded local entries.
//! Resource identity is a payload digest, with no ordering between siblings.
//! Safely verified non-current owned contracts require explicit reinstall.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::Digest;

use super::model::{IntegrationId, State};

#[cfg(windows)]
mod windows;

pub(crate) const CLAUDE_PLUGIN_ID: &str = "orchbee-agent-observation@orchbee-local";
pub(crate) const CODEX_PLUGIN_ID: &str = "orchbee-agent-observation@orchbee-local";
pub(crate) const GITHUB_COPILOT_PLUGIN_ID: &str = "orchbee-agent-observation@orchbee-local";
pub(crate) const GROK_PLUGIN_NAME: &str = "orchbee-agent-observation";
pub(crate) const MARKETPLACE_NAME: &str = "orchbee-local";
const CLINE_PLUGIN_NAME: &str = "orchbee-agent-observation";
const CLINE_CONTRACT_VERSION: u64 = 1;
pub(crate) const ENTRY_CONTRACT_VERSION: u64 = 1;
const MAX_CLINE_INSTALLS: usize = 128;
const MAX_CLINE_MANIFEST_BYTES: u64 = 16 * 1024;
const MAX_CODEX_CONFIGURATION_BYTES: u64 = 512 * 1024;
const MAX_ENTRY_DEPTH: usize = 8;
const MAX_ENTRY_FILES: usize = 64;
const MAX_ENTRY_FILE_BYTES: u64 = 128 * 1024;
const MAX_ENTRY_TOTAL_BYTES: u64 = 512 * 1024;
const MAX_MARKER_BYTES: u64 = 16 * 1024;
pub(crate) const DIRECTORY_MARKER_NAME: &str = ".orchbee-agent-observation.json";
const BOOTSTRAP_MARKER_PREFIX: &str = "// orchbee-agent-observation-bootstrap-v1:";
const REQUIRED_RUNTIME_FILES: [&str; 3] = [
    "forward-hook.mjs",
    "hook-sanitizers.mjs",
    "observation-transport.mjs",
];

/// What a resource root means relative to this build's managed root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceOwnership {
    Current,
    /// A different content identity under the same managed installation parent.
    Outdated,
    Foreign,
}

/// The classifier's current content-addressed capsule root.
pub(crate) struct ResourceOptions<'a> {
    pub(crate) resource_root: &'a Path,
}

/// Exact current identity or an outdated sibling; paths outside the managed
/// installation parent never establish ownership.
pub(crate) fn classify_resource_root(value: &str, options: &ResourceOptions) -> ResourceOwnership {
    let candidate = Path::new(value);
    if !candidate.is_absolute() {
        return ResourceOwnership::Foreign;
    }
    let resolved = normalize(candidate);
    let current = normalize(options.resource_root);
    if resolved == current {
        return ResourceOwnership::Current;
    }
    if resolved.parent() == current.parent() {
        return ResourceOwnership::Outdated;
    }
    ResourceOwnership::Foreign
}

fn normalize(path: &Path) -> PathBuf {
    // `path.resolve` equivalent for absolute inputs: lexical `.`/`..`
    // collapse without touching the filesystem.
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            #[cfg(windows)]
            std::path::Component::Prefix(prefix) => match prefix.kind() {
                std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) => {
                    out.push(format!("{}:", char::from(drive.to_ascii_uppercase())));
                }
                _ => out.push(component.as_os_str()),
            },
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn read_string<'a>(record: &'a Value, key: &str) -> Option<&'a str> {
    record.get(key).and_then(Value::as_str)
}

fn read_boolean(record: &Value, key: &str) -> Option<bool> {
    record.get(key).and_then(Value::as_bool)
}

fn record_array(value: &[u8]) -> Option<Vec<Value>> {
    match serde_json::from_slice::<Value>(value).ok()? {
        Value::Array(entries) if entries.iter().all(|entry| entry.is_object()) => Some(entries),
        _ => None,
    }
}

/// Shared shape for the CLI/marketplace plugin providers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PluginInspection {
    pub(crate) marketplace: Marketplace,
    pub(crate) plugin_present: bool,
    pub(crate) state: State,
}

pub use orchbee_agent_integration::{Marketplace, TargetState};

/// claude-code — `claude plugin list --json` + `plugin marketplace list
/// --json`, canonical verdict order preserved.
pub(crate) fn inspect_claude(
    plugin_stdout: &[u8],
    marketplace_stdout: &[u8],
    options: &ResourceOptions,
) -> Option<PluginInspection> {
    let plugins = record_array(plugin_stdout)?;
    let marketplaces = record_array(marketplace_stdout)?;
    let plugin_matches: Vec<&Value> = plugins
        .iter()
        .filter(|entry| read_string(entry, "id") == Some(CLAUDE_PLUGIN_ID))
        .collect();
    let marketplace_matches: Vec<&Value> = marketplaces
        .iter()
        .filter(|entry| read_string(entry, "name") == Some(MARKETPLACE_NAME))
        .collect();
    if plugin_matches.len() > 1 || marketplace_matches.len() > 1 {
        return Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: false,
            state: State::Conflict,
        });
    }
    let plugin = plugin_matches.first().copied();
    let marketplace = match marketplace_matches.first() {
        None => Marketplace::Absent,
        Some(entry) => {
            let source_kind = read_string(entry, "source");
            let source_path = read_string(entry, "path");
            match (source_kind, source_path) {
                (Some("directory") | Some("local"), Some(path)) => {
                    match classify_resource_root(path, options) {
                        ResourceOwnership::Foreign => {
                            return Some(PluginInspection {
                                marketplace: Marketplace::Absent,
                                plugin_present: plugin.is_some(),
                                state: State::Conflict,
                            });
                        }
                        ResourceOwnership::Outdated => Marketplace::Outdated,
                        ResourceOwnership::Current => Marketplace::Current,
                    }
                }
                _ => {
                    return Some(PluginInspection {
                        marketplace: Marketplace::Absent,
                        plugin_present: plugin.is_some(),
                        state: State::Conflict,
                    });
                }
            }
        }
    };
    if marketplace == Marketplace::Outdated {
        return Some(PluginInspection {
            marketplace,
            plugin_present: plugin.is_some(),
            state: State::UpdateRequired,
        });
    }
    let Some(plugin) = plugin else {
        return Some(PluginInspection {
            marketplace,
            plugin_present: false,
            state: State::NotInstalled,
        });
    };
    if marketplace == Marketplace::Absent {
        return Some(PluginInspection {
            marketplace,
            plugin_present: true,
            state: State::Conflict,
        });
    }
    read_string(plugin, "version")?;
    let enabled = read_boolean(plugin, "enabled")?;
    Some(PluginInspection {
        marketplace,
        plugin_present: true,
        state: if enabled {
            State::Enabled
        } else {
            State::Disabled
        },
    })
}

#[cfg(windows)]
pub(crate) fn inspect_grok(
    plugin_stdout: &[u8],
    options: &ResourceOptions,
    home: &Path,
) -> Option<PluginInspection> {
    windows::inspect_grok(plugin_stdout, options, home)
}

/// Unix provider inventory contract.
#[cfg(unix)]
pub(crate) fn inspect_grok(
    plugin_stdout: &[u8],
    options: &ResourceOptions,
    _home: &Path,
) -> Option<PluginInspection> {
    let entries = record_array(plugin_stdout)?;
    let matches: Vec<&Value> = entries
        .iter()
        .filter(|entry| read_string(entry, "name") == Some(GROK_PLUGIN_NAME))
        .collect();
    if matches.is_empty() {
        return Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: false,
            state: State::NotInstalled,
        });
    }
    if matches.len() != 1 {
        return Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: false,
            state: State::Conflict,
        });
    }
    let plugin = matches[0];
    let source = read_string(plugin, "source");
    let version = read_string(plugin, "version");
    let (Some(source), Some(_)) = (source, version) else {
        return Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: true,
            state: State::Conflict,
        });
    };
    if Path::new(source).file_name().and_then(|name| name.to_str()) != Some("bundle") {
        return Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: true,
            state: State::Conflict,
        });
    }
    let parent = Path::new(source).parent().unwrap_or_else(|| Path::new("/"));
    match classify_resource_root(&parent.to_string_lossy(), options) {
        ResourceOwnership::Foreign => {
            return Some(PluginInspection {
                marketplace: Marketplace::Absent,
                plugin_present: true,
                state: State::Conflict,
            });
        }
        ResourceOwnership::Outdated => {
            return Some(PluginInspection {
                marketplace: Marketplace::Absent,
                plugin_present: true,
                state: State::UpdateRequired,
            });
        }
        ResourceOwnership::Current => {}
    }
    if let Some(enabled) = read_boolean(plugin, "enabled") {
        return Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: true,
            state: if enabled {
                State::Enabled
            } else {
                State::Disabled
            },
        });
    }
    match read_string(plugin, "status") {
        Some("disabled") => Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: true,
            state: State::Disabled,
        }),
        Some("enabled") | Some("installed") => Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: true,
            state: State::Enabled,
        }),
        _ => None,
    }
}

/// Windows Copilot 1.0.88 uses a flat plugin array, explicit live marketplace
/// identity and version. Unix retains its earlier resource-command contract.
/// Neither path confuses a plugin name with installed source ownership.
pub(crate) fn inspect_copilot(
    plugin_stdout: &[u8],
    marketplace_stdout: &[u8],
    options: &ResourceOptions,
) -> Option<PluginInspection> {
    #[cfg(unix)]
    let root = serde_json::from_slice::<Value>(plugin_stdout).ok()?;
    #[cfg(unix)]
    let plugins = match root.get("plugins")?.as_array() {
        Some(entries) if entries.iter().all(|entry| entry.is_object()) => entries.clone(),
        _ => return None,
    };
    #[cfg(windows)]
    let plugins = record_array(plugin_stdout)?;
    let marketplaces = record_array(marketplace_stdout)?;
    let plugin_matches: Vec<&Value> = plugins
        .iter()
        .filter(|entry| read_string(entry, "name") == Some(GROK_PLUGIN_NAME))
        .collect();
    let marketplace_matches: Vec<&Value> = marketplaces
        .iter()
        .filter(|entry| read_string(entry, "name") == Some(MARKETPLACE_NAME))
        .collect();
    if plugin_matches.len() > 1 || marketplace_matches.len() > 1 {
        return Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present: false,
            state: State::Conflict,
        });
    }
    let plugin = plugin_matches.first().copied();
    let marketplace = match marketplace_matches.first() {
        None => Marketplace::Absent,
        Some(entry) => {
            let Some(source) = read_string(entry, "source") else {
                return Some(PluginInspection {
                    marketplace: Marketplace::Absent,
                    plugin_present: plugin.is_some(),
                    state: State::Conflict,
                });
            };
            let Some(rest) = source.strip_prefix("Local: ") else {
                return Some(PluginInspection {
                    marketplace: Marketplace::Absent,
                    plugin_present: plugin.is_some(),
                    state: State::Conflict,
                });
            };
            match classify_resource_root(rest, options) {
                ResourceOwnership::Foreign => {
                    return Some(PluginInspection {
                        marketplace: Marketplace::Absent,
                        plugin_present: plugin.is_some(),
                        state: State::Conflict,
                    });
                }
                ResourceOwnership::Outdated => Marketplace::Outdated,
                ResourceOwnership::Current => Marketplace::Current,
            }
        }
    };
    let Some(plugin) = plugin else {
        return Some(PluginInspection {
            marketplace,
            plugin_present: false,
            state: if marketplace == Marketplace::Outdated {
                State::UpdateRequired
            } else {
                State::NotInstalled
            },
        });
    };
    let expected_source = if cfg!(windows) {
        "live"
    } else {
        "live-marketplace:orchbee-local"
    };
    if read_string(plugin, "source") != Some(expected_source)
        || read_string(plugin, "installedFrom").is_none()
        || (cfg!(windows) && read_string(plugin, "marketplace") != Some(MARKETPLACE_NAME))
    {
        return Some(PluginInspection {
            marketplace,
            plugin_present: true,
            state: State::Conflict,
        });
    }
    let installed_from = read_string(plugin, "installedFrom").unwrap_or_default();
    let ownership = classify_resource_root(installed_from, options);
    if ownership == ResourceOwnership::Foreign || marketplace == Marketplace::Absent {
        return Some(PluginInspection {
            marketplace,
            plugin_present: true,
            state: State::Conflict,
        });
    }
    if ownership == ResourceOwnership::Outdated || marketplace == Marketplace::Outdated {
        return Some(PluginInspection {
            marketplace,
            plugin_present: true,
            state: State::UpdateRequired,
        });
    }
    #[cfg(windows)]
    read_string(plugin, "version")?;
    let enabled = read_boolean(plugin, "enabled")?;
    Some(PluginInspection {
        marketplace,
        plugin_present: true,
        state: if enabled {
            State::Enabled
        } else {
            State::Disabled
        },
    })
}

/// codex — the bounded `~/.codex/config.toml` line scanner, verbatim from
/// canonical (accepted TOML subset: single-line `[table]` / `[table."k"]`
/// headers and `k = v` assignments; anything else is ignored or fails
/// conservatively — a real-TOML parser is a deferred canonical change).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CodexInspection {
    pub(crate) marketplace: Marketplace,
    pub(crate) plugin_present: bool,
    pub(crate) state: State,
}

#[cfg(windows)]
pub(crate) fn inspect_codex(home: &Path, options: &ResourceOptions) -> CodexInspection {
    windows::inspect_codex(home, options)
}

/// One validated Windows profile selection shared by reads and provider mutations.
#[cfg(windows)]
pub(crate) fn codex_home(home: &Path) -> Option<PathBuf> {
    windows::codex_home(home)
}

#[cfg(unix)]
pub(crate) fn inspect_codex(home: &Path, options: &ResourceOptions) -> CodexInspection {
    let configuration_path = home.join(".codex/config.toml");
    let bytes = match bounded_file_follow(&configuration_path, MAX_CODEX_CONFIGURATION_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return CodexInspection {
                marketplace: Marketplace::Absent,
                plugin_present: false,
                state: State::NotInstalled,
            };
        }
        Err(_) => {
            return CodexInspection {
                marketplace: Marketplace::Absent,
                plugin_present: false,
                state: State::Unknown,
            };
        }
    };
    parse_codex_configuration(&String::from_utf8_lossy(&bytes), options)
}

#[cfg(unix)]
fn parse_codex_configuration(value: &str, options: &ResourceOptions) -> CodexInspection {
    enum Section {
        Marketplace,
        Plugin,
        Other,
    }
    let mut section = Section::Other;
    let mut marketplace_count = 0usize;
    let mut plugin_count = 0usize;
    let mut marketplace_values: Vec<(&str, &str)> = Vec::new();
    let mut plugin_values: Vec<(&str, &str)> = Vec::new();
    let conflict = || CodexInspection {
        marketplace: Marketplace::Absent,
        plugin_present: false,
        state: State::Conflict,
    };
    for raw_line in value.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if line == format!("[marketplaces.{MARKETPLACE_NAME}]") {
                marketplace_count += 1;
                section = Section::Marketplace;
            } else if line
                == format!(
                    "[plugins.{}]",
                    serde_json::to_string(CODEX_PLUGIN_ID).unwrap_or_default()
                )
            {
                plugin_count += 1;
                section = Section::Plugin;
            } else {
                section = Section::Other;
            }
            continue;
        }
        if matches!(section, Section::Other) {
            continue;
        }
        let Some(assignment) = line.split_once('=') else {
            continue;
        };
        let key = assignment.0.trim();
        if !key.bytes().enumerate().all(|(index, b)| {
            if index == 0 {
                b.is_ascii_alphabetic()
            } else {
                b.is_ascii_alphanumeric() || b == b'_'
            }
        }) || key.is_empty()
        {
            continue;
        }
        let raw_value = assignment.1.trim();
        let target = match section {
            Section::Marketplace => &mut marketplace_values,
            Section::Plugin => &mut plugin_values,
            Section::Other => unreachable!(),
        };
        if target.iter().any(|(existing, _)| *existing == key) {
            return conflict();
        }
        target.push((key, raw_value));
    }
    if marketplace_count > 1 || plugin_count > 1 {
        return conflict();
    }
    let plugin_present = plugin_count == 1;
    let plugin_value = |key: &str| {
        plugin_values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    };
    let marketplace_value = |key: &str| {
        marketplace_values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    };
    let enabled = if plugin_present {
        match plugin_value("enabled") {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        }
    } else {
        None
    };
    if plugin_present && enabled.is_none() {
        return CodexInspection {
            marketplace: Marketplace::Absent,
            plugin_present: true,
            state: State::Conflict,
        };
    }
    if marketplace_count == 0 {
        return CodexInspection {
            marketplace: Marketplace::Absent,
            plugin_present,
            state: if plugin_present {
                State::Conflict
            } else {
                State::NotInstalled
            },
        };
    }
    let source_type = marketplace_value("source_type").and_then(parse_basic_string);
    let source = marketplace_value("source").and_then(parse_basic_string);
    let (Some(source_type), Some(source)) = (source_type, source) else {
        return CodexInspection {
            marketplace: Marketplace::Absent,
            plugin_present,
            state: State::Conflict,
        };
    };
    if source_type != "local" || !Path::new(&source).is_absolute() {
        return CodexInspection {
            marketplace: Marketplace::Absent,
            plugin_present,
            state: State::Conflict,
        };
    }
    match classify_resource_root(&source, options) {
        ResourceOwnership::Current => CodexInspection {
            marketplace: Marketplace::Current,
            plugin_present,
            state: if !plugin_present {
                State::NotInstalled
            } else if enabled == Some(true) {
                State::Enabled
            } else {
                State::Disabled
            },
        },
        ResourceOwnership::Outdated => CodexInspection {
            marketplace: Marketplace::Outdated,
            plugin_present,
            state: State::UpdateRequired,
        },
        ResourceOwnership::Foreign => CodexInspection {
            marketplace: Marketplace::Absent,
            plugin_present,
            state: State::Conflict,
        },
    }
}

#[cfg(unix)]
fn parse_basic_string(value: &str) -> Option<String> {
    match serde_json::from_str::<Value>(value).ok()? {
        Value::String(text) => Some(text),
        _ => None,
    }
}

/// `lstat`-disciplined bounded read for guarded targets, markers, and
/// fingerprint entries — a symlink is rejected before any follow.
#[cfg(unix)]
fn bounded_file(path: &Path, maximum: u64) -> std::io::Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(io_invalid("unreadable configuration"));
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > maximum {
        return Err(io_invalid("oversized configuration"));
    }
    Ok(Some(bytes))
}

#[cfg(windows)]
fn bounded_file(path: &Path, maximum: u64) -> std::io::Result<Option<Vec<u8>>> {
    match super::files::read_bounded(path, maximum) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn bounded_file_follow(path: &Path, maximum: u64) -> std::io::Result<Option<Vec<u8>>> {
    bounded_file(path, maximum)
}

/// `open`+`fstat` bounded read — follows a symlinked configuration file,
/// because dotfile managers legitimately symlink these paths.
#[cfg(unix)]
fn bounded_file_follow(path: &Path, maximum: u64) -> std::io::Result<Option<Vec<u8>>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(io_invalid("unreadable configuration"));
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(io_invalid("oversized configuration"));
    }
    Ok(Some(bytes))
}

#[cfg(unix)]
fn io_invalid(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

/// cline — `~/.cline/plugins` scan: direct-name conflicts, then
/// `_installed/local/*/package` ownership markers.
pub(crate) fn inspect_cline(home: &Path) -> State {
    let plugin_root = home.join(".cline").join("plugins");
    for direct_name in [
        format!("{CLINE_PLUGIN_NAME}.js"),
        format!("{CLINE_PLUGIN_NAME}.ts"),
        CLINE_PLUGIN_NAME.to_owned(),
    ] {
        match path_state(&plugin_root.join(&direct_name)) {
            PathState::Present | PathState::Symlink => return State::Conflict,
            PathState::Unknown => return State::Unknown,
            PathState::Missing => {}
        }
    }
    let installed_root = plugin_root.join("_installed").join("local");
    let entries = match fs::read_dir(&installed_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return State::NotInstalled;
        }
        Err(_) => return State::Unknown,
    };
    let entries: Vec<_> = match entries.collect::<Result<Vec<_>, _>>() {
        Ok(entries) => entries,
        Err(_) => return State::Unknown,
    };
    if entries.len() > MAX_CLINE_INSTALLS {
        return State::Unknown;
    }
    let mut exact = 0usize;
    let mut outdated = 0usize;
    for entry in entries {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let candidate_root = entry.path().join("package");
        match path_state(&candidate_root.join("cline/orchbee-agent-observation.js")) {
            PathState::Missing => continue,
            PathState::Unknown => return State::Unknown,
            PathState::Symlink => return State::Conflict,
            PathState::Present => {}
        }
        match cline_package_state(&candidate_root) {
            ClinePackage::Conflict => return State::Conflict,
            ClinePackage::UpdateRequired => outdated += 1,
            ClinePackage::Current => exact += 1,
        }
    }
    if exact + outdated > 1 {
        return State::Conflict;
    }
    if outdated == 1 {
        return State::UpdateRequired;
    }
    if exact == 1 {
        State::Enabled
    } else {
        State::NotInstalled
    }
}

pub(crate) enum ClinePackage {
    Conflict,
    Current,
    UpdateRequired,
}

/// The installed `package/package.json` contract: name must be the plugin
/// name; `orchbeeAgentObservation.contractVersion`==1 current, another
/// positive safe integer requires reinstall, otherwise conflict. Also used on the
/// capsule's `bundle/package.json` as the source-readiness check.
pub(crate) fn cline_package_state(package_root: &Path) -> ClinePackage {
    if !fs::symlink_metadata(package_root).is_ok_and(|metadata| metadata.is_dir()) {
        return ClinePackage::Conflict;
    }
    let Some(value) =
        read_json_record(&package_root.join("package.json"), MAX_CLINE_MANIFEST_BYTES)
    else {
        return ClinePackage::Conflict;
    };
    if value.get("name").and_then(Value::as_str) != Some(CLINE_PLUGIN_NAME) {
        return ClinePackage::Conflict;
    }
    let marker = value.get("orchbeeAgentObservation");
    let contract = marker
        .and_then(|marker| marker.get("contractVersion"))
        .and_then(parse_contract_version);
    match contract {
        Some(CLINE_CONTRACT_VERSION) => ClinePackage::Current,
        Some(_) => ClinePackage::UpdateRequired,
        _ => ClinePackage::Conflict,
    }
}

enum PathState {
    Missing,
    Present,
    Symlink,
    Unknown,
}

fn path_state(path: &Path) -> PathState {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => PathState::Symlink,
        Ok(_) => PathState::Present,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => PathState::Missing,
        Err(_) => PathState::Unknown,
    }
}

/// The lstat-disciplined read for ownership markers — a symlinked marker is
/// foreign content, not a readable record.
fn read_json_record(path: &Path, maximum: u64) -> Option<Value> {
    match bounded_file(path, maximum) {
        Ok(Some(bytes)) => match serde_json::from_slice::<Value>(&bytes).ok()? {
            Value::Object(_) => serde_json::from_slice::<Value>(&bytes).ok(),
            _ => None,
        },
        _ => None,
    }
}

/// `pathToFileURL` equivalent for POSIX paths, pinned by the Node v22.23.2
/// receipt: `:` stays literal (pchar), `~` encodes `%7E`, `/` separators and
/// UTF-8 bytes stay/encode per the runtime's own table. The bootstrap bytes
/// must match those earlier Desktop releases wrote, or inspection sees
/// `conflict`.
fn file_url(path: &Path) -> String {
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    #[cfg(unix)]
    let bytes = path.as_os_str().as_bytes();
    #[cfg(windows)]
    let normalized = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    let bytes = normalized.as_bytes();
    let mut url = String::with_capacity(bytes.len() + 8);
    url.push_str("file://");
    #[cfg(windows)]
    if !bytes.starts_with(b"/") {
        url.push('/');
    }
    for &byte in bytes {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'='
            | b'@'
            | b':'
            | b'/' => url.push(byte as char),
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

/// opencode/pi/omp module bootstrap bodies — verbatim canonical templates.
pub(crate) fn module_bootstrap(
    kind: BootstrapKind,
    integration_id: IntegrationId,
    source_path: &Path,
) -> String {
    let module_url = serde_json::to_string(&file_url(source_path)).unwrap_or_default();
    let marker = format!("{BOOTSTRAP_MARKER_PREFIX}{}\n", integration_id.as_str());
    match kind {
        BootstrapKind::Opencode => format!(
            "{marker}const orchbeeAgentObservationModuleUrl = {module_url};\nexport async function OrchbeeAgentObservation(...arguments_) {{\n\ttry {{\n\t\tconst loaded = await import(orchbeeAgentObservationModuleUrl);\n\t\treturn typeof loaded.OrchbeeAgentObservation === 'function'\n\t\t\t? await loaded.OrchbeeAgentObservation(...arguments_)\n\t\t\t: {{}};\n\t}} catch {{\n\t\treturn {{}};\n\t}}\n}}\n"
        ),
        _ => format!(
            "{marker}const orchbeeAgentObservationModuleUrl = {module_url};\nmodule.exports = async function orchbeeAgentObservation(api) {{\n\ttry {{\n\t\tconst loaded = await import(orchbeeAgentObservationModuleUrl);\n\t\tif (typeof loaded.orchbeeAgentObservation === 'function') {{\n\t\t\tawait loaded.orchbeeAgentObservation(api);\n\t\t}}\n\t}} catch {{}}\n}};\n"
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BootstrapKind {
    ExtensionDirectory,
    Omp,
    Opencode,
    Pi,
}

/// The guarded target inspection for module-file providers (opencode/pi/omp)
/// and extension-directory providers (gemini/qwen-code).
pub(crate) fn inspect_guarded_target(
    target: &Path,
    kind: BootstrapKind,
    integration_id: IntegrationId,
    source_relative: &Path,
    options: &ResourceOptions,
) -> TargetState {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return TargetState::Missing;
        }
        Err(_) => return TargetState::Unknown,
    };
    if metadata.file_type().is_symlink() {
        return TargetState::Conflict;
    }
    if kind == BootstrapKind::ExtensionDirectory {
        if !metadata.is_dir() {
            return TargetState::Conflict;
        }
        let state = inspect_owned_directory(target, integration_id);
        #[cfg(windows)]
        return windows::inspect_extension_manifest(target, integration_id, state);
        #[cfg(unix)]
        return state;
    }
    if !metadata.is_file() {
        return TargetState::Conflict;
    }
    let bytes = match bounded_file(target, 16 * 1024) {
        Ok(Some(bytes)) => bytes,
        Err(_) => return TargetState::Unknown,
        _ => return TargetState::Conflict,
    };
    let Some(source) = module_source(&bytes, kind, integration_id) else {
        return TargetState::Conflict;
    };
    let Some(root) = resource_root_for_source(&source, source_relative) else {
        return TargetState::Conflict;
    };
    match classify_resource_root(&root.to_string_lossy(), options) {
        ResourceOwnership::Current => TargetState::Current,
        ResourceOwnership::Outdated => TargetState::UpdateRequired,
        ResourceOwnership::Foreign => TargetState::Conflict,
    }
}

fn module_source(bytes: &[u8], kind: BootstrapKind, id: IntegrationId) -> Option<PathBuf> {
    let text = std::str::from_utf8(bytes).ok()?;
    let rest = text.strip_prefix(&format!("{BOOTSTRAP_MARKER_PREFIX}{}\n", id.as_str()))?;
    let assignment = rest
        .lines()
        .next()?
        .strip_prefix("const orchbeeAgentObservationModuleUrl = ")?
        .strip_suffix(';')?;
    let url: String = serde_json::from_str(assignment).ok()?;
    let encoded = url.strip_prefix("file://")?.as_bytes();
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut index = 0;
    while index < encoded.len() {
        if encoded[index] == b'%' {
            let digits = std::str::from_utf8(encoded.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(digits, 16).ok()?);
            index += 3;
        } else {
            decoded.push(encoded[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded).ok()?;
    #[cfg(windows)]
    let decoded = decoded.strip_prefix('/').unwrap_or(&decoded).to_owned();
    let source = PathBuf::from(decoded);
    if !source.is_absolute() {
        return None;
    }
    #[cfg(windows)]
    if !super::files::path_admitted(&source) {
        return None;
    }
    // Round-trip validates canonical URL spelling and every executable byte,
    // rather than accepting an edited body that happens to carry our marker.
    (module_bootstrap(kind, id, &source).as_bytes() == bytes).then_some(source)
}

/// `<source dir>` minus its path inside the resource root → the root the
/// entry points at; `None` when the relationship does not hold.
fn resource_root_for_source(source_path: &Path, relative_source: &Path) -> Option<PathBuf> {
    let mut candidate = source_path.to_path_buf();
    for _ in relative_source.components() {
        candidate.pop();
    }
    (normalize(&candidate.join(relative_source)) == normalize(source_path))
        .then(|| normalize(&candidate))
}

/// The extension-directory marker check: `.orchbee-agent-observation.json`
/// pins the entry to this integration id plus a sorted file-digest list.
/// A non-current contract requires reinstall only after the marker's ownership
/// and every actual file fingerprint agree.
pub(crate) fn inspect_owned_directory(root: &Path, integration_id: IntegrationId) -> TargetState {
    let Some(value) = read_json_record(&root.join(DIRECTORY_MARKER_NAME), MAX_MARKER_BYTES) else {
        return TargetState::Conflict;
    };
    let Some(marker) = parse_directory_marker(&value, integration_id) else {
        return TargetState::Conflict;
    };
    let Some(files) = fingerprint_directory(root) else {
        return TargetState::Conflict;
    };
    if files != marker.files {
        return TargetState::Conflict;
    }
    if marker.contract_version == ENTRY_CONTRACT_VERSION {
        TargetState::Current
    } else {
        TargetState::UpdateRequired
    }
}

struct OwnedMarker {
    contract_version: u64,
    files: Vec<(String, String)>,
}

fn parse_directory_marker(value: &Value, integration_id: IntegrationId) -> Option<OwnedMarker> {
    if value.get("kind").and_then(Value::as_str) != Some("guarded-extension")
        || value.get("integrationId").and_then(Value::as_str) != Some(integration_id.as_str())
    {
        return None;
    }
    let contract_version = value
        .get("contractVersion")
        .and_then(parse_contract_version)?;
    let files = value.get("files")?.as_array()?;
    if files.len() > MAX_ENTRY_FILES {
        return None;
    }
    let mut parsed: Vec<(String, String)> = Vec::new();
    for file in files {
        let path = file.get("path").and_then(Value::as_str)?;
        let sha256 = file.get("sha256").and_then(Value::as_str)?;
        if !valid_relative_path(path) || !valid_digest(sha256) {
            return None;
        }
        parsed.push((path.to_owned(), sha256.to_owned()));
    }
    for index in 1..parsed.len() {
        if parsed[index - 1].0 >= parsed[index].0 {
            return None;
        }
    }
    Some(OwnedMarker {
        contract_version,
        files: parsed,
    })
}

fn parse_contract_version(value: &Value) -> Option<u64> {
    let number = value.as_f64()?;
    (number.fract() == 0.0 && (1.0..=9_007_199_254_740_991.0).contains(&number))
        .then_some(number as u64)
}

pub(crate) fn fingerprint_directory(root: &Path) -> Option<Vec<(String, String)>> {
    let mut pending: Vec<(usize, PathBuf)> = vec![(0, root.to_path_buf())];
    let mut files: Vec<(String, String)> = Vec::new();
    let mut total_bytes = 0u64;
    while let Some((depth, directory)) = pending.pop() {
        #[cfg(windows)]
        let _pins = super::files::pin_tree(&directory, false).ok()?;
        if depth > MAX_ENTRY_DEPTH {
            return None;
        }
        for entry in fs::read_dir(&directory).ok()? {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() {
                return None;
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .ok()?
                .to_str()?
                .replace(std::path::MAIN_SEPARATOR, "/");
            if relative == DIRECTORY_MARKER_NAME {
                continue;
            }
            if file_type.is_dir() {
                pending.push((depth + 1, path));
                continue;
            }
            if !file_type.is_file() || files.len() >= MAX_ENTRY_FILES {
                return None;
            }
            let bytes = bounded_file(&path, MAX_ENTRY_FILE_BYTES).ok()??;
            total_bytes += bytes.len() as u64;
            if total_bytes > MAX_ENTRY_TOTAL_BYTES {
                return None;
            }
            files.push((relative, format!("{:x}", sha2::Sha256::digest(&bytes))));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Some(files)
}

fn valid_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The extension-directory source readiness: source dir present plus the
/// three shared runtime files.
pub(crate) fn guarded_source_ready(
    source_path: &Path,
    kind: BootstrapKind,
    runtime_source: Option<&Path>,
) -> bool {
    match fs::metadata(source_path) {
        Ok(metadata) => {
            if kind != BootstrapKind::ExtensionDirectory {
                return metadata.is_file();
            }
            let Some(runtime) = runtime_source else {
                return false;
            };
            if !metadata.is_dir() {
                return false;
            }
            REQUIRED_RUNTIME_FILES.iter().all(|name| {
                fs::metadata(runtime.join(name))
                    .map(|m| m.is_file())
                    .unwrap_or(false)
            })
        }
        Err(_) => false,
    }
}

/// Windows Qwen uses its official per-hook shell selector. Build before the
/// owned staging tree's fingerprint; this never writes provider configuration.
#[cfg(windows)]
pub(crate) fn windows_qwen_manifest(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(bytes).ok()?;
    if read_string(&value, "name") != Some("orchbee-agent-observation") {
        return None;
    }
    let hooks = value.get_mut("hooks")?.as_object_mut()?;
    if hooks.is_empty() {
        return None;
    }
    for groups in hooks.values_mut() {
        for group in groups.as_array_mut()? {
            for hook in group.get_mut("hooks")?.as_array_mut()? {
                let hook = hook.as_object_mut()?;
                if hook.get("type")?.as_str()? != "command" {
                    return None;
                }
                hook.insert("shell".to_string(), Value::String("powershell".to_string()));
            }
        }
    }
    serde_json::to_vec(&value).ok()
}
