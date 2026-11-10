//! Windows event-set byte mutation; parsing and ownership proofs are shared.
use super::{Change, RootPolicy, declaration, entries, grammar, member, splice};
pub(super) use grammar::event_sets::EventSet;
use serde_json::{Map, Value};
use std::path::Path;

pub(super) fn patch(
    set: EventSet,
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
    policy: RootPolicy<'_>,
) -> Option<Vec<u8>> {
    let (parent, object, _, owned, _) =
        grammar::event_sets::parse(set, bytes.unwrap_or(b"{}"), policy).ok()?;
    let accepts = |edited: &[u8]| grammar::event_sets::parse(set, edited, policy).is_ok();
    let item = if enable {
        let entry = declaration(set.id(), root)?;
        Some(match set {
            EventSet::Mastracode => entry,
            EventSet::Devin => serde_json::json!({"hooks":[entry]}),
        })
    } else {
        None
    };
    let mut members = Vec::new();
    let mut append = Vec::new();
    let mut created = Map::new();
    for event in set.events() {
        let Change::Object {
            members: changed,
            append: added,
        } = member(
            &object,
            event,
            entries(owned.get(*event).copied(), item.as_ref()),
            item.as_ref().map(|item| Value::Array(vec![item.clone()])),
        )
        else {
            return None;
        };
        members.extend(changed);
        for (event, value) in added {
            created.insert(event.into(), value.clone());
            append.push((event, value));
        }
    }
    let events = Change::Object { members, append };
    let Some(parent) = parent else {
        return splice(bytes, &events, accepts);
    };
    let created = (!created.is_empty()).then_some(Value::Object(created));
    splice(
        bytes,
        &member(&parent, "hooks", Some(events), created),
        accepts,
    )
}

#[cfg(test)]
mod tests;
