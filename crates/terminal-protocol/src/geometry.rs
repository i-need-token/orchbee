//! Refines terminal cell and pixel dimensions against scalar and product bounds for wire
//! controls. It validates geometry values without measuring an outer terminal, selecting a
//! resize lease, or applying a resize.

use serde::{Deserialize, Serialize};

use crate::{
    ProtocolError, ProtocolErrorCode,
    control::SafeInteger,
    limits::{
        MAXIMUM_CELL_PIXEL_DIMENSION, MAXIMUM_FRAME_CELLS, MAXIMUM_FRAME_COLUMNS,
        MAXIMUM_FRAME_ROWS, MAXIMUM_TERMINAL_PIXEL_EXTENT,
    },
    terminal_fact::bounded_u16,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalPixelGeometry {
    pub cell_pixel_height: u16,
    pub cell_pixel_width: u16,
    pub columns: u16,
    pub rows: u16,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TerminalPixelGeometryWire {
    pub(crate) cell_pixel_height: SafeInteger,
    pub(crate) cell_pixel_width: SafeInteger,
    pub(crate) columns: SafeInteger,
    pub(crate) rows: SafeInteger,
}

pub(crate) fn geometry_from_wire(
    wire: TerminalPixelGeometryWire,
) -> Result<TerminalPixelGeometry, ProtocolError> {
    let geometry = geometry_dimensions_from_wire(wire)?;
    validate_geometry(geometry)?;
    Ok(geometry)
}

pub(crate) fn geometry_dimensions_from_wire(
    wire: TerminalPixelGeometryWire,
) -> Result<TerminalPixelGeometry, ProtocolError> {
    let geometry = TerminalPixelGeometry {
        cell_pixel_height: bounded_u16(wire.cell_pixel_height, 0, MAXIMUM_CELL_PIXEL_DIMENSION)?,
        cell_pixel_width: bounded_u16(wire.cell_pixel_width, 0, MAXIMUM_CELL_PIXEL_DIMENSION)?,
        columns: bounded_u16(wire.columns, 1, MAXIMUM_FRAME_COLUMNS)?,
        rows: bounded_u16(wire.rows, 1, MAXIMUM_FRAME_ROWS)?,
    };
    if (geometry.cell_pixel_width == 0) != (geometry.cell_pixel_height == 0) {
        return Err(invalid_field(
            "terminal pixel geometry mixes known and unknown cell pixel dimensions",
        ));
    }
    Ok(geometry)
}

pub(crate) fn geometry_to_wire(
    value: TerminalPixelGeometry,
) -> Result<TerminalPixelGeometryWire, ProtocolError> {
    validate_geometry(value)?;
    Ok(TerminalPixelGeometryWire {
        cell_pixel_height: SafeInteger::new(u64::from(value.cell_pixel_height)),
        cell_pixel_width: SafeInteger::new(u64::from(value.cell_pixel_width)),
        columns: SafeInteger::new(u64::from(value.columns)),
        rows: SafeInteger::new(u64::from(value.rows)),
    })
}

pub(crate) fn validate_geometry(value: TerminalPixelGeometry) -> Result<(), ProtocolError> {
    validate_geometry_dimensions(value)?;
    if (value.cell_pixel_width == 0) != (value.cell_pixel_height == 0) {
        return Err(invalid_field(
            "terminal pixel geometry mixes known and unknown cell pixel dimensions",
        ));
    }

    let columns = u32::from(value.columns);
    let rows = u32::from(value.rows);
    if usize::try_from(columns * rows).map_or(true, |cells| cells > MAXIMUM_FRAME_CELLS) {
        return Err(invalid_field(
            "terminal pixel geometry exceeds the maximum grid cell count",
        ));
    }
    if columns * u32::from(value.cell_pixel_width) > MAXIMUM_TERMINAL_PIXEL_EXTENT
        || rows * u32::from(value.cell_pixel_height) > MAXIMUM_TERMINAL_PIXEL_EXTENT
    {
        return Err(invalid_field(
            "terminal pixel geometry exceeds the PTY unsigned 16-bit extent",
        ));
    }
    Ok(())
}

fn validate_geometry_dimensions(value: TerminalPixelGeometry) -> Result<(), ProtocolError> {
    if usize::from(value.cell_pixel_height) > MAXIMUM_CELL_PIXEL_DIMENSION
        || usize::from(value.cell_pixel_width) > MAXIMUM_CELL_PIXEL_DIMENSION
        || value.columns == 0
        || usize::from(value.columns) > MAXIMUM_FRAME_COLUMNS
        || value.rows == 0
        || usize::from(value.rows) > MAXIMUM_FRAME_ROWS
    {
        return Err(invalid_field(
            "terminal pixel geometry dimension is outside its protocol bounds",
        ));
    }
    Ok(())
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
