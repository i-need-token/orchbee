//! Exact read-only ownership of flat Mastracode and grouped Devin event sets.
use super::{HookConfigurationScan, Object, RootPolicy, entry_root, item_root};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, path::PathBuf};

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
pub(in crate::agent_integrations) enum EventSet {
    Mastracode,
    Devin,
}
impl EventSet {
    pub(in crate::agent_integrations) fn id(self) -> &'static str {
        match self {
            Self::Mastracode => "mastracode",
            Self::Devin => "devin",
        }
    }
    pub(in crate::agent_integrations) fn events(self) -> &'static [&'static str] {
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
/// Raw container, event object/entries, owned entry indices and their agreed root.
pub(in crate::agent_integrations) type Parsed = (
    Option<Object>,
    Object,
    BTreeMap<String, Vec<Box<RawValue>>>,
    BTreeMap<String, usize>,
    Option<PathBuf>,
);

pub(in crate::agent_integrations) fn parse(
    set: EventSet,
    bytes: &[u8],
    policy: RootPolicy,
) -> Result<Parsed, HookConfigurationScan> {
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
    let mut root: Option<PathBuf> = None;
    for (event, raw) in &object.0 {
        if !set.events().contains(&event.as_str()) && !set.extra_events().contains(&event.as_str())
        {
            continue;
        }
        let values: Vec<Box<RawValue>> =
            serde_json::from_str(raw.get()).map_err(|_| HookConfigurationScan::Malformed)?;
        for (index, value) in values.iter().enumerate() {
            let found = match set {
                EventSet::Mastracode => entry_root(set.id(), value, policy),
                EventSet::Devin => item_root(set.id(), value, policy),
            }
            .map_err(|_| HookConfigurationScan::Conflict)?;
            if let Some(found) = found {
                if !set.events().contains(&event.as_str())
                    || owned.insert(event.clone(), index).is_some()
                    || root
                        .as_ref()
                        .is_some_and(|previous| !(policy.same)(previous, &found))
                {
                    return Err(HookConfigurationScan::Conflict);
                }
                root = Some(found);
            }
        }
        entries.insert(event.clone(), values);
    }
    Ok((parent, object, entries, owned, root))
}

pub(super) fn scan(set: EventSet, bytes: &[u8], policy: RootPolicy) -> HookConfigurationScan {
    match parse(set, bytes, policy) {
        Ok((_, _, _, owned, root)) => match root {
            None => HookConfigurationScan::Absent,
            Some(root) if owned.len() == set.events().len() => HookConfigurationScan::Owned(root),
            Some(root) => HookConfigurationScan::Incomplete(root),
        },
        Err(state) => state,
    }
}
