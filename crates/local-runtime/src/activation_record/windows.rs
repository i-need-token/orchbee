//! A retained Windows activation file is the completion capability. No pathname
//! deletion, process discovery or stop authority is provided by this primitive.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};

const STOPPED_PREFIX: &[u8] = b"orchbee-stopped-v1:";
const CRASHED_PREFIX: &[u8] = b"orchbee-crashed-v1:";
const MAXIMUM_RECORD_BYTES: usize = STOPPED_PREFIX.len() + 32;

use super::ActivationState;

/// Pins an admitted active file without delete sharing for same-identity completion publication.
pub struct RetainedActivationRecord {
    file: File,
    attempted: Option<[u8; 16]>,
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid Windows activation record",
    )
}

fn parse(bytes: &[u8]) -> io::Result<ActivationState> {
    if bytes.is_empty() {
        return Ok(ActivationState::Active);
    }
    let (hex, crashed) = if let Some(hex) = bytes.strip_prefix(STOPPED_PREFIX) {
        (hex, false)
    } else {
        (
            bytes.strip_prefix(CRASHED_PREFIX).ok_or_else(invalid)?,
            true,
        )
    };
    if hex.len() != 32 {
        return Err(invalid());
    }
    let mut request_id = [0; 16];
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(invalid()),
    };
    for (index, pair) in hex.chunks_exact(2).enumerate() {
        request_id[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    if request_id == [0; 16] {
        return Err(invalid());
    }
    if crashed {
        Ok(ActivationState::Crashed {
            activation_id: request_id,
        })
    } else {
        Ok(ActivationState::Stopped { request_id })
    }
}

fn crashed_bytes(activation_id: [u8; 16]) -> io::Result<Vec<u8>> {
    if activation_id == [0; 16] {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut bytes = CRASHED_PREFIX.to_vec();
    for byte in activation_id {
        bytes.push(b"0123456789abcdef"[(byte >> 4) as usize]);
        bytes.push(b"0123456789abcdef"[(byte & 15) as usize]);
    }
    Ok(bytes)
}

/// Only the catalog's proved-dead, same-generation recovery owner may call this.
pub(crate) fn publish_crashed(
    file: &mut File,
    activation_id: [u8; 16],
    retry: bool,
    allow_previous_receipt: bool,
) -> io::Result<()> {
    let desired = crashed_bytes(activation_id)?;
    if !crash_matches(file, activation_id, retry, allow_previous_receipt)? {
        return Err(invalid());
    }
    // Clear a complete prior receipt before writing: an interrupted write must not
    // combine this generation's prefix with the previous generation's suffix.
    file.set_len(0)?;
    file.sync_data()?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&desired)?;
    file.set_len(desired.len() as u64)?;
    file.sync_data()
}
pub(crate) fn crash_matches(
    file: &mut File,
    activation_id: [u8; 16],
    retry: bool,
    allow_previous_receipt: bool,
) -> io::Result<bool> {
    let desired = crashed_bytes(activation_id)?;
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take((MAXIMUM_RECORD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    // Retry only a matching interrupted prefix, never an unrelated malformed marker.
    Ok(bytes.is_empty()
        || bytes == desired
        || (retry && desired.starts_with(&bytes))
        || (allow_previous_receipt
            && matches!(
                parse(&bytes),
                Ok(ActivationState::Stopped { .. } | ActivationState::Crashed { .. })
            )))
}

pub(crate) fn read(file: &mut File) -> io::Result<ActivationState> {
    if file.metadata()?.len() > MAXIMUM_RECORD_BYTES as u64 {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(MAXIMUM_RECORD_BYTES + 1);
    file.take((MAXIMUM_RECORD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    parse(&bytes)
}

impl RetainedActivationRecord {
    pub(crate) fn new(mut file: File) -> io::Result<Self> {
        if read(&mut file)? != ActivationState::Active {
            return Err(invalid());
        }
        Ok(Self {
            file,
            attempted: None,
        })
    }

    /// Call only after the lifecycle owner has confirmed all owned child exits.
    /// I/O failure may leave a partial record; only this retained owner and ID
    /// may retry it. Success means flushed record publication, not process proof.
    pub fn complete(&mut self, request_id: [u8; 16]) -> io::Result<()> {
        if request_id == [0; 16] {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if let Some(attempted) = self.attempted {
            if attempted != request_id {
                return Err(io::ErrorKind::AlreadyExists.into());
            }
        } else {
            if read(&mut self.file)? != ActivationState::Active {
                return Err(invalid());
            }
            self.attempted = Some(request_id);
        }
        let mut bytes = Vec::with_capacity(MAXIMUM_RECORD_BYTES);
        bytes.extend_from_slice(STOPPED_PREFIX);
        for byte in request_id {
            bytes.push(b"0123456789abcdef"[(byte >> 4) as usize]);
            bytes.push(b"0123456789abcdef"[(byte & 15) as usize]);
        }
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&bytes)?;
        self.file.set_len(bytes.len() as u64)?;
        self.file.sync_data()
    }
}

#[cfg(test)]
mod tests;
