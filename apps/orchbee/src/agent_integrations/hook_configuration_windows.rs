//! Provider-owned hook configuration: exact launcher ownership, bounded reads
//! and state classification shared by JSON and syntax-preserving Kimi TOML.
use std::{
    collections::BTreeMap,
    fmt, io,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_core::{
    Deserialize, Deserializer,
    de::{Error as _, MapAccess, Visitor},
};
use serde_json::{Value, json, value::RawValue};

use super::{
    files,
    inspect::{ResourceOptions, ResourceOwnership, classify_resource_root},
    model::State,
};

mod event_sets;
mod kimi;
use crate::agent_configuration_windows::{
    HookConfigurationKind, HookConfigurationTarget, hook_configuration_candidates,
};

pub(super) const LIMIT: u64 = 512 * 1024;
const PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ";
const MARKER: &str = "# orchbee-agent-observation:";
const ROOT: &str = "$env:ORCHBEE_AGENT_HOOK_ROOT='";
const NODE: &str = r#"(async()=>{try{const root=Buffer.from(process.env.ORCHBEE_AGENT_HOOK_ROOT||'','base64').toString('utf8');if(!require('node:path').isAbsolute(root))return;const file=require('node:path').join(root,'bundle','scripts','forward-hook.mjs');process.argv=[process.execPath,file,'cursor'];await import(require('node:url').pathToFileURL(file).href)}catch{}})()"#;

/// The edited object rejects duplicate keys; unrelated values remain opaque.
#[derive(Default)]
struct Object(BTreeMap<String, Box<RawValue>>);
impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = Object;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON object without duplicate members")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Object, M::Error> {
                let mut result = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
                    if result.insert(key, value).is_some() {
                        return Err(M::Error::custom("duplicate configuration member"));
                    }
                }
                Ok(Object(result))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}
impl Object {
    fn bytes(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(&self.0).ok()
    }
    fn set(&mut self, key: &str, value: impl serde_core::Serialize) -> Option<()> {
        self.0
            .insert(key.into(), serde_json::value::to_raw_value(&value).ok()?);
        Some(())
    }
}

fn declaration(id: &str, root: &Path) -> Option<Value> {
    if !matches!(id, "mastracode" | "devin") {
        event_name(id)?;
    }
    let mut entry = json!({"command": command(id, root)?, "timeout":if matches!(id,"mastracode"|"letta") {10000} else {10}});
    if id == "letta" {
        entry["quiet"] = json!(true);
    }
    if id != "cursor" {
        entry["type"] = json!("command");
        if id == "qodercli" {
            entry["shell"] = json!("powershell");
        }
    }
    Some(entry)
}

fn command(id: &str, root: &Path) -> Option<String> {
    let node = NODE.replace("'cursor'", &format!("'{id}'"));
    let encoded_root = STANDARD.encode(root.to_str()?.as_bytes());
    let output = if id == "letta" {
        ""
    } else {
        "[Console]::Out.Write('{}')\n"
    };
    let script = format!(
        "{MARKER}{id}\n{ROOT}{encoded_root}';\ntry {{ & node -e \"{node}\" 2>$null | Out-Null }} catch {{}}\n{output}"
    );
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    Some(format!("{PREFIX}{}", STANDARD.encode(utf16)))
}

/// Exact round trip proves the complete owned entry, including its launcher.
fn entry_root(id: &str, entry: &RawValue) -> Result<Option<PathBuf>, ()> {
    let object: Object = serde_json::from_str(entry.get()).map_err(|_| ())?;
    let Some(command) = object.0.get("command") else {
        return Ok(None);
    };
    let command: String = match serde_json::from_str(command.get()) {
        Ok(command) => command,
        Err(_) => return Err(()),
    };
    let Some(root) = command_root(id, &command)? else {
        return Ok(None);
    };
    let actual: Value = serde_json::from_str(entry.get()).map_err(|_| ())?;
    if declaration(id, &root).as_ref() != Some(&actual) {
        return Err(());
    }
    Ok(Some(root))
}

/// A complete fixed launcher is proof; the marker alone is never sufficient.
fn command_root(id: &str, command: &str) -> Result<Option<PathBuf>, ()> {
    let Some(encoded) = command.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    let bytes = match STANDARD.decode(encoded) {
        Ok(bytes) => bytes,
        Err(_) => return Err(()),
    };
    if bytes.len() % 2 != 0 {
        return Err(());
    }
    let words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let script = String::from_utf16(&words).map_err(|_| ())?;
    if !script.starts_with("# orchbee-agent-observation:") {
        return Ok(None);
    }
    let encoded_root = script
        .strip_prefix(&format!("{MARKER}{id}\n"))
        .and_then(|s| s.strip_prefix(ROOT))
        .and_then(|s| s.split_once("';\n"))
        .map(|(root, _)| root)
        .ok_or(())?;
    let root = PathBuf::from(
        String::from_utf8(STANDARD.decode(encoded_root).map_err(|_| ())?).map_err(|_| ())?,
    );
    if !files::path_admitted(&root) {
        return Err(());
    }
    if self::command(id, &root).as_deref() != Some(command) {
        return Err(());
    }
    Ok(Some(root))
}

fn event_name(id: &str) -> Option<&'static str> {
    match id {
        "cursor" => Some("sessionStart"),
        "qodercli" | "droid" | "letta" => Some("SessionStart"),
        "antigravity-cli" => Some("PreInvocation"),
        _ => None,
    }
}

// Qoder owns one complete generated group; unrelated groups remain raw.
fn item_root(id: &str, value: &RawValue) -> Result<Option<PathBuf>, ()> {
    if id == "cursor" {
        return entry_root(id, value);
    }
    let group: Object = serde_json::from_str(value.get()).map_err(|_| ())?;
    let raw = group.0.get("hooks").ok_or(())?;
    let entries: Vec<Box<RawValue>> = serde_json::from_str(raw.get()).map_err(|_| ())?;
    let mut owned = None;
    for entry in &entries {
        if let Some(root) = entry_root(id, entry)? {
            if owned.is_some() || group.0.len() != 1 || entries.len() != 1 {
                return Err(());
            }
            owned = Some(root);
        }
    }
    Ok(owned)
}

pub(crate) enum HookConfigurationScan {
    Absent,
    Owned(PathBuf),
    Incomplete(PathBuf),
    Disabled(PathBuf),
    Conflict,
    Malformed,
}

const NAMED_HOOK: &str = "orchbee-agent-observation";

struct NamedDocument {
    object: Object,
    owned: Option<(PathBuf, bool)>,
}

fn parse_named(bytes: &[u8]) -> Result<NamedDocument, HookConfigurationScan> {
    let object: Object =
        serde_json::from_slice(bytes).map_err(|_| HookConfigurationScan::Malformed)?;
    let Some(raw) = object.0.get(NAMED_HOOK) else {
        return Ok(NamedDocument {
            object,
            owned: None,
        });
    };
    let block: Object =
        serde_json::from_str(raw.get()).map_err(|_| HookConfigurationScan::Malformed)?;
    if block
        .0
        .keys()
        .any(|key| !matches!(key.as_str(), "PreInvocation" | "enabled"))
    {
        return Err(HookConfigurationScan::Conflict);
    }
    let enabled = match block.0.get("enabled") {
        Some(raw) => {
            serde_json::from_str::<bool>(raw.get()).map_err(|_| HookConfigurationScan::Malformed)?
        }
        None => true,
    };
    let events = block
        .0
        .get("PreInvocation")
        .ok_or(HookConfigurationScan::Conflict)?;
    let entries: Vec<Box<RawValue>> =
        serde_json::from_str(events.get()).map_err(|_| HookConfigurationScan::Malformed)?;
    let [entry] = entries.as_slice() else {
        return Err(HookConfigurationScan::Conflict);
    };
    let root = entry_root("antigravity-cli", entry)
        .map_err(|_| HookConfigurationScan::Conflict)?
        .ok_or(HookConfigurationScan::Conflict)?;
    Ok(NamedDocument {
        object,
        owned: Some((root, enabled)),
    })
}

fn scan_named(bytes: &[u8]) -> HookConfigurationScan {
    match parse_named(bytes) {
        Ok(doc) => match doc.owned {
            Some((root, true)) => HookConfigurationScan::Owned(root),
            Some((root, false)) => HookConfigurationScan::Disabled(root),
            None => HookConfigurationScan::Absent,
        },
        Err(state) => state,
    }
}

fn patch_named(bytes: Option<&[u8]>, root: &Path, enable: bool) -> Option<Vec<u8>> {
    let mut doc = parse_named(bytes.unwrap_or(b"{}")).ok()?;
    if enable {
        doc.object.set(
            NAMED_HOOK,
            json!({"enabled":true,"PreInvocation":[declaration("antigravity-cli",root)?]}),
        )?;
    } else {
        doc.object.0.remove(NAMED_HOOK);
    }
    let bytes = doc.object.bytes()?;
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

struct Document {
    object: Object,
    hooks: Object,
    entries: Vec<Box<RawValue>>,
    owned: Option<(usize, PathBuf)>,
}

fn parse(kind: HookConfigurationKind, bytes: &[u8]) -> Result<Document, HookConfigurationScan> {
    let id = kind.id();
    let own_event = event_name(id).ok_or(HookConfigurationScan::Malformed)?;
    let mut object: Object =
        serde_json::from_slice(bytes).map_err(|_| HookConfigurationScan::Malformed)?;
    if id == "cursor" && object.0.get("version").is_some_and(|v| v.get() != "1") {
        return Err(HookConfigurationScan::Malformed);
    }
    let hooks: Object = if kind == HookConfigurationKind::DroidStandalone {
        std::mem::take(&mut object)
    } else {
        match object.0.get("hooks") {
            Some(value) => {
                serde_json::from_str(value.get()).map_err(|_| HookConfigurationScan::Malformed)?
            }
            None => Object::default(),
        }
    };
    let mut entries = Vec::new();
    let mut owned = None;
    for (event, raw) in &hooks.0 {
        if matches!(id, "droid" | "letta")
            && ![
                "PreToolUse",
                "PostToolUse",
                "UserPromptSubmit",
                "Notification",
                "Stop",
                "SubagentStop",
                "PreCompact",
                "SessionStart",
                "SessionEnd",
            ]
            .contains(&event.as_str())
            && !(id == "letta"
                && ["PostToolUseFailure", "PermissionRequest"].contains(&event.as_str()))
        {
            continue;
        }

        let values: Vec<Box<RawValue>> =
            serde_json::from_str(raw.get()).map_err(|_| HookConfigurationScan::Malformed)?;
        for (index, value) in values.iter().enumerate() {
            if let Some(root) = item_root(id, value).map_err(|_| HookConfigurationScan::Conflict)? {
                if event != own_event || owned.is_some() {
                    return Err(HookConfigurationScan::Conflict);
                }
                owned = Some((index, root));
            }
        }
        if event == own_event {
            entries = values;
        }
    }
    Ok(Document {
        object,
        hooks,
        entries,
        owned,
    })
}

/// Budgeted doctor and onboarding share the same pure ownership classifier.
pub(crate) fn scan_hook_configuration(
    kind: HookConfigurationKind,
    bytes: &[u8],
) -> HookConfigurationScan {
    if kind == HookConfigurationKind::Kimi {
        return kimi::scan(bytes);
    }
    let id = kind.id();
    if id == "mastracode" {
        return event_sets::scan(event_sets::EventSet::Mastracode, bytes);
    }
    if id == "devin" {
        return event_sets::scan(event_sets::EventSet::Devin, bytes);
    }
    if id == "antigravity-cli" {
        return scan_named(bytes);
    }
    match parse(kind, bytes) {
        Ok(document) => match document.owned {
            Some((_, root)) => HookConfigurationScan::Owned(root),
            None => HookConfigurationScan::Absent,
        },
        Err(state) => state,
    }
}

pub(super) fn read(path: &Path) -> Result<Option<Vec<u8>>, io::Error> {
    match files::read_bounded(path, LIMIT) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn state(
    kind: HookConfigurationKind,
    bytes: Option<&[u8]>,
    options: &ResourceOptions,
) -> State {
    let Some(bytes) = bytes else {
        return State::NotInstalled;
    };
    let scan = scan_hook_configuration(kind, bytes);
    let disabled = matches!(&scan, HookConfigurationScan::Disabled(_));
    let incomplete = matches!(&scan, HookConfigurationScan::Incomplete(_));
    match scan {
        HookConfigurationScan::Absent => State::NotInstalled,
        HookConfigurationScan::Conflict => State::Conflict,
        HookConfigurationScan::Malformed => State::Unknown,
        HookConfigurationScan::Owned(root)
        | HookConfigurationScan::Disabled(root)
        | HookConfigurationScan::Incomplete(root) => {
            match classify_resource_root(&root.to_string_lossy(), options) {
                ResourceOwnership::Current => {
                    if incomplete {
                        State::UpdateRequired
                    } else if disabled {
                        State::Disabled
                    } else {
                        State::Enabled
                    }
                }
                ResourceOwnership::Legacy => State::UpdateRequired,
                ResourceOwnership::Newer => State::StaleSide,
                ResourceOwnership::Foreign => State::Conflict,
            }
        }
    }
}

/// Bounded source selection. If all candidates are absent, create the preferred target;
/// an unsafe/unreadable earlier candidate stops discovery instead of being skipped.
pub(super) fn load(
    id: &str,
    home: &Path,
) -> io::Result<(HookConfigurationTarget, Option<Vec<u8>>)> {
    let candidates = hook_configuration_candidates(id, home)
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let first = candidates
        .first()
        .cloned()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    for target in candidates {
        if let Some(bytes) = read(&target.path)? {
            return Ok((target, Some(bytes)));
        }
    }
    Ok((first, None))
}
pub(super) fn inspect(id: &str, home: &Path, options: &ResourceOptions) -> State {
    match load(id, home) {
        Ok((target, bytes)) => state(target.kind, bytes.as_deref(), options),
        Err(_) => State::Unknown,
    }
}

pub(super) fn patch(
    kind: HookConfigurationKind,
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
) -> Option<Vec<u8>> {
    if kind == HookConfigurationKind::Kimi {
        return kimi::patch(bytes, root, enable);
    }
    let id = kind.id();
    if id == "mastracode" {
        return event_sets::patch(event_sets::EventSet::Mastracode, bytes, root, enable);
    }
    if id == "devin" {
        return event_sets::patch(event_sets::EventSet::Devin, bytes, root, enable);
    }
    if id == "antigravity-cli" {
        return patch_named(bytes, root, enable);
    }
    let mut doc = parse(kind, bytes.unwrap_or(b"{}")).ok()?;
    if let Some((index, _)) = doc.owned {
        doc.entries.remove(index);
    }
    if enable {
        let entry = declaration(id, root)?;
        let item = if matches!(
            kind,
            HookConfigurationKind::Qoder
                | HookConfigurationKind::Letta
                | HookConfigurationKind::DroidStandalone
                | HookConfigurationKind::DroidSettings
        ) {
            json!({"hooks":[entry]})
        } else {
            entry
        };
        doc.entries
            .push(serde_json::value::to_raw_value(&item).ok()?);
    }
    doc.hooks.set(event_name(id)?, doc.entries)?;
    if kind == HookConfigurationKind::DroidStandalone {
        let bytes = doc.hooks.bytes()?;
        return (bytes.len() as u64 <= LIMIT).then_some(bytes);
    }
    doc.object.set("hooks", doc.hooks.0)?;
    if id == "cursor" && !doc.object.0.contains_key("version") {
        doc.object.set("version", 1)?;
    }
    let bytes = doc.object.bytes()?;
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

#[cfg(test)]
mod tests;
