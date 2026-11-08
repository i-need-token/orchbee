//! Spells the names the plugin store gives what it keeps: a plugin's folder from its id and a
//! managed source's folder from its manifest version, and the cleanup key of each source folder.
//! A name reads as the text it comes from, made valid on every file system; the store appends the
//! next free `-2`, `-3`, and so on when a name is taken. It does not decide which candidate is
//! free or record which folder a plugin received.

use crate::data_root::PluginStorePaths;
use std::path::PathBuf;

/// The longest name spelled from an id or version, before a `-<n>` suffix, so a source path stays
/// well inside the Windows path limits that Git and build tools still apply.
const NAME_LIMIT: usize = 64;
/// How many candidates one name may try before the store reports its limit.
const CANDIDATES: usize = 64;

/// The spelling of `text` as one folder name: ASCII letters, digits, `.`, `_`, and `-` stay, each
/// run of other characters becomes one `-`, leading and trailing `.` and `-` go, and the result
/// is bounded. A Windows device name in any letter case, with or without an extension, gains `_`
/// after its stem. `fallback` names what leaves nothing.
pub(super) fn spelling(text: &str, fallback: &str) -> String {
    let mut name = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            name.push(character);
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let mut name = trim(&name).to_owned();
    if name.len() > NAME_LIMIT {
        name.truncate(NAME_LIMIT);
        name = trim(&name).to_owned();
    }
    if name.is_empty() {
        name = fallback.to_owned();
    }
    let stem = name.split('.').next().unwrap_or_default();
    if device(stem) {
        name.insert(stem.len(), '_');
    }
    name
}

/// The names to try for `base`, in order: `base`, `base-2`, `base-3`, and so on.
pub(super) fn candidates(base: &str) -> impl Iterator<Item = String> + '_ {
    std::iter::once(base.to_owned()).chain((2..=CANDIDATES).map(move |n| format!("{base}-{n}")))
}

/// Whether `name` is a folder name that `spelling` and `candidates` can produce.
pub(super) fn valid(name: &str) -> bool {
    // A spelled name may gain a device `_` and a `-64` suffix.
    !name.is_empty()
        && name.len() <= NAME_LIMIT + 4
        && trim(name) == name
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !device(name.split('.').next().unwrap_or_default())
}

fn trim(name: &str) -> &str {
    name.trim_matches(|c| c == '.' || c == '-')
}

/// Windows refuses these names, and treats them with any extension as the device too.
fn device(stem: &str) -> bool {
    let stem = stem.to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|digit| digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit())
}

/// A source folder recorded before it is created, so a crash leaves it discoverable: the source a
/// running install fetches, or one version of a plugin's managed source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Generation {
    Pending,
    Source { folder: String, version: String },
}

impl Generation {
    /// Reads a recorded key: `source.pending`, or `<folder>/source/<version>` with valid names.
    pub(super) fn parse(key: &str) -> Option<Self> {
        if key == PluginStorePaths::PENDING_SOURCE {
            return Some(Self::Pending);
        }
        let mut parts = key.split('/');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(folder), Some(PluginStorePaths::SOURCES), Some(version), None)
                if valid(folder) && valid(version) && !PluginStorePaths::reserved(folder) =>
            {
                Some(Self::Source {
                    folder: folder.to_owned(),
                    version: version.to_owned(),
                })
            }
            _ => None,
        }
    }

    pub(super) fn key(&self) -> String {
        match self {
            Self::Pending => PluginStorePaths::PENDING_SOURCE.to_owned(),
            Self::Source { folder, version } => {
                format!("{folder}/{}/{version}", PluginStorePaths::SOURCES)
            }
        }
    }

    pub(super) fn path(&self, paths: &PluginStorePaths) -> PathBuf {
        match self {
            Self::Pending => paths.pending_source(),
            Self::Source { folder, version } => paths.source(folder, version),
        }
    }

    /// Whether both name the same folder on Windows, where letter case does not distinguish names.
    pub(super) fn same_folder(&self, other: &Self) -> bool {
        self.key().eq_ignore_ascii_case(&other.key())
    }
}
