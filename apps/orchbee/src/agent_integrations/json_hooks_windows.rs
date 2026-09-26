//! Provider-owned JSON hook declarations. Untouched values stay raw: user
//! numbers, future fields and unrelated hook bodies never pass through f64.
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
    event_name(id)?;
    let node = NODE.replace("'cursor'", &format!("'{id}'"));
    let encoded_root = STANDARD.encode(root.to_str()?.as_bytes());
    let script = format!(
        "{MARKER}{id}\n{ROOT}{encoded_root}';\ntry {{ & node -e \"{node}\" 2>$null | Out-Null }} catch {{}}\n[Console]::Out.Write('{{}}')\n"
    );
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut entry = json!({"command":format!("{PREFIX}{}",STANDARD.encode(utf16)),"timeout":10});
    if id != "cursor" {
        entry["type"] = json!("command");
        if id == "qodercli" {
            entry["shell"] = json!("powershell");
        }
    }
    Some(entry)
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
    let actual: Value = serde_json::from_str(entry.get()).map_err(|_| ())?;
    if declaration(id, &root).as_ref() != Some(&actual) {
        return Err(());
    }
    Ok(Some(root))
}

fn event_name(id: &str) -> Option<&'static str> {
    match id {
        "cursor" => Some("sessionStart"),
        "qodercli" => Some("SessionStart"),
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

pub(crate) enum JsonHookScan {
    Absent,
    Owned(PathBuf),
    Disabled(PathBuf),
    Conflict,
    Malformed,
}

const NAMED_HOOK: &str = "orchbee-agent-observation";

struct NamedDocument {
    object: Object,
    owned: Option<(PathBuf, bool)>,
}

fn parse_named(bytes: &[u8]) -> Result<NamedDocument, JsonHookScan> {
    let object: Object = serde_json::from_slice(bytes).map_err(|_| JsonHookScan::Malformed)?;
    let Some(raw) = object.0.get(NAMED_HOOK) else {
        return Ok(NamedDocument {
            object,
            owned: None,
        });
    };
    let block: Object = serde_json::from_str(raw.get()).map_err(|_| JsonHookScan::Malformed)?;
    if block
        .0
        .keys()
        .any(|key| !matches!(key.as_str(), "PreInvocation" | "enabled"))
    {
        return Err(JsonHookScan::Conflict);
    }
    let enabled = match block.0.get("enabled") {
        Some(raw) => {
            serde_json::from_str::<bool>(raw.get()).map_err(|_| JsonHookScan::Malformed)?
        }
        None => true,
    };
    let events = block.0.get("PreInvocation").ok_or(JsonHookScan::Conflict)?;
    let entries: Vec<Box<RawValue>> =
        serde_json::from_str(events.get()).map_err(|_| JsonHookScan::Malformed)?;
    let [entry] = entries.as_slice() else {
        return Err(JsonHookScan::Conflict);
    };
    let root = entry_root("antigravity-cli", entry)
        .map_err(|_| JsonHookScan::Conflict)?
        .ok_or(JsonHookScan::Conflict)?;
    Ok(NamedDocument {
        object,
        owned: Some((root, enabled)),
    })
}

fn scan_named(bytes: &[u8]) -> JsonHookScan {
    match parse_named(bytes) {
        Ok(doc) => match doc.owned {
            Some((root, true)) => JsonHookScan::Owned(root),
            Some((root, false)) => JsonHookScan::Disabled(root),
            None => JsonHookScan::Absent,
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

fn parse(id: &str, bytes: &[u8]) -> Result<Document, JsonHookScan> {
    let own_event = event_name(id).ok_or(JsonHookScan::Malformed)?;
    let object: Object = serde_json::from_slice(bytes).map_err(|_| JsonHookScan::Malformed)?;
    if id == "cursor" && object.0.get("version").is_some_and(|v| v.get() != "1") {
        return Err(JsonHookScan::Malformed);
    }
    let hooks: Object = match object.0.get("hooks") {
        Some(value) => serde_json::from_str(value.get()).map_err(|_| JsonHookScan::Malformed)?,
        None => Object::default(),
    };
    let mut entries = Vec::new();
    let mut owned = None;
    for (event, raw) in &hooks.0 {
        let values: Vec<Box<RawValue>> =
            serde_json::from_str(raw.get()).map_err(|_| JsonHookScan::Malformed)?;
        for (index, value) in values.iter().enumerate() {
            if let Some(root) = item_root(id, value).map_err(|_| JsonHookScan::Conflict)? {
                if event != own_event || owned.is_some() {
                    return Err(JsonHookScan::Conflict);
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
pub(crate) fn scan_json_hooks(id: &str, bytes: &[u8]) -> JsonHookScan {
    if id == "antigravity-cli" {
        return scan_named(bytes);
    }
    match parse(id, bytes) {
        Ok(document) => match document.owned {
            Some((_, root)) => JsonHookScan::Owned(root),
            None => JsonHookScan::Absent,
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

pub(super) fn state(id: &str, bytes: Option<&[u8]>, options: &ResourceOptions) -> State {
    let Some(bytes) = bytes else {
        return State::NotInstalled;
    };
    let scan = scan_json_hooks(id, bytes);
    let disabled = matches!(&scan, JsonHookScan::Disabled(_));
    match scan {
        JsonHookScan::Absent => State::NotInstalled,
        JsonHookScan::Conflict => State::Conflict,
        JsonHookScan::Malformed => State::Unknown,
        JsonHookScan::Owned(root) | JsonHookScan::Disabled(root) => {
            match classify_resource_root(&root.to_string_lossy(), options) {
                ResourceOwnership::Current => {
                    if disabled {
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

pub(super) fn inspect(id: &str, home: &Path, options: &ResourceOptions) -> State {
    let Some(path) = crate::agent_configuration_windows::json_hooks(id, home) else {
        return State::Unknown;
    };
    match read(&path) {
        Ok(bytes) => state(id, bytes.as_deref(), options),
        Err(_) => State::Unknown,
    }
}

pub(super) fn patch(id: &str, bytes: Option<&[u8]>, root: &Path, enable: bool) -> Option<Vec<u8>> {
    if id == "antigravity-cli" {
        return patch_named(bytes, root, enable);
    }
    let mut doc = parse(id, bytes.unwrap_or(b"{}")).ok()?;
    if let Some((index, _)) = doc.owned {
        doc.entries.remove(index);
    }
    if enable {
        let entry = declaration(id, root)?;
        let item = if id == "qodercli" {
            json!({"hooks":[entry]})
        } else {
            entry
        };
        doc.entries
            .push(serde_json::value::to_raw_value(&item).ok()?);
    }
    doc.hooks.set(event_name(id)?, doc.entries)?;
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
    fn raw_user_values_survive_enable_repair_and_disable() {
        let input = br#"{"large":123456789012345678901234567890,"nested":{"future":1e999,"duplicate":1,"duplicate":2},"hooks":{"stop":[{"command":"echo user"}],"sessionStart":[{"command":"echo before"}]}}"#;
        let root = Path::new(r"C:\Users\用户 & ' $ `\.orchbee\agent-observation-adapters\0.4.10");
        let installed = patch("cursor", Some(input), root, true).unwrap();
        assert!(
            matches!(scan_json_hooks("cursor", &installed), JsonHookScan::Owned(found) if found == root)
        );
        let repaired = patch("cursor", Some(&installed), root, true).unwrap();
        assert_eq!(repaired, installed);
        let disabled = patch("cursor", Some(&installed), root, false).unwrap();
        let document: Object = serde_json::from_slice(&disabled).unwrap();
        let original: Object = serde_json::from_slice(input).unwrap();
        for key in ["large", "nested"] {
            assert_eq!(document.0[key].get(), original.0[key].get());
        }
        let hooks: Value = serde_json::from_str(document.0["hooks"].get()).unwrap();
        assert_eq!(hooks["sessionStart"], json!([{"command":"echo before"}]));
        assert_eq!(hooks["stop"], json!([{"command":"echo user"}]));
        assert!(matches!(
            scan_json_hooks("cursor", &disabled),
            JsonHookScan::Absent
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
                scan_json_hooks("cursor", input.as_bytes()),
                JsonHookScan::Malformed
            ));
        }
        let root = Path::new(r"C:\owned\0.4.10");
        let bytes = patch("cursor", None, root, true).unwrap();
        let mut doc: Value = serde_json::from_slice(&bytes).unwrap();
        doc["hooks"]["sessionStart"][0]["timeout"] = json!(99);
        assert!(matches!(
            scan_json_hooks("cursor", &serde_json::to_vec(&doc).unwrap()),
            JsonHookScan::Conflict
        ));
        let entry = declaration("cursor", root).unwrap();
        for hooks in [
            json!({"stop":[entry]}),
            json!({"sessionStart":[entry,entry]}),
        ] {
            assert!(matches!(
                scan_json_hooks(
                    "cursor",
                    &serde_json::to_vec(&json!({"hooks":hooks})).unwrap()
                ),
                JsonHookScan::Conflict
            ));
        }
    }
}

#[test]
fn qoder_groups_keep_foreign_raw_values_and_refuse_edited_or_misplaced_ownership() {
    let root = Path::new(r"C:\owned\0.4.10");
    let input = br#"{"version":1e999,"hooks":{"SessionStart":[{"matcher":"resume","hooks":[{"type":"prompt","prompt":"keep","future":123456789012345678901234567890}]}]}}"#;
    let installed = patch("qodercli", Some(input), root, true).unwrap();
    assert!(
        matches!(scan_json_hooks("qodercli", &installed), JsonHookScan::Owned(found) if found == root)
    );
    let text = std::str::from_utf8(&installed).unwrap();
    assert!(text.contains("1e999") && text.contains("123456789012345678901234567890"));
    assert_eq!(
        patch("qodercli", Some(&installed), root, true).unwrap(),
        installed
    );
    let disabled = patch("qodercli", Some(&installed), root, false).unwrap();
    assert!(matches!(
        scan_json_hooks("qodercli", &disabled),
        JsonHookScan::Absent
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
            scan_json_hooks("qodercli", &bytes),
            JsonHookScan::Conflict
        ));
        assert!(patch("qodercli", Some(&bytes), root, false).is_none());
    }
    for input in [
        r#"{"hooks":{"SessionStart":[],"SessionStart":[]}}"#,
        r#"{"hooks":{"SessionStart":[{"hooks":[],"hooks":[]}]}}"#,
        r#"{"hooks":{"SessionStart":false}}"#,
    ] {
        assert!(patch("qodercli", Some(input.as_bytes()), root, true).is_none());
    }
}

#[test]
fn antigravity_named_blocks_preserve_foreign_values_and_validate_disabled_ownership() {
    let id = "antigravity-cli";
    let root = Path::new(r"C:\owned\0.4.10");
    let options = ResourceOptions {
        resource_root: root,
        expected_version: "0.4.10",
        legacy_roots: &[],
    };
    for (version, expected) in [
        ("0.4.10", State::Disabled),
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
    assert!(matches!(scan_json_hooks(id, &installed), JsonHookScan::Owned(found) if found == root));
    assert!(std::str::from_utf8(&installed).unwrap().contains("1e999"));
    assert_eq!(patch(id, Some(&installed), root, false).unwrap(), input);
    let owned = patch(id, None, root, true).unwrap();
    let mut value: Value = serde_json::from_slice(&owned).unwrap();
    value["orchbee-agent-observation"]["enabled"] = json!(false);
    assert!(
        matches!(scan_json_hooks(id, &serde_json::to_vec(&value).unwrap()), JsonHookScan::Disabled(found) if found == root)
    );
    value["orchbee-agent-observation"]["enabled"] = json!(null);
    assert!(matches!(
        scan_json_hooks(id, &serde_json::to_vec(&value).unwrap()),
        JsonHookScan::Malformed
    ));
    value["orchbee-agent-observation"]["enabled"] = json!(true);
    value["orchbee-agent-observation"]["PreInvocation"][0]["timeout"] = json!(99);
    assert!(matches!(
        scan_json_hooks(id, &serde_json::to_vec(&value).unwrap()),
        JsonHookScan::Conflict
    ));
    for input in [
        r#"{"orchbee-agent-observation":{}}"#,
        r#"{"orchbee-agent-observation":{"PreInvocation":[{"command":"echo foreign"}]}}"#,
        r#"{"orchbee-agent-observation":{},"orchbee-agent-observation":{}}"#,
    ] {
        assert!(patch(id, Some(input.as_bytes()), root, true).is_none());
    }
}
