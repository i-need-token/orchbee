//! Syntax-preserving controlled-leaf transformation with complete semantic readmission.
use super::{
    WindowsNativeSettingsChanges, WindowsNativeSettingsFailure, WindowsNotificationDelivery,
    WindowsUserSettings,
};
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

pub(super) fn edit(
    text: &str,
    changes: WindowsNativeSettingsChanges,
    settings: &WindowsUserSettings,
) -> Result<String, WindowsNativeSettingsFailure> {
    let invalid = WindowsNativeSettingsFailure::InvalidChanges;
    if changes.is_empty() {
        return Err(invalid);
    }
    let mut document: DocumentMut = text.parse().map_err(|_| invalid)?;
    let mut expected: toml::Table = toml::from_str(text).map_err(|_| invalid)?;
    if let Some(theme) = changes.theme {
        set(
            document.as_item_mut(),
            &["theme", "name"],
            Value::from(theme.as_str()),
        )?;
        semantic_set(
            &mut expected,
            &["theme", "name"],
            toml::Value::String(theme.as_str().into()),
        )?;
    }
    if let Some(enabled) = changes.background_reminders {
        let channel = if !enabled {
            WindowsNotificationDelivery::Off
        } else if settings.notifications.agent_delivery == WindowsNotificationDelivery::Off {
            WindowsNotificationDelivery::Tui
        } else {
            settings.notifications.agent_delivery
        };
        set(
            document.as_item_mut(),
            &["ui", "toast", "agent_delivery"],
            Value::from(channel.as_str()),
        )?;
        semantic_set(
            &mut expected,
            &["ui", "toast", "agent_delivery"],
            toml::Value::String(channel.as_str().into()),
        )?;
    }
    if let Some(enabled) = changes.sound_enabled {
        set(
            document.as_item_mut(),
            &["ui", "sound", "enabled"],
            Value::from(enabled),
        )?;
        semantic_set(
            &mut expected,
            &["ui", "sound", "enabled"],
            toml::Value::Boolean(enabled),
        )?;
    }
    let result = document.to_string();
    let actual: toml::Table = toml::from_str(&result).map_err(|_| invalid)?;
    if actual != expected {
        return Err(invalid);
    }
    Ok(result)
}

fn set(
    item: &mut Item,
    path: &[&str],
    mut value: Value,
) -> Result<(), WindowsNativeSettingsFailure> {
    let invalid = WindowsNativeSettingsFailure::InvalidChanges;
    let (key, rest) = path.split_first().ok_or(invalid)?;
    let inline = item.as_inline_table().is_some();
    let table = item.as_table_like_mut().ok_or(invalid)?;
    if rest.is_empty() {
        let previous = table.entry(key).or_insert(Item::None);
        if let Some(previous) = previous.as_value() {
            if (previous.as_str().is_some() && previous.as_str() == value.as_str())
                || (previous.as_bool().is_some() && previous.as_bool() == value.as_bool())
            {
                return Ok(());
            }
            *value.decor_mut() = previous.decor().clone();
        } else if !previous.is_none() {
            // Do not discard comments and nested syntax inside a malformed owned table leaf.
            return Err(invalid);
        }
        *previous = Item::Value(value);
        return Ok(());
    }
    let child = table.entry(key).or_insert_with(|| {
        if inline {
            Item::Value(Value::InlineTable(InlineTable::new()))
        } else {
            Item::Table(Table::new())
        }
    });
    set(child, rest, value)
}

fn semantic_set(
    table: &mut toml::Table,
    path: &[&str],
    value: toml::Value,
) -> Result<(), WindowsNativeSettingsFailure> {
    let invalid = WindowsNativeSettingsFailure::InvalidChanges;
    let (key, rest) = path.split_first().ok_or(invalid)?;
    if rest.is_empty() {
        table.insert((*key).to_owned(), value);
        return Ok(());
    }
    let child = table
        .entry((*key).to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or(invalid)?;
    semantic_set(child, rest, value)
}
