//! Single-key edits of `config.toml` that keep every other byte the person wrote.
//!
//! An edit takes `internal/config.lock` in the data root, reads the file and records its digest,
//! changes only the one key with `toml_edit`, validates the candidate with the reader, writes
//! `config.pending` beside the file, flushes it, confirms the file still has the recorded digest
//! (re-reading and redoing the edit once when it does not), and atomically replaces the file. A
//! symbolic link keeps pointing at its target, which is the file replaced. A file that does not
//! parse is never written.
use super::{
    LIMIT, SettingsError, SettingsSyntaxError, parse, parse_bytes, registry::find_setting,
    unknown_field, user_settings_path,
};
use crate::DataRoot;
use sha2::{Digest, Sha256};
use std::{
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

/// Why an edit wrote nothing.
#[derive(Debug, Eq, PartialEq)]
pub enum SettingsEditError {
    /// The key is not a registered setting, or names a part of one that is edited as a whole.
    UnknownKey,
    /// The reader refuses the value; `field` and `reason` are the diagnostic it reports.
    InvalidValue { field: String, reason: &'static str },
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
            Self::UnknownKey => "unknown-key",
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
            Self::InvalidValue { field, reason } => {
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

/// The projection onto the file failure vocabulary that `channel set` already publishes: a
/// syntax error and an oversized result keep their reasons, and any write that cannot complete is
/// `io-error`. Callers that show the typed reason use [`SettingsEditError`] itself.
impl From<SettingsEditError> for SettingsError {
    fn from(error: SettingsEditError) -> Self {
        match error {
            SettingsEditError::Source(error) => error,
            SettingsEditError::Syntax(syntax) => SettingsError::new("file", syntax.reason),
            SettingsEditError::TooLarge => SettingsError::new("file", "too-large"),
            _ => SettingsError::new("file", "io-error"),
        }
    }
}

enum Change<'a> {
    Set(&'a toml::Value),
    Restore,
}

/// Set the key that `key` names, one segment per table level, to `value`, and return the file's
/// path. The value must be one the reader admits for that key.
pub fn set_setting(
    key: &[&str],
    value: &toml::Value,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsEditError> {
    edit(key, &Change::Set(value), &env)
}

/// Restore the default of the key that `key` names by removing it; a table left empty and
/// without a comment goes with it. A key already absent writes nothing.
pub fn restore_setting_default(
    key: &[&str],
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsEditError> {
    edit(key, &Change::Restore, &env)
}

fn edit(
    key: &[&str],
    change: &Change<'_>,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, SettingsEditError> {
    let setting = find_setting(key).ok_or(SettingsEditError::UnknownKey)?;
    if setting.path.contains("[]") {
        return Err(SettingsEditError::UnknownKey);
    }
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
    let target = resolve_link(&path).map_err(io_failure)?;
    let pending = target.with_file_name(PENDING_NAME);
    let outcome = edit_locked(key, change, &path, &target, env, &mut read_target);
    if outcome.is_err() {
        let _ = fs::remove_file(&pending);
    }
    outcome.map(|()| path)
}

/// The file's bytes, `None` when it does not exist.
type TargetRead = Result<Option<Vec<u8>>, SettingsEditError>;

/// One edit under the held lock. `read` reads the target; every read goes through it so the
/// comparison before replacement sees exactly what another writer left.
fn edit_locked(
    key: &[&str],
    change: &Change<'_>,
    path: &Path,
    target: &Path,
    env: &impl Fn(&str) -> Option<OsString>,
    read: &mut dyn FnMut(&Path) -> TargetRead,
) -> Result<(), SettingsEditError> {
    let pending = target.with_file_name(PENDING_NAME);
    for _ in 0..2 {
        let before = read(target)?;
        let Some(after) = candidate(before.as_deref(), key, change, path, env)? else {
            return Ok(());
        };
        write_pending(target, &pending, after.as_bytes())?;
        if digest(read(target)?.as_deref()) != digest(before.as_deref()) {
            continue;
        }
        return platform::replace(&pending, target);
    }
    Err(SettingsEditError::Changed)
}

/// The edited text, or `None` when the file already says what the change asks for.
fn candidate(
    before: Option<&[u8]>,
    key: &[&str],
    change: &Change<'_>,
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
    let field = key
        .iter()
        .map(|segment| unknown_field("", segment))
        .collect::<Vec<_>>()
        .join(".");
    match change {
        Change::Set(value) => {
            let item = item(value).ok_or(SettingsEditError::InvalidValue {
                field: field.clone(),
                reason: "wrong-type",
            })?;
            // Comments that end the file stay above a table the edit adds there, so a file that
            // began as the printed reference gains its settings below the reference.
            let trailing = document.trailing().as_str().unwrap_or_default().to_owned();
            let mut leading = trailing
                .contains('#')
                .then(|| format!("{}\n\n", trailing.trim_end()));
            if leading.is_some() {
                document.set_trailing("");
            }
            set_in(document.as_table_mut(), key, item, "", &mut leading)?;
            if leading.is_some() {
                document.set_trailing(trailing);
            }
        }
        Change::Restore => {
            if !restore_in(document.as_table_mut(), key) {
                return Ok(None);
            }
        }
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
    let related = |candidate: &str| {
        candidate == field
            || candidate.starts_with(&format!("{field}."))
            || field.starts_with(&format!("{candidate}."))
    };
    if let Some(refused) = after_load.diagnostics.items().iter().find(|diagnostic| {
        related(&diagnostic.field) && !before_load.diagnostics.items().contains(diagnostic)
    }) {
        return Err(SettingsEditError::InvalidValue {
            field: refused.field.clone(),
            reason: refused.reason,
        });
    }
    Ok(Some(after))
}

/// The TOML item a value is written as: an array of tables keeps the `[[...]]` form, any other
/// value is written inline on its key's line.
fn item(value: &toml::Value) -> Option<Item> {
    if let toml::Value::Array(entries) = value
        && !entries.is_empty()
        && entries.iter().all(toml::Value::is_table)
    {
        let mut array = toml_edit::ArrayOfTables::new();
        for entry in entries {
            let mut table = toml_edit::Table::new();
            for (key, value) in entry.as_table()? {
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
/// inline comment; `leading` becomes the prefix of a table header the edit adds.
fn set_in(
    table: &mut dyn TableLike,
    segments: &[&str],
    mut item: Item,
    parent: &str,
    leading: &mut Option<String>,
) -> Result<(), SettingsEditError> {
    let Some((first, rest)) = segments.split_first() else {
        return Err(SettingsEditError::UnknownKey);
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
    let path = if parent.is_empty() {
        unknown_field("", first)
    } else {
        unknown_field(parent, first)
    };
    if table.get(first).is_none() {
        let mut child = toml_edit::Table::new();
        // Only the table that receives the key gets a header; tables above it stay implicit.
        child.set_implicit(rest.len() > 1);
        if rest.len() == 1
            && let Some(prefix) = leading.take()
        {
            child.decor_mut().set_prefix(prefix);
        }
        table.insert(first, Item::Table(child));
    }
    let child = table
        .get_mut(first)
        .and_then(Item::as_table_like_mut)
        .ok_or(SettingsEditError::InvalidValue {
            field: path.clone(),
            reason: "wrong-type",
        })?;
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

/// The file's bytes, or `None` when it does not exist. A read-only file is refused here, before
/// anything is written.
fn read_target(target: &Path) -> TargetRead {
    let file = match File::open(target) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_failure(error)),
    };
    let metadata = file.metadata().map_err(io_failure)?;
    if !metadata.is_file() {
        return Err(SettingsEditError::Source(SettingsError::new(
            "file", "not-file",
        )));
    }
    if metadata.permissions().readonly() {
        return Err(SettingsEditError::ReadOnly);
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(io_failure)?;
    if bytes.len() as u64 > LIMIT {
        return Err(SettingsEditError::Source(SettingsError::new(
            "file",
            "too-large",
        )));
    }
    Ok(Some(bytes))
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
