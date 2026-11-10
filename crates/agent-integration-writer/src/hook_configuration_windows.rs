//! Windows configuration observations, resource classification and byte mutations.
//! The pure document scanner belongs to the common hook_configuration owner.
use super::{
    files,
    hook_configuration::{
        self as grammar, HookConfigurationKind, HookConfigurationScan, NAMED_HOOK, Object,
        RootPolicy, command, declaration, event_name,
    },
    inspect::{ResourceOptions, ResourceOwnership, classify_resource_root, owned_elsewhere},
    model::State,
};
use crate::configuration_windows::{HookConfigurationTarget, hook_configuration_candidates};
use serde_json::{Value, json};
use splice::Change;
use std::{io, path::Path};

mod event_sets;
mod intent;
mod kimi;
mod splice;

pub(super) const LIMIT: u64 = 512 * 1024;
/// No root belongs to another environment: every Orchbee entry counts as this environment's.
fn nowhere(_: &Path) -> bool {
    false
}

/// The Windows path grammar; a reader sets which roots another environment owns with
/// [`RootPolicy::owned_elsewhere`].
pub const ROOT_POLICY: RootPolicy<'static> = RootPolicy {
    admitted: files::path_admitted,
    same: |left, right| left == right,
    elsewhere: &nowhere,
};

/// Scans with no other environment's entries, as the grammar tests read documents.
#[cfg(test)]
fn scan_hook_configuration(kind: HookConfigurationKind, bytes: &[u8]) -> HookConfigurationScan {
    grammar::scan_hook_configuration(kind, bytes, ROOT_POLICY)
}

/// Calls `read` with the policy that leaves the entries of other environments out.
fn with_policy<T>(options: &ResourceOptions, read: impl FnOnce(RootPolicy<'_>) -> T) -> T {
    let elsewhere = |root: &Path| owned_elsewhere(root, options);
    read(ROOT_POLICY.owned_elsewhere(&elsewhere))
}

/// The change to the member `key` of `object`: `change` applies to an existing member in place,
/// and `value` is appended as a new member when there is none.
fn member(
    object: &Object,
    key: &'static str,
    change: Option<Change>,
    value: Option<Value>,
) -> Change {
    let (members, append) = if object.0.contains_key(key) {
        (
            change.map(|change| (key, change)).into_iter().collect(),
            Vec::new(),
        )
    } else {
        (
            Vec::new(),
            value.map(|value| (key, value)).into_iter().collect(),
        )
    };
    Change::Object { members, append }
}

/// The change to one event's entries: the owned entry at `owned` is replaced with `item` or
/// removed, and without an owned entry `item` is appended.
fn entries(owned: Option<usize>, item: Option<&Value>) -> Option<Change> {
    match (owned, item) {
        (Some(index), Some(item)) => Some(Change::Array {
            elements: vec![(index, Change::Replace(item.clone()))],
            append: Vec::new(),
        }),
        (Some(index), None) => Some(Change::Array {
            elements: vec![(index, Change::Remove)],
            append: Vec::new(),
        }),
        (None, Some(item)) => Some(Change::Array {
            elements: Vec::new(),
            append: vec![item.clone()],
        }),
        (None, None) => None,
    }
}

/// `bytes` with `change` applied and the user's other bytes kept. `accepts` is the read path's
/// strict grammar for the edited document.
fn splice(
    bytes: Option<&[u8]>,
    change: &Change,
    accepts: impl FnOnce(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(bytes.unwrap_or(b"{}")).ok()?;
    publishable(text, splice::apply(text, change)?, change, accepts)
}

/// `edited` only when it stays within the read bound, passes the strict grammar and holds exactly
/// the value that `change` makes of `text`; otherwise the step fails and writes nothing.
fn publishable(
    text: &str,
    edited: String,
    change: &Change,
    accepts: impl FnOnce(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    let fits = edited.len() as u64 <= LIMIT
        && accepts(edited.as_bytes())
        && intent::agrees(text, &edited, change);
    fits.then(|| edited.into_bytes())
}

fn patch_named(
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
    policy: RootPolicy<'_>,
) -> Option<Vec<u8>> {
    let (object, _) = grammar::parse_named(bytes.unwrap_or(b"{}"), policy).ok()?;
    let block = if enable {
        Some(json!({"enabled":true,"PreInvocation":[declaration("antigravity-cli",root)?]}))
    } else {
        None
    };
    let change = block.clone().map_or(Change::Remove, Change::Replace);
    splice(
        bytes,
        &member(&object, NAMED_HOOK, Some(change), block),
        |edited| grammar::parse_named(edited, policy).is_ok(),
    )
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
    let scan = with_policy(options, |policy| {
        grammar::scan_hook_configuration(kind, bytes, policy)
    });
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
                ResourceOwnership::Outdated => State::UpdateRequired,
                ResourceOwnership::Foreign => State::Conflict,
            }
        }
    }
}

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

/// Rewrites this environment's entries to name `options.resource_root`, or removes them; entries
/// of other environments stay as they are.
pub(super) fn patch(
    kind: HookConfigurationKind,
    bytes: Option<&[u8]>,
    options: &ResourceOptions,
    enable: bool,
) -> Option<Vec<u8>> {
    with_policy(options, |policy| {
        patch_with(kind, bytes, options.resource_root, enable, policy)
    })
}

fn patch_with(
    kind: HookConfigurationKind,
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
    policy: RootPolicy<'_>,
) -> Option<Vec<u8>> {
    if kind == HookConfigurationKind::Kimi {
        return kimi::patch(bytes, root, enable, policy);
    }
    let id = kind.id();
    if id == "mastracode" {
        return event_sets::patch(
            event_sets::EventSet::Mastracode,
            bytes,
            root,
            enable,
            policy,
        );
    }
    if id == "devin" {
        return event_sets::patch(event_sets::EventSet::Devin, bytes, root, enable, policy);
    }
    if id == "antigravity-cli" {
        return patch_named(bytes, root, enable, policy);
    }
    let (object, hooks, _, owned) = grammar::parse(kind, bytes.unwrap_or(b"{}"), policy).ok()?;
    let accepts = |edited: &[u8]| grammar::parse(kind, edited, policy).is_ok();
    let item = if enable {
        let entry = declaration(id, root)?;
        Some(
            if matches!(
                kind,
                HookConfigurationKind::Qoder
                    | HookConfigurationKind::Letta
                    | HookConfigurationKind::DroidStandalone
                    | HookConfigurationKind::DroidSettings
            ) {
                json!({"hooks":[entry]})
            } else {
                entry
            },
        )
    } else {
        None
    };
    let event = event_name(id)?;
    let events = member(
        &hooks,
        event,
        entries(owned.map(|(index, _)| index), item.as_ref()),
        item.as_ref().map(|item| json!([item])),
    );
    if kind == HookConfigurationKind::DroidStandalone {
        return splice(bytes, &events, accepts);
    }
    let Change::Object {
        members,
        mut append,
    } = member(
        &object,
        "hooks",
        Some(events),
        item.as_ref().map(|item| json!({ event: [item] })),
    )
    else {
        return None;
    };
    if id == "cursor" && enable && !object.0.contains_key("version") {
        append.push(("version", json!(1)));
    }
    splice(bytes, &Change::Object { members, append }, accepts)
}

#[cfg(test)]
mod tests;
