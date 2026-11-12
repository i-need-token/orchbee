//! The user `Path` edit of the Windows PATH step. One entry naming `<store root>\bin` goes at the
//! front when no entry names it, and entries naming another folder inside the store root, which the
//! earlier layout added, go in the same write. Every other entry, empty ones included, stays byte
//! for byte. Names compare case-insensitively, with or without surrounding quotes and trailing
//! separators; an entry with `.` or `..` segments names nothing this edit recognizes. Uninstall's
//! removal is the exact inverse: the entries this edit writes or recognizes go, and nothing else.

/// The longest value the edit writes, in UTF-16 units: the 32,767-character environment variable
/// limit less a margin, as the earlier layout's writer bounded it.
pub(super) const MAXIMUM_LENGTH: usize = 32_760;

/// How the user `Path` changes.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Edit {
    /// An entry already names `bin`, and none names another folder inside the store root.
    Unchanged,
    /// The value to write. `removed` is whether entries naming other folders inside the store root
    /// were left out.
    Write { value: String, removed: bool },
    /// The edited value would be longer than [`MAXIMUM_LENGTH`].
    TooLong,
    /// The root has `.` or `..` segments or a `\\?\` or `\\.\` prefix, so no entry written for it
    /// would be recognized by a later edit.
    UnnamedRoot,
}

/// `<root>\bin`, the folder the PATH step puts on the user `Path`.
pub(super) fn bin(root: &str) -> String {
    format!("{}\\bin", root.trim_end_matches(['\\', '/']))
}

/// The edit that makes `current` name [`bin`] of `root`.
pub(super) fn edit(current: &str, root: &str) -> Edit {
    let bin = bin(root);
    let Some(bin_key) =
        fold(&bin).filter(|key| !key.starts_with("\\\\?\\") && !key.starts_with("\\\\.\\"))
    else {
        return Edit::UnnamedRoot;
    };
    let root_key = fold(root).filter(|key| below_volume(key));
    let mut present = false;
    let mut removed = false;
    let mut kept = Vec::new();
    // An empty value holds no entry, not one empty entry.
    let entries = (!current.is_empty()).then(|| current.split(';'));
    for entry in entries.into_iter().flatten() {
        let key = fold(entry);
        if key.as_ref() == Some(&bin_key) {
            present = true;
        } else if let (Some(key), Some(root_key)) = (&key, &root_key)
            && key
                .strip_prefix(root_key.as_str())
                .is_some_and(|rest| rest.starts_with('\\'))
        {
            removed = true;
            continue;
        }
        kept.push(entry);
    }
    if present && !removed {
        return Edit::Unchanged;
    }
    if !present {
        kept.insert(0, &bin);
    }
    let value = kept.join(";");
    if value.encode_utf16().count() > MAXIMUM_LENGTH {
        return Edit::TooLong;
    }
    Edit::Write { value, removed }
}

/// The inverse of [`edit`] for uninstall: every entry naming [`bin`] of `root` or another folder
/// inside the store root goes, and every other entry, empty ones included, stays byte for byte.
/// `removed` is always `true` in the write; nothing to remove is [`Edit::Unchanged`].
pub(super) fn removal(current: &str, root: &str) -> Edit {
    let bin_key = fold(&bin(root));
    let root_key = fold(root).filter(|key| below_volume(key));
    let mut removed = false;
    let mut kept = Vec::new();
    let entries = (!current.is_empty()).then(|| current.split(';'));
    for entry in entries.into_iter().flatten() {
        let key = fold(entry);
        let inside = match (&key, &root_key) {
            (Some(key), Some(root_key)) => key
                .strip_prefix(root_key.as_str())
                .is_some_and(|rest| rest.starts_with('\\')),
            _ => false,
        };
        if inside || (key.is_some() && key == bin_key) {
            removed = true;
        } else {
            kept.push(entry);
        }
    }
    if removed {
        Edit::Write {
            value: kept.join(";"),
            removed,
        }
    } else {
        Edit::Unchanged
    }
}

/// The comparison key of one `Path` entry, or `None` when it names no folder this edit compares.
fn fold(entry: &str) -> Option<String> {
    let unquoted = entry
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(entry);
    let separated = unquoted.replace('/', "\\");
    let trimmed = separated.trim_end_matches('\\');
    if trimmed.is_empty()
        || trimmed.contains('"')
        || trimmed.split('\\').any(|part| part == "." || part == "..")
    {
        return None;
    }
    Some(
        trimmed
            .chars()
            .map(|character| {
                let mut upper = character.to_uppercase();
                match (upper.next(), upper.next()) {
                    (Some(single), None) => single,
                    _ => character,
                }
            })
            .collect(),
    )
}

/// Whether a folded root names a folder below a drive or share root, so that every folder inside it
/// belongs to the store alone.
fn below_volume(root: &str) -> bool {
    let rest = match root.strip_prefix("\\\\") {
        Some(share) => share.splitn(3, '\\').nth(2).unwrap_or(""),
        None if root.as_bytes().get(1) == Some(&b':') => &root[2..],
        None => root,
    };
    !rest.trim_matches('\\').is_empty()
}
