//! Syntax-preserving ownership of Kimi config.toml hook rules.
use super::{HookConfigurationScan, LIMIT, command, command_root};
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, TableLike, value};

const EVENTS: [&str; 2] = ["SessionStart", "SessionEnd"];

struct Document {
    value: DocumentMut,
    owned: Vec<usize>,
    root: Option<PathBuf>,
}

fn parse(bytes: &[u8]) -> Result<Document, HookConfigurationScan> {
    let source = std::str::from_utf8(bytes).map_err(|_| HookConfigurationScan::Malformed)?;
    let value = source
        .parse::<DocumentMut>()
        .map_err(|_| HookConfigurationScan::Malformed)?;
    let mut owned = Vec::new();
    let mut root = None;
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
        let Some(found) =
            command_root("kimi", declaration).map_err(|_| HookConfigurationScan::Conflict)?
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
            || root.as_ref().is_some_and(|prior| prior != &found)
        {
            return Err(HookConfigurationScan::Conflict);
        }
        root = Some(found);
        owned.push(index);
        events.push(event);
    }
    Ok(Document { value, owned, root })
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
    let mut doc = parse(bytes.unwrap_or(b"")).ok()?;
    if !doc.value.contains_key("hooks") {
        if !enable {
            return Some(bytes.unwrap_or(b"").to_vec());
        }
        doc.value
            .insert("hooks", Item::ArrayOfTables(ArrayOfTables::new()));
    }
    let mut entries = Vec::new();
    if enable {
        for event in EVENTS {
            let mut table = Table::new();
            table.insert("event", value(event));
            table.insert("command", value(command("kimi", root)?));
            table.insert("timeout", value(10));
            entries.push(table);
        }
    }
    let hooks = doc.value.get_mut("hooks")?;
    if let Some(array) = hooks.as_array_of_tables_mut() {
        for index in doc.owned.into_iter().rev() {
            array.remove(index);
        }
        for table in entries {
            array.push(table);
        }
    } else {
        let array = hooks.as_array_mut()?;
        for index in doc.owned.into_iter().rev() {
            array.remove(index);
        }
        for table in entries {
            array.push(table.into_inline_table());
        }
    }
    let result = doc.value.to_string().into_bytes();
    (result.len() as u64 <= LIMIT).then_some(result)
}

#[cfg(test)]
mod tests;
