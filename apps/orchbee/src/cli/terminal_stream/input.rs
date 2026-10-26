//! Bounded NDJSON commands and an invocation-owned cancellable native stdin reader.
use crate::cli::stdin_bytes::{Poll as BytePoll, Reader as ByteReader};
use crate::client_view::Modifiers;
use base64::Engine;
use orchbee_client_server_protocol::Geometry;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};
use std::{io, process::Stdio};

pub(super) const MAX_INPUT: usize = 256 * 1024;
const MAX_LINE: usize = 2 * 1024 * 1024;
// The stream wire format accepts SHIFT, CONTROL, ALT, SUPER, HYPER and META.
// Pane input projects the first four; the last two remain accepted but inert.
const SCROLL_MODIFIER_MASK: u16 = 0b0011_1111;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Direction {
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ScrollSource {
    #[default]
    Wheel,
    PageKey,
}
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Command {
    Input(Vec<u8>),
    Resize(Geometry),
    Scroll(Scroll),
    Release,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Scroll {
    pub(super) direction: Direction,
    pub(super) lines: u16,
    pub(super) source: ScrollSource,
    pub(super) column: Option<u16>,
    pub(super) row: Option<u16>,
    pub(super) modifiers: u8,
}

impl Scroll {
    pub(super) fn pane_modifiers(self) -> Modifiers {
        Modifiers {
            shift: self.modifiers & 0b0000_0001 != 0,
            ctrl: self.modifiers & 0b0000_0010 != 0,
            alt: self.modifiers & 0b0000_0100 != 0,
            super_: self.modifiers & 0b0000_1000 != 0,
        }
    }
}
struct Fields(BTreeMap<String, Value>);
impl<'de> Deserialize<'de> for Fields {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> Visitor<'de> for Object {
            type Value = Fields;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a closed terminal control command")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Fields, M::Error> {
                let mut fields = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    if fields.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate command field"));
                    }
                }
                Ok(Fields(fields))
            }
        }
        deserializer.deserialize_map(Object)
    }
}
impl Fields {
    fn string(&mut self, name: &str) -> Result<String, &'static str> {
        match self.0.remove(name) {
            Some(Value::String(value)) => Ok(value),
            _ => Err("invalid-command"),
        }
    }
    fn number(&mut self, name: &str, default: Option<u16>) -> Result<u16, &'static str> {
        match self.0.remove(name) {
            Some(value) => value
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or("invalid-command"),
            None => default.ok_or("invalid-command"),
        }
    }
    fn coordinate(&mut self, name: &str) -> Result<Option<u16>, &'static str> {
        if !self.0.contains_key(name) {
            Ok(None)
        } else {
            self.number(name, None).map(Some)
        }
    }
}
pub(super) fn parse(line: &[u8]) -> Result<Command, &'static str> {
    if line.len() > MAX_LINE {
        return Err("record-too-large");
    }
    let mut fields: Fields = serde_json::from_slice(line).map_err(|_| "invalid-command")?;
    let command = match fields.string("type")?.as_str() {
        "terminal.input" => {
            if fields.0.contains_key("text") == fields.0.contains_key("bytes") {
                return Err("input-source-conflict");
            }
            let bytes = if fields.0.contains_key("text") {
                fields.string("text")?.into_bytes()
            } else {
                base64::engine::general_purpose::STANDARD
                    .decode(fields.string("bytes")?)
                    .map_err(|_| "invalid-base64")?
            };
            if bytes.len() > MAX_INPUT {
                return Err("input-too-large");
            }
            Command::Input(bytes)
        }
        "terminal.resize" => Command::Resize(
            Geometry::new(
                fields.number("cols", None)?,
                fields.number("rows", None)?,
                fields.number("cell_width_px", Some(0))?,
                fields.number("cell_height_px", Some(0))?,
            )
            .map_err(|_| "invalid-geometry")?,
        ),
        "terminal.scroll" => {
            let direction = match fields.string("direction")?.as_str() {
                "up" => Direction::Up,
                "down" => Direction::Down,
                _ => return Err("invalid-scroll"),
            };
            let lines = fields.number("lines", None)?;
            let source = if fields.0.contains_key("source") {
                match fields.string("source")?.as_str() {
                    "wheel" => ScrollSource::Wheel,
                    "page_key" => ScrollSource::PageKey,
                    _ => return Err("invalid-scroll"),
                }
            } else {
                ScrollSource::Wheel
            };
            let column = fields.coordinate("column")?;
            let row = fields.coordinate("row")?;
            let modifiers = fields.number("modifiers", Some(0))?;
            if lines == 0 || modifiers & !SCROLL_MODIFIER_MASK != 0 {
                return Err("invalid-scroll");
            }
            Command::Scroll(Scroll {
                direction,
                lines,
                source,
                column,
                row,
                // Only the six admitted bits remain, so conversion cannot truncate.
                modifiers: modifiers as u8,
            })
        }
        "terminal.release" => Command::Release,
        _ => return Err("invalid-command"),
    };
    if !fields.0.is_empty() {
        return Err("invalid-command");
    }
    Ok(command)
}

pub(super) enum Poll {
    Pending,
    Line(Vec<u8>),
    Eof,
}
pub(super) struct Reader {
    bytes: ByteReader,
    pending: Vec<u8>,
    eof: bool,
    first: bool,
}
impl Reader {
    pub(super) fn start() -> io::Result<Self> {
        Self::with_input(Stdio::inherit())
    }
    pub(super) fn with_input(input: Stdio) -> io::Result<Self> {
        let bytes = ByteReader::with_input(input)?;
        Ok(Self {
            bytes,
            pending: Vec::new(),
            eof: false,
            first: true,
        })
    }
    pub(super) fn poll(&mut self) -> io::Result<Poll> {
        if let Some(line) = self.line()? {
            return Ok(Poll::Line(line));
        }
        if self.eof {
            return self.ending();
        }
        let mut buffer = [0; 8192];
        match self.bytes.poll(&mut buffer) {
            Ok(BytePoll::Eof) => {
                self.eof = true;
                self.ending()
            }
            Ok(BytePoll::Bytes(n)) => {
                self.pending.extend_from_slice(&buffer[..n]);
                Ok(self.line()?.map_or(Poll::Pending, Poll::Line))
            }
            Ok(BytePoll::Pending) => Ok(Poll::Pending),
            Err(e) => Err(e),
        }
    }
    fn line(&mut self) -> io::Result<Option<Vec<u8>>> {
        let end = self.pending.iter().position(|b| *b == b'\n');
        if end.unwrap_or(self.pending.len()) > MAX_LINE {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(end.map(|end| {
            let mut line = self.pending.drain(..=end).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.strip_bom(line)
        }))
    }
    fn strip_bom(&mut self, mut line: Vec<u8>) -> Vec<u8> {
        if self.first && line.starts_with(&[0xef, 0xbb, 0xbf]) {
            line.drain(..3);
        }
        self.first = false;
        line
    }
    fn ending(&mut self) -> io::Result<Poll> {
        if self.pending.is_empty() {
            Ok(Poll::Eof)
        } else {
            let line = std::mem::take(&mut self.pending);
            Ok(Poll::Line(self.strip_bom(line)))
        }
    }
}

#[cfg(test)]
mod tests;
