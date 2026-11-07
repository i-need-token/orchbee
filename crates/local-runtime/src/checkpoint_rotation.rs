//! Pure framing and slot selection; only the Windows storage adapter performs filesystem effects.
use sha2::{Digest, Sha256};
use std::{io, num::NonZeroU64};

pub(super) const SLOTS: [&str; 3] = [
    "checkpoint.json",
    "checkpoint.previous-1.json",
    "checkpoint.previous-2.json",
];
pub(super) const HEADER_BYTES: usize = 45;
const PREFIX: &[u8; 4] = b"OBCK";
const MAGIC: &[u8; 5] = b"OBCK\x01";

#[derive(Debug)]
pub(super) enum Candidate {
    Missing,
    Invalid { generation: Option<u64> },
    Valid { generation: u64, payload: Vec<u8> },
}

impl Candidate {
    pub(super) fn decode(bytes: &[u8], maximum: usize) -> Self {
        if bytes.starts_with(PREFIX) {
            let generation = (bytes.get(..5) == Some(MAGIC.as_slice()))
                .then(|| bytes.get(5..13))
                .flatten()
                .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
                .map(u64::from_le_bytes)
                .filter(|generation| *generation != 0);
            if let Some(generation) = generation
                && let Some(payload) = bytes.get(HEADER_BYTES..)
                && payload.len() <= maximum
            {
                let mut digest = Sha256::new();
                digest.update(&bytes[..13]);
                digest.update(payload);
                if bytes[13..HEADER_BYTES] != digest.finalize()[..] {
                    return Self::Invalid {
                        generation: Some(generation),
                    };
                }
                return Self::Valid {
                    generation,
                    payload: payload.to_vec(),
                };
            }
            Self::Invalid { generation }
        } else {
            Self::Invalid { generation: None }
        }
    }

    pub(super) fn payload(&self) -> Option<&[u8]> {
        match self {
            Self::Valid { payload, .. } => Some(payload),
            Self::Missing | Self::Invalid { .. } => None,
        }
    }

    pub(super) fn generation(&self) -> Option<u64> {
        match self {
            Self::Valid { generation, .. } => Some(*generation),
            Self::Invalid { generation } => *generation,
            Self::Missing => None,
        }
    }

    fn reject(&mut self) {
        *self = Self::Invalid {
            generation: self.generation(),
        };
    }
}

/// Validate once per complete payload; generation conflicts never select an arbitrary copy.
pub(super) fn validate<T>(
    candidates: &mut [Candidate; 3],
    mut parse: impl FnMut(&[u8]) -> Option<T>,
) -> [Option<T>; 3] {
    let mut values = std::array::from_fn(|_| None);
    for (index, candidate) in candidates.iter_mut().enumerate() {
        let value = match candidate.payload() {
            Some(bytes) => parse(bytes),
            None => None,
        };
        if value.is_none() && matches!(candidate, Candidate::Valid { .. }) {
            candidate.reject();
        }
        values[index] = value;
    }
    let mut conflicting = [false; 3];
    for first in 0..3 {
        for second in (first + 1)..3 {
            if candidates[first].payload().is_some()
                && candidates[second].payload().is_some()
                && candidates[first].generation() == candidates[second].generation()
                && candidates[first].payload() != candidates[second].payload()
            {
                conflicting[first] = true;
                conflicting[second] = true;
            }
        }
    }
    for (index, conflict) in conflicting.into_iter().enumerate() {
        if conflict {
            candidates[index].reject();
            values[index] = None;
        }
    }
    values
}

pub(super) fn latest(candidates: &[Candidate; 3]) -> Option<usize> {
    (0..3)
        .filter(|index| candidates[*index].payload().is_some())
        .max_by_key(|index| candidates[*index].generation())
}

pub(super) struct Publication {
    pub(super) slot: usize,
    pub(super) preserve_invalid: bool,
    pub(super) encoded: Vec<u8>,
}

pub(super) fn prepare(
    candidates: &[Candidate; 3],
    bytes: &[u8],
    maximum: usize,
) -> io::Result<Option<Publication>> {
    if bytes.len() > maximum {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    if latest(candidates).is_some_and(|index| candidates[index].payload() == Some(bytes)) {
        return Ok(None);
    }
    let generation = candidates
        .iter()
        .filter_map(|candidate| match candidate {
            Candidate::Valid { generation, .. } => Some(*generation),
            Candidate::Missing | Candidate::Invalid { .. } => None,
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .and_then(NonZeroU64::new)
        .ok_or(io::ErrorKind::InvalidData)?;
    let slot = candidates
        .iter()
        .position(|candidate| matches!(candidate, Candidate::Missing))
        .or_else(|| {
            candidates
                .iter()
                .position(|candidate| matches!(candidate, Candidate::Invalid { .. }))
        })
        .unwrap_or_else(|| {
            (0..3)
                .min_by_key(|index| candidates[*index].generation())
                .expect("three present valid slots")
        });
    Ok(Some(Publication {
        slot,
        preserve_invalid: matches!(candidates[slot], Candidate::Invalid { .. }),
        encoded: encode(bytes, generation),
    }))
}

pub(super) fn encode(bytes: &[u8], generation: NonZeroU64) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(HEADER_BYTES + bytes.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&generation.get().to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(&encoded);
    digest.update(bytes);
    encoded.extend_from_slice(&digest.finalize());
    encoded.extend_from_slice(bytes);
    encoded
}

pub(super) fn is_corrupt_original(name: &str) -> bool {
    name.strip_prefix("checkpoint.corrupt-")
        .and_then(|name| name.strip_suffix(".raw"))
        .is_some_and(|nonce| {
            nonce.len() == 32
                && nonce
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                && nonce.bytes().any(|byte| byte != b'0')
        })
}

#[cfg(test)]
mod tests;
