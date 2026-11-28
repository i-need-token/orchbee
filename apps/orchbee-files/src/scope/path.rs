//! The path forms a Client may name on this machine and how they compare. A Client path is
//! absolute and native, without an empty, `.` or `..` segment: on macOS and Linux it starts with
//! `/`; on Windows it starts with a drive letter, a colon and `\`, uses `\` alone, and has no
//! segment Win32 would alias, such as one with a trailing dot or space, or open as something other
//! than the named file, such as a device or an alternate data stream. A UNC path is refused as
//! unsupported, as the Worker refuses one. Segments compare as the machine's default volumes do:
//! bytewise on Linux, and on Windows and macOS ignoring case by simple per-character folding.
//! Names are never normalized.

#[cfg(windows)]
use std::path::Prefix;
use std::{
    ffi::OsStr,
    io,
    path::{Component, Path},
};

use orchbee_files_protocol::{ErrorCode, FilesError, OperatingSystem, Separator};

/// A path whose form this machine admits from a Client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativePath(String);

impl NativePath {
    /// Admits `path` when its form is one a Client may name here.
    ///
    /// # Errors
    ///
    /// `unsupported-location` for a UNC path, and `invalid-request` for any other form this
    /// machine does not admit.
    pub(crate) fn parse(path: &str) -> Result<Self, FilesError> {
        form(path).map(|()| Self(path.to_owned()))
    }

    /// The path as the machine spells it.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// The path for a filesystem call.
    pub(crate) fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    /// The final segment; empty for a filesystem root.
    pub(crate) fn name(&self) -> &str {
        self.segments().last().unwrap_or_default()
    }

    /// The folder that holds this path; `None` for a filesystem root.
    pub(crate) fn parent(&self) -> Option<Self> {
        let name = self.name();
        if name.is_empty() {
            return None;
        }
        let parent = &self.0[..self.0.len() - name.len()];
        // Below the root's own separator, the separator before the name goes with it.
        let parent = if parent.len() > ROOT_BYTES {
            &parent[..parent.len() - 1]
        } else {
            parent
        };
        Some(Self(parent.to_owned()))
    }

    /// How many segments lie below the filesystem root; 0 for the root itself.
    pub(crate) fn depth(&self) -> usize {
        self.segments().count()
    }

    /// Whether this path is `member` or lies below it, segment by segment.
    pub(crate) fn is_within(&self, member: &Self) -> bool {
        let mut segments = self.segments();
        same_volume(&self.0, &member.0)
            && member
                .segments()
                .all(|expected| segments.next().is_some_and(|actual| same(actual, expected)))
    }

    /// The segments of this path below `member`, when it is within it.
    pub(crate) fn below<'a>(&'a self, member: &Self) -> Option<impl Iterator<Item = &'a str>> {
        self.is_within(member)
            .then(|| self.segments().skip(member.segments().count()))
    }

    /// A key that two paths share exactly when each is within the other: the same location by the
    /// machine's comparison.
    pub(crate) fn key(&self) -> String {
        if CASE_SENSITIVE {
            self.0.clone()
        } else {
            self.0.chars().map(fold).collect()
        }
    }

    /// The segments below the root: the drive letter's or `/`'s.
    fn segments(&self) -> impl Iterator<Item = &str> {
        let below = &self.0[ROOT_BYTES..];
        below
            .split(SEPARATOR_CHARACTER)
            .filter(move |_| !below.is_empty())
    }
}

/// Where this machine's real location of `path` is: links, junctions, 8.3 names and, on Windows,
/// case resolved by the operating system. It can block on a network volume, so it runs where
/// waiting is allowed.
///
/// # Errors
///
/// The operating system's failure, and [`io::ErrorKind::Unsupported`] for a real location that
/// is not a form [`NativePath::parse`] admits, such as one on a UNC share.
pub(crate) fn real(path: &NativePath) -> io::Result<NativePath> {
    let real = std::fs::canonicalize(path.as_path())?;
    let real = real
        .to_str()
        .ok_or_else(|| io::Error::from(io::ErrorKind::Unsupported))?;
    // Windows answers in the verbatim form, which a Client never names.
    #[cfg(windows)]
    let real = real.strip_prefix(r"\\?\").unwrap_or(real);
    NativePath::parse(real).map_err(|_| io::ErrorKind::Unsupported.into())
}

/// Whether the real location `path` is the real location `member` or lies below it, component by
/// component. Both are spelled as [`std::fs::canonicalize`] reads them, so on Windows they compare
/// exactly: that spelling is `GetFinalPathNameByHandleW`'s, which resolves junctions, 8.3 names and
/// case, and only a drive letter's case may differ. On macOS names compare ignoring case, as
/// [`NativePath::is_within`] compares them, and on Linux bytewise; a name that is not UTF-8
/// compares bytewise everywhere. Names are never normalized.
pub(crate) fn real_within(path: &Path, member: &Path) -> bool {
    let mut components = path.components();
    member.components().all(|expected| {
        components
            .next()
            .is_some_and(|actual| same_component(actual, expected))
    })
}

fn same_component(actual: Component<'_>, expected: Component<'_>) -> bool {
    match (actual, expected) {
        (Component::Normal(actual), Component::Normal(expected)) => same_real(actual, expected),
        #[cfg(windows)]
        (Component::Prefix(actual), Component::Prefix(expected)) => {
            same_prefix(actual.kind(), expected.kind())
        }
        (actual, expected) => actual == expected,
    }
}

#[cfg(target_os = "macos")]
fn same_real(actual: &OsStr, expected: &OsStr) -> bool {
    match (actual.to_str(), expected.to_str()) {
        (Some(actual), Some(expected)) => same(actual, expected),
        _ => actual == expected,
    }
}

#[cfg(not(target_os = "macos"))]
fn same_real(actual: &OsStr, expected: &OsStr) -> bool {
    actual == expected
}

/// A drive letter compares ignoring case, whichever form spells it; any other prefix exactly.
#[cfg(windows)]
fn same_prefix(actual: Prefix<'_>, expected: Prefix<'_>) -> bool {
    match (actual, expected) {
        (
            Prefix::Disk(actual) | Prefix::VerbatimDisk(actual),
            Prefix::Disk(expected) | Prefix::VerbatimDisk(expected),
        ) => actual.eq_ignore_ascii_case(&expected),
        (actual, expected) => actual == expected,
    }
}

/// Whether this machine's default volumes compare names case-sensitively, as the root facts
/// report it: Linux does; Windows and macOS do not, as VS Code assumes.
pub(crate) const CASE_SENSITIVE: bool = cfg!(not(any(windows, target_os = "macos")));

/// The operating system whose path rules this build applies, as the root facts report it.
#[cfg(windows)]
pub(crate) const OPERATING_SYSTEM: OperatingSystem = OperatingSystem::Windows;
#[cfg(target_os = "macos")]
pub(crate) const OPERATING_SYSTEM: OperatingSystem = OperatingSystem::Macos;
#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) const OPERATING_SYSTEM: OperatingSystem = OperatingSystem::Linux;

/// The separator between segments, as the root facts report it.
#[cfg(windows)]
pub(crate) const SEPARATOR: Separator = Separator::Backslash;
#[cfg(not(windows))]
pub(crate) const SEPARATOR: Separator = Separator::Slash;

const SEPARATOR_CHARACTER: char = match SEPARATOR {
    Separator::Backslash => '\\',
    Separator::Slash => '/',
};
/// `C:\` on Windows, `/` elsewhere.
#[cfg(windows)]
const ROOT_BYTES: usize = 3;
#[cfg(not(windows))]
const ROOT_BYTES: usize = 1;

fn invalid() -> FilesError {
    FilesError::new(ErrorCode::InvalidRequest)
}

#[cfg(not(windows))]
fn form(path: &str) -> Result<(), FilesError> {
    let below = path.strip_prefix('/').ok_or_else(invalid)?;
    if below.is_empty()
        || below
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

#[cfg(windows)]
fn form(path: &str) -> Result<(), FilesError> {
    if let Some(prefixed) = path.strip_prefix(r"\\") {
        // The verbatim and device namespaces reach what Win32 paths cannot; any other form is UNC.
        return Err(
            if prefixed.starts_with(r"?\") || prefixed.starts_with(r".\") {
                invalid()
            } else {
                FilesError::new(ErrorCode::UnsupportedLocation)
            },
        );
    }
    let [drive, b':', b'\\', ..] = path.as_bytes() else {
        return Err(invalid());
    };
    let below = &path[ROOT_BYTES..];
    if drive.is_ascii_alphabetic() && (below.is_empty() || below.split('\\').all(admitted_segment))
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// A segment Win32 opens as exactly the named file: nonempty, without a character Windows refuses
/// in names, a stream separator, or a trailing dot or space, which Win32 strips, and not a device
/// name, which Win32 opens in every folder.
#[cfg(windows)]
fn admitted_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.ends_with(['.', ' '])
        && !segment
            .chars()
            .any(|character| character < ' ' || "/:*?\"<>|".contains(character))
        && !device_name(segment)
}

#[cfg(windows)]
fn device_name(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let mut characters = stem.chars();
    let prefix: String = characters.by_ref().take(3).collect();
    matches!(prefix.as_str(), "COM" | "LPT")
        && matches!(
            (characters.next(), characters.next()),
            (Some('0'..='9' | '¹' | '²' | '³'), None)
        )
}

/// Whether both paths are on the same volume root: the same drive letter on Windows.
#[cfg(windows)]
fn same_volume(path: &str, member: &str) -> bool {
    path.as_bytes()[0].eq_ignore_ascii_case(&member.as_bytes()[0])
}

#[cfg(not(windows))]
const fn same_volume(_path: &str, _member: &str) -> bool {
    true
}

fn same(actual: &str, expected: &str) -> bool {
    if CASE_SENSITIVE {
        actual == expected
    } else {
        actual.chars().map(fold).eq(expected.chars().map(fold))
    }
}

/// The character's simple lowercase mapping, or the character itself where lowercasing it takes
/// more than one character.
fn fold(character: char) -> char {
    let mut lower = character.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => character,
    }
}

#[cfg(test)]
mod tests;
