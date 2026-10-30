//! Pure provider hook document grammar and ownership proofs shared by offline diagnosis
//! and the Windows integration adapter. This owner performs no environment or filesystem IO.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{
    Deserialize, Deserializer,
    de::{Error as _, MapAccess, Visitor},
};
use serde_json::{Value, json, value::RawValue};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

pub(super) mod event_sets;
pub(super) mod kimi;

/// The actual provider document shape; both Droid origins share one adapter identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookConfigurationKind {
    Cursor,
    Qoder,
    Antigravity,
    Mastracode,
    DroidStandalone,
    DroidSettings,
    Kimi,
    Letta,
    Devin,
}
impl HookConfigurationKind {
    /// Canonical observation and integration identity independent of file representation.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Kimi => "kimi",
            Self::Letta => "letta",
            Self::Devin => "devin",
            Self::Cursor => "cursor",
            Self::Qoder => "qodercli",
            Self::Antigravity => "antigravity-cli",
            Self::Mastracode => "mastracode",
            Self::DroidStandalone | Self::DroidSettings => "droid",
        }
    }
}

/// Ordered document shapes for one provider; Droid's two standalone candidates precede settings.
pub fn configuration_kinds(id: &str) -> Option<&'static [HookConfigurationKind]> {
    use HookConfigurationKind as Kind;
    Some(match id {
        "droid" => &[
            Kind::DroidStandalone,
            Kind::DroidStandalone,
            Kind::DroidSettings,
        ],
        "kimi" => &[Kind::Kimi],
        "letta" => &[Kind::Letta],
        "devin" => &[Kind::Devin],
        "cursor" => &[Kind::Cursor],
        "qodercli" => &[Kind::Qoder],
        "antigravity-cli" => &[Kind::Antigravity],
        "mastracode" => &[Kind::Mastracode],
        _ => return None,
    })
}

/// The owning Windows adapter supplies exact path grammar and equality; the scanner never
/// interprets a decoded Windows root through the host's path rules.
#[derive(Clone, Copy)]
pub struct RootPolicy {
    pub(super) admitted: fn(&Path) -> bool,
    pub(super) same: fn(&Path, &Path) -> bool,
}

const PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ";
const MARKER: &str = "# orchbee-agent-observation:";
const ROOT: &str = "$env:ORCHBEE_AGENT_HOOK_ROOT='";
const NODE: &str = r#"(async()=>{try{const root=Buffer.from(process.env.ORCHBEE_AGENT_HOOK_ROOT||'','base64').toString('utf8');if(!require('node:path').isAbsolute(root))return;const file=require('node:path').join(root,'bundle','scripts','forward-hook.mjs');process.argv=[process.execPath,file,'cursor'];await import(require('node:url').pathToFileURL(file).href)}catch{}})()"#;

/// The edited object rejects duplicate keys; unrelated values remain opaque.
#[derive(Default)]
pub(super) struct Object(pub(super) BTreeMap<String, Box<RawValue>>);
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

pub(super) fn declaration(id: &str, root: &Path) -> Option<Value> {
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

pub(super) fn command(id: &str, root: &Path) -> Option<String> {
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

fn entry_root(id: &str, entry: &RawValue, policy: RootPolicy) -> Result<Option<PathBuf>, ()> {
    let object: Object = serde_json::from_str(entry.get()).map_err(|_| ())?;
    let Some(command) = object.0.get("command") else {
        return Ok(None);
    };
    let command: String = match serde_json::from_str(command.get()) {
        Ok(command) => command,
        Err(_) => return Err(()),
    };
    let Some(root) = command_root(id, &command, policy)? else {
        return Ok(None);
    };
    let actual: Value = serde_json::from_str(entry.get()).map_err(|_| ())?;
    if declaration(id, &root).as_ref() != Some(&actual) {
        return Err(());
    }
    Ok(Some(root))
}

fn command_root(id: &str, command: &str, policy: RootPolicy) -> Result<Option<PathBuf>, ()> {
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
    if !(policy.admitted)(&root) {
        return Err(());
    }
    if self::command(id, &root).as_deref() != Some(command) {
        return Err(());
    }
    Ok(Some(root))
}

pub(super) fn event_name(id: &str) -> Option<&'static str> {
    match id {
        "cursor" => Some("sessionStart"),
        "qodercli" | "droid" | "letta" => Some("SessionStart"),
        "antigravity-cli" => Some("PreInvocation"),
        _ => None,
    }
}

fn item_root(id: &str, value: &RawValue, policy: RootPolicy) -> Result<Option<PathBuf>, ()> {
    if id == "cursor" {
        return entry_root(id, value, policy);
    }
    let group: Object = serde_json::from_str(value.get()).map_err(|_| ())?;
    let raw = group.0.get("hooks").ok_or(())?;
    let entries: Vec<Box<RawValue>> = serde_json::from_str(raw.get()).map_err(|_| ())?;
    let mut owned = None;
    for entry in &entries {
        if let Some(root) = entry_root(id, entry, policy)? {
            if owned.is_some() || group.0.len() != 1 || entries.len() != 1 {
                return Err(());
            }
            owned = Some(root);
        }
    }
    Ok(owned)
}

pub enum HookConfigurationScan {
    Absent,
    Owned(PathBuf),
    Incomplete(PathBuf),
    Disabled(PathBuf),
    Conflict,
    Malformed,
}

pub(super) const NAMED_HOOK: &str = "orchbee-agent-observation";
/// Retains the original raw document for the mutation owner without reparsing it.
pub(super) type ParsedNamed = (Object, Option<(PathBuf, bool)>);
/// Raw parent, hook object, selected event entries and optional owned entry.
pub(super) type ParsedHooks = (Object, Object, Vec<Box<RawValue>>, Option<(usize, PathBuf)>);

pub(super) fn parse_named(
    bytes: &[u8],
    policy: RootPolicy,
) -> Result<ParsedNamed, HookConfigurationScan> {
    let object: Object =
        serde_json::from_slice(bytes).map_err(|_| HookConfigurationScan::Malformed)?;
    let Some(raw) = object.0.get(NAMED_HOOK) else {
        return Ok((object, None));
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
    let root = entry_root("antigravity-cli", entry, policy)
        .map_err(|_| HookConfigurationScan::Conflict)?
        .ok_or(HookConfigurationScan::Conflict)?;
    Ok((object, Some((root, enabled))))
}

fn scan_named(bytes: &[u8], policy: RootPolicy) -> HookConfigurationScan {
    match parse_named(bytes, policy) {
        Ok((_, owned)) => match owned {
            Some((root, true)) => HookConfigurationScan::Owned(root),
            Some((root, false)) => HookConfigurationScan::Disabled(root),
            None => HookConfigurationScan::Absent,
        },
        Err(state) => state,
    }
}

pub(super) fn parse(
    kind: HookConfigurationKind,
    bytes: &[u8],
    policy: RootPolicy,
) -> Result<ParsedHooks, HookConfigurationScan> {
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
            if let Some(root) =
                item_root(id, value, policy).map_err(|_| HookConfigurationScan::Conflict)?
            {
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
    Ok((object, hooks, entries, owned))
}

pub fn scan_hook_configuration(
    kind: HookConfigurationKind,
    bytes: &[u8],
    policy: RootPolicy,
) -> HookConfigurationScan {
    if kind == HookConfigurationKind::Kimi {
        return kimi::scan(bytes, policy);
    }
    let id = kind.id();
    if id == "mastracode" {
        return event_sets::scan(event_sets::EventSet::Mastracode, bytes, policy);
    }
    if id == "devin" {
        return event_sets::scan(event_sets::EventSet::Devin, bytes, policy);
    }
    if id == "antigravity-cli" {
        return scan_named(bytes, policy);
    }
    match parse(kind, bytes, policy) {
        Ok((_, _, _, owned)) => match owned {
            Some((_, root)) => HookConfigurationScan::Owned(root),
            None => HookConfigurationScan::Absent,
        },
        Err(state) => state,
    }
}
