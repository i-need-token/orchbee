//! The names under which this process stages a file beside its target while the file's data
//! arrives: a hidden `.<name>.orchbee-<16 lowercase hexadecimal digits>`, the name shortened so
//! the whole stays within what a filesystem holds. The watcher drops their events and a listing
//! leaves them out, so a staging file never shows; a Client sees the file once it is published.

use std::{fmt::Write as _, io};

/// The marker between a staging name's file name and its random digits.
const MARKER: &str = ".orchbee-";
/// The random digits a staging name ends with.
const DIGITS: usize = 16;
/// The most bytes of the file's name a staging name keeps, so it stays within 255.
const KEPT_BYTES: usize = 200;

/// Whether `name` is a staging name.
pub(crate) fn is_staging(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('.') else {
        return false;
    };
    let Some(digits) = rest.len().checked_sub(DIGITS).and_then(|at| rest.get(at..)) else {
        return false;
    };
    let before = &rest[..rest.len() - DIGITS];
    before.ends_with(MARKER)
        && digits
            .bytes()
            .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit))
}

/// A fresh staging name for a file named `name`.
///
/// # Errors
///
/// The operating system's failure to supply random digits.
pub(crate) fn name(name: &str) -> io::Result<String> {
    let mut bytes = [0; DIGITS / 2];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("no random digits"))?;
    let mut kept = name.len().min(KEPT_BYTES);
    while !name.is_char_boundary(kept) {
        kept -= 1;
    }
    let mut staged = format!(".{}{MARKER}", &name[..kept]);
    for byte in bytes {
        // Writing to a string cannot fail.
        let _ = write!(staged, "{byte:02x}");
    }
    Ok(staged)
}

#[cfg(test)]
mod tests;
