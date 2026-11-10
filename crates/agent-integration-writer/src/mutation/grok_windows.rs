//! Grok keeps an uninstalled plugin's activation entry in its user configuration. After the
//! uninstall, disable takes back the bare plugin name that install listed there, so grok's
//! configuration returns to what enable found.
use std::io;

use toml_edit::{DocumentMut, Item};

use super::super::providers::ProviderContext;
use super::{Attempt, EffectTrace, FailureCode, file_publication_windows, files, inspect};

const LIMIT: u64 = 512 * 1024;

pub(super) fn forget_activation(context: &ProviderContext, trace: EffectTrace) -> Attempt {
    let Some(path) = inspect::grok_configuration(context.home) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    let expected = match files::read_bounded(&path, LIMIT) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Attempt::Attempted,
        Err(_) => return Attempt::Failed(FailureCode::OperationFailed),
    };
    let Some(replacement) = without_activation(&expected) else {
        return Attempt::Failed(FailureCode::OperationFailed);
    };
    if replacement == expected {
        return Attempt::Attempted;
    }
    file_publication_windows::replace(&path, Some(expected), replacement, context, trace)
}

/// `bytes` without the plugin's bare name in `[plugins]` `enabled` and `disabled`. A list or
/// table that the removal empties goes too; every other byte stays. Entries naming the plugin by
/// its full ID are the user's and stay.
fn without_activation(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut document: DocumentMut = std::str::from_utf8(bytes).ok()?.parse().ok()?;
    let Some(plugins) = document
        .get_mut("plugins")
        .and_then(Item::as_table_like_mut)
    else {
        return Some(bytes.to_vec());
    };
    let mut removed = false;
    for key in ["enabled", "disabled"] {
        let Some(names) = plugins.get_mut(key).and_then(Item::as_array_mut) else {
            continue;
        };
        let before = names.len();
        names.retain(|name| name.as_str() != Some(inspect::GROK_PLUGIN_NAME));
        if names.len() != before {
            removed = true;
            if names.is_empty() {
                plugins.remove(key);
            }
        }
    }
    if !removed {
        return Some(bytes.to_vec());
    }
    if plugins.is_empty() {
        document.remove("plugins");
    }
    Some(document.to_string().into_bytes())
}

#[cfg(test)]
mod tests;
