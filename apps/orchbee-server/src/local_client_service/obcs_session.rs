use std::{
    fmt,
    io::{Read, Write},
};

use orchbee_client_server_protocol::{
    ClientMessage, CreateOutcome, CreateRejectedReason, ReadMessageError, ServerMessage,
    StartOutcome, StartRejectedReason, TerminalCreateResult, TerminalStartResult,
    WriteMessageError, read_client_message, write_server_message,
};

pub(super) fn run<Stream>(stream: &mut Stream) -> Result<(), ObcsSessionError>
where
    Stream: Read + Write,
{
    loop {
        let request = read_client_message(stream).map_err(ObcsSessionError::Read)?;
        let result = deny(request);
        write_server_message(stream, &result).map_err(ObcsSessionError::Write)?;
    }
}

fn deny(request: ClientMessage) -> ServerMessage {
    match request {
        ClientMessage::TerminalCreate(request) => {
            ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
                request.request_id().clone(),
                CreateOutcome::Rejected {
                    reason: CreateRejectedReason::NotAuthorized,
                },
            ))
        }
        ClientMessage::TerminalStart(request) => {
            ServerMessage::TerminalStartResult(TerminalStartResult::new(
                request.request_id().clone(),
                StartOutcome::Rejected {
                    reason: StartRejectedReason::NotAuthorized,
                },
            ))
        }
    }
}

#[derive(Debug)]
pub(super) enum ObcsSessionError {
    Read(ReadMessageError),
    Write(WriteMessageError),
}

impl fmt::Display for ObcsSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "OBCS request read failed: {error}"),
            Self::Write(error) => write!(formatter, "OBCS result write failed: {error}"),
        }
    }
}

impl std::error::Error for ObcsSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Write(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests;
