//! Pure naming and framing for the Session's earlier-layout history. Each entry is one complete
//! opaque checkpoint payload named by its save time; only the Windows storage adapter performs
//! filesystem effects and the Server owns what a payload means and when one is written.
use crate::checkpoint_rotation::{Candidate, HEADER_BYTES, encode};
use std::num::NonZeroU64;

/// The history folder beside the three checkpoint slots.
pub(crate) const DIRECTORY: &str = "layout-history";
/// The one staging file a recording writes before its atomic rename.
pub(crate) const PENDING: &str = "layout.pending";
const PREFIX: &str = "layout-";
const SUFFIX: &str = ".json";

/// The exact file name of the entry saved at `saved_at`.
pub(crate) fn entry_name(saved_at: NonZeroU64) -> String {
    format!("{PREFIX}{:016x}{SUFFIX}", saved_at.get())
}

/// The save time an exact entry name carries; any other name is not an entry.
pub(crate) fn entry_time(name: &str) -> Option<NonZeroU64> {
    let hex = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    if hex.len() != 16
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    u64::from_str_radix(hex, 16).ok().and_then(NonZeroU64::new)
}

/// Whether `name` belongs in the history folder: an entry or the staging file.
pub(crate) fn is_owned_name(name: &str) -> bool {
    name == PENDING || entry_time(name).is_some()
}

/// The checkpoint frame, with the save time as its generation so a renamed file cannot pass.
pub(crate) fn frame(saved_at: NonZeroU64, payload: &[u8]) -> Vec<u8> {
    encode(payload, saved_at)
}

/// The payload of a complete frame whose generation is `saved_at`.
pub(crate) fn unframe(saved_at: NonZeroU64, bytes: &[u8], maximum: usize) -> Option<Vec<u8>> {
    match Candidate::decode(bytes, maximum) {
        Candidate::Valid {
            generation,
            payload,
        } if generation == saved_at.get() => Some(payload),
        _ => None,
    }
}

/// The largest frame a reader admits for a `maximum`-byte payload.
pub(crate) const fn frame_limit(maximum: usize) -> usize {
    maximum + HEADER_BYTES + 1
}
