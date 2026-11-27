//! `fs.guess-encoding`: the encoding a file's first 64 KiB suggest, within the set VS Code
//! guesses. A byte order mark or UTF-16 zero bytes decide first, as for a read; binary content and
//! ASCII alone suggest nothing, as VS Code ignores jschardet's `ascii`. Otherwise chardetng
//! guesses, and its answer maps to the VS Code identifier of that encoding, or to none when VS
//! Code does not guess it. chardetng names KOI8 text KOI8-U, which VS Code guesses as `koi8r`, and
//! Big5 maps to `cp950` as VS Code maps jschardet's `big5`.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
};

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use orchbee_files_protocol::{ErrorCode, FilesError};

use super::detection::detect;
use crate::error;

/// The most a guess reads, VS Code's limit for jschardet.
const GUESS_BYTES: usize = 64 * 1024;

/// chardetng's answers that VS Code also guesses, with VS Code's identifiers.
const GUESSABLE: [(&encoding_rs::Encoding, &str); 19] = [
    (encoding_rs::UTF_8, "utf8"),
    (encoding_rs::WINDOWS_1252, "windows1252"),
    (encoding_rs::WINDOWS_1250, "windows1250"),
    (encoding_rs::ISO_8859_2, "iso88592"),
    (encoding_rs::WINDOWS_1251, "windows1251"),
    (encoding_rs::IBM866, "cp866"),
    (encoding_rs::ISO_8859_5, "iso88595"),
    (encoding_rs::KOI8_R, "koi8r"),
    (encoding_rs::KOI8_U, "koi8r"),
    (encoding_rs::WINDOWS_1253, "windows1253"),
    (encoding_rs::ISO_8859_7, "iso88597"),
    (encoding_rs::WINDOWS_1255, "windows1255"),
    (encoding_rs::ISO_8859_8, "iso88598"),
    (encoding_rs::ISO_8859_8_I, "iso88598"),
    (encoding_rs::GBK, "gbk"),
    (encoding_rs::BIG5, "cp950"),
    (encoding_rs::SHIFT_JIS, "shiftjis"),
    (encoding_rs::EUC_JP, "eucjp"),
    (encoding_rs::EUC_KR, "euckr"),
];

/// The encoding the file at `path` suggests.
///
/// # Errors
///
/// `is-directory`, `failed` for what is neither file nor directory, or the system's error.
pub(crate) fn guess_file(path: &Path) -> Result<Option<&'static str>, FilesError> {
    let failed = |failure: io::Error| error::from_io(&failure);
    let found = fs::metadata(path).map_err(failed)?;
    if found.is_dir() {
        return Err(FilesError::new(ErrorCode::IsDirectory));
    }
    if !found.is_file() {
        return Err(FilesError::new(ErrorCode::Failed));
    }
    let mut content = Vec::with_capacity(GUESS_BYTES);
    File::open(path)
        .and_then(|file| file.take(GUESS_BYTES as u64).read_to_end(&mut content))
        .map_err(failed)?;
    Ok(guess(&content, content.len() < GUESS_BYTES))
}

/// The encoding `content`, the start of a file, suggests; `whole` when it is the whole file.
pub(crate) fn guess(content: &[u8], whole: bool) -> Option<&'static str> {
    let detection = detect(content);
    if detection.binary {
        return None;
    }
    if let Some(encoding) = detection.encoding {
        return Some(encoding.name());
    }
    if content.is_ascii() {
        return None;
    }
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
    detector.feed(content, whole);
    guessable(detector.guess(None, Utf8Detection::Allow))
}

fn guessable(encoding: &'static encoding_rs::Encoding) -> Option<&'static str> {
    GUESSABLE
        .iter()
        .find(|(candidate, _)| *candidate == encoding)
        .map(|(_, name)| *name)
}

#[cfg(test)]
mod tests;
