//! Server-private minting and verification of `agent list` page cursors.
//!
//! The HMAC key and the canonical-runtime instance identity are drawn from
//! the operating system entropy source and live only for the owning runtime instance. Cursors minted
//! under a previous boot can never verify — they fail the MAC check before any
//! semantic field is read.
//!
//! Verification order is fixed: shape, canonical base64url, constant-time MAC,
//! then version, operation, flags, reserved fields, instance, revision, and
//! finally position. Authentic cursors minted by this same instance against an
//! older revision report `Stale`; everything else reports `Invalid`.

use std::io;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use orchbee_agent_observation_protocol::{
    AGENT_CURSOR_PAYLOAD_BYTES, AGENT_CURSOR_TAG_BYTES, AgentCursorPayload, AgentListCursor,
    MAXIMUM_SAFE_INTEGER,
};
use sha2::Sha256;

const MAC_DOMAIN: &[u8] = b"orchbee.agent.list.cursor.v1\0";
const KEY_BYTES: usize = 32;

type CursorMac = Hmac<Sha256>;

/// Why a presented list cursor cannot be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentCursorRejection {
    /// Malformed, non-canonical, unauthentic, or semantically impossible.
    Invalid,
    /// Authentic, minted by this instance, but against an older revision.
    Stale,
}

/// Owns the boot-scoped HMAC key and instance identity for list cursors.
pub(crate) struct AgentListCursorAuthority {
    key: [u8; KEY_BYTES],
    instance: u64,
}

impl AgentListCursorAuthority {
    /// Draws a fresh key and nonzero instance identity from the operating system entropy source.
    pub(crate) fn open() -> io::Result<Self> {
        let mut key = [0_u8; KEY_BYTES];
        getrandom::fill(&mut key).map_err(|error| std::io::Error::other(error.to_string()))?;
        let mut instance = 0_u64;
        while instance == 0 {
            let mut bytes = [0_u8; 8];
            getrandom::fill(&mut bytes)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            instance = u64::from_be_bytes(bytes);
        }
        Ok(Self { key, instance })
    }

    #[cfg(test)]
    fn fixed(key: [u8; KEY_BYTES], instance: u64) -> Self {
        assert_ne!(instance, 0);
        Self { key, instance }
    }

    /// Mints the canonical `v1.<payload>.<tag>` cursor for one list page.
    pub(crate) fn mint(
        &self,
        include_ended: bool,
        revision: u64,
        position: u64,
    ) -> AgentListCursor {
        let payload = AgentCursorPayload {
            version: AgentCursorPayload::VERSION,
            operation: AgentCursorPayload::LIST_OPERATION,
            flags: if include_ended {
                AgentCursorPayload::FLAG_INCLUDE_ENDED
            } else {
                0
            },
            instance: self.instance,
            target: 0,
            revision,
            position,
        };
        let payload_bytes = payload.encode();
        let tag = self.tag(&payload_bytes);
        AgentListCursor::parse(&format!(
            "v1.{}.{}",
            URL_SAFE_NO_PAD.encode(payload_bytes),
            URL_SAFE_NO_PAD.encode(tag)
        ))
        .expect("a minted cursor always has the canonical wire shape")
    }

    /// Verifies a presented cursor and returns the sequence position it
    /// resumes from. `filtered_length` is the length of the filtered,
    /// canonically ordered session sequence at `current_revision`.
    pub(crate) fn verify(
        &self,
        cursor_text: &str,
        include_ended: bool,
        current_revision: u64,
        filtered_length: u64,
    ) -> Result<u64, AgentCursorRejection> {
        let invalid = || Err(AgentCursorRejection::Invalid);
        let cursor =
            AgentListCursor::parse(cursor_text).map_err(|_| AgentCursorRejection::Invalid)?;

        let payload_bytes = decode_canonical_segment(cursor.payload_segment())
            .and_then(|bytes| <[u8; AGENT_CURSOR_PAYLOAD_BYTES]>::try_from(bytes.as_slice()).ok())
            .ok_or(AgentCursorRejection::Invalid)?;
        let tag_bytes = decode_canonical_segment(cursor.tag_segment())
            .and_then(|bytes| <[u8; AGENT_CURSOR_TAG_BYTES]>::try_from(bytes.as_slice()).ok())
            .ok_or(AgentCursorRejection::Invalid)?;

        let mut mac = self.mac();
        mac.update(&payload_bytes);
        mac.verify_slice(&tag_bytes)
            .map_err(|_| AgentCursorRejection::Invalid)?;

        let payload =
            AgentCursorPayload::decode(&payload_bytes).ok_or(AgentCursorRejection::Invalid)?;
        let flags = if include_ended {
            AgentCursorPayload::FLAG_INCLUDE_ENDED
        } else {
            0
        };
        if payload.version != AgentCursorPayload::VERSION
            || payload.operation != AgentCursorPayload::LIST_OPERATION
            || payload.flags != flags
            || payload.target != 0
            || payload.revision > MAXIMUM_SAFE_INTEGER
            || payload.instance != self.instance
        {
            return invalid();
        }

        if payload.revision > current_revision {
            return invalid();
        }
        if payload.revision < current_revision {
            return Err(AgentCursorRejection::Stale);
        }
        if payload.position >= filtered_length {
            return invalid();
        }
        Ok(payload.position)
    }

    fn mac(&self) -> CursorMac {
        let mut mac =
            <CursorMac as Mac>::new_from_slice(&self.key).expect("HMAC accepts keys of any length");
        mac.update(MAC_DOMAIN);
        mac
    }

    fn tag(&self, payload: &[u8; AGENT_CURSOR_PAYLOAD_BYTES]) -> [u8; AGENT_CURSOR_TAG_BYTES] {
        let mut mac = self.mac();
        mac.update(payload);
        mac.finalize().into_bytes().into()
    }
}

/// Decodes an unpadded base64url segment and requires canonical encoding —
/// re-encoding must reproduce the segment byte-for-byte, which rejects
/// non-zero pad bits and padded forms.
fn decode_canonical_segment(segment: &str) -> Option<Vec<u8>> {
    let bytes = URL_SAFE_NO_PAD.decode(segment).ok()?;
    if URL_SAFE_NO_PAD.encode(&bytes) != segment {
        return None;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests;
