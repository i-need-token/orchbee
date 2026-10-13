//! Windows provider configuration and fixed-bootstrap inspection. Installation state is local,
//! never a claim about project policy or an already-running provider session.
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::Digest;
use toml::Table;

use super::super::model::IntegrationId;
use super::{
    BOOTSTRAP_MARKER_PREFIX, BootstrapKind, CODEX_PLUGIN_ID, CodexInspection, GROK_PLUGIN_NAME,
    MARKETPLACE_NAME, MAX_CODEX_CONFIGURATION_BYTES, Marketplace, PluginInspection,
    ResourceOptions, ResourceOwnership, State, TargetState, bounded_file_follow,
    classify_resource_root, module_bootstrap, read_string, record_array, resource_root_for_source,
    windows_plugin_version,
};

pub(super) fn inspect_module_target(
    target: &Path,
    kind: BootstrapKind,
    id: IntegrationId,
    relative: &Path,
    options: &ResourceOptions,
) -> TargetState {
    let bytes = match bounded_file_follow(target, 16 * 1024) {
        Ok(Some(bytes)) => bytes,
        _ => return TargetState::Unknown,
    };
    let source = module_source(&bytes, kind, id);
    let Some(root) = source.and_then(|source| resource_root_for_source(&source, relative)) else {
        return TargetState::Conflict;
    };
    match classify_resource_root(&root.to_string_lossy(), options) {
        ResourceOwnership::Current => TargetState::Current,
        ResourceOwnership::Legacy => TargetState::Legacy,
        ResourceOwnership::Newer => TargetState::Newer,
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
    let encoded = url.strip_prefix("file:///")?.as_bytes();
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
    let source = PathBuf::from(String::from_utf8(decoded).ok()?);
    if !super::super::files::path_admitted(&source) {
        return None;
    }
    // Round-trip validates canonical URL spelling and every executable byte,
    // rather than accepting an edited body that happens to carry our marker.
    (module_bootstrap(kind, id, &source).as_bytes() == bytes).then_some(source)
}

fn configuration_home(home: &Path, variable: &str, default: &str) -> Option<PathBuf> {
    let configured = std::env::var_os(variable);
    let root = match configured.as_deref().filter(|value| !value.is_empty()) {
        Some(value) => {
            let root = PathBuf::from(value);
            let _pins = super::super::files::pin_tree(&root, false).ok()?;
            root
        }
        None => home.join(default),
    };
    super::super::files::path_admitted(&root).then_some(root)
}

pub(super) fn codex_home(home: &Path) -> Option<PathBuf> {
    configuration_home(home, "CODEX_HOME", ".codex")
}

fn configuration_path(home: &Path, variable: &str, default: &str) -> Option<PathBuf> {
    configuration_home(home, variable, default).map(|root| root.join("config.toml"))
}

fn read_configuration(path: &Path) -> Option<Table> {
    match bounded_file_follow(path, MAX_CODEX_CONFIGURATION_BYTES).ok()? {
        Some(bytes) => std::str::from_utf8(&bytes).ok()?.parse().ok(),
        None => Some(Table::new()),
    }
}

fn codex_state(state: State) -> CodexInspection {
    CodexInspection {
        marketplace: Marketplace::Absent,
        plugin_present: false,
        state,
    }
}

pub(super) fn inspect_codex(home: &Path, options: &ResourceOptions) -> CodexInspection {
    codex_home(home)
        .map(|root| root.join("config.toml"))
        .and_then(|path| read_configuration(&path))
        .map(|configuration| codex_configuration(&configuration, options))
        .unwrap_or_else(|| codex_state(State::Unknown))
}

/// An absent entry differs from a present entry with an invalid table type.
fn entry_table<'a>(root: &'a Table, group: &str, name: &str) -> Result<Option<&'a Table>, ()> {
    let Some(group) = root.get(group) else {
        return Ok(None);
    };
    let group = group.as_table().ok_or(())?;
    group
        .get(name)
        .map(|entry| entry.as_table().ok_or(()))
        .transpose()
}

fn codex_configuration(root: &Table, options: &ResourceOptions) -> CodexInspection {
    let Ok(plugin) = entry_table(root, "plugins", CODEX_PLUGIN_ID) else {
        return codex_state(State::Conflict);
    };
    let plugin_present = plugin.is_some();
    let conflict = || CodexInspection {
        marketplace: Marketplace::Absent,
        plugin_present,
        state: State::Conflict,
    };
    let enabled = plugin
        .and_then(|entry| entry.get("enabled"))
        .and_then(toml::Value::as_bool);
    if plugin_present && enabled.is_none() {
        return conflict();
    }
    let Ok(marketplace) = entry_table(root, "marketplaces", MARKETPLACE_NAME) else {
        return conflict();
    };
    let Some(marketplace) = marketplace else {
        return if plugin_present {
            conflict()
        } else {
            codex_state(State::NotInstalled)
        };
    };
    if marketplace.get("source_type").and_then(toml::Value::as_str) != Some("local") {
        return conflict();
    }
    let Some(source) = marketplace.get("source").and_then(toml::Value::as_str) else {
        return conflict();
    };
    let (marketplace, state) = match classify_resource_root(source, options) {
        ResourceOwnership::Foreign => return conflict(),
        ResourceOwnership::Newer => (Marketplace::Absent, State::StaleSide),
        ResourceOwnership::Legacy => (Marketplace::Legacy, State::UpdateRequired),
        ResourceOwnership::Current => (
            Marketplace::Current,
            match enabled {
                Some(true) => State::Enabled,
                Some(false) => State::Disabled,
                None => State::NotInstalled,
            },
        ),
    };
    CodexInspection {
        marketplace,
        plugin_present,
        state,
    }
}

fn plugin_names<'a>(root: &'a Table, field: &str) -> Option<Vec<&'a str>> {
    let Some(plugins) = root.get("plugins") else {
        return Some(Vec::new());
    };
    let Some(values) = plugins.as_table()?.get(field) else {
        return Some(Vec::new());
    };
    values.as_array()?.iter().map(toml::Value::as_str).collect()
}

fn grok_activation(root: &Table, plugin: &Value) -> Option<State> {
    let enabled = plugin_names(root, "enabled")?;
    let disabled = plugin_names(root, "disabled")?;
    // Expanded variables or path-derived IDs cannot be mistaken for unrelated
    // entries. Resolve a provider ID only when the user's lists actually use it.
    if enabled
        .iter()
        .chain(&disabled)
        .any(|value| value.contains('$'))
    {
        return None;
    }
    let uses_id = enabled
        .iter()
        .chain(&disabled)
        .any(|value| value.ends_with(&format!("/{GROK_PLUGIN_NAME}")));
    let id = if uses_id {
        let canonical = std::fs::canonicalize(read_string(plugin, "path")?).ok()?;
        let path = canonical.to_str()?;
        // Upstream dunce removes Windows verbatim prefixes before hashing.
        let path = path.strip_prefix(r"\\?\").unwrap_or(path);
        let digest = format!("{:x}", sha2::Sha256::digest(path.as_bytes()));
        Some(format!("user/{}/{GROK_PLUGIN_NAME}", &digest[..8]))
    } else {
        None
    };
    let matches = |name: &&str| *name == GROK_PLUGIN_NAME || Some(*name) == id.as_deref();
    Some(
        if disabled.iter().any(matches) || !enabled.iter().any(matches) {
            State::Disabled
        } else {
            State::Enabled
        },
    )
}

pub(super) fn inspect_grok(
    stdout: &[u8],
    options: &ResourceOptions,
    home: &Path,
) -> Option<PluginInspection> {
    // Even a missing plugin must not authorize install through an invalid
    // configuration root or a broken document that the provider cannot update.
    let path = configuration_path(home, "GROK_HOME", ".grok")?;
    let configuration = read_configuration(&path)?;
    plugin_names(&configuration, "enabled")?;
    plugin_names(&configuration, "disabled")?;
    let entries = record_array(stdout)?;
    let matches: Vec<_> = entries
        .iter()
        .filter(|entry| read_string(entry, "name") == Some(GROK_PLUGIN_NAME))
        .collect();
    let result = |state, plugin_present| {
        Some(PluginInspection {
            marketplace: Marketplace::Absent,
            plugin_present,
            state,
        })
    };
    let plugin = match matches.as_slice() {
        [] => return result(State::NotInstalled, false),
        [plugin] => *plugin,
        _ => return result(State::Conflict, false),
    };
    if read_string(plugin, "status") != Some("installed") {
        return None;
    }
    let source = Path::new(read_string(plugin, "source")?);
    if source.file_name().and_then(|name| name.to_str()) != Some("bundle") {
        return result(State::Conflict, true);
    }
    let ownership = classify_resource_root(source.parent()?.to_str()?, options);
    match ownership {
        ResourceOwnership::Foreign => return result(State::Conflict, true),
        ResourceOwnership::Newer => return result(State::StaleSide, true),
        _ => {}
    }
    let version = windows_plugin_version(plugin, options)?;
    if version == State::StaleSide {
        return result(version, true);
    }
    if ownership == ResourceOwnership::Legacy || version == State::UpdateRequired {
        return result(State::UpdateRequired, true);
    }
    result(grok_activation(&configuration, plugin)?, true)
}

/// A self-consistent directory marker proves ownership, not the copied payload
/// version. Refuse a newer payload even when the marker contract is older.
pub(super) fn inspect_extension_version(
    target: &Path,
    id: IntegrationId,
    state: TargetState,
    options: &ResourceOptions,
) -> TargetState {
    if !matches!(state, TargetState::Current | TargetState::UpdateRequired) {
        return state;
    }
    let name = match id {
        IntegrationId::Gemini => "gemini-extension.json",
        IntegrationId::QwenCode => "qwen-extension.json",
        _ => return TargetState::Unknown,
    };
    let Some(value) = super::read_json_record(&target.join(name), 512 * 1024) else {
        return TargetState::Unknown;
    };
    if read_string(&value, "name") != Some("orchbee-agent-observation") {
        return TargetState::Conflict;
    }
    match windows_plugin_version(&value, options) {
        Some(State::Enabled) => state,
        Some(State::UpdateRequired) => TargetState::UpdateRequired,
        Some(State::StaleSide) => TargetState::Newer,
        _ => TargetState::Unknown,
    }
}
