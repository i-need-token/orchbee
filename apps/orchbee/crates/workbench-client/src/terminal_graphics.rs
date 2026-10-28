//! Bounded host graphics facts; only the admitted Windows shell activates probes.
use crate::terminal_device::WindowDimensions;
pub(crate) use crate::terminal_events::graphics::Reply;
use orchbee_workbench_protocol::{
    PaneGraphicsCapabilities, PaneGraphicsCellPixels, PaneGraphicsTransport,
};
use std::time::{Duration, Instant};

const QUERY_DEADLINE: Duration = Duration::from_millis(300);
pub(crate) mod placement;

/// An emitted query creates no placement and never guesses a terminal capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Query {
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
pub struct Measurement {
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
    pub fn apply(self, mut dimensions: WindowDimensions) -> WindowDimensions {
        if self.cells == (dimensions.columns, dimensions.rows) {
            dimensions.pixel_width = self.pixels.0 * self.cells.0;
            dimensions.pixel_height = self.pixels.1 * self.cells.1;
        }
        dimensions
    }
}

/// Query support remains an internal fact; it cannot advertise a renderer by itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Facts {
    pub measurement: Option<Measurement>,
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
#[cfg(test)]
mod tests;
