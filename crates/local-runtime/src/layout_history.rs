//! Pure naming and framing for the Session's earlier-layout history. Each entry is one complete
//! opaque checkpoint payload named by the UTC second it was saved, `-2`, `-3` and so on when an
//! entry with different content already holds that second; only the Windows storage adapter
//! performs filesystem effects and the Server owns what a payload means and when one is written.
use crate::checkpoint_rotation::{Candidate, HEADER_BYTES, encode};
use std::num::NonZeroU64;

/// The history folder beside the three checkpoint slots.
pub(crate) const DIRECTORY: &str = "layouts";
/// The one staging file a recording writes before its atomic rename.
pub(crate) const PENDING: &str = "layout.pending";
const SUFFIX: &str = ".json";

/// The file name of an entry saved at `saved_at` milliseconds, with `number` 1 for the plain time.
pub(crate) fn entry_name(saved_at: NonZeroU64, number: u32) -> String {
    format!(
        "{}{SUFFIX}",
        crate::time_name::format(saved_at.get() / 1000, number)
    )
}

/// The UTC second and number an exact entry name carries; any other name is not an entry.
pub(crate) fn entry_key(name: &str) -> Option<(u64, u32)> {
    crate::time_name::parse(name.strip_suffix(SUFFIX)?)
}

/// Whether `name` belongs in the history folder: an entry or the staging file.
pub(crate) fn is_owned_name(name: &str) -> bool {
    name == PENDING || entry_key(name).is_some()
}

/// The checkpoint frame, with the save time in milliseconds as its generation.
pub(crate) fn frame(saved_at: NonZeroU64, payload: &[u8]) -> Vec<u8> {
    encode(payload, saved_at)
}

/// The save time and payload of a complete frame saved during `second`, the second its name
/// carries, so a renamed file cannot pass.
pub(crate) fn unframe(second: u64, bytes: &[u8], maximum: usize) -> Option<(NonZeroU64, Vec<u8>)> {
    match Candidate::decode(bytes, maximum) {
        Candidate::Valid {
            generation,
            payload,
        } if generation / 1000 == second => NonZeroU64::new(generation).map(|at| (at, payload)),
        _ => None,
    }
}

/// The largest frame a reader admits for a `maximum`-byte payload.
pub(crate) const fn frame_limit(maximum: usize) -> usize {
    maximum + HEADER_BYTES + 1
}
