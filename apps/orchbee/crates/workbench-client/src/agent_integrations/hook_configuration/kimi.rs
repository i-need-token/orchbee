//! Kimi's syntax-preserving read-only ownership grammar.
use super::{HookConfigurationScan, RootPolicy, command_root};
use std::path::PathBuf;
use toml_edit::{DocumentMut, Item, TableLike};

pub(in crate::agent_integrations) const EVENTS: [&str; 2] = ["SessionStart", "SessionEnd"];
/// The unchanged TOML document, owned indices and the agreed declaration root.
pub(in crate::agent_integrations) type Parsed = (DocumentMut, Vec<usize>, Option<PathBuf>);

pub(in crate::agent_integrations) fn parse(
    bytes: &[u8],
    policy: RootPolicy,
) -> Result<Parsed, HookConfigurationScan> {
    let source = std::str::from_utf8(bytes).map_err(|_| HookConfigurationScan::Malformed)?;
    let value = source
        .parse::<DocumentMut>()
        .map_err(|_| HookConfigurationScan::Malformed)?;
    let mut owned = Vec::new();
    let mut root: Option<PathBuf> = None;
    let mut events = Vec::new();
    let tables: Vec<&dyn TableLike> = match value.get("hooks") {
        None => Vec::new(),
        Some(Item::ArrayOfTables(array)) => {
            array.iter().map(|table| table as &dyn TableLike).collect()
        }
        Some(item) => item
            .as_array()
            .ok_or(HookConfigurationScan::Malformed)?
            .iter()
            .map(|entry| {
                entry
                    .as_inline_table()
                    .map(|t| t as &dyn TableLike)
                    .ok_or(HookConfigurationScan::Malformed)
            })
            .collect::<Result<_, _>>()?,
    };
    for (index, table) in tables.into_iter().enumerate() {
        let declaration = table
            .get("command")
            .and_then(Item::as_str)
            .ok_or(HookConfigurationScan::Malformed)?;
        let Some(found) = command_root("kimi", declaration, policy)
            .map_err(|_| HookConfigurationScan::Conflict)?
        else {
            continue;
        };
        let event = table
            .get("event")
            .and_then(Item::as_str)
            .ok_or(HookConfigurationScan::Conflict)?;
        if table.len() != 3
            || !EVENTS.contains(&event)
            || events.contains(&event)
            || table.get("timeout").and_then(Item::as_integer) != Some(10)
            || root
                .as_ref()
                .is_some_and(|prior| !(policy.same)(prior, &found))
        {
            return Err(HookConfigurationScan::Conflict);
        }
        root = Some(found);
        owned.push(index);
        events.push(event);
    }
    Ok((value, owned, root))
}

pub(super) fn scan(bytes: &[u8], policy: RootPolicy) -> HookConfigurationScan {
    match parse(bytes, policy) {
        Ok((_, owned, root)) => match root {
            None => HookConfigurationScan::Absent,
            Some(root) if owned.len() == EVENTS.len() => HookConfigurationScan::Owned(root),
            Some(root) => HookConfigurationScan::Incomplete(root),
        },
        Err(state) => state,
    }
}
