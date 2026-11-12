//! Hex, frame comparison, and the byte queues that join two state machines in memory.

use std::{collections::VecDeque, error::Error};

use orchbee_machine_protocol::{
    Connection, Delivery, Frame, FrameDecoder, PathText, Protocol, SessionName, Token,
    link::{Channel, Facts},
};
use serde_json::Value;

pub type TestResult = Result<(), Box<dyn Error>>;

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

/// Whether two frames are the same record: equal kind and channel, and equal JSON values or equal
/// bytes.
pub fn same_frame(actual: &Frame, expected: &Frame) -> bool {
    if actual.kind() != expected.kind() || actual.channel() != expected.channel() {
        return false;
    }
    if !actual.kind().carries_json() {
        return actual.payload() == expected.payload();
    }
    let parse = |frame: &Frame| serde_json::from_slice::<Value>(frame.payload()).ok();
    parse(actual).is_some() && parse(actual) == parse(expected)
}

/// One direction of a carrier: the bytes written and not yet read, and the reader's decoder.
#[derive(Default)]
pub struct Wire {
    bytes: VecDeque<u8>,
    decoder: FrameDecoder,
    /// The largest piece the reader takes at once, to exercise partial reads.
    piece: usize,
}

impl Wire {
    pub fn new(piece: usize) -> Self {
        Self {
            piece,
            ..Self::default()
        }
    }

    pub fn write(&mut self, frame: &Frame) {
        self.bytes.extend(frame.encode());
    }

    /// The next whole frame the reader decodes, if the written bytes hold one.
    pub fn read(&mut self) -> Result<Option<Frame>, Box<dyn Error>> {
        while !self.bytes.is_empty() {
            let available = self.bytes.make_contiguous();
            let piece = available.len().min(self.piece.max(1));
            let (consumed, frame) = self
                .decoder
                .decode(available.get(..piece).unwrap_or_default())?;
            self.bytes.drain(..consumed);
            if frame.is_some() {
                return Ok(frame);
            }
        }
        Ok(None)
    }
}

/// Writes every frame `connection` has queued to `wire` and returns how many it wrote.
pub fn transmit<P: Protocol>(connection: &mut Connection<P>, wire: &mut Wire) -> usize {
    let mut written = 0;
    while let Some(frame) = connection.poll_transmit() {
        wire.write(&frame);
        written += 1;
    }
    written
}

/// Feeds every whole frame on `wire` to `connection` and returns what they delivered.
pub fn absorb<P: Protocol>(
    connection: &mut Connection<P>,
    wire: &mut Wire,
) -> Result<Vec<Delivery<P>>, Box<dyn Error>> {
    let mut deliveries = Vec::new();
    while let Some(frame) = wire.read()? {
        let receipt = connection.receive(&frame)?;
        deliveries.extend(receipt.delivery);
    }
    Ok(deliveries)
}

/// The facts of a Linux helper.
pub fn facts() -> Result<Facts, Box<dyn Error>> {
    Ok(Facts {
        platform: Token::new("linux")?,
        architecture: Token::new("x86_64")?,
        executable: PathText::new("/opt/orchbee/orchbee")?,
        store_root: None,
    })
}

/// A link `session` channel.
pub fn session(name: &str) -> Result<Channel, Box<dyn Error>> {
    Ok(Channel::Session {
        session: SessionName::new(name)?,
    })
}

/// The bytes a test writes at offset `index` of a stream, so order and loss show.
pub fn pattern(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}
