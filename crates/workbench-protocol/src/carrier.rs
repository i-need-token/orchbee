use std::{
    fmt,
    io::{self, Read, Write},
};

use crate::{
    command::{
        MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES, WorkbenchRequest, WorkbenchResponse,
        parse_workbench_request, parse_workbench_response, serialize_workbench_request,
        serialize_workbench_response,
    },
    validation::{ContractError, MAXIMUM_ENCODED_INTENT_BYTES},
};

pub const WORKBENCH_MAGIC: [u8; 4] = *b"OBWP";
const HEADER_BYTES: usize = 12;
const VERSION: u16 = 1;

#[derive(Debug)]
pub enum WorkbenchWireError {
    Io(io::Error),
    InvalidHeader,
    Contract(ContractError),
}

impl fmt::Display for WorkbenchWireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::InvalidHeader => {
                formatter.write_str("invalid Workbench frame header or direction")
            }
            Self::Contract(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkbenchWireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::InvalidHeader => None,
        }
    }
}

pub fn read_workbench_request(
    reader: &mut impl Read,
) -> Result<WorkbenchRequest, WorkbenchWireError> {
    let (kind, payload) = read_frame(reader, true)?;
    let request = parse_workbench_request(&payload).map_err(WorkbenchWireError::Contract)?;
    if request_kind(&request) != kind {
        return Err(WorkbenchWireError::InvalidHeader);
    }
    Ok(request)
}

pub fn read_workbench_response(
    reader: &mut impl Read,
) -> Result<WorkbenchResponse, WorkbenchWireError> {
    let (kind, payload) = read_frame(reader, false)?;
    let response = parse_workbench_response(&payload).map_err(WorkbenchWireError::Contract)?;
    if response_kind(&response) != kind {
        return Err(WorkbenchWireError::InvalidHeader);
    }
    Ok(response)
}

pub fn write_workbench_request(
    writer: &mut impl Write,
    request: &WorkbenchRequest,
) -> Result<(), WorkbenchWireError> {
    let payload = serialize_workbench_request(request).map_err(WorkbenchWireError::Contract)?;
    write_frame(writer, request_kind(request), &payload)
}

pub fn write_workbench_response(
    writer: &mut impl Write,
    response: &WorkbenchResponse,
) -> Result<(), WorkbenchWireError> {
    let payload = serialize_workbench_response(response).map_err(WorkbenchWireError::Contract)?;
    write_frame(writer, response_kind(response), &payload)
}

fn request_kind(request: &WorkbenchRequest) -> u16 {
    match request {
        WorkbenchRequest::SnapshotRead(_) => 1,
        WorkbenchRequest::Command(_) => 2,
        WorkbenchRequest::TabCloseRead(_) => 5,
        WorkbenchRequest::TabCloseResume(_) => 7,
        WorkbenchRequest::PublicationRead(_) => 9,
        WorkbenchRequest::WorktreeListRead(_) => 11,
        WorkbenchRequest::LaunchProfileCatalogRead(_) => 13,
        WorkbenchRequest::WorkspaceCloseRead(_) => 15,
        WorkbenchRequest::WorkspaceCloseResume(_) => 17,
    }
}

fn response_kind(response: &WorkbenchResponse) -> u16 {
    match response {
        WorkbenchResponse::Snapshot(_) => 3,
        WorkbenchResponse::Command(_) => 4,
        WorkbenchResponse::TabCloseRead(_) => 6,
        WorkbenchResponse::TabCloseResume(_) => 8,
        WorkbenchResponse::Publication(_) => 10,
        WorkbenchResponse::WorktreeList(_) => 12,
        WorkbenchResponse::LaunchProfileCatalog(_) => 14,
        WorkbenchResponse::WorkspaceCloseRead(_) => 16,
        WorkbenchResponse::WorkspaceCloseResume(_) => 18,
    }
}

fn read_frame(reader: &mut impl Read, request: bool) -> Result<(u16, Vec<u8>), WorkbenchWireError> {
    let mut header = [0; HEADER_BYTES];
    reader
        .read_exact(&mut header)
        .map_err(WorkbenchWireError::Io)?;
    let kind = u16::from_be_bytes([header[6], header[7]]);
    if header[..4] != WORKBENCH_MAGIC
        || u16::from_be_bytes([header[4], header[5]]) != VERSION
        || (request && !matches!(kind, 1 | 2 | 5 | 7 | 9 | 11 | 13 | 15 | 17))
        || (!request && !matches!(kind, 3 | 4 | 6 | 8 | 10 | 12 | 14 | 16 | 18))
    {
        return Err(WorkbenchWireError::InvalidHeader);
    }
    let length = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    let actual = usize::try_from(length).map_err(|_error| WorkbenchWireError::InvalidHeader)?;
    let maximum = if matches!(kind, 3 | 10) {
        MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES
    } else if request && kind == 2 {
        crate::MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES
    } else if kind == 12 {
        crate::MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES
    } else if kind == 16 {
        crate::MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES
    } else {
        MAXIMUM_ENCODED_INTENT_BYTES
    };
    if actual > maximum {
        return Err(WorkbenchWireError::Contract(
            ContractError::EncodedValueTooLarge { actual, maximum },
        ));
    }
    let mut payload = vec![0; actual];
    reader
        .read_exact(&mut payload)
        .map_err(WorkbenchWireError::Io)?;
    Ok((kind, payload))
}

fn write_frame(
    writer: &mut impl Write,
    kind: u16,
    payload: &[u8],
) -> Result<(), WorkbenchWireError> {
    let length =
        u32::try_from(payload.len()).map_err(|_error| WorkbenchWireError::InvalidHeader)?;
    let mut header = [0; HEADER_BYTES];
    header[..4].copy_from_slice(&WORKBENCH_MAGIC);
    header[4..6].copy_from_slice(&VERSION.to_be_bytes());
    header[6..8].copy_from_slice(&kind.to_be_bytes());
    header[8..].copy_from_slice(&length.to_be_bytes());
    writer
        .write_all(&header)
        .and_then(|()| writer.write_all(payload))
        .map_err(WorkbenchWireError::Io)
}

#[cfg(test)]
mod tests;
