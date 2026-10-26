//! Pure bounded graphics reply parsing; host probes, measurements and effects retain their owners.
use std::time::{Duration, Instant};

const REPLY_BOUND: usize = 256;

/// Metadata decoded outside literal paste, never a key or a pane input packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reply {
    Cell(Option<(u16, u16)>),
    Kitty {
        image_id: u32,
        placement_id: Option<u32>,
        success: bool,
    },
}

pub(crate) fn cell_reply(body: &[u8]) -> Option<Reply> {
    let dimensions = body.strip_prefix(b"6;")?.strip_suffix(b"t")?;
    let mut fields = dimensions.split(|byte| *byte == b';');
    let height = fields
        .next()
        .and_then(decimal_u32)
        .and_then(|n| u16::try_from(n).ok());
    let width = fields
        .next()
        .and_then(decimal_u32)
        .and_then(|n| u16::try_from(n).ok());
    let pixels = match (width, height, fields.next()) {
        (Some(width), Some(height), None) if width > 0 && height > 0 => Some((width, height)),
        _ => None,
    };
    Some(Reply::Cell(pixels))
}

fn decimal_u32(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }
    bytes.iter().try_fold(0_u32, |value, byte| {
        byte.is_ascii_digit().then_some(())?;
        value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))
    })
}

/// Shared bounded graphics APC framing for ordinary input and native text capture.
#[derive(Debug)]
pub(crate) struct ReplyString {
    bytes: [u8; REPLY_BOUND],
    length: usize,
    escape: bool,
    drain: bool,
    deadline: Instant,
    wait: Duration,
}
impl ReplyString {
    pub(crate) fn new(now: Instant, wait: Duration) -> Self {
        Self {
            bytes: [0; REPLY_BOUND],
            length: 0,
            escape: false,
            drain: false,
            deadline: now + wait,
            wait,
        }
    }
    pub(crate) fn tick(&mut self, now: Instant) {
        self.drain |= now >= self.deadline;
    }
    /// `None` means the frame still owns its tail; `Some` ends it atomically.
    pub(crate) fn push(&mut self, byte: u8, now: Instant) -> Option<Option<Reply>> {
        self.tick(now);
        if byte == 0x9c || (self.escape && byte == b'\\') {
            return Some(
                (!self.drain)
                    .then(|| kitty_reply(&self.bytes[..self.length]))
                    .flatten(),
            );
        }
        if matches!(byte, 0x18 | 0x1a) {
            return Some(None);
        }
        if self.escape {
            self.drain = true;
        }
        self.escape = byte == 0x1b;
        if !self.escape && !self.drain {
            if self.length == REPLY_BOUND || !(0x20..=0x7e).contains(&byte) {
                self.drain = true;
            } else {
                self.bytes[self.length] = byte;
                self.length += 1;
            }
        }
        self.deadline = now + self.wait;
        None
    }
}
fn kitty_reply(bytes: &[u8]) -> Option<Reply> {
    let separator = bytes.iter().position(|byte| *byte == b';')?;
    let mut id = None;
    let mut placement_id = None;
    for field in bytes[..separator].split(|byte| *byte == b',') {
        let (key, value) = field.split_at_checked(2)?;
        match key {
            b"i=" if id.is_none() => id = Some(decimal_u32(value).filter(|id| *id != 0)?),
            b"p=" if placement_id.is_none() => {
                placement_id = Some(decimal_u32(value)?);
            }
            _ => return None,
        }
    }
    let response = &bytes[separator + 1..];
    if response.is_empty() || !response.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
        return None;
    }
    Some(Reply::Kitty {
        image_id: id?,
        placement_id,
        success: response == b"OK",
    })
}

#[cfg(test)]
mod tests;
