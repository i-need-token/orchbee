//! Windows event-set byte mutation; parsing and ownership proofs are shared.
use super::{LIMIT, ROOT_POLICY, declaration, grammar};
pub(super) use grammar::event_sets::EventSet;
use std::path::Path;

pub(super) fn patch(
    set: EventSet,
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
) -> Option<Vec<u8>> {
    let (parent, mut object, mut entries_by_event, owned, _) =
        grammar::event_sets::parse(set, bytes.unwrap_or(b"{}"), ROOT_POLICY).ok()?;
    for event in set.events() {
        let mut entries = entries_by_event.remove(*event).unwrap_or_default();
        if let Some(index) = owned.get(*event) {
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
            object.0.remove(*event);
        } else {
            object.set(event, entries)?;
        }
    }
    let bytes = if let Some(mut parent) = parent {
        parent.set("hooks", object.0)?;
        parent.bytes()?
    } else {
        object.bytes()?
    };
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

#[cfg(test)]
mod tests;
