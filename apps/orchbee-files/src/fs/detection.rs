//! What a file's first bytes say about its content, by VS Code's `detectEncodingFromBuffer` with
//! guessing off: a byte order mark names UTF-8, UTF-16LE or UTF-16BE; otherwise a zero byte in the
//! first 512 bytes means UTF-16 when every zero sits where UTF-16 of one byte order puts them, and
//! binary content when none does.

use super::encoding::{self, Encoding};

/// How many leading bytes the zero-byte test reads; a read detects once it holds this many bytes
/// or the whole file.
pub(crate) const DETECTION_BYTES: usize = 512;

/// The facts a file's first bytes establish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Detection {
    /// The encoding the content names, by its byte order mark or its zero bytes.
    pub(crate) encoding: Option<&'static Encoding>,
    /// The byte order mark's length, which decoding skips; 0 without one.
    pub(crate) bom: usize,
    /// A zero byte that no UTF-16 pattern explains.
    pub(crate) binary: bool,
}

/// Detects what `head`, the file's first bytes, says. Only the first [`DETECTION_BYTES`] bytes
/// count.
pub(crate) fn detect(head: &[u8]) -> Detection {
    let (mut found, bom) = match head {
        [0xFE, 0xFF, ..] => (Some(&encoding::UTF16BE), 2),
        [0xFF, 0xFE, ..] => (Some(&encoding::UTF16LE), 2),
        [0xEF, 0xBB, 0xBF, ..] => (Some(&encoding::UTF8), 3),
        _ => (None, 0),
    };
    if bom == 2 {
        return Detection {
            encoding: found,
            bom,
            binary: false,
        };
    }
    // A zero byte rules out the byte order whose code units would not hold it there; VS Code
    // stops at the first zero byte that rules out both.
    let mut little_endian = true;
    let mut big_endian = true;
    let mut zero = false;
    for (index, &byte) in head.iter().take(DETECTION_BYTES).enumerate() {
        let odd = index % 2 == 1;
        let is_zero = byte == 0;
        zero |= is_zero;
        little_endian &= odd == is_zero;
        big_endian &= odd != is_zero;
        if is_zero && !little_endian && !big_endian {
            break;
        }
    }
    let mut binary = false;
    if zero {
        if little_endian {
            found = Some(&encoding::UTF16LE);
        } else if big_endian {
            found = Some(&encoding::UTF16BE);
        } else {
            binary = true;
        }
    }
    Detection {
        encoding: found,
        bom,
        binary,
    }
}

#[cfg(test)]
mod tests;
