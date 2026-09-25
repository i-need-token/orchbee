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
const MARKER: &str = "# orchbee-agent-observation:cursor\n";
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

fn declaration(root: &Path) -> Option<Value> {
    let encoded_root = STANDARD.encode(root.to_str()?.as_bytes());
    let script = format!(
        "{MARKER}{ROOT}{encoded_root}';\ntry {{ & node -e \"{NODE}\" 2>$null | Out-Null }} catch {{}}\n[Console]::Out.Write('{{}}')\n"
    );
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    Some(json!({"command":format!("{PREFIX}{}",STANDARD.encode(utf16)),"timeout":10}))
}

/// Exact round trip proves the complete owned entry, including its launcher.
fn entry_root(entry: &RawValue) -> Result<Option<PathBuf>, ()> {
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
        .strip_prefix(MARKER)
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
    if declaration(&root).as_ref() != Some(&actual) {
        return Err(());
    }
    Ok(Some(root))
}

pub(crate) enum JsonHookScan {
    Absent,
    Owned(PathBuf),
    Conflict,
    Malformed,
}

struct Document {
    object: Object,
    hooks: Object,
    entries: Vec<Box<RawValue>>,
    owned: Option<(usize, PathBuf)>,
}

fn parse(bytes: &[u8]) -> Result<Document, JsonHookScan> {
    let object: Object = serde_json::from_slice(bytes).map_err(|_| JsonHookScan::Malformed)?;
    if object.0.get("version").is_some_and(|v| v.get() != "1") {
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
            if let Some(root) = entry_root(value).map_err(|_| JsonHookScan::Conflict)? {
                if event != "sessionStart" || owned.is_some() {
                    return Err(JsonHookScan::Conflict);
                }
                owned = Some((index, root));
            }
        }
        if event == "sessionStart" {
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
pub(crate) fn scan_json_hooks(bytes: &[u8]) -> JsonHookScan {
    match parse(bytes) {
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

pub(super) fn state(bytes: Option<&[u8]>, options: &ResourceOptions) -> State {
    let Some(bytes) = bytes else {
        return State::NotInstalled;
    };
    match scan_json_hooks(bytes) {
        JsonHookScan::Absent => State::NotInstalled,
        JsonHookScan::Conflict => State::Conflict,
        JsonHookScan::Malformed => State::Unknown,
        JsonHookScan::Owned(root) => match classify_resource_root(&root.to_string_lossy(), options)
        {
            ResourceOwnership::Current => State::Enabled,
            ResourceOwnership::Legacy => State::UpdateRequired,
            ResourceOwnership::Newer => State::StaleSide,
            ResourceOwnership::Foreign => State::Conflict,
        },
    }
}

pub(super) fn inspect(home: &Path, options: &ResourceOptions) -> State {
    let Some(path) = crate::agent_configuration_windows::cursor_hooks(home) else {
        return State::Unknown;
    };
    match read(&path) {
        Ok(bytes) => state(bytes.as_deref(), options),
        Err(_) => State::Unknown,
    }
}

pub(super) fn patch(bytes: Option<&[u8]>, root: &Path, enable: bool) -> Option<Vec<u8>> {
    let mut doc = parse(bytes.unwrap_or(b"{\"version\":1,\"hooks\":{}}")).ok()?;
    if let Some((index, _)) = doc.owned {
        doc.entries.remove(index);
    }
    if enable {
        doc.entries
            .push(serde_json::value::to_raw_value(&declaration(root)?).ok()?);
    }
    doc.hooks.set("sessionStart", doc.entries)?;
    doc.object.set("hooks", doc.hooks.0)?;
    if !doc.object.0.contains_key("version") {
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
        let root = Path::new(r"C:\Users\用户 & ' $ `\.orchbee\agent-observation-adapters\0.4.7");
        let installed = patch(Some(input), root, true).unwrap();
        assert!(matches!(scan_json_hooks(&installed), JsonHookScan::Owned(found) if found == root));
        let repaired = patch(Some(&installed), root, true).unwrap();
        assert_eq!(repaired, installed);
        let disabled = patch(Some(&installed), root, false).unwrap();
        let document: Object = serde_json::from_slice(&disabled).unwrap();
        let original: Object = serde_json::from_slice(input).unwrap();
        for key in ["large", "nested"] {
            assert_eq!(document.0[key].get(), original.0[key].get());
        }
        let hooks: Value = serde_json::from_str(document.0["hooks"].get()).unwrap();
        assert_eq!(hooks["sessionStart"], json!([{"command":"echo before"}]));
        assert_eq!(hooks["stop"], json!([{"command":"echo user"}]));
        assert!(matches!(scan_json_hooks(&disabled), JsonHookScan::Absent));
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
                scan_json_hooks(input.as_bytes()),
                JsonHookScan::Malformed
            ));
        }
        let root = Path::new(r"C:\owned\0.4.7");
        let bytes = patch(None, root, true).unwrap();
        let mut doc: Value = serde_json::from_slice(&bytes).unwrap();
        doc["hooks"]["sessionStart"][0]["timeout"] = json!(99);
        assert!(matches!(
            scan_json_hooks(&serde_json::to_vec(&doc).unwrap()),
            JsonHookScan::Conflict
        ));
        let entry = declaration(root).unwrap();
        for hooks in [
            json!({"stop":[entry]}),
            json!({"sessionStart":[entry,entry]}),
        ] {
            assert!(matches!(
                scan_json_hooks(&serde_json::to_vec(&json!({"hooks":hooks})).unwrap()),
                JsonHookScan::Conflict
            ));
        }
    }
}
