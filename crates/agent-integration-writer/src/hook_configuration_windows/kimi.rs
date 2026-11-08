//! Windows Kimi byte mutation; parsing and ownership proofs are shared.
use super::{LIMIT, RootPolicy, command, grammar};
use grammar::kimi::EVENTS;
use std::path::Path;
use toml_edit::{ArrayOfTables, Item, Table, value};

pub(super) fn patch(
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
    policy: RootPolicy<'_>,
) -> Option<Vec<u8>> {
    let (mut document, owned, _) = grammar::kimi::parse(bytes.unwrap_or(b""), policy).ok()?;
    if !document.contains_key("hooks") {
        if !enable {
            return Some(bytes.unwrap_or(b"").to_vec());
        }
        document.insert("hooks", Item::ArrayOfTables(ArrayOfTables::new()));
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
    let hooks = document.get_mut("hooks")?;
    if let Some(array) = hooks.as_array_of_tables_mut() {
        for index in owned.into_iter().rev() {
            array.remove(index);
        }
        for table in entries {
            array.push(table);
        }
    } else {
        let array = hooks.as_array_mut()?;
        for index in owned.into_iter().rev() {
            array.remove(index);
        }
        for table in entries {
            array.push(table.into_inline_table());
        }
    }
    let result = document.to_string().into_bytes();
    (result.len() as u64 <= LIMIT).then_some(result)
}

#[cfg(test)]
mod tests;
