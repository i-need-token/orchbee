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
mod tests {
    use super::*;
    #[test]
    fn letta_owns_a_silent_millisecond_hook_and_preserves_policy_and_foreign_values() {
        let root = Path::new(r"C:\owned\0.4.17");
        let input = br#"{"large":123456789012345678901234567890,"hooks":{"disabled":true,"future":{"number":1e999},"SessionStart":[{"hooks":[{"type":"prompt","prompt":"USER"}]}]}}"#;
        let installed = patch(HookConfigurationKind::Letta, Some(input), root, true).unwrap();
        assert!(
            matches!(scan_hook_configuration(HookConfigurationKind::Letta, &installed), HookConfigurationScan::Owned(found) if found == root)
        );
        let doc: Object = serde_json::from_slice(&installed).unwrap();
        let hooks: Object = serde_json::from_str(doc.0["hooks"].get()).unwrap();
        assert_eq!(hooks.0["disabled"].get(), "true");
        assert_eq!(hooks.0["future"].get(), r#"{"number":1e999}"#);
        let entries: Value = serde_json::from_str(hooks.0["SessionStart"].get()).unwrap();
        let entry = &entries[1]["hooks"][0];
        assert_eq!(entry["timeout"], 10000);
        assert_eq!(entry["quiet"], true);
        let command = entry["command"].as_str().unwrap();
        let bytes = STANDARD
            .decode(command.strip_prefix(PREFIX).unwrap())
            .unwrap();
        let words: Vec<_> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        assert!(!String::from_utf16(&words).unwrap().contains("Out.Write"));
        assert_eq!(
            patch(HookConfigurationKind::Letta, Some(&installed), root, true).unwrap(),
            installed
        );
        let disabled = patch(HookConfigurationKind::Letta, Some(&installed), root, false).unwrap();
        let disabled: Object = serde_json::from_slice(&disabled).unwrap();
        let original: Object = serde_json::from_slice(input).unwrap();
        assert_eq!(disabled.0["large"].get(), original.0["large"].get());
        let restored: Object = serde_json::from_str(disabled.0["hooks"].get()).unwrap();
        let original: Object = serde_json::from_str(original.0["hooks"].get()).unwrap();
        for key in ["disabled", "future", "SessionStart"] {
            assert_eq!(restored.0[key].get(), original.0[key].get());
        }
        let changed = String::from_utf8(installed)
            .unwrap()
            .replace("\"quiet\":true", "\"quiet\":false");
        assert!(matches!(
            scan_hook_configuration(HookConfigurationKind::Letta, changed.as_bytes()),
            HookConfigurationScan::Conflict
        ));
    }
    #[test]
    fn raw_user_values_survive_enable_repair_and_disable() {
        let input = br#"{"large":123456789012345678901234567890,"nested":{"future":1e999,"duplicate":1,"duplicate":2},"hooks":{"stop":[{"command":"echo user"}],"sessionStart":[{"command":"echo before"}]}}"#;
        let root = Path::new(r"C:\Users\用户 & ' $ `\.orchbee\agent-observation-adapters\0.4.17");
        let installed = patch(HookConfigurationKind::Cursor, Some(input), root, true).unwrap();
        assert!(
            matches!(scan_hook_configuration(HookConfigurationKind::Cursor, &installed), HookConfigurationScan::Owned(found) if found == root)
        );
        let repaired = patch(HookConfigurationKind::Cursor, Some(&installed), root, true).unwrap();
        assert_eq!(repaired, installed);
        let disabled = patch(HookConfigurationKind::Cursor, Some(&installed), root, false).unwrap();
        let document: Object = serde_json::from_slice(&disabled).unwrap();
        let original: Object = serde_json::from_slice(input).unwrap();
        for key in ["large", "nested"] {
            assert_eq!(document.0[key].get(), original.0[key].get());
        }
        let hooks: Value = serde_json::from_str(document.0["hooks"].get()).unwrap();
        assert_eq!(hooks["sessionStart"], json!([{"command":"echo before"}]));
        assert_eq!(hooks["stop"], json!([{"command":"echo user"}]));
        assert!(matches!(
            scan_hook_configuration(HookConfigurationKind::Cursor, &disabled),
            HookConfigurationScan::Absent
        ));
    }
    #[test]
    fn duplicated_edited_members_and_changed_owned_commands_are_refused() {
        for input in [
            r#"{"hooks":{},"hooks":{}}"#,
            r#"{"hooks":{"sessionStart":[],"sessionStart":[]}}"#,
            r#"{"version":2,"hooks":{}}"#,
            r#"{"hooks":{"sessionStart":null}}"#,
        ] {
            assert!(matches!(
                scan_hook_configuration(HookConfigurationKind::Cursor, input.as_bytes()),
                HookConfigurationScan::Malformed
            ));
        }
        let root = Path::new(r"C:\owned\0.4.17");
        let bytes = patch(HookConfigurationKind::Cursor, None, root, true).unwrap();
        let mut doc: Value = serde_json::from_slice(&bytes).unwrap();
        doc["hooks"]["sessionStart"][0]["timeout"] = json!(99);
        assert!(matches!(
            scan_hook_configuration(
                HookConfigurationKind::Cursor,
                &serde_json::to_vec(&doc).unwrap()
            ),
            HookConfigurationScan::Conflict
        ));
        let entry = declaration("cursor", root).unwrap();
        for hooks in [
            json!({"stop":[entry]}),
            json!({"sessionStart":[entry,entry]}),
        ] {
            assert!(matches!(
                scan_hook_configuration(
                    HookConfigurationKind::Cursor,
                    &serde_json::to_vec(&json!({"hooks":hooks})).unwrap()
                ),
                HookConfigurationScan::Conflict
            ));
        }
    }
}

#[test]
fn qoder_groups_keep_foreign_raw_values_and_refuse_edited_or_misplaced_ownership() {
    let root = Path::new(r"C:\owned\0.4.17");
    let input = br#"{"version":1e999,"hooks":{"SessionStart":[{"matcher":"resume","hooks":[{"type":"prompt","prompt":"keep","future":123456789012345678901234567890}]}]}}"#;
    let installed = patch(HookConfigurationKind::Qoder, Some(input), root, true).unwrap();
    assert!(
        matches!(scan_hook_configuration(HookConfigurationKind::Qoder, &installed), HookConfigurationScan::Owned(found) if found == root)
    );
    let text = std::str::from_utf8(&installed).unwrap();
    assert!(text.contains("1e999") && text.contains("123456789012345678901234567890"));
    assert_eq!(
        patch(HookConfigurationKind::Qoder, Some(&installed), root, true).unwrap(),
        installed
    );
    let disabled = patch(HookConfigurationKind::Qoder, Some(&installed), root, false).unwrap();
    assert!(matches!(
        scan_hook_configuration(HookConfigurationKind::Qoder, &disabled),
        HookConfigurationScan::Absent
    ));
    assert!(std::str::from_utf8(&disabled).unwrap().contains("1e999"));
    assert!(
        !std::str::from_utf8(&disabled)
            .unwrap()
            .contains("EncodedCommand")
    );
    let entry = declaration("qodercli", root).unwrap();
    let mut edited = entry.clone();
    edited["shell"] = json!("bash");
    for hooks in [
        json!({"Stop":[{"hooks":[entry.clone()]}]}),
        json!({"SessionStart":[{"hooks":[entry.clone()]},{"hooks":[entry.clone()]}]}),
        json!({"SessionStart":[{"matcher":"resume","hooks":[entry.clone()]}]}),
        json!({"SessionStart":[{"hooks":[entry.clone(),{"command":"echo keep"}]}]}),
        json!({"SessionStart":[{"hooks":[edited]}]}),
    ] {
        let bytes = serde_json::to_vec(&json!({"hooks":hooks})).unwrap();
        assert!(matches!(
            scan_hook_configuration(HookConfigurationKind::Qoder, &bytes),
            HookConfigurationScan::Conflict
        ));
        assert!(patch(HookConfigurationKind::Qoder, Some(&bytes), root, false).is_none());
    }
    for input in [
        r#"{"hooks":{"SessionStart":[],"SessionStart":[]}}"#,
        r#"{"hooks":{"SessionStart":[{"hooks":[],"hooks":[]}]}}"#,
        r#"{"hooks":{"SessionStart":false}}"#,
    ] {
        assert!(
            patch(
                HookConfigurationKind::Qoder,
                Some(input.as_bytes()),
                root,
                true
            )
            .is_none()
        );
    }
}

#[test]
fn antigravity_named_blocks_preserve_foreign_values_and_validate_disabled_ownership() {
    let id = HookConfigurationKind::Antigravity;
    let root = Path::new(r"C:\owned\0.4.17");
    let options = ResourceOptions {
        resource_root: root,
        expected_version: "0.4.17",
        legacy_roots: &[],
    };
    for (version, expected) in [
        ("0.4.17", State::Disabled),
        ("0.4.9", State::UpdateRequired),
        ("9.0.0", State::StaleSide),
    ] {
        let bytes = patch(id, None, &root.with_file_name(version), true).unwrap();
        let text = String::from_utf8(bytes)
            .unwrap()
            .replace("\"enabled\":true", "\"enabled\":false");
        assert_eq!(state(id, Some(text.as_bytes()), &options), expected);
    }
    let input = br#"{"user-hook":{"future":1e999,"enabled":false,"PreInvocation":[{"command":"echo user"}]}}"#;
    let installed = patch(id, Some(input), root, true).unwrap();
    assert!(
        matches!(scan_hook_configuration(id, &installed), HookConfigurationScan::Owned(found) if found == root)
    );
    assert!(std::str::from_utf8(&installed).unwrap().contains("1e999"));
    assert_eq!(patch(id, Some(&installed), root, false).unwrap(), input);
    let owned = patch(id, None, root, true).unwrap();
    let mut value: Value = serde_json::from_slice(&owned).unwrap();
    value["orchbee-agent-observation"]["enabled"] = json!(false);
    assert!(
        matches!(scan_hook_configuration(id, &serde_json::to_vec(&value).unwrap()), HookConfigurationScan::Disabled(found) if found == root)
    );
    value["orchbee-agent-observation"]["enabled"] = json!(null);
    assert!(matches!(
        scan_hook_configuration(id, &serde_json::to_vec(&value).unwrap()),
        HookConfigurationScan::Malformed
    ));
    value["orchbee-agent-observation"]["enabled"] = json!(true);
    value["orchbee-agent-observation"]["PreInvocation"][0]["timeout"] = json!(99);
    assert!(matches!(
        scan_hook_configuration(id, &serde_json::to_vec(&value).unwrap()),
        HookConfigurationScan::Conflict
    ));
    for input in [
        r#"{"orchbee-agent-observation":{}}"#,
        r#"{"orchbee-agent-observation":{"PreInvocation":[{"command":"echo foreign"}]}}"#,
        r#"{"orchbee-agent-observation":{},"orchbee-agent-observation":{}}"#,
    ] {
        assert!(patch(id, Some(input.as_bytes()), root, true).is_none());
    }
}

#[test]
fn droid_document_shapes_preserve_foreign_data_and_own_only_session_group() {
    let root = Path::new(r"C:\owned\0.4.17");
    for (kind,input) in [
        (HookConfigurationKind::DroidStandalone,br#"{"SessionStart":[{"matcher":"resume","hooks":[{"type":"command","command":"echo user"}]}],"future":{"big":1e999}}"#.as_slice()),
        (HookConfigurationKind::DroidSettings,br#"{"hooks":{"SessionStart":[{"matcher":"resume","hooks":[{"type":"command","command":"echo user"}]}]},"future":{"big":1e999}}"#.as_slice()),
    ] {
        let installed=patch(kind,Some(input),root,true).unwrap();
        assert!(matches!(scan_hook_configuration(kind,&installed),HookConfigurationScan::Owned(found) if found==root));
        assert_eq!(installed,patch(kind,Some(&installed),root,true).unwrap());
        let disabled=patch(kind,Some(&installed),root,false).unwrap();
        let original:Object=serde_json::from_slice(input).unwrap();
        let restored:Object=serde_json::from_slice(&disabled).unwrap();
        assert_eq!(restored.0["future"].get(),original.0["future"].get());
        assert!(matches!(scan_hook_configuration(kind,&disabled),HookConfigurationScan::Absent));
        let mut owned:Value=serde_json::from_slice(&patch(kind,None,root,true).unwrap()).unwrap();
        let hooks=if kind==HookConfigurationKind::DroidSettings {&mut owned["hooks"]} else {&mut owned};
        let entry=hooks["SessionStart"][0].clone();
        hooks["Stop"]=json!([entry]);
        assert!(matches!(scan_hook_configuration(kind,&serde_json::to_vec(&owned).unwrap()),HookConfigurationScan::Conflict));
    }
}
