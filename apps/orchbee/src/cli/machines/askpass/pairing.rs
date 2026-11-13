//! The askpass helper's private pairing with the hub: the variable, its value, and the record the
//! helper hands the hub. Internal to this program; a release may change any of it.

use serde::{Deserialize, Serialize};

/// The private variable that makes this program the askpass helper. Only the hub sets it, in its
/// carrier children.
pub(crate) const VARIABLE: &str = "ORCHBEE_MACHINE_ASKPASS";
/// The bytes of a carrier's ticket.
pub(crate) const TICKET_BYTES: usize = 16;
/// The first bytes of the helper's record.
const RAISE_MAGIC: [u8; 4] = *b"OBAQ";
/// The magic and the length before the record's JSON.
pub(crate) const RAISE_HEADER_BYTES: usize = 8;
/// The most JSON bytes the record carries: a 4,096-byte prompt whose every byte JSON escapes in
/// six fits with room to spare, so a helper's own record is never too long.
const RAISE_LIMIT: usize = 32 * 1024;
/// The byte the helper writes to the hub once it took an answer, before it prints it.
pub(crate) const TAKEN: u8 = b'T';

/// Where a carrier's helper reaches the hub, and the ticket that names the carrier.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Pairing {
    pub(crate) endpoint: String,
    pub(crate) ticket: [u8; TICKET_BYTES],
}

impl Pairing {
    /// The variable's value: the endpoint, one space, and the ticket's 32 hexadecimal digits.
    pub(crate) fn encode(&self) -> String {
        format!("{} {}", self.endpoint, hex(&self.ticket))
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        let (endpoint, ticket) = text.rsplit_once(' ')?;
        Some(Self {
            endpoint: endpoint
                .starts_with(r"\\.\pipe\")
                .then(|| endpoint.to_owned())?,
            ticket: unhex(ticket)?,
        })
    }
}

/// What a helper hands the hub: its carrier's ticket and, when it can ask, the question. No member
/// holds an answer.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Raise {
    pub(crate) ticket: String,
    /// `None` from a helper that cannot ask, whose carrier the hub then ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) question: Option<Asked>,
}

/// A question with the endpoint and the token its answer must come with.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Asked {
    pub(crate) kind: String,
    pub(crate) prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) host_key: Option<orchbee_machine_protocol::HostKey>,
    pub(crate) endpoint: String,
    pub(crate) token: String,
}

impl Raise {
    /// The record of a helper that cannot ask.
    pub(crate) fn unable(ticket: String) -> Self {
        Self {
            ticket,
            question: None,
        }
    }

    /// The record: magic, a big-endian length and the JSON.
    pub(crate) fn encode(&self) -> Option<Vec<u8>> {
        let json = serde_json::to_vec(self).ok()?;
        let length = u32::try_from(json.len())
            .ok()
            .filter(|length| *length as usize <= RAISE_LIMIT)?;
        let mut record = Vec::with_capacity(RAISE_HEADER_BYTES + json.len());
        record.extend_from_slice(&RAISE_MAGIC);
        record.extend_from_slice(&length.to_be_bytes());
        record.extend_from_slice(&json);
        Some(record)
    }

    /// The JSON length a record's header announces, within the bound.
    pub(crate) fn length(header: &[u8; RAISE_HEADER_BYTES]) -> Option<usize> {
        let (magic, length) = header.split_at(4);
        let length = u32::from_be_bytes(length.try_into().ok()?) as usize;
        (magic == RAISE_MAGIC && length <= RAISE_LIMIT).then_some(length)
    }
}

/// Lowercase hexadecimal digits of `bytes`.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The 16 bytes 32 lowercase hexadecimal digits spell.
pub(crate) fn unhex(text: &str) -> Option<[u8; TICKET_BYTES]> {
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    if text.len() != TICKET_BYTES * 2 {
        return None;
    }
    let mut bytes = [0; TICKET_BYTES];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *byte = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(bytes)
}
