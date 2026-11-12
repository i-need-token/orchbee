//! The tokens release records admit as values: product versions, which also name store version
//! directories, and SHA-256 digests.

/// Whether `text` is a SemVer 2.0.0 version of at most 64 bytes without build metadata, so a
/// manifest admits it and it may name a store version directory.
pub fn valid_version(text: &str) -> bool {
    if text.is_empty() || text.len() > 64 {
        return false;
    }
    let (core, prerelease) = match text.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (text, None),
    };
    let core: Vec<&str> = core.split('.').collect();
    if core.len() != 3 || !core.iter().all(|part| numeric_identifier(part)) {
        return false;
    }
    prerelease.is_none_or(|prerelease| {
        prerelease.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && (!part.bytes().all(|b| b.is_ascii_digit()) || numeric_identifier(part))
        })
    })
}

/// Digits without a leading zero, except `0` itself.
fn numeric_identifier(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|b| b.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
}

/// Whether `text` is a SHA-256 digest in 64 lowercase hexadecimal digits.
pub fn lowercase_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
