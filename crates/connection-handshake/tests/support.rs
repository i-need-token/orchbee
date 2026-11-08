//! Fixture direction, framing, and the schema validator the conformance tests share.

use std::{error::Error, io::Read};

use orchbee_connection_handshake::{
    AcceptorMessage, DecodeError, EncodeError, FRAME_HEADER_BYTES, FRAME_MAGIC, InitiatorMessage,
    ReadError, decode_acceptor_frame, decode_initiator_frame, encode_acceptor_frame,
    encode_initiator_frame, read_acceptor_message, read_initiator_message,
};
use serde::Deserialize;

pub type TestResult = Result<(), Box<dyn Error>>;

const SCHEMA: &str = include_str!("../../../schemas/connection/v1/handshake.schema.json");

/// The sender of a fixture message.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Initiator,
    Acceptor,
}

impl Direction {
    pub fn opposite(self) -> Self {
        match self {
            Self::Initiator => Self::Acceptor,
            Self::Acceptor => Self::Initiator,
        }
    }
}

/// A message decoded in a fixture's direction.
#[derive(Clone, Debug, PartialEq)]
pub enum Decoded {
    Initiator(InitiatorMessage),
    Acceptor(AcceptorMessage),
}

pub fn decode(direction: Direction, frame: &[u8]) -> Result<Decoded, DecodeError> {
    match direction {
        Direction::Initiator => decode_initiator_frame(frame).map(Decoded::Initiator),
        Direction::Acceptor => decode_acceptor_frame(frame).map(Decoded::Acceptor),
    }
}

pub fn read(direction: Direction, reader: &mut impl Read) -> Result<Decoded, ReadError> {
    match direction {
        Direction::Initiator => read_initiator_message(reader).map(Decoded::Initiator),
        Direction::Acceptor => read_acceptor_message(reader).map(Decoded::Acceptor),
    }
}

pub fn encode(message: &Decoded) -> Result<Vec<u8>, EncodeError> {
    match message {
        Decoded::Initiator(message) => encode_initiator_frame(message),
        Decoded::Acceptor(message) => encode_acceptor_frame(message),
    }
}

/// Frames `payload` without checking its length bound, so tests can build refused frames.
pub fn frame(payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut frame = FRAME_MAGIC.to_vec();
    frame.extend_from_slice(&u32::try_from(payload.len())?.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

pub fn payload(frame: &[u8]) -> Result<&[u8], Box<dyn Error>> {
    Ok(frame
        .get(FRAME_HEADER_BYTES..)
        .ok_or("a frame holds its header")?)
}

pub fn from_hex(text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return Err(format!("{text} has an odd number of hexadecimal digits").into());
    }
    digits
        .chunks(2)
        .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
        .collect()
}

pub fn schema() -> Result<jsonschema::Validator, Box<dyn Error>> {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA)?;
    if !jsonschema::draft202012::meta::is_valid(&schema) {
        return Err("the handshake schema is not a valid draft 2020-12 schema".into());
    }
    Ok(jsonschema::draft202012::new(&schema)?)
}
