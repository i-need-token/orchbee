//! Pure naming and framing for kept terminal output. Each file is one opaque payload named by a
//! nonzero token the Server chooses; only the Windows storage adapter performs filesystem effects
//! and the Server owns what a payload means, which tokens its checkpoint names and when to write.
use crate::checkpoint_rotation::{Candidate, HEADER_BYTES, encode};
use std::num::NonZeroU64;

/// The output folder beside the three checkpoint slots.
pub(crate) const DIRECTORY: &str = "terminal-output";
/// The one staging file a write uses before its atomic rename.
pub(crate) const PENDING: &str = "output.pending";
/// The largest payload: 1 MiB of output plus the Server's small identity header.
pub(crate) const MAXIMUM_BYTES: usize = 1024 * 1024 + 1024;
const PREFIX: &str = "output-";
const SUFFIX: &str = ".bin";

pub(crate) fn entry_name(token: NonZeroU64) -> String {
    format!("{PREFIX}{:016x}{SUFFIX}", token.get())
}

/// The token an exact entry name carries; any other name is not an entry.
pub(crate) fn entry_token(name: &str) -> Option<NonZeroU64> {
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

/// Whether `name` belongs in the output folder: an entry or the staging file.
pub(crate) fn is_owned_name(name: &str) -> bool {
    name == PENDING || entry_token(name).is_some()
}

/// The checkpoint frame, with the token as its generation so a renamed file cannot pass.
pub(crate) fn frame(token: NonZeroU64, payload: &[u8]) -> Vec<u8> {
    encode(payload, token)
}

pub(crate) fn unframe(token: NonZeroU64, bytes: &[u8]) -> Option<Vec<u8>> {
    match Candidate::decode(bytes, MAXIMUM_BYTES) {
        Candidate::Valid {
            generation,
            payload,
        } if generation == token.get() => Some(payload),
        _ => None,
    }
}

pub(crate) const FRAME_LIMIT: usize = MAXIMUM_BYTES + HEADER_BYTES + 1;
