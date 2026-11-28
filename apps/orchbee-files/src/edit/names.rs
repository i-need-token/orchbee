//! The names a mutation gives an item: whether this machine can hold a name, by the path forms a
//! Client may name here, and VS Code's next free name for a copy. Whether a name is already taken
//! is the filesystem's to say, by its own comparison: case-insensitive on Windows and macOS,
//! where macOS also compares Unicode forms as equal, and bytewise on Linux.

use orchbee_files_protocol::{ErrorCode, FilePath, FilesError, MAXIMUM_PATH_BYTES, Separator};

use crate::scope::path::{self, NativePath};

const SEPARATOR: char = match path::SEPARATOR {
    Separator::Backslash => '\\',
    Separator::Slash => '/',
};

/// The largest copy number VS Code increments, `Constants.MAX_SAFE_SMALL_INTEGER`.
const MAXIMUM_COPY_NUMBER: u64 = 1 << 30;

/// Admits `path` for a mutation that gives its final segment as a new name.
///
/// # Errors
///
/// `invalid-name` when the folder is a form this machine admits and the name is not one it can
/// hold: empty, `.` or `..`, and on Windows a name with a character Windows refuses, a trailing
/// dot or space, or a device name; otherwise the error of the path's form.
pub(super) fn created(path: &FilePath) -> Result<NativePath, FilesError> {
    let refused = match NativePath::parse(path.as_str()) {
        Ok(path) => return Ok(path),
        Err(refused) => refused,
    };
    let text = path.as_str();
    let folder = text.rfind(SEPARATOR).and_then(|at| {
        // A folder at a filesystem root keeps the root's own separator.
        NativePath::parse(&text[..at])
            .or_else(|_| NativePath::parse(&text[..=at]))
            .ok()
    });
    match folder {
        Some(_) if refused.code == ErrorCode::InvalidRequest => {
            Err(FilesError::new(ErrorCode::InvalidName))
        }
        _ => Err(refused),
    }
}

/// The item named `name` in `folder`.
///
/// # Errors
///
/// `invalid-name` for a name this machine cannot hold in a folder, as [`created`] refuses one, for
/// a name holding a separator, and for a path longer than a Client may name.
pub(super) fn child(folder: &NativePath, name: &str) -> Result<NativePath, FilesError> {
    let invalid = || FilesError::new(ErrorCode::InvalidName);
    if name.contains(['/', SEPARATOR]) {
        return Err(invalid());
    }
    let joined = if folder.parent().is_some() {
        format!("{}{SEPARATOR}{name}", folder.as_str())
    } else {
        format!("{}{name}", folder.as_str())
    };
    if joined.len() > MAXIMUM_PATH_BYTES {
        return Err(invalid());
    }
    NativePath::parse(&joined).map_err(|_| invalid())
}

/// VS Code's `simple` incremental name for a copy of `name`, which `incrementFileName` computes
/// in `fileActions.ts`: `name copy`, then `name copy 2`, `name copy 3` and on, before a file's
/// extension, which a folder's name never has. `copy` is file-name data, never localized.
pub(super) fn increment(name: &str, folder: bool) -> String {
    let (stem, extension) = if folder {
        (name, "")
    } else {
        split_extension(name)
    };
    let Some((copy, number)) = numbered_copy(stem) else {
        return format!("{stem} copy{extension}");
    };
    let next = match number {
        None => format!("{copy} 2"),
        // ` copy 0` names the plain copy.
        Some((_, 0)) => copy.to_owned(),
        Some((_, number)) if number < MAXIMUM_COPY_NUMBER => format!("{copy} {}", number + 1),
        Some((digits, _)) => format!("{copy} {digits} copy"),
    };
    format!("{next}{extension}")
}

/// `name` split before its extension, as VS Code's `extname` splits it: from its last dot, unless
/// that dot starts the name or the name is `..`.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 && name != ".." => name.split_at(at),
        _ => (name, ""),
    }
}

/// How `stem` matches VS Code's `/^(.+ copy)( \d+)?$/`: the part through ` copy`, and the digits
/// after it with their value, which saturates past what VS Code compares.
fn numbered_copy(stem: &str) -> Option<(&str, Option<(&str, u64)>)> {
    let numbered = stem.rsplit_once(' ').and_then(|(copy, digits)| {
        let value = digits
            .bytes()
            .try_fold(0_u64, |value, digit| {
                digit.is_ascii_digit().then(|| {
                    value
                        .saturating_mul(10)
                        .saturating_add(u64::from(digit - b'0'))
                })
            })
            .filter(|_| !digits.is_empty())?;
        copied(copy).then_some((copy, Some((digits, value))))
    });
    numbered.or_else(|| copied(stem).then_some((stem, None)))
}

/// Whether `text` ends with ` copy` after at least one character, none of which ends a line, as
/// the pattern's `.+` reads them.
fn copied(text: &str) -> bool {
    text.strip_suffix(" copy").is_some_and(|before| {
        !before.is_empty() && !before.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
    })
}

#[cfg(test)]
mod tests;
