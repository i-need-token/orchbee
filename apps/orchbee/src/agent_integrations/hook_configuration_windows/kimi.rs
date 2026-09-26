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
mod tests {
    use super::*;
    #[test]
    fn both_representations_preserve_foreign_values_and_repair_partial_sets() {
        let root = Path::new(r"C:\owned\0.4.15");
        for original in [
            "# user's header\nlarge = 9223372036854775807 # keep integer\nhooks = [{ event = 'PreToolUse', command = 'echo user', matcher = 'Bash' }] # keep rule\n[custom]\nvalue = '''not a [[hooks]] header'''\n",
            "# user's header\nlarge = 9223372036854775807 # keep integer\n[[hooks]] # keep rule\nevent = 'PreToolUse'\ncommand = 'echo user'\nmatcher = 'Bash'\n[custom]\nvalue = '''not a [[hooks]] header'''\n",
            "hooks = [] # empty inline\n",
            "",
        ] {
            let installed = patch(Some(original.as_bytes()), root, true).unwrap();
            assert!(
                matches!(scan(&installed), HookConfigurationScan::Owned(found) if found == root)
            );
            assert_eq!(patch(Some(&installed), root, true).unwrap(), installed);
            let disabled = patch(Some(&installed), root, false).unwrap();
            assert!(matches!(scan(&disabled), HookConfigurationScan::Absent));
            let text = std::str::from_utf8(&disabled).unwrap();
            if original.contains("9223372036854775807") {
                for retained in [
                    "# user's header",
                    "9223372036854775807 # keep integer",
                    "# keep rule",
                    "'''not a [[hooks]] header'''",
                    "'echo user'",
                ] {
                    assert!(text.contains(retained), "{retained}: {text}");
                }
            }
            let mut doc = std::str::from_utf8(&installed)
                .unwrap()
                .parse::<DocumentMut>()
                .unwrap();
            remove_last_hook(&mut doc);
            let partial = doc.to_string().into_bytes();
            assert!(
                matches!(scan(&partial), HookConfigurationScan::Incomplete(found) if found == root)
            );
            let repaired = patch(Some(&partial), root, true).unwrap();
            assert!(matches!(scan(&repaired), HookConfigurationScan::Owned(_)));
        }
    }
    fn remove_last_hook(doc: &mut DocumentMut) {
        let hooks = doc.get_mut("hooks").unwrap();
        if let Some(array) = hooks.as_array_of_tables_mut() {
            array.remove(array.len() - 1);
        } else {
            let array = hooks.as_array_mut().unwrap();
            array.remove(array.len() - 1);
        }
    }
    #[test]
    fn edited_duplicate_mixed_root_and_malformed_rules_refuse() {
        let root = Path::new(r"C:\owned\0.4.15");
        let installed = String::from_utf8(patch(None, root, true).unwrap()).unwrap();
        for edited in [
            installed.replace("timeout = 10", "timeout = 11"),
            installed.replace("SessionEnd", "Stop"),
            format!("{installed}{installed}"),
            installed.replace("timeout = 10", "timeout = 10\nmatcher = '*'"),
            installed.replace("timeout = 10", "timeout = 10\nunknown = true"),
        ] {
            assert!(matches!(
                scan(edited.as_bytes()),
                HookConfigurationScan::Conflict
            ));
            assert!(patch(Some(edited.as_bytes()), root, false).is_none());
        }
        let other =
            String::from_utf8(patch(None, Path::new(r"C:\other\0.4.15"), true).unwrap()).unwrap();
        let mixed = format!(
            "[[hooks]]{}[[hooks]]{}",
            installed.split("[[hooks]]").nth(1).unwrap(),
            other.split("[[hooks]]").nth(2).unwrap()
        );
        assert!(matches!(
            scan(mixed.as_bytes()),
            HookConfigurationScan::Conflict
        ));
        for invalid in [
            "hooks = 1",
            "hooks = [{}]",
            "hooks = []\nhooks = []",
            "[[hooks]",
            "hooks = [1]",
        ] {
            assert!(matches!(
                scan(invalid.as_bytes()),
                HookConfigurationScan::Malformed
            ));
        }
        assert!(matches!(scan(&[255]), HookConfigurationScan::Malformed));
    }
}
