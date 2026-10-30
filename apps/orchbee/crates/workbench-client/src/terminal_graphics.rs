//! Bounded host graphics facts; only the admitted Windows shell activates probes.
pub use crate::terminal_events::graphics::Reply;
use std::time::{Duration, Instant};

#[cfg(any(test, feature = "test-support"))]
pub use crate::host_graphics::FactsTestAccess;
pub use crate::host_graphics::{Facts, Measurement};

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
pub struct Probe {
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

/// Field-for-field `Query` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct QueryFixture {
    pub cell: bool,
    pub image_id: Option<u32>,
}

#[cfg(any(test, feature = "test-support"))]
impl From<QueryFixture> for Query {
    fn from(fixture: QueryFixture) -> Self {
        let QueryFixture { cell, image_id } = fixture;
        Self { cell, image_id }
    }
}

/// Crate-internal `Probe` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait ProbeTestAccess {
    fn activate(&mut self, image_id: u32);
    fn facts(&self) -> Facts;
    fn query(&mut self, cells: (u16, u16), now: Instant) -> Option<Query>;
    fn reply(&mut self, reply: Reply, now: Instant);
}

#[cfg(any(test, feature = "test-support"))]
impl ProbeTestAccess for Probe {
    fn activate(&mut self, image_id: u32) {
        Probe::activate(self, image_id);
    }
    fn facts(&self) -> Facts {
        Probe::facts(self)
    }
    fn query(&mut self, cells: (u16, u16), now: Instant) -> Option<Query> {
        Probe::query(self, cells, now)
    }
    fn reply(&mut self, reply: Reply, now: Instant) {
        Probe::reply(self, reply, now);
    }
}

#[cfg(test)]
mod tests;
