//! Windows configuration observations, resource classification and byte mutations.
//! The pure document scanner belongs to the common hook_configuration owner.
use super::{
    files,
    hook_configuration::{
        self as grammar, HookConfigurationKind, HookConfigurationScan, NAMED_HOOK, Object,
        RootPolicy, command, declaration, event_name,
    },
    inspect::{ResourceOptions, ResourceOwnership, classify_resource_root},
    model::State,
};
use crate::agent_configuration_windows::{HookConfigurationTarget, hook_configuration_candidates};
use serde_json::json;
use std::{io, path::Path};

mod event_sets;
mod kimi;

pub(super) const LIMIT: u64 = 512 * 1024;
pub(crate) const ROOT_POLICY: RootPolicy = RootPolicy {
    admitted: files::path_admitted,
    same: |left, right| left == right,
};

fn scan_hook_configuration(kind: HookConfigurationKind, bytes: &[u8]) -> HookConfigurationScan {
    grammar::scan_hook_configuration(kind, bytes, ROOT_POLICY)
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

fn patch_named(bytes: Option<&[u8]>, root: &Path, enable: bool) -> Option<Vec<u8>> {
    let (mut object, _) = grammar::parse_named(bytes.unwrap_or(b"{}"), ROOT_POLICY).ok()?;
    if enable {
        object.set(
            NAMED_HOOK,
            json!({"enabled":true,"PreInvocation":[declaration("antigravity-cli",root)?]}),
        )?;
    } else {
        object.0.remove(NAMED_HOOK);
    }
    let bytes = object.bytes()?;
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
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
    let (mut object, mut hooks, mut entries, owned) =
        grammar::parse(kind, bytes.unwrap_or(b"{}"), ROOT_POLICY).ok()?;
    if let Some((index, _)) = owned {
        entries.remove(index);
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
        entries.push(serde_json::value::to_raw_value(&item).ok()?);
    }
    hooks.set(event_name(id)?, entries)?;
    if kind == HookConfigurationKind::DroidStandalone {
        let bytes = hooks.bytes()?;
        return (bytes.len() as u64 <= LIMIT).then_some(bytes);
    }
    object.set("hooks", hooks.0)?;
    if id == "cursor" && !object.0.contains_key("version") {
        object.set("version", 1)?;
    }
    let bytes = object.bytes()?;
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

#[cfg(test)]
mod tests;
