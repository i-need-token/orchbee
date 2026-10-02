//! Closed provider event sets: flat Mastracode and grouped Devin hooks. Own only exact generated entries;
//! foreign values remain raw and publication belongs to the parent workflow.
use super::{HookConfigurationScan, LIMIT, Object, declaration, entry_root, item_root};
use serde_json::value::RawValue;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const MASTRACODE_EVENTS: &[&str] = &[
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
const DEVIN_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Stop",
];
#[derive(Clone, Copy)]
pub(super) enum EventSet {
    Mastracode,
    Devin,
}
impl EventSet {
    fn id(self) -> &'static str {
        match self {
            Self::Mastracode => "mastracode",
            Self::Devin => "devin",
        }
    }
    fn events(self) -> &'static [&'static str] {
        match self {
            Self::Mastracode => MASTRACODE_EVENTS,
            Self::Devin => DEVIN_EVENTS,
        }
    }
    fn extra_events(self) -> &'static [&'static str] {
        match self {
            Self::Mastracode => &["Stop", "Interrupt", "Notification", "UserPromptSubmit"],
            Self::Devin => &["SessionEnd", "PostCompaction"],
        }
    }
}
struct Document {
    parent: Option<Object>,
    object: Object,
    entries: BTreeMap<String, Vec<Box<RawValue>>>,
    owned: BTreeMap<String, usize>,
    root: Option<PathBuf>,
}
fn parse(set: EventSet, bytes: &[u8]) -> Result<Document, HookConfigurationScan> {
    let object: Object =
        serde_json::from_slice(bytes).map_err(|_| HookConfigurationScan::Malformed)?;
    let (parent, object) = match set {
        EventSet::Mastracode => (None, object),
        EventSet::Devin => {
            let hooks = object
                .0
                .get("hooks")
                .map(|raw| serde_json::from_str(raw.get()))
                .transpose()
                .map_err(|_| HookConfigurationScan::Malformed)?
                .unwrap_or_default();
            (Some(object), hooks)
        }
    };
    let mut entries = BTreeMap::new();
    let mut owned = BTreeMap::new();
    let mut root = None;
    for (event, raw) in &object.0 {
        if !set.events().contains(&event.as_str()) && !set.extra_events().contains(&event.as_str())
        {
            continue;
        }
        let values: Vec<Box<RawValue>> =
            serde_json::from_str(raw.get()).map_err(|_| HookConfigurationScan::Malformed)?;
        for (index, value) in values.iter().enumerate() {
            let found = match set {
                EventSet::Mastracode => entry_root(set.id(), value),
                EventSet::Devin => item_root(set.id(), value),
            }
            .map_err(|_| HookConfigurationScan::Conflict)?;
            if let Some(found) = found {
                if !set.events().contains(&event.as_str())
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
        parent,
        object,
        entries,
        owned,
        root,
    })
}
pub(super) fn scan(set: EventSet, bytes: &[u8]) -> HookConfigurationScan {
    match parse(set, bytes) {
        Ok(doc) => match doc.root {
            None => HookConfigurationScan::Absent,
            Some(root) if doc.owned.len() == set.events().len() => {
                HookConfigurationScan::Owned(root)
            }
            Some(root) => HookConfigurationScan::Incomplete(root),
        },
        Err(state) => state,
    }
}
pub(super) fn patch(
    set: EventSet,
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
) -> Option<Vec<u8>> {
    let mut doc = parse(set, bytes.unwrap_or(b"{}")).ok()?;
    for event in set.events() {
        let mut entries = doc.entries.remove(*event).unwrap_or_default();
        if let Some(index) = doc.owned.get(*event) {
            entries.remove(*index);
        }
        if enable {
            let entry = declaration(set.id(), root)?;
            let entry = match set {
                EventSet::Mastracode => entry,
                EventSet::Devin => serde_json::json!({"hooks":[entry]}),
            };
            entries.push(serde_json::value::to_raw_value(&entry).ok()?);
        }
        if entries.is_empty() {
            doc.object.0.remove(*event);
        } else {
            doc.object.set(event, entries)?;
        }
    }
    let bytes = if let Some(mut parent) = doc.parent {
        parent.set("hooks", doc.object.0)?;
        parent.bytes()?
    } else {
        doc.object.bytes()?
    };
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn complete_partial_conflicting_and_foreign_flat_hooks() {
        let root = Path::new(r"C:\owned\0.4.17");
        let input=br#"{"future":{"huge":1e999},"SessionStart":[{"type":"command","command":"echo foreign"}]}"#;
        let installed = patch(EventSet::Mastracode, Some(input), root, true).unwrap();
        assert!(
            matches!(scan(EventSet::Mastracode, &installed),HookConfigurationScan::Owned(found) if found==root)
        );
        assert_eq!(
            installed,
            patch(EventSet::Mastracode, Some(&installed), root, true).unwrap()
        );
        let removed = patch(EventSet::Mastracode, Some(&installed), root, false).unwrap();
        let original: Object = serde_json::from_slice(input).unwrap();
        let restored: Object = serde_json::from_slice(&removed).unwrap();
        assert_eq!(original.0["future"].get(), restored.0["future"].get());
        assert_eq!(
            original.0["SessionStart"].get(),
            restored.0["SessionStart"].get()
        );
        assert!(matches!(
            scan(EventSet::Mastracode, &removed),
            HookConfigurationScan::Absent
        ));
        let mut doc: Value =
            serde_json::from_slice(&patch(EventSet::Mastracode, None, root, true).unwrap())
                .unwrap();
        doc.as_object_mut().unwrap().remove("AgentEnd");
        let partial = serde_json::to_vec(&doc).unwrap();
        assert!(
            matches!(scan(EventSet::Mastracode, &partial),HookConfigurationScan::Incomplete(found) if found==root)
        );
        assert!(matches!(
            scan(
                EventSet::Mastracode,
                &patch(EventSet::Mastracode, Some(&partial), root, true).unwrap()
            ),
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
                scan(EventSet::Mastracode, &serde_json::to_vec(&wrong).unwrap()),
                HookConfigurationScan::Conflict
            ));
        }
        doc["SessionStart"][0]["timeout"] = json!(10);
        assert!(matches!(
            scan(EventSet::Mastracode, &serde_json::to_vec(&doc).unwrap()),
            HookConfigurationScan::Conflict
        ));
        for bad in [
            br#"{"AgentStart":[],"AgentStart":[]}"#.as_slice(),
            br#"{"AgentEnd":null}"#,
        ] {
            assert!(matches!(
                scan(EventSet::Mastracode, bad),
                HookConfigurationScan::Malformed
            ));
        }
    }
}

#[cfg(test)]
mod grouped_tests {
    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn devin_group_set_is_complete_repairable_and_preserves_foreign_settings() {
        let set = EventSet::Devin;
        let root = Path::new(r"C:\owned\0.4.17");
        let input = br#"{"user":123456789012345678901234567890,"hooks":{"future":{"number":1e999},"Stop":[{"matcher":"custom","hooks":[{"type":"prompt","prompt":"USER"}]}]}}"#;
        let installed = patch(set, Some(input), root, true).unwrap();
        assert!(matches!(scan(set,&installed),HookConfigurationScan::Owned(found) if found==root));
        assert_eq!(patch(set, Some(&installed), root, true).unwrap(), installed);
        let removed = patch(set, Some(&installed), root, false).unwrap();
        let original: Object = serde_json::from_slice(input).unwrap();
        let restored: Object = serde_json::from_slice(&removed).unwrap();
        assert_eq!(original.0["user"].get(), restored.0["user"].get());
        let original: Object = serde_json::from_str(original.0["hooks"].get()).unwrap();
        let restored: Object = serde_json::from_str(restored.0["hooks"].get()).unwrap();
        for key in ["future", "Stop"] {
            assert_eq!(original.0[key].get(), restored.0[key].get());
        }
        let mut doc: Value =
            serde_json::from_slice(&patch(set, None, root, true).unwrap()).unwrap();
        doc["hooks"].as_object_mut().unwrap().remove("PreToolUse");
        assert!(matches!(
            scan(set, &serde_json::to_vec(&doc).unwrap()),
            HookConfigurationScan::Incomplete(_)
        ));
        assert!(matches!(
            scan(
                set,
                &patch(set, Some(&serde_json::to_vec(&doc).unwrap()), root, true).unwrap()
            ),
            HookConfigurationScan::Owned(_)
        ));
        let entry = json!({"hooks":[declaration("devin",root).unwrap()]});
        for wrong in [
            json!({"SessionStart":[entry,entry]}),
            json!({"SessionEnd":[entry]}),
            json!({"SessionStart":[entry],"Stop":[{"hooks":[declaration("devin",Path::new(r"C:\other\0.4.17")).unwrap()]}]}),
        ] {
            assert!(matches!(
                scan(set, &serde_json::to_vec(&json!({"hooks":wrong})).unwrap()),
                HookConfigurationScan::Conflict
            ));
        }
        doc["hooks"]["Stop"][0]["matcher"] = json!("*");
        assert!(matches!(
            scan(set, &serde_json::to_vec(&doc).unwrap()),
            HookConfigurationScan::Conflict
        ));
    }
}
