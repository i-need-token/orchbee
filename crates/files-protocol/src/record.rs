//! OBFB, the records File Bar traffic carries inside the Client multiplex: a 12-byte header
//! (`OBFB`, version, kind, length) and a body that is JSON for Request, Result and Notice records
//! and a fixed binary prefix with data for Put and Chunk records. A record is admitted only where
//! its kind belongs: Requests and Puts open streams, Results, Chunks and Notices answer them. This
//! codec frames and validates; the JSON inside belongs to the request, reply and notice codecs, and
//! which carrier negotiated `files` belongs to the multiplex.

use std::fmt;

use crate::identity::TransferId;

/// The record magic.
pub const RECORD_MAGIC: [u8; 4] = *b"OBFB";
/// The only record version of the `files` capability.
pub const RECORD_VERSION: u16 = 1;
/// The largest Request body.
pub const MAXIMUM_REQUEST_BYTES: usize = 16 * 1024;
/// The largest Result or Notice body.
pub const MAXIMUM_REPLY_BYTES: usize = 128 * 1024;
/// The most data one Put or Chunk carries.
pub const MAXIMUM_DATA_BYTES: usize = 128 * 1024;

const HEADER_BYTES: usize = 12;
const PUT_PREFIX_BYTES: usize = 17;
const CHUNK_PREFIX_BYTES: usize = 18;
const DIGEST_BYTES: usize = 32;

const REQUEST: u16 = 1;
const RESULT: u16 = 2;
const NOTICE: u16 = 3;
const PUT: u16 = 4;
const CHUNK: u16 = 5;

const LAST: u8 = 1;
const COMMIT: u8 = 2;
const CHANGED: u8 = 2;
const DIGEST: u8 = 4;

/// Where a record travels, which decides the kinds it may be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordPlace {
    /// A Client's StreamOpen in Request mode: a Request or a Put.
    RequestOpen,
    /// A Client's StreamOpen in Events mode: a subscribing Request.
    EventsOpen,
    /// A Server's StreamFrame Result: a Result or a Chunk.
    ResultFrame,
    /// A Server's StreamFrame Snapshot or Update: a Notice.
    NoticeFrame,
}

/// How a Put relates to the end of its transfer's data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PutEnd {
    /// More data follows.
    Continues,
    /// The transfer's data ends with this Put.
    Last,
    /// The data ends with this Put, and its operation commits it.
    Commit,
}

/// One piece of a transfer's data the Client sends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Put {
    /// The transfer the data belongs to.
    pub transfer: TransferId,
    /// Where the data starts in the transfer.
    pub offset: u64,
    /// Whether the data ends here.
    pub end: PutEnd,
    /// At most [`MAXIMUM_DATA_BYTES`] bytes.
    pub bytes: Vec<u8>,
}

/// The line ending a text read reports with its final chunk, by VS Code's buffer rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEnding {
    /// The text has no line break.
    None,
    /// Line feeds.
    Lf,
    /// Carriage return and line feed pairs.
    CrLf,
}

/// What the final chunk of a read reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkEnd {
    /// The text's line ending; [`LineEnding::None`] for a byte read.
    pub line_ending: LineEnding,
    /// The file's modification time or size moved while it was read.
    pub changed: bool,
    /// The SHA-256 of the file bytes a text read consumed, in order; a save sends it back as its
    /// precondition. A byte read carries none.
    pub digest: Option<[u8; DIGEST_BYTES]>,
}

/// One piece of a read the Server sends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    /// The transfer the data belongs to.
    pub transfer: TransferId,
    /// Where the data starts: in decoded UTF-8 output bytes for a text read, in file bytes for a
    /// byte read.
    pub offset: u64,
    /// Present on the read's final chunk only.
    pub end: Option<ChunkEnd>,
    /// At most [`MAXIMUM_DATA_BYTES`] bytes.
    pub bytes: Vec<u8>,
}

/// One OBFB record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilesRecord {
    /// Client to Server: an operation's JSON request, at most [`MAXIMUM_REQUEST_BYTES`].
    Request(Vec<u8>),
    /// Server to Client: a request's JSON result or error, at most [`MAXIMUM_REPLY_BYTES`].
    Result(Vec<u8>),
    /// Server to Client: a subscription's JSON snapshot or update, at most
    /// [`MAXIMUM_REPLY_BYTES`].
    Notice(Vec<u8>),
    /// Client to Server: transfer data.
    Put(Put),
    /// Server to Client: read data.
    Chunk(Chunk),
}

/// A record this codec cannot write or does not admit where it travels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordError {
    /// The header is short, foreign or of another version.
    InvalidHeader,
    /// The kind is unknown, or does not belong where the record travels.
    MisplacedKind,
    /// The length disagrees with the header or a bound.
    InvalidLength,
    /// A flag, line ending or identity is outside its domain.
    InvalidBody,
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidHeader => "invalid OBFB header",
            Self::MisplacedKind => "OBFB kind not admitted here",
            Self::InvalidLength => "invalid OBFB length",
            Self::InvalidBody => "invalid OBFB body",
        })
    }
}

impl std::error::Error for RecordError {}

/// Whether `record` claims to be an OBFB record, which only a carrier that negotiated `files`
/// admits.
#[must_use]
pub fn is_files_record(record: &[u8]) -> bool {
    record.starts_with(&RECORD_MAGIC)
}

/// Encodes one record.
///
/// # Errors
///
/// [`RecordError::InvalidLength`] for an empty JSON body or a body or data over its bound.
pub fn encode_files_record(record: &FilesRecord) -> Result<Vec<u8>, RecordError> {
    let (kind, body_length) = match record {
        FilesRecord::Request(json) => (REQUEST, json_length(json, MAXIMUM_REQUEST_BYTES)?),
        FilesRecord::Result(json) => (RESULT, json_length(json, MAXIMUM_REPLY_BYTES)?),
        FilesRecord::Notice(json) => (NOTICE, json_length(json, MAXIMUM_REPLY_BYTES)?),
        FilesRecord::Put(put) => (PUT, PUT_PREFIX_BYTES + data_length(&put.bytes)?),
        FilesRecord::Chunk(chunk) => {
            let digest = chunk
                .end
                .and_then(|end| end.digest)
                .map_or(0, |_| DIGEST_BYTES);
            (
                CHUNK,
                CHUNK_PREFIX_BYTES + digest + data_length(&chunk.bytes)?,
            )
        }
    };
    let mut bytes = Vec::with_capacity(HEADER_BYTES + body_length);
    bytes.extend_from_slice(&RECORD_MAGIC);
    bytes.extend_from_slice(&RECORD_VERSION.to_be_bytes());
    bytes.extend_from_slice(&kind.to_be_bytes());
    let length = u32::try_from(body_length).map_err(|_| RecordError::InvalidLength)?;
    bytes.extend_from_slice(&length.to_be_bytes());
    match record {
        FilesRecord::Request(json) | FilesRecord::Result(json) | FilesRecord::Notice(json) => {
            bytes.extend_from_slice(json);
        }
        FilesRecord::Put(put) => {
            bytes.extend_from_slice(&put.transfer.get().to_be_bytes());
            bytes.extend_from_slice(&put.offset.to_be_bytes());
            bytes.push(match put.end {
                PutEnd::Continues => 0,
                PutEnd::Last => LAST,
                PutEnd::Commit => LAST | COMMIT,
            });
            bytes.extend_from_slice(&put.bytes);
        }
        FilesRecord::Chunk(chunk) => {
            bytes.extend_from_slice(&chunk.transfer.get().to_be_bytes());
            bytes.extend_from_slice(&chunk.offset.to_be_bytes());
            let (flags, line_ending) = chunk.end.map_or((0, 0), |end| {
                let changed = if end.changed { CHANGED } else { 0 };
                let digest = if end.digest.is_some() { DIGEST } else { 0 };
                let line_ending = match end.line_ending {
                    LineEnding::None => 1,
                    LineEnding::Lf => 2,
                    LineEnding::CrLf => 3,
                };
                (LAST | changed | digest, line_ending)
            });
            bytes.push(flags);
            bytes.push(line_ending);
            if let Some(digest) = chunk.end.and_then(|end| end.digest) {
                bytes.extend_from_slice(&digest);
            }
            bytes.extend_from_slice(&chunk.bytes);
        }
    }
    Ok(bytes)
}

/// Decodes one complete record that travels at `place`.
///
/// # Errors
///
/// [`RecordError`] for a record [`check_files_record`] refuses.
pub fn decode_files_record(record: &[u8], place: RecordPlace) -> Result<FilesRecord, RecordError> {
    let (kind, body) = admit(record, place)?;
    Ok(match kind {
        REQUEST => FilesRecord::Request(body.to_vec()),
        RESULT => FilesRecord::Result(body.to_vec()),
        NOTICE => FilesRecord::Notice(body.to_vec()),
        PUT => FilesRecord::Put(put(body)?),
        _ => FilesRecord::Chunk(chunk(body)?),
    })
}

/// Checks one complete record that travels at `place` without copying it: a known header, a kind
/// that belongs at `place`, a length the header states and the kind bounds, and a binary prefix in
/// its domain.
///
/// # Errors
///
/// [`RecordError`] names the first rule the record breaks.
pub fn check_files_record(record: &[u8], place: RecordPlace) -> Result<(), RecordError> {
    let (kind, body) = admit(record, place)?;
    match kind {
        PUT => put(body).map(drop),
        CHUNK => chunk(body).map(drop),
        _ => Ok(()),
    }
}

fn json_length(json: &[u8], maximum: usize) -> Result<usize, RecordError> {
    if json.is_empty() || json.len() > maximum {
        return Err(RecordError::InvalidLength);
    }
    Ok(json.len())
}

fn data_length(bytes: &[u8]) -> Result<usize, RecordError> {
    if bytes.len() > MAXIMUM_DATA_BYTES {
        return Err(RecordError::InvalidLength);
    }
    Ok(bytes.len())
}

/// The kind and body of a record whose header, placement and length hold.
fn admit(record: &[u8], place: RecordPlace) -> Result<(u16, &[u8]), RecordError> {
    let header = record
        .get(..HEADER_BYTES)
        .ok_or(RecordError::InvalidHeader)?;
    if header[..4] != RECORD_MAGIC || header[4..6] != RECORD_VERSION.to_be_bytes() {
        return Err(RecordError::InvalidHeader);
    }
    let kind = u16::from_be_bytes([header[6], header[7]]);
    let placed = match place {
        RecordPlace::RequestOpen => matches!(kind, REQUEST | PUT),
        RecordPlace::EventsOpen => kind == REQUEST,
        RecordPlace::ResultFrame => matches!(kind, RESULT | CHUNK),
        RecordPlace::NoticeFrame => kind == NOTICE,
    };
    if !placed {
        return Err(RecordError::MisplacedKind);
    }
    let length = usize::try_from(u32::from_be_bytes([
        header[8], header[9], header[10], header[11],
    ]))
    .map_err(|_| RecordError::InvalidLength)?;
    let body = &record[HEADER_BYTES..];
    let bounded = match kind {
        REQUEST => (1..=MAXIMUM_REQUEST_BYTES).contains(&length),
        RESULT | NOTICE => (1..=MAXIMUM_REPLY_BYTES).contains(&length),
        PUT => (PUT_PREFIX_BYTES..=PUT_PREFIX_BYTES + MAXIMUM_DATA_BYTES).contains(&length),
        _ => (CHUNK_PREFIX_BYTES..=CHUNK_PREFIX_BYTES + DIGEST_BYTES + MAXIMUM_DATA_BYTES)
            .contains(&length),
    };
    if !bounded || body.len() != length {
        return Err(RecordError::InvalidLength);
    }
    Ok((kind, body))
}

fn transfer(bytes: &[u8]) -> Result<TransferId, RecordError> {
    let value = u64::from_be_bytes(bytes.try_into().map_err(|_| RecordError::InvalidBody)?);
    TransferId::new(value).ok_or(RecordError::InvalidBody)
}

fn offset(bytes: &[u8]) -> Result<u64, RecordError> {
    Ok(u64::from_be_bytes(
        bytes.try_into().map_err(|_| RecordError::InvalidBody)?,
    ))
}

fn put(body: &[u8]) -> Result<Put, RecordError> {
    let end = match body[16] {
        0 => PutEnd::Continues,
        LAST => PutEnd::Last,
        flags if flags == LAST | COMMIT => PutEnd::Commit,
        _ => return Err(RecordError::InvalidBody),
    };
    Ok(Put {
        transfer: transfer(&body[..8])?,
        offset: offset(&body[8..16])?,
        end,
        bytes: body[PUT_PREFIX_BYTES..].to_vec(),
    })
}

fn chunk(body: &[u8]) -> Result<Chunk, RecordError> {
    let (flags, line_ending) = (body[16], body[17]);
    if flags & !(LAST | CHANGED | DIGEST) != 0 {
        return Err(RecordError::InvalidBody);
    }
    let mut data = &body[CHUNK_PREFIX_BYTES..];
    let end = if flags & LAST == 0 {
        // Only a final chunk reports its read's facts.
        if flags != 0 || line_ending != 0 {
            return Err(RecordError::InvalidBody);
        }
        None
    } else {
        let line_ending = match line_ending {
            1 => LineEnding::None,
            2 => LineEnding::Lf,
            3 => LineEnding::CrLf,
            _ => return Err(RecordError::InvalidBody),
        };
        let digest = if flags & DIGEST == 0 {
            None
        } else {
            let (digest, rest) = data
                .split_first_chunk::<DIGEST_BYTES>()
                .ok_or(RecordError::InvalidLength)?;
            data = rest;
            Some(*digest)
        };
        Some(ChunkEnd {
            line_ending,
            changed: flags & CHANGED != 0,
            digest,
        })
    };
    if data.len() > MAXIMUM_DATA_BYTES {
        return Err(RecordError::InvalidLength);
    }
    Ok(Chunk {
        transfer: transfer(&body[..8])?,
        offset: offset(&body[8..16])?,
        end,
        bytes: data.to_vec(),
    })
}
