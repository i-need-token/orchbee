//! The `OBMX` frame both machine connections share: a 16-byte header — the bytes `OBMX`, a kind,
//! three zero bytes, a big-endian channel, and a big-endian payload length — followed by the
//! payload. Every reader checks the whole header against the kind's channel and length rules
//! before it takes or allocates a payload byte and never takes a byte past the declared payload.
//! What a payload means belongs to the record layer; carriers and partial writes stay with the
//! caller.

use std::{error::Error, fmt};

/// The four bytes that open every frame.
pub const FRAME_MAGIC: [u8; 4] = *b"OBMX";
/// The bytes of a frame header.
pub const FRAME_HEADER_BYTES: usize = 16;
/// The largest JSON payload of a `message`, `open`, `accept`, `close`, or `reset` frame.
pub const MAXIMUM_JSON_PAYLOAD_BYTES: usize = 65_536;
/// The largest payload of one `data` frame.
pub const MAXIMUM_DATA_PAYLOAD_BYTES: usize = 32_768;

const MINIMUM_JSON_PAYLOAD_BYTES: usize = 2;
const CREDIT_PAYLOAD_BYTES: usize = 4;
const PING_PAYLOAD_BYTES: usize = 8;
const KIND_OFFSET: usize = 4;
const RESERVED: std::ops::Range<usize> = 5..8;

/// The kind of a frame, which fixes its channel rule and payload bounds.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FrameKind {
    /// A control message on channel 0.
    Message,
    /// A channel's opening record, from its opener.
    Open,
    /// A channel's accepting record, from its responder.
    Accept,
    /// Opaque channel bytes, counted against credit.
    Data,
    /// A credit grant of 1 to 4,194,304 `data` bytes.
    Credit,
    /// Its sender sends no more `data` on the channel.
    End,
    /// The responder's result, closing the channel for both sides.
    Close,
    /// An immediate close of the channel by either side.
    Reset,
    /// Eight opaque bytes on channel 0, answered by a `pong`.
    Ping,
    /// The eight bytes of the `ping` it answers.
    Pong,
}

impl FrameKind {
    /// Every kind, in code order.
    pub const ALL: [Self; 10] = [
        Self::Message,
        Self::Open,
        Self::Accept,
        Self::Data,
        Self::Credit,
        Self::End,
        Self::Close,
        Self::Reset,
        Self::Ping,
        Self::Pong,
    ];

    /// The header's kind byte.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Message => 1,
            Self::Open => 2,
            Self::Accept => 3,
            Self::Data => 4,
            Self::Credit => 5,
            Self::End => 6,
            Self::Close => 7,
            Self::Reset => 8,
            Self::Ping => 9,
            Self::Pong => 10,
        }
    }

    /// The kind a header byte denotes, if any.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == code)
    }

    /// The contract's name of the kind, as fixtures spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Open => "open",
            Self::Accept => "accept",
            Self::Data => "data",
            Self::Credit => "credit",
            Self::End => "end",
            Self::Close => "close",
            Self::Reset => "reset",
            Self::Ping => "ping",
            Self::Pong => "pong",
        }
    }

    /// The kind `name` denotes, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Whether the payload is one JSON object.
    #[must_use]
    pub const fn carries_json(self) -> bool {
        matches!(
            self,
            Self::Message | Self::Open | Self::Accept | Self::Close | Self::Reset
        )
    }

    /// Whether the kind travels on channel 0; every other kind travels on a channel id.
    #[must_use]
    pub const fn on_channel_zero(self) -> bool {
        matches!(self, Self::Message | Self::Ping | Self::Pong)
    }

    const fn payload_bounds(self) -> (usize, usize) {
        match self {
            Self::Message | Self::Open | Self::Accept | Self::Close | Self::Reset => {
                (MINIMUM_JSON_PAYLOAD_BYTES, MAXIMUM_JSON_PAYLOAD_BYTES)
            }
            Self::Data => (1, MAXIMUM_DATA_PAYLOAD_BYTES),
            Self::Credit => (CREDIT_PAYLOAD_BYTES, CREDIT_PAYLOAD_BYTES),
            Self::End => (0, 0),
            Self::Ping | Self::Pong => (PING_PAYLOAD_BYTES, PING_PAYLOAD_BYTES),
        }
    }

    fn admits(self, channel: u32, length: usize) -> Result<(), FrameError> {
        if self.on_channel_zero() != (channel == 0) {
            return Err(FrameError::Channel);
        }
        let (lowest, highest) = self.payload_bounds();
        if (lowest..=highest).contains(&length) {
            Ok(())
        } else {
            Err(FrameError::Length)
        }
    }
}

/// Why bytes are not one valid frame. Every variant is a violation that ends the connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    /// The bytes do not start with `OBMX`.
    Magic,
    /// The kind byte names no frame kind.
    Kind,
    /// A reserved header byte is not zero.
    Reserved,
    /// The channel is not one the kind admits: 0 for `message`, `ping`, and `pong`, nonzero for
    /// the rest.
    Channel,
    /// The payload length is outside the kind's bounds.
    Length,
    /// The bytes end before the header or the declared payload does.
    Truncated,
    /// Bytes follow the declared payload.
    TrailingBytes,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Magic => "frame does not start with OBMX",
            Self::Kind => "frame kind is unknown",
            Self::Reserved => "frame reserved bytes are not zero",
            Self::Channel => "frame channel is not one its kind admits",
            Self::Length => "frame payload length is outside its kind's bounds",
            Self::Truncated => "frame ends before its declared payload",
            Self::TrailingBytes => "bytes follow the frame payload",
        })
    }
}

impl Error for FrameError {}

/// One frame whose channel and payload length satisfy its kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    kind: FrameKind,
    channel: u32,
    payload: Vec<u8>,
}

impl Frame {
    /// The frame of `kind` on `channel` with `payload`.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError::Channel`] or [`FrameError::Length`] when the kind does not admit the
    /// channel or the payload length.
    pub fn new(kind: FrameKind, channel: u32, payload: Vec<u8>) -> Result<Self, FrameError> {
        kind.admits(channel, payload.len())?;
        Ok(Self {
            kind,
            channel,
            payload,
        })
    }

    /// The frame's kind.
    #[must_use]
    pub const fn kind(&self) -> FrameKind {
        self.kind
    }

    /// The frame's channel; 0 for `message`, `ping`, and `pong`.
    #[must_use]
    pub const fn channel(&self) -> u32 {
        self.channel
    }

    /// The payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// The bytes of the whole frame.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(FRAME_HEADER_BYTES + self.payload.len());
        self.encode_into(&mut bytes);
        bytes
    }

    /// Appends the bytes of the whole frame to `bytes`, never interleaved with another frame.
    pub fn encode_into(&self, bytes: &mut Vec<u8>) {
        let length = u32::try_from(self.payload.len())
            .expect("a frame's kind bounds its payload far below u32::MAX");
        bytes.extend_from_slice(&FRAME_MAGIC);
        bytes.push(self.kind.code());
        bytes.extend_from_slice(&[0; 3]);
        bytes.extend_from_slice(&self.channel.to_be_bytes());
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(&self.payload);
    }
}

/// Checks every header field `prefix` already holds.
fn check_prefix(prefix: &[u8]) -> Result<(), FrameError> {
    let magic = prefix.get(..FRAME_MAGIC.len()).unwrap_or(prefix);
    if FRAME_MAGIC.get(..magic.len()) != Some(magic) {
        return Err(FrameError::Magic);
    }
    if let Some(&code) = prefix.get(KIND_OFFSET)
        && FrameKind::from_code(code).is_none()
    {
        return Err(FrameError::Kind);
    }
    let reserved = prefix
        .get(RESERVED.start..prefix.len().min(RESERVED.end))
        .unwrap_or_default();
    if reserved.iter().any(|&byte| byte != 0) {
        return Err(FrameError::Reserved);
    }
    Ok(())
}

/// Checks a complete header and returns its kind, channel, and payload length.
fn check_header(header: &[u8; FRAME_HEADER_BYTES]) -> Result<(FrameKind, u32, usize), FrameError> {
    check_prefix(header)?;
    let [_, _, _, _, code, _, _, _, c0, c1, c2, c3, l0, l1, l2, l3] = *header;
    let kind = FrameKind::from_code(code).ok_or(FrameError::Kind)?;
    let channel = u32::from_be_bytes([c0, c1, c2, c3]);
    let length =
        usize::try_from(u32::from_be_bytes([l0, l1, l2, l3])).map_err(|_| FrameError::Length)?;
    kind.admits(channel, length)?;
    Ok((kind, channel, length))
}

/// Decodes exactly one complete in-memory frame.
///
/// # Errors
///
/// Returns the first [`FrameError`] of the header, then [`FrameError::Truncated`] or
/// [`FrameError::TrailingBytes`] when the bytes do not end exactly with the declared payload.
pub fn decode_frame(bytes: &[u8]) -> Result<Frame, FrameError> {
    let header = bytes.get(..FRAME_HEADER_BYTES).unwrap_or(bytes);
    check_prefix(header)?;
    let header = <[u8; FRAME_HEADER_BYTES]>::try_from(header).map_err(|_| FrameError::Truncated)?;
    let (kind, channel, length) = check_header(&header)?;
    let payload = bytes.get(FRAME_HEADER_BYTES..).unwrap_or_default();
    match payload.len().cmp(&length) {
        std::cmp::Ordering::Less => Err(FrameError::Truncated),
        std::cmp::Ordering::Greater => Err(FrameError::TrailingBytes),
        std::cmp::Ordering::Equal => Ok(Frame {
            kind,
            channel,
            payload: payload.to_vec(),
        }),
    }
}

/// A decoder of a frame stream that takes input in pieces of any size. It rejects a header as soon
/// as a received header byte breaks a rule, allocates a payload only after the whole header
/// passed, and consumes at most the bytes of one frame per call, so it never reads past the
/// declared payload.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    header: [u8; FRAME_HEADER_BYTES],
    filled: usize,
    body: Option<PartialBody>,
    failure: Option<FrameError>,
}

#[derive(Debug)]
struct PartialBody {
    kind: FrameKind,
    channel: u32,
    length: usize,
    payload: Vec<u8>,
}

impl FrameDecoder {
    /// A decoder at the start of a frame.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumes bytes of at most one frame from the start of `input` and returns how many it
    /// consumed with the frame once complete. The caller passes the unconsumed rest again.
    ///
    /// # Errors
    ///
    /// Returns the header's first [`FrameError`]; the decoder then repeats it on every call.
    pub fn decode(&mut self, input: &[u8]) -> Result<(usize, Option<Frame>), FrameError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let mut consumed = 0;
        if self.body.is_none() {
            let wanted = FRAME_HEADER_BYTES - self.filled;
            let taken = wanted.min(input.len());
            let (piece, _) = input.split_at(taken);
            let (_, unfilled) = self.header.split_at_mut(self.filled);
            let (target, _) = unfilled.split_at_mut(taken);
            target.copy_from_slice(piece);
            self.filled += taken;
            consumed = taken;
            let checked = if self.filled < FRAME_HEADER_BYTES {
                check_prefix(self.header.get(..self.filled).unwrap_or_default()).map(|()| None)
            } else {
                check_header(&self.header).map(Some)
            };
            match checked {
                Err(failure) => {
                    self.failure = Some(failure);
                    return Err(failure);
                }
                Ok(None) => return Ok((consumed, None)),
                Ok(Some((kind, channel, length))) => {
                    self.body = Some(PartialBody {
                        kind,
                        channel,
                        length,
                        payload: Vec::with_capacity(length),
                    });
                }
            }
        }
        let Some(body) = self.body.as_mut() else {
            return Ok((consumed, None));
        };
        let rest = input.get(consumed..).unwrap_or_default();
        let taken = (body.length - body.payload.len()).min(rest.len());
        let (piece, _) = rest.split_at(taken);
        body.payload.extend_from_slice(piece);
        consumed += taken;
        if body.payload.len() < body.length {
            return Ok((consumed, None));
        }
        self.filled = 0;
        Ok((
            consumed,
            self.body.take().map(|body| Frame {
                kind: body.kind,
                channel: body.channel,
                payload: body.payload,
            }),
        ))
    }

    /// Whether the decoder holds no partial frame, so a carrier that ends now ends between
    /// frames.
    #[must_use]
    pub const fn is_between_frames(&self) -> bool {
        self.filled == 0 && self.body.is_none()
    }
}
