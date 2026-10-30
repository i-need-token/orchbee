//! Window geometry and graphics facts measured from the outer host terminal; the device adapter
//! and the probes stay with `terminal_device` and `terminal_graphics`.

use orchbee_workbench_protocol::{
    PaneGraphicsCapabilities, PaneGraphicsCellPixels, PaneGraphicsTransport,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowDimensions {
    pub columns: u16,
    pub pixel_height: u16,
    pub pixel_width: u16,
    pub rows: u16,
}

/// Exact cell geometry fences the lifetime of measured physical pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Measurement {
    cells: (u16, u16),
    pixels: (u16, u16),
}
impl Measurement {
    pub(crate) fn new(cells: (u16, u16), pixels: (u16, u16)) -> Option<Self> {
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

/// Recognize cell-report syntax even when its dimensions must be refused.
/// Crate-internal `Facts` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait FactsTestAccess {
    fn kitty(&self) -> bool;
    fn cell_capabilities(self) -> PaneGraphicsCapabilities;
}

#[cfg(any(test, feature = "test-support"))]
impl FactsTestAccess for Facts {
    fn kitty(&self) -> bool {
        self.kitty
    }
    fn cell_capabilities(self) -> PaneGraphicsCapabilities {
        Facts::cell_capabilities(self)
    }
}
