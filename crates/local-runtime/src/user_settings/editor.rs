//! Edits of `config.toml` that change only the keys they name and keep every other byte the person
//! wrote.
//!
//! An edit takes `internal/config.lock` in the data root, reads the file and records its digest,
//! applies every change to one candidate with `toml_edit`, validates that candidate once with the
//! reader, writes `config.pending` beside the file, flushes it, confirms the file still has the
//! recorded digest (re-reading and redoing the edit once when it does not), and atomically replaces
//! the file. Every change lands or none does. A symbolic link keeps pointing at its target, which is
//! the file replaced. A file that does not parse is never written.
use super::{
    LIMIT, SettingsError, SettingsSyntaxError, parse, parse_bytes,
    registry::{SETTING_KEYS, SettingValue, find_setting, recognized_path},
    unknown_field, user_settings_path,
};
use crate::DataRoot;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use toml_edit::{DocumentMut, Item, TableLike};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

/// How long an edit waits for another writer's lock.
const LOCK_WAIT: Duration = Duration::from_secs(2);
const RETRY_PAUSE: Duration = Duration::from_millis(25);
const LOCK_NAME: &str = "config.lock";
const PENDING_NAME: &str = "config.pending";

/// Most changes one edit carries.
pub const MAXIMUM_SETTING_CHANGES: usize = 64;
/// Most entries in the array or table a structured key is set to.
pub const MAXIMUM_STRUCTURED_ENTRIES: usize = 64;
/// Largest JSON encoding of the value a structured key is set to.
pub const MAXIMUM_STRUCTURED_VALUE_BYTES: usize = 64 * 1024;

/// One change to `config.toml`. A key names one segment per table level.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingChange {
    /// Set the key to `value`. A structured key such as `terminal.profiles.<platform>` takes its
    /// whole value in the file's form, an array of tables included.
    Set {
        key: Vec<String>,
        value: toml::Value,
    },
    /// Restore the key's default by removing it from the file.
    Restore { key: Vec<String> },
}

impl SettingChange {
    /// The key the change names.
    pub fn key(&self) -> &[String] {
        match self {
            Self::Set { key, .. } | Self::Restore { key } => key,
        }
    }
}

/// From one to [`MAXIMUM_SETTING_CHANGES`] changes, no two naming the same key.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingChanges(Vec<SettingChange>);

impl SettingChanges {
    /// The changes, or `None` when there are none, more than [`MAXIMUM_SETTING_CHANGES`], or two
    /// that name the same key.
    pub fn new(changes: Vec<SettingChange>) -> Option<Self> {
        let keys: BTreeSet<&[String]> = changes.iter().map(SettingChange::key).collect();
        ((1..=MAXIMUM_SETTING_CHANGES).contains(&changes.len()) && keys.len() == changes.len())
            .then_some(Self(changes))
    }

    /// The changes in the order they were given.
    pub fn as_slice(&self) -> &[SettingChange] {
        &self.0
    }
}

/// Why an edit wrote nothing.
#[derive(Debug, Eq, PartialEq)]
pub enum SettingsEditError {
    /// `key` is not a registered setting, or names a part of one that is edited as a whole.
    UnknownKey { key: String },
    /// The reader would not take the file this edit makes. `key` is the change it is charged to:
    /// the one the diagnostic concerns, else the edit's first change. `field` and `reason` are the
    /// reader's diagnostic, which can name another key the change affects, such as a default
    /// profile that no longer exists.
    InvalidValue {
        key: String,
        field: String,
        reason: &'static str,
    },
    /// The file is not valid UTF-8 or TOML, so it is never written.
    Syntax(SettingsSyntaxError),
    /// The settings location or the existing file cannot be used.
    Source(SettingsError),
    /// The file or its folder does not admit writing.
    ReadOnly,
    /// Another program kept the file open past the retries.
    InUse,
    /// Another edit held the lock past the wait.
    Locked,
    /// The file changed during the edit, and again during the one redo.
    Changed,
    /// The edited file would exceed the size the reader admits.
    TooLarge,
    /// Any other input or output failure.
    Io(io::ErrorKind),
}

impl SettingsEditError {
    /// Stable lowercase kebab token for diagnostics.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::UnknownKey { .. } => "unknown-key",
            Self::InvalidValue { reason, .. } => reason,
            Self::Syntax(syntax) => syntax.reason,
            Self::Source(error) => error.reason,
            Self::ReadOnly => "read-only",
            Self::InUse => "file-in-use",
            Self::Locked => "locked",
            Self::Changed => "changed",
            Self::TooLarge => "too-large",
            Self::Io(_) => "io-error",
        }
    }
}

impl std::fmt::Display for SettingsEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownKey { key } => write!(f, "cannot write config.toml: {key} (unknown-key)"),
            Self::InvalidValue { field, reason, .. } => {
                write!(f, "cannot write config.toml: {field} ({reason})")
            }
            Self::Syntax(syntax) => write!(
                f,
                "cannot write config.toml: {} at line {}, column {}",
                syntax.reason, syntax.line, syntax.column
            ),
            other => write!(f, "cannot write config.toml: {}", other.reason()),
        }
    }
}

impl std::error::Error for SettingsEditError {}

/// Apply every change in one write, or none, and return whether the file changed. Each value must
/// be one the reader admits for its key, and the file the edit makes must give the reader no new
/// diagnostic on a registered setting. Restoring a key removes it, and a table left empty and
/// without a comment goes with it. A file that already says what the changes ask for is not
/// written.
///
/// # Errors
/// Returns the first refusal; nothing is written then.
pub fn edit_settings(
    changes: &SettingChanges,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<bool, SettingsEditError> {
    edit(changes.as_slice(), &env).map(|(_, changed)| changed)
}

/// Set the key that `key` names, one segment per table level, to `value`, and return the file's
/// path: [`edit_settings`] with that one change.
pub fn set_setting(
    key: &[&str],
    value: &toml::Value,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsEditError> {
    let change = SettingChange::Set {
        key: owned(key),
        value: value.clone(),
    };
    edit(&[change], &env).map(|(path, _)| path)
}

/// Restore the default of the key that `key` names and return the file's path: [`edit_settings`]
/// with that one change. A key already absent writes nothing.
pub fn restore_setting_default(
    key: &[&str],
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsEditError> {
    let change = SettingChange::Restore { key: owned(key) };
    edit(&[change], &env).map(|(path, _)| path)
}

fn owned(key: &[&str]) -> Vec<String> {
    key.iter().map(|segment| (*segment).to_owned()).collect()
}

/// The key as diagnostics name fields: escaped segments joined with dots.
fn dotted(key: &[String]) -> String {
    key.iter()
        .map(|segment| unknown_field("", segment))
        .collect::<Vec<_>>()
        .join(".")
}

/// Refuse a change whose key the editor cannot write, or whose structured value is over its
/// bounds, before the file is opened.
fn admit(change: &SettingChange) -> Result<(), SettingsEditError> {
    let key = change.key();
    let segments: Vec<&str> = key.iter().map(String::as_str).collect();
    let setting = find_setting(&segments)
        .filter(|setting| !setting.path.contains("[]"))
        .ok_or_else(|| SettingsEditError::UnknownKey { key: dotted(key) })?;
    if let (SettingChange::Set { value, .. }, SettingValue::Structured(_)) = (change, setting.value)
    {
        let entries = match value {
            toml::Value::Array(entries) => entries.len(),
            toml::Value::Table(entries) => entries.len(),
            _ => 0,
        };
        let bytes = serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len());
        if entries > MAXIMUM_STRUCTURED_ENTRIES || bytes > MAXIMUM_STRUCTURED_VALUE_BYTES {
            return Err(SettingsEditError::InvalidValue {
                key: dotted(key),
                field: dotted(key),
                reason: "value-too-long",
            });
        }
    }
    Ok(())
}

fn edit(
    changes: &[SettingChange],
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<(PathBuf, bool), SettingsEditError> {
    changes.iter().try_for_each(admit)?;
    let path = user_settings_path(env)
        .map_err(SettingsEditError::Source)?
        .ok_or(SettingsEditError::Source(SettingsError::new(
            "file",
            "invalid-path",
        )))?;
    let lock_directory = DataRoot::resolve(env)
        .map_err(|error| {
            SettingsEditError::Source(SettingsError::new(error.variable, error.reason))
        })?
        .config_lock_directory();
    let _lock = acquire_lock(&lock_directory)?;
    let target = resolve_link(&path).map_err(|_| unreadable("io-error"))?;
    let pending = target.with_file_name(PENDING_NAME);
    let outcome = edit_locked(changes, &path, &target, env, &mut read_target);
    if outcome.is_err() {
        let _ = fs::remove_file(&pending);
    }
    outcome.map(|changed| (path, changed))
}

/// The file's bytes, `None` when it does not exist.
type TargetRead = Result<Option<Vec<u8>>, SettingsEditError>;

/// One edit under the held lock; returns whether the file was replaced. `read` reads the target;
/// every read goes through it so the comparison before replacement sees exactly what another
/// writer left.
fn edit_locked(
    changes: &[SettingChange],
    path: &Path,
    target: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
    read: &mut dyn FnMut(&Path) -> TargetRead,
) -> Result<bool, SettingsEditError> {
    let pending = target.with_file_name(PENDING_NAME);
    for _ in 0..2 {
        let before = read(target)?;
        let Some(after) = candidate(before.as_deref(), changes, path, env)? else {
            return Ok(false);
        };
        write_pending(target, &pending, after.as_bytes())?;
        if digest(read(target)?.as_deref()) != digest(before.as_deref()) {
            continue;
        }
        return platform::replace(&pending, target).map(|()| true);
    }
    Err(SettingsEditError::Changed)
}

/// The edited text, or `None` when the file already says what the changes ask for.
fn candidate(
    before: Option<&[u8]>,
    changes: &[SettingChange],
    path: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<Option<String>, SettingsEditError> {
    let before_load = parse_bytes(before.unwrap_or_default(), path, env);
    if let Some(syntax) = before_load.syntax {
        return Err(SettingsEditError::Syntax(syntax));
    }
    let text = before
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| SettingsEditError::Io(io::ErrorKind::InvalidData))?
        .unwrap_or_default();
    let mut document = text
        .parse::<DocumentMut>()
        .map_err(|_| SettingsEditError::Io(io::ErrorKind::InvalidData))?;
    // Comments that end the file stay above a table the edit adds there, so a file that began as
    // the printed reference gains its settings below the reference.
    let trailing = document.trailing().as_str().unwrap_or_default().to_owned();
    let mut leading = trailing
        .contains('#')
        .then(|| format!("{}\n\n", trailing.trim_end()));
    if leading.is_some() {
        document.set_trailing("");
    }
    for change in changes {
        let segments: Vec<&str> = change.key().iter().map(String::as_str).collect();
        match change {
            SettingChange::Set { value, .. } => {
                let refused = |field: String| SettingsEditError::InvalidValue {
                    key: dotted(change.key()),
                    field,
                    reason: "wrong-type",
                };
                let pattern = find_setting(&segments).map_or("", |setting| setting.path);
                let item = item(value, pattern).ok_or_else(|| refused(dotted(change.key())))?;
                set_in(document.as_table_mut(), &segments, item, "", &mut leading)
                    .map_err(refused)?;
            }
            SettingChange::Restore { .. } => {
                restore_in(document.as_table_mut(), &segments);
            }
        }
    }
    if leading.is_some() {
        document.set_trailing(trailing);
    }
    let after = document.to_string();
    if after == text {
        return Ok(None);
    }
    if after.len() as u64 > LIMIT {
        return Err(SettingsEditError::TooLarge);
    }
    let after_load = parse(&after, path, env);
    if let Some(syntax) = after_load.syntax {
        return Err(SettingsEditError::Syntax(syntax));
    }
    // A diagnostic the file did not have refuses the edit when it concerns a changed key or any
    // registered setting, such as a default profile left naming a removed profile. Fields outside
    // the registry are the person's own and can only shift in the bounded list.
    let keys: Vec<String> = changes.iter().map(|change| dotted(change.key())).collect();
    if let Some(refused) = after_load.diagnostics.items().iter().find(|diagnostic| {
        !before_load.diagnostics.items().contains(diagnostic)
            && (recognized_path(&diagnostic.field)
                || keys.iter().any(|key| related(key, &diagnostic.field)))
    }) {
        let owner = keys
            .iter()
            .find(|key| related(key, &refused.field))
            .or(keys.first());
        return Err(SettingsEditError::InvalidValue {
            key: owner.cloned().unwrap_or_default(),
            field: refused.field.clone(),
            reason: refused.reason,
        });
    }
    Ok(Some(after))
}

/// Whether a diagnostic on `field` concerns `key`: the key itself, a table that holds it, or a
/// value or array entry inside it.
fn related(key: &str, field: &str) -> bool {
    field == key
        || field.starts_with(&format!("{key}."))
        || field.starts_with(&format!("{key}[]"))
        || key.starts_with(&format!("{field}."))
}

/// The TOML item a value is written as: an array of tables keeps the `[[...]]` form, with each
/// entry's keys in the order the registry declares them under `pattern`; any other value is
/// written inline on its key's line.
fn item(value: &toml::Value, pattern: &str) -> Option<Item> {
    if let toml::Value::Array(entries) = value
        && !entries.is_empty()
        && entries.iter().all(toml::Value::is_table)
    {
        let rank = |key: &str| {
            let path = format!("{pattern}[].{key}");
            SETTING_KEYS
                .iter()
                .position(|setting| setting.path == path)
                .unwrap_or(SETTING_KEYS.len())
        };
        let mut array = toml_edit::ArrayOfTables::new();
        for entry in entries {
            let mut fields: Vec<_> = entry.as_table()?.iter().collect();
            fields.sort_by_key(|(key, _)| rank(key));
            let mut table = toml_edit::Table::new();
            for (key, value) in fields {
                table.insert(key, Item::Value(inline(value)?));
            }
            array.push(table);
        }
        return Some(Item::ArrayOfTables(array));
    }
    inline(value).map(Item::Value)
}

fn inline(value: &toml::Value) -> Option<toml_edit::Value> {
    let mut converted = match value {
        toml::Value::String(value) => toml_edit::Value::from(value.as_str()),
        toml::Value::Integer(value) => toml_edit::Value::from(*value),
        toml::Value::Float(value) => toml_edit::Value::from(*value),
        toml::Value::Boolean(value) => toml_edit::Value::from(*value),
        toml::Value::Datetime(value) => value.to_string().parse().ok()?,
        toml::Value::Array(values) => {
            let mut array = toml_edit::Array::new();
            for value in values {
                array.push(inline(value)?);
            }
            toml_edit::Value::Array(array)
        }
        toml::Value::Table(values) => {
            let mut table = toml_edit::InlineTable::new();
            for (key, value) in values {
                table.insert(key, inline(value)?);
            }
            toml_edit::Value::InlineTable(table)
        }
    };
    converted.decor_mut().clear();
    Some(converted)
}

/// Place `item` at `segments` below `table`. A key already present keeps its own comments and
/// inline comment; `leading` becomes the prefix of the first table header the edit adds. Fails with
/// the field of a value on the way that is not a table.
fn set_in(
    table: &mut dyn TableLike,
    segments: &[&str],
    mut item: Item,
    parent: &str,
    leading: &mut Option<String>,
) -> Result<(), String> {
    let Some((first, rest)) = segments.split_first() else {
        return Ok(());
    };
    if rest.is_empty() {
        match table.get_mut(first) {
            Some(slot) => {
                if let (Some(previous), Item::Value(value)) = (slot.as_value(), &mut item) {
                    *value.decor_mut() = previous.decor().clone();
                }
                *slot = item;
            }
            None => {
                table.insert(first, item);
            }
        }
        return Ok(());
    }
    let path = unknown_field(parent, first);
    if table.get(first).is_none() {
        let mut child = toml_edit::Table::new();
        // Only the table that receives a value gets a header; tables above it, and the table that
        // holds an array of tables, stay implicit.
        let header = rest.len() == 1 && !item.is_array_of_tables();
        child.set_implicit(!header);
        if rest.len() == 1
            && let Some(prefix) = leading.take()
        {
            match &mut item {
                Item::ArrayOfTables(array) => {
                    if let Some(entry) = array.get_mut(0) {
                        entry.decor_mut().set_prefix(prefix);
                    }
                }
                _ => child.decor_mut().set_prefix(prefix),
            }
        }
        table.insert(first, Item::Table(child));
    }
    let child = table
        .get_mut(first)
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| path.clone())?;
    set_in(child, rest, item, &path, leading)
}

/// Remove the key; report whether anything was removed. Tables emptied by the removal go too
/// unless they carry a comment.
fn restore_in(table: &mut dyn TableLike, segments: &[&str]) -> bool {
    let Some((first, rest)) = segments.split_first() else {
        return false;
    };
    if rest.is_empty() {
        return table.remove(first).is_some();
    }
    let (removed, empty) = match table.get_mut(first).and_then(Item::as_table_like_mut) {
        Some(child) => {
            let removed = restore_in(child, rest);
            (removed, child.is_empty())
        }
        None => return false,
    };
    if removed && empty && !table.get(first).is_some_and(commented) {
        table.remove(first);
    }
    removed
}

fn commented(item: &Item) -> bool {
    let decor = match item {
        Item::Table(table) => table.decor(),
        Item::Value(value) => value.decor(),
        Item::ArrayOfTables(_) | Item::None => return true,
    };
    [decor.prefix(), decor.suffix()]
        .into_iter()
        .flatten()
        .any(|raw| raw.as_str().is_none_or(|text| text.contains('#')))
}

fn acquire_lock(directory: &Path) -> Result<File, SettingsEditError> {
    platform::create_lock_directory(directory).map_err(io_failure)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(LOCK_NAME))
        .map_err(io_failure)?;
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(RETRY_PAUSE);
            }
            Err(TryLockError::WouldBlock) => return Err(SettingsEditError::Locked),
            Err(TryLockError::Error(error)) => return Err(io_failure(error)),
        }
    }
}

/// The file a chain of symbolic links ends at; a missing file resolves to itself.
fn resolve_link(path: &Path) -> io::Result<PathBuf> {
    let mut current = path.to_owned();
    for _ in 0..32 {
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let link = fs::read_link(&current)?;
                current = match current.parent() {
                    Some(parent) if link.is_relative() => parent.join(link),
                    _ => link,
                };
            }
            Ok(_) => return Ok(current),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(current),
            Err(error) => return Err(error),
        }
    }
    Err(io::ErrorKind::InvalidInput.into())
}

/// The file's bytes, or `None` when it does not exist. A file that cannot be read, or is not a
/// regular file, answers with the reader's reason for it; only a readable file marked read-only is
/// refused as read-only, here, before anything is written.
fn read_target(target: &Path) -> TargetRead {
    let file = match File::open(target) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        // Opening a folder is denied on Windows, so the reason comes from what the path names.
        Err(_) if fs::metadata(target).is_ok_and(|metadata| !metadata.is_file()) => {
            return Err(unreadable("not-file"));
        }
        Err(_) => return Err(unreadable("io-error")),
    };
    let metadata = file.metadata().map_err(|_| unreadable("io-error"))?;
    if !metadata.is_file() {
        return Err(unreadable("not-file"));
    }
    if metadata.permissions().readonly() {
        return Err(SettingsEditError::ReadOnly);
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unreadable("io-error"))?;
    if bytes.len() as u64 > LIMIT {
        return Err(unreadable("too-large"));
    }
    Ok(Some(bytes))
}

/// The existing file cannot be read, for the same reason the reader gives.
const fn unreadable(reason: &'static str) -> SettingsEditError {
    SettingsEditError::Source(SettingsError::new("file", reason))
}

fn write_pending(target: &Path, pending: &Path, bytes: &[u8]) -> Result<(), SettingsEditError> {
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(io_failure)?;
    }
    let mut file = File::create(pending).map_err(io_failure)?;
    file.write_all(bytes).map_err(io_failure)?;
    file.sync_all().map_err(io_failure)
}

fn digest(bytes: Option<&[u8]>) -> Option<[u8; 32]> {
    bytes.map(|bytes| Sha256::digest(bytes).into())
}

fn io_failure(error: io::Error) -> SettingsEditError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        SettingsEditError::ReadOnly
    } else {
        SettingsEditError::Io(error.kind())
    }
}

#[cfg(test)]
mod tests;
