use std::{
    fmt,
    io::{self, Read, Write},
};

use crate::contract::{
    AgentContractError, parse_agent_request, parse_agent_result, serialize_agent_request,
    serialize_agent_result,
};
use crate::launch::{AgentLaunchRequest, AgentLaunchResult};
use crate::names::{AgentNamesRequest, AgentNamesResult};
use crate::read::{AgentReadRequest, AgentReadResult, AgentRequest, AgentResult};

pub const AGENT_MAGIC: [u8; 4] = *b"OBAP";
const HEADER_BYTES: usize = 12;
const VERSION: u16 = 1;

pub const MAXIMUM_AGENT_REQUEST_BYTES: usize = 16 * 1024;
pub const MAXIMUM_AGENT_CHANGES_RESULT_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_AGENT_LIST_RESULT_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_AGENT_SESSION_RESULT_BYTES: usize = 128 * 1024;
/// The names-list envelope: ≤512 bindings of name/identity/lifecycle is far
/// under this bound, which exists to refuse hostile declared lengths before
/// allocation rather than to shape the payload.
pub const MAXIMUM_AGENT_NAMES_RESULT_BYTES: usize = 256 * 1024;
pub const MAXIMUM_AGENT_SMALL_OUTCOME_BYTES: usize = 1024;

#[derive(Debug)]
pub enum AgentWireError {
    Io(io::Error),
    InvalidHeader,
    Contract(AgentContractError),
}

impl fmt::Display for AgentWireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::InvalidHeader => {
                formatter.write_str("invalid Agent observation frame header or direction")
            }
            Self::Contract(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AgentWireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::InvalidHeader => None,
        }
    }
}

pub fn read_agent_request(reader: &mut impl Read) -> Result<AgentRequest, AgentWireError> {
    let (kind, payload) = read_frame(reader, Direction::Request)?;
    let request = parse_agent_request(&payload).map_err(AgentWireError::Contract)?;
    if request_kind(&request) != kind {
        return Err(AgentWireError::InvalidHeader);
    }
    Ok(request)
}

pub fn read_agent_result(reader: &mut impl Read) -> Result<AgentResult, AgentWireError> {
    let (kind, payload) = read_frame(reader, Direction::Result)?;
    let result = parse_agent_result(&payload).map_err(AgentWireError::Contract)?;
    if result_kind(&result) != kind {
        return Err(AgentWireError::InvalidHeader);
    }
    Ok(result)
}

pub fn write_agent_request(
    writer: &mut impl Write,
    request: &AgentRequest,
) -> Result<(), AgentWireError> {
    let payload = serialize_agent_request(request).map_err(AgentWireError::Contract)?;
    if payload.len() > MAXIMUM_AGENT_REQUEST_BYTES {
        return Err(AgentWireError::Contract(
            AgentContractError::EncodedValueTooLarge {
                actual: payload.len(),
                maximum: MAXIMUM_AGENT_REQUEST_BYTES,
            },
        ));
    }
    write_frame(writer, request_kind(request), &payload)
}

pub fn write_agent_result(
    writer: &mut impl Write,
    result: &AgentResult,
) -> Result<(), AgentWireError> {
    let payload = serialize_agent_result(result).map_err(AgentWireError::Contract)?;
    // A rejected/unavailable outcome stays inside the small-document bound;
    // the header does not identify the outcome, so this is enforced after
    // typed construction and before serialization, never by preallocation.
    let maximum = if result.is_small_outcome() {
        MAXIMUM_AGENT_SMALL_OUTCOME_BYTES
    } else {
        match result {
            AgentResult::Read(AgentReadResult::List { .. }) => MAXIMUM_AGENT_LIST_RESULT_BYTES,
            AgentResult::Read(AgentReadResult::Get { .. } | AgentReadResult::Explain { .. }) => {
                MAXIMUM_AGENT_SESSION_RESULT_BYTES
            }
            AgentResult::Names(AgentNamesResult::NamesList { .. }) => {
                MAXIMUM_AGENT_NAMES_RESULT_BYTES
            }
            AgentResult::Launch(_) => MAXIMUM_AGENT_SMALL_OUTCOME_BYTES,
            AgentResult::Changes(_) => MAXIMUM_AGENT_CHANGES_RESULT_BYTES,
            // Every non-small names outcome is the list; the other variants
            // are always small outcomes.
            AgentResult::Names(
                AgentNamesResult::Resolve { .. }
                | AgentNamesResult::NameAssign { .. }
                | AgentNamesResult::NameRelease { .. },
            ) => MAXIMUM_AGENT_SMALL_OUTCOME_BYTES,
        }
    };
    if payload.len() > maximum {
        return Err(AgentWireError::Contract(
            AgentContractError::EncodedValueTooLarge {
                actual: payload.len(),
                maximum,
            },
        ));
    }
    write_frame(writer, result_kind(result), &payload)
}

fn request_kind(request: &AgentRequest) -> u16 {
    match request {
        AgentRequest::Changes(_) => 23,
        AgentRequest::Launch(AgentLaunchRequest::TerminalStatus { .. }) => 27,
        AgentRequest::Launch(AgentLaunchRequest::Status { .. }) => 25,
        AgentRequest::Launch(AgentLaunchRequest::Reserve { .. }) => 15,
        AgentRequest::Launch(AgentLaunchRequest::Inspect { .. }) => 17,
        AgentRequest::Launch(AgentLaunchRequest::Complete { .. }) => 19,
        AgentRequest::Launch(AgentLaunchRequest::Cancel { .. }) => 21,
        AgentRequest::Read(AgentReadRequest::List { .. }) => 1,
        AgentRequest::Read(AgentReadRequest::Get { .. }) => 3,
        AgentRequest::Read(AgentReadRequest::Explain { .. }) => 5,
        AgentRequest::Names(AgentNamesRequest::Resolve { .. }) => 7,
        AgentRequest::Names(AgentNamesRequest::NamesList { .. }) => 9,
        AgentRequest::Names(AgentNamesRequest::NameAssign { .. }) => 11,
        AgentRequest::Names(AgentNamesRequest::NameRelease { .. }) => 13,
    }
}

fn result_kind(result: &AgentResult) -> u16 {
    match result {
        AgentResult::Changes(_) => 24,
        AgentResult::Launch(AgentLaunchResult::TerminalStatus { .. }) => 28,
        AgentResult::Launch(AgentLaunchResult::Status { .. }) => 26,
        AgentResult::Launch(AgentLaunchResult::Reserve { .. }) => 16,
        AgentResult::Launch(AgentLaunchResult::Inspect { .. }) => 18,
        AgentResult::Launch(AgentLaunchResult::Complete { .. }) => 20,
        AgentResult::Launch(AgentLaunchResult::Cancel { .. }) => 22,
        AgentResult::Read(AgentReadResult::List { .. }) => 2,
        AgentResult::Read(AgentReadResult::Get { .. }) => 4,
        AgentResult::Read(AgentReadResult::Explain { .. }) => 6,
        AgentResult::Names(AgentNamesResult::Resolve { .. }) => 8,
        AgentResult::Names(AgentNamesResult::NamesList { .. }) => 10,
        AgentResult::Names(AgentNamesResult::NameAssign { .. }) => 12,
        AgentResult::Names(AgentNamesResult::NameRelease { .. }) => 14,
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Request,
    Result,
}

fn read_frame(
    reader: &mut impl Read,
    direction: Direction,
) -> Result<(u16, Vec<u8>), AgentWireError> {
    let mut header = [0; HEADER_BYTES];
    reader.read_exact(&mut header).map_err(AgentWireError::Io)?;
    let kind = u16::from_be_bytes([header[6], header[7]]);
    let kind_valid = match direction {
        Direction::Request => matches!(
            kind,
            1 | 3 | 5 | 7 | 9 | 11 | 13 | 15 | 17 | 19 | 21 | 23 | 25 | 27
        ),
        Direction::Result => matches!(
            kind,
            2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 18 | 20 | 22 | 24 | 26 | 28
        ),
    };
    if header[..4] != AGENT_MAGIC
        || u16::from_be_bytes([header[4], header[5]]) != VERSION
        || !kind_valid
    {
        return Err(AgentWireError::InvalidHeader);
    }
    let length = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    let actual = usize::try_from(length).map_err(|_error| AgentWireError::InvalidHeader)?;
    // The declared length is hostile until bounded: the per-kind ceiling is
    // checked before any allocation or body read.
    let maximum = match direction {
        Direction::Request => MAXIMUM_AGENT_REQUEST_BYTES,
        Direction::Result => match kind {
            2 => MAXIMUM_AGENT_LIST_RESULT_BYTES,
            24 => MAXIMUM_AGENT_CHANGES_RESULT_BYTES,
            10 => MAXIMUM_AGENT_NAMES_RESULT_BYTES,
            8 | 12 | 14 | 16 | 18 | 20 | 22 | 26 | 28 => MAXIMUM_AGENT_SMALL_OUTCOME_BYTES,
            _ => MAXIMUM_AGENT_SESSION_RESULT_BYTES,
        },
    };
    if actual > maximum {
        return Err(AgentWireError::Contract(
            AgentContractError::EncodedValueTooLarge { actual, maximum },
        ));
    }
    let mut payload = vec![0; actual];
    reader
        .read_exact(&mut payload)
        .map_err(AgentWireError::Io)?;
    Ok((kind, payload))
}

fn write_frame(writer: &mut impl Write, kind: u16, payload: &[u8]) -> Result<(), AgentWireError> {
    let length = u32::try_from(payload.len()).map_err(|_error| AgentWireError::InvalidHeader)?;
    let mut header = [0; HEADER_BYTES];
    header[..4].copy_from_slice(&AGENT_MAGIC);
    header[4..6].copy_from_slice(&VERSION.to_be_bytes());
    header[6..8].copy_from_slice(&kind.to_be_bytes());
    header[8..].copy_from_slice(&length.to_be_bytes());
    writer
        .write_all(&header)
        .and_then(|()| writer.write_all(payload))
        .map_err(AgentWireError::Io)
}
