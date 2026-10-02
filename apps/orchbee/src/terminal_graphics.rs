//! Bounded host graphics facts; only the admitted Windows shell activates probes.
use crate::terminal_device::WindowDimensions;
use orchbee_workbench_protocol::{
    PaneGraphicsCapabilities, PaneGraphicsCellPixels, PaneGraphicsTransport,
};
use std::time::{Duration, Instant};

const QUERY_DEADLINE: Duration = Duration::from_millis(300);
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

/// An emitted query creates no placement and never guesses a terminal capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Query {
    pub(crate) cell: bool,
    pub(crate) image_id: Option<u32>,
}
impl Query {
    pub(crate) fn sequence(self) -> String {
        let mut sequence = String::new();
        if self.cell {
            sequence.push_str("\x1b[16t");
        }
        if let Some(id) = self.image_id {
            // One transparent RGBA pixel, direct transport, query rather than upload.
            sequence.push_str(&format!("\x1b_Ga=q,t=d,f=32,s=1,v=1,i={id};AAAAAA==\x1b\\"));
        }
        sequence
    }
}

/// Exact cell geometry fences the lifetime of measured physical pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Measurement {
    cells: (u16, u16),
    pixels: (u16, u16),
}
impl Measurement {
    fn new(cells: (u16, u16), pixels: (u16, u16)) -> Option<Self> {
        let (width, height) = pixels;
        let (columns, rows) = cells;
        (columns > 0
            && rows > 0
            && width > 0
            && height > 0
            && width.checked_mul(columns).is_some()
            && height.checked_mul(rows).is_some())
        .then_some(Self { cells, pixels })
    }
    pub(crate) fn apply(self, mut dimensions: WindowDimensions) -> WindowDimensions {
        if self.cells == (dimensions.columns, dimensions.rows) {
            dimensions.pixel_width = self.pixels.0 * self.cells.0;
            dimensions.pixel_height = self.pixels.1 * self.cells.1;
        }
        dimensions
    }
}

/// Query support remains an internal fact; it cannot advertise a renderer by itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Facts {
    pub(crate) measurement: Option<Measurement>,
    pub(crate) kitty: bool,
}
impl Facts {
    pub(crate) fn cell_capabilities(self) -> PaneGraphicsCapabilities {
        PaneGraphicsCapabilities {
            transport: PaneGraphicsTransport::CellRaster,
            cell_pixels: self.measurement.map(|measurement| PaneGraphicsCellPixels {
                width: measurement.pixels.0,
                height: measurement.pixels.1,
            }),
            pixel_mouse: false,
        }
    }
}

#[derive(Default)]
enum CellState {
    #[default]
    Unissued,
    Pending {
        cells: (u16, u16),
        deadline: Instant,
    },
    /// The uncorrelated reply still owes a terminator, even after timeout/resize.
    Debt,
    Settled(Option<Measurement>),
}
#[derive(Default)]
enum KittyState {
    #[default]
    Unissued,
    Pending {
        deadline: Instant,
    },
    Settled(bool),
}

/// One outstanding legacy measurement plus one correlated graphics support query.
#[derive(Default)]
pub(crate) struct Probe {
    image_id: Option<u32>,
    cells: Option<(u16, u16)>,
    cell: CellState,
    kitty: KittyState,
}
impl Probe {
    pub(crate) fn activate(&mut self, image_id: u32) {
        if image_id != 0 && self.image_id.is_none() {
            self.image_id = Some(image_id);
        }
    }
    pub(crate) fn facts(&self) -> Facts {
        Facts {
            measurement: match self.cell {
                CellState::Settled(measurement) => measurement,
                _ => None,
            },
            kitty: matches!(self.kitty, KittyState::Settled(true)),
        }
    }
    pub(crate) fn query(&mut self, cells: (u16, u16), now: Instant) -> Option<Query> {
        let id = self.image_id?;
        if cells.0 == 0 || cells.1 == 0 {
            return None;
        }
        self.tick(now);
        if self.cells != Some(cells) {
            self.cell = match self.cell {
                CellState::Pending { .. } | CellState::Debt => CellState::Debt,
                _ => CellState::Unissued,
            };
            self.cells = Some(cells);
        }
        let cell = matches!(self.cell, CellState::Unissued);
        let image_id = matches!(self.kitty, KittyState::Unissued).then_some(id);
        if !cell && image_id.is_none() {
            return None;
        }
        let deadline = now + QUERY_DEADLINE;
        if cell {
            self.cell = CellState::Pending { cells, deadline };
        }
        if image_id.is_some() {
            self.kitty = KittyState::Pending { deadline };
        }
        Some(Query { cell, image_id })
    }
    pub(crate) fn tick(&mut self, now: Instant) {
        if matches!(self.cell, CellState::Pending { deadline, .. } if now >= deadline) {
            self.cell = CellState::Debt;
        }
        if matches!(self.kitty, KittyState::Pending { deadline } if now >= deadline) {
            self.kitty = KittyState::Settled(false);
        }
    }
    pub(crate) fn reply(&mut self, reply: Reply, now: Instant) {
        self.tick(now);
        match reply {
            Reply::Cell(pixels) => match self.cell {
                CellState::Pending { cells, .. } if self.cells == Some(cells) => {
                    self.cell = CellState::Settled(
                        pixels.and_then(|pixels| Measurement::new(cells, pixels)),
                    );
                }
                CellState::Debt => self.cell = CellState::Unissued,
                _ => {}
            },
            Reply::Kitty {
                image_id,
                placement_id: None,
                success,
            } if self.image_id == Some(image_id)
                && matches!(self.kitty, KittyState::Pending { .. }) =>
            {
                self.kitty = KittyState::Settled(success);
            }
            Reply::Kitty { .. } => {}
        }
    }
}

/// Recognize cell-report syntax even when its dimensions must be refused.
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
