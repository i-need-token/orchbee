//! Carries the Server's Host query as kind 34 and its reply in a dedicated `OBNH` record, which
//! admits 256 maximal process entries beyond the 40 KiB frame. It neither dials a Host nor decides
//! which Hosts a reply lists.

use std::{
    io::Read,
    num::{NonZeroU32, NonZeroU64},
};

use orchbee_connection_handshake::{
    Build, CohortIdentity, ConnectionEntry, ContractName, EnvironmentName, Generation,
    GenerationRange, MAXIMUM_PROCESS_CONNECTIONS, ProcessEntry, ProcessRole, ProcessState,
    ProductVersion, SourceCommit,
};

use super::PayloadReader;
use crate::{
    CapabilityNodeMessage, HostQueryRequest, HostQueryResult, MAXIMUM_QUERY_HOSTS, OperationId,
    ProtocolError, ReadMessageError, WorkerGeneration, WorkerId,
};

pub(super) const REQUEST_KIND: u8 = 34;
const RECORD_HEADER: [u8; 8] = *b"OBNH\0\x01\0\0";
const RECORD_PREFIX_BYTES: usize = 12;
const REQUEST_BYTES: usize = 40;
/// A version or name: one length byte, then at most 64 bytes.
const TEXT_BYTES: usize = 1 + 64;
/// The kind, then the lowest and highest generation.
const CONNECTION_BYTES: usize = TEXT_BYTES + 4;
/// PID, state, terminals, version, environment, optional commit and cohort, the connection count
/// and the connections.
const MAXIMUM_ENTRY_BYTES: usize =
    4 + 1 + 4 + 2 * TEXT_BYTES + 21 + 33 + 1 + MAXIMUM_PROCESS_CONNECTIONS * CONNECTION_BYTES;
/// The echoed request and the Host count.
const MINIMUM_BODY_BYTES: usize = REQUEST_BYTES + 2;
const MAXIMUM_BODY_BYTES: usize = MINIMUM_BODY_BYTES + MAXIMUM_QUERY_HOSTS * MAXIMUM_ENTRY_BYTES;

pub(super) fn encode_request(value: HostQueryRequest) -> Vec<u8> {
    let mut bytes = vec![REQUEST_KIND];
    encode_request_body(&mut bytes, value);
    bytes
}

pub(super) fn decode_request(payload: &[u8]) -> Result<HostQueryRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, REQUEST_KIND)?;
    let request = decode_request_body(&mut reader)?;
    reader.finish()?;
    Ok(request)
}

pub(super) fn encode_result(value: &HostQueryResult) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    let mut body = Vec::new();
    encode_request_body(&mut body, value.request);
    let count = u16::try_from(value.hosts.len()).map_err(|_| ProtocolError::InvalidPayload)?;
    body.extend(count.to_be_bytes());
    for host in &value.hosts {
        encode_entry(&mut body, host);
    }
    let length = u32::try_from(body.len()).map_err(|_| ProtocolError::InvalidPayload)?;
    let mut record = Vec::with_capacity(RECORD_PREFIX_BYTES + body.len());
    record.extend(RECORD_HEADER);
    record.extend(length.to_be_bytes());
    record.extend(body);
    Ok(record)
}

pub(super) fn decode_result(record: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let prefix = record
        .get(..RECORD_PREFIX_BYTES)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_| ProtocolError::InvalidPayload)?;
    let length = body_length(prefix)?;
    match record.get(RECORD_PREFIX_BYTES..) {
        Some(body) if body.len() == length => decode_body(body),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

/// Reads the rest of the record whose first eight bytes are `header`, refusing an out-of-bounds
/// length before allocating the body.
pub(super) fn read_result(
    reader: &mut impl Read,
    header: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut prefix = [0; RECORD_PREFIX_BYTES];
    prefix[..8].copy_from_slice(&header);
    reader.read_exact(&mut prefix[8..])?;
    let mut body = vec![0; body_length(&prefix)?];
    reader.read_exact(&mut body)?;
    decode_body(&body).map_err(Into::into)
}

fn body_length(prefix: &[u8; RECORD_PREFIX_BYTES]) -> Result<usize, ProtocolError> {
    let (header, length) = prefix.split_at(8);
    if header != RECORD_HEADER {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = length
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| ProtocolError::InvalidPayload)?;
    usize::try_from(length)
        .ok()
        .filter(|length| (MINIMUM_BODY_BYTES..=MAXIMUM_BODY_BYTES).contains(length))
        .ok_or(ProtocolError::InvalidPayload)
}

fn decode_body(body: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let mut reader = PayloadReader::whole(body);
    let request = decode_request_body(&mut reader)?;
    let count = usize::from(reader.read_u16()?);
    if count > MAXIMUM_QUERY_HOSTS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut hosts = Vec::with_capacity(count);
    for _ in 0..count {
        hosts.push(decode_entry(&mut reader)?);
    }
    reader.finish()?;
    let value = HostQueryResult { request, hosts };
    value.validate()?;
    Ok(CapabilityNodeMessage::HostQueryResult(value))
}

fn encode_request_body(bytes: &mut Vec<u8>, value: HostQueryRequest) {
    bytes.extend(value.request_id.as_bytes());
    bytes.extend(value.worker_id.as_bytes());
    bytes.extend(value.worker_generation.get().to_be_bytes());
}

fn decode_request_body(reader: &mut PayloadReader<'_>) -> Result<HostQueryRequest, ProtocolError> {
    Ok(HostQueryRequest {
        request_id: OperationId::from_bytes(reader.read_array()?)?,
        worker_id: WorkerId::from_bytes(reader.read_array()?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        ),
    })
}

fn encode_entry(bytes: &mut Vec<u8>, host: &ProcessEntry) {
    bytes.extend(host.pid.get().to_be_bytes());
    bytes.push(match host.state {
        ProcessState::Unknown => 0,
        ProcessState::Ready => 1,
        ProcessState::Draining => 2,
        ProcessState::Stopping => 3,
    });
    bytes.extend(host.terminals.to_be_bytes());
    encode_text(bytes, host.product_version.as_str());
    encode_text(bytes, host.environment.as_str());
    encode_optional(
        bytes,
        host.build.commit.as_ref().map(SourceCommit::as_bytes),
    );
    encode_optional(
        bytes,
        host.build.cohort.as_ref().map(CohortIdentity::as_bytes),
    );
    // Validation bounds the connections to 64.
    bytes.push(host.connections.len() as u8);
    for connection in &host.connections {
        encode_text(bytes, connection.kind.as_str());
        bytes.extend(connection.generations.lowest().get().to_be_bytes());
        bytes.extend(connection.generations.highest().get().to_be_bytes());
    }
}

fn decode_entry(reader: &mut PayloadReader<'_>) -> Result<ProcessEntry, ProtocolError> {
    let pid = NonZeroU32::new(reader.read_u32()?).ok_or(ProtocolError::InvalidPayload)?;
    let state = match reader.read_u8()? {
        0 => ProcessState::Unknown,
        1 => ProcessState::Ready,
        2 => ProcessState::Draining,
        3 => ProcessState::Stopping,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    let terminals = reader.read_u32()?;
    let product_version = ProductVersion::new(decode_text(reader)?).map_err(|_| invalid())?;
    let environment = EnvironmentName::new(decode_text(reader)?).map_err(|_| invalid())?;
    let commit = decode_optional(reader)?.map(SourceCommit::from_bytes);
    let cohort = decode_optional(reader)?.map(CohortIdentity::from_bytes);
    let count = usize::from(reader.read_u8()?);
    if count > MAXIMUM_PROCESS_CONNECTIONS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut connections = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = ContractName::new(decode_text(reader)?).map_err(|_| invalid())?;
        let lowest = Generation::new(reader.read_u16()?).ok_or(ProtocolError::InvalidPayload)?;
        let highest = Generation::new(reader.read_u16()?).ok_or(ProtocolError::InvalidPayload)?;
        connections.push(ConnectionEntry {
            kind,
            generations: GenerationRange::new(lowest, highest).map_err(|_| invalid())?,
            capabilities: None,
        });
    }
    Ok(ProcessEntry {
        role: ProcessRole::TerminalHost,
        product_version,
        build: Build { commit, cohort },
        environment,
        pid,
        state,
        terminals,
        connections,
    })
}

/// Versions and names are at most 64 bytes, so one length byte holds them.
fn encode_text(bytes: &mut Vec<u8>, text: &str) {
    bytes.push(text.len() as u8);
    bytes.extend(text.as_bytes());
}

fn decode_text(reader: &mut PayloadReader<'_>) -> Result<String, ProtocolError> {
    let length = usize::from(reader.read_u8()?);
    std::str::from_utf8(reader.read_slice(length)?)
        .map(str::to_owned)
        .map_err(|_| ProtocolError::InvalidPayload)
}

fn encode_optional<const LENGTH: usize>(bytes: &mut Vec<u8>, value: Option<&[u8; LENGTH]>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend(value);
        }
    }
}

fn decode_optional<const LENGTH: usize>(
    reader: &mut PayloadReader<'_>,
) -> Result<Option<[u8; LENGTH]>, ProtocolError> {
    match reader.read_u8()? {
        0 => Ok(None),
        1 => reader.read_array().map(Some),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

const fn invalid() -> ProtocolError {
    ProtocolError::InvalidPayload
}
