//! Multi-event flat hook maps for Mastracode. Own only exact generated entries;
//! foreign values remain raw and publication belongs to the parent workflow.
use super::{HookConfigurationScan, LIMIT, Object, declaration, entry_root};
use serde_json::value::RawValue;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "AgentStart",
    "AgentEnd",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "PermissionResult",
    "SubagentStart",
    "SubagentEnd",
];
struct Document {
    object: Object,
    entries: BTreeMap<String, Vec<Box<RawValue>>>,
    owned: BTreeMap<String, usize>,
    root: Option<PathBuf>,
}
fn parse(bytes: &[u8]) -> Result<Document, HookConfigurationScan> {
    let object: Object =
        serde_json::from_slice(bytes).map_err(|_| HookConfigurationScan::Malformed)?;
    let mut entries = BTreeMap::new();
    let mut owned = BTreeMap::new();
    let mut root = None;
    for (event, raw) in &object.0 {
        if !EVENTS.contains(&event.as_str())
            && !["Stop", "Interrupt", "Notification", "UserPromptSubmit"].contains(&event.as_str())
        {
            continue;
        }
        let values: Vec<Box<RawValue>> =
            serde_json::from_str(raw.get()).map_err(|_| HookConfigurationScan::Malformed)?;
        for (index, value) in values.iter().enumerate() {
            if let Some(found) =
                entry_root("mastracode", value).map_err(|_| HookConfigurationScan::Conflict)?
            {
                if !EVENTS.contains(&event.as_str())
                    || owned.insert(event.clone(), index).is_some()
                    || root.as_ref().is_some_and(|previous| previous != &found)
                {
                    return Err(HookConfigurationScan::Conflict);
                }
                root = Some(found);
            }
        }
        entries.insert(event.clone(), values);
    }
    Ok(Document {
        object,
        entries,
        owned,
        root,
    })
}
pub(super) fn scan(bytes: &[u8]) -> HookConfigurationScan {
    match parse(bytes) {
        Ok(doc) => match doc.root {
            None => HookConfigurationScan::Absent,
            Some(root) if doc.owned.len() == EVENTS.len() => HookConfigurationScan::Owned(root),
            Some(root) => HookConfigurationScan::Incomplete(root),
        },
        Err(state) => state,
    }
}
pub(super) fn patch(bytes: Option<&[u8]>, root: &Path, enable: bool) -> Option<Vec<u8>> {
    let mut doc = parse(bytes.unwrap_or(b"{}")).ok()?;
    for event in EVENTS {
        let mut entries = doc.entries.remove(*event).unwrap_or_default();
        if let Some(index) = doc.owned.get(*event) {
            entries.remove(*index);
        }
        if enable {
            entries.push(serde_json::value::to_raw_value(&declaration("mastracode", root)?).ok()?);
        }
        if entries.is_empty() {
            doc.object.0.remove(*event);
        } else {
            doc.object.set(event, entries)?;
        }
    }
    let bytes = doc.object.bytes()?;
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn complete_partial_conflicting_and_foreign_flat_hooks() {
        let root = Path::new(r"C:\owned\0.4.14");
        let input=br#"{"future":{"huge":1e999},"SessionStart":[{"type":"command","command":"echo foreign"}]}"#;
        let installed = patch(Some(input), root, true).unwrap();
        assert!(matches!(scan(&installed),HookConfigurationScan::Owned(found) if found==root));
        assert_eq!(installed, patch(Some(&installed), root, true).unwrap());
        let removed = patch(Some(&installed), root, false).unwrap();
        let original: Object = serde_json::from_slice(input).unwrap();
        let restored: Object = serde_json::from_slice(&removed).unwrap();
        assert_eq!(original.0["future"].get(), restored.0["future"].get());
        assert_eq!(
            original.0["SessionStart"].get(),
            restored.0["SessionStart"].get()
        );
        assert!(matches!(scan(&removed), HookConfigurationScan::Absent));
        let mut doc: Value = serde_json::from_slice(&patch(None, root, true).unwrap()).unwrap();
        doc.as_object_mut().unwrap().remove("AgentEnd");
        let partial = serde_json::to_vec(&doc).unwrap();
        assert!(matches!(scan(&partial),HookConfigurationScan::Incomplete(found) if found==root));
        assert!(matches!(
            scan(&patch(Some(&partial), root, true).unwrap()),
            HookConfigurationScan::Owned(_)
        ));
        let entry = declaration("mastracode", root).unwrap();
        assert_eq!(entry["timeout"], 10000);
        for wrong in [
            json!({"SessionStart":[entry,entry]}),
            json!({"Stop":[entry]}),
            json!({"SessionStart":[entry],"AgentStart":[declaration("mastracode",Path::new(r"C:\other\9.0.0")).unwrap()]}),
        ] {
            assert!(matches!(
                scan(&serde_json::to_vec(&wrong).unwrap()),
                HookConfigurationScan::Conflict
            ));
        }
        doc["SessionStart"][0]["timeout"] = json!(10);
        assert!(matches!(
            scan(&serde_json::to_vec(&doc).unwrap()),
            HookConfigurationScan::Conflict
        ));
        for bad in [
            br#"{"AgentStart":[],"AgentStart":[]}"#.as_slice(),
            br#"{"AgentEnd":null}"#,
        ] {
            assert!(matches!(scan(bad), HookConfigurationScan::Malformed));
        }
    }
}
