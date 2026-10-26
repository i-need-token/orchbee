//! Admits terminal dimensions only when scalar, cell-count, pixel-extent, and paired
//! cell-pixel bounds hold, including through JSON deserialization. It validates requested
//! geometry without observing a display or resizing a terminal.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::number::Binary64SafeInteger;

pub const MAXIMUM_GEOMETRY_VALUE: u16 = 4096;
pub const MAXIMUM_TERMINAL_CELLS: u32 = 262_144;
pub const MAXIMUM_PIXEL_EXTENT: u32 = 65_535;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryField {
    Columns,
    Rows,
    CellPixelWidth,
    CellPixelHeight,
}

impl fmt::Display for GeometryField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Columns => "columns",
            Self::Rows => "rows",
            Self::CellPixelWidth => "cellPixelWidth",
            Self::CellPixelHeight => "cellPixelHeight",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryError {
    ValueOutOfRange { field: GeometryField, value: u16 },
    MixedCellPixelPair,
    TerminalCellCountTooLarge { actual: u32, maximum: u32 },
    PixelWidthTooLarge { actual: u32, maximum: u32 },
    PixelHeightTooLarge { actual: u32, maximum: u32 },
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueOutOfRange { field, value } => {
                let minimum = match field {
                    GeometryField::Columns | GeometryField::Rows => 1,
                    GeometryField::CellPixelWidth | GeometryField::CellPixelHeight => 0,
                };
                write!(
                    formatter,
                    "geometry {field} is {value}; expected {minimum} through {MAXIMUM_GEOMETRY_VALUE}"
                )
            }
            Self::MixedCellPixelPair => write!(
                formatter,
                "geometry cellPixelWidth and cellPixelHeight must be both positive or both zero"
            ),
            Self::TerminalCellCountTooLarge { actual, maximum } => write!(
                formatter,
                "geometry contains {actual} cells; maximum is {maximum}"
            ),
            Self::PixelWidthTooLarge { actual, maximum } => write!(
                formatter,
                "geometry pixel width is {actual}; maximum is {maximum}"
            ),
            Self::PixelHeightTooLarge { actual, maximum } => write!(
                formatter,
                "geometry pixel height is {actual}; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for GeometryError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
}

impl Geometry {
    pub fn new(
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) -> Result<Self, GeometryError> {
        validate_value(GeometryField::Columns, columns)?;
        validate_value(GeometryField::Rows, rows)?;
        validate_cell_pixel_value(GeometryField::CellPixelWidth, cell_pixel_width)?;
        validate_cell_pixel_value(GeometryField::CellPixelHeight, cell_pixel_height)?;
        if (cell_pixel_width == 0) != (cell_pixel_height == 0) {
            return Err(GeometryError::MixedCellPixelPair);
        }

        let terminal_cell_count = u32::from(columns) * u32::from(rows);
        if terminal_cell_count > MAXIMUM_TERMINAL_CELLS {
            return Err(GeometryError::TerminalCellCountTooLarge {
                actual: terminal_cell_count,
                maximum: MAXIMUM_TERMINAL_CELLS,
            });
        }
        let pixel_width = u32::from(columns) * u32::from(cell_pixel_width);
        if pixel_width > MAXIMUM_PIXEL_EXTENT {
            return Err(GeometryError::PixelWidthTooLarge {
                actual: pixel_width,
                maximum: MAXIMUM_PIXEL_EXTENT,
            });
        }
        let pixel_height = u32::from(rows) * u32::from(cell_pixel_height);
        if pixel_height > MAXIMUM_PIXEL_EXTENT {
            return Err(GeometryError::PixelHeightTooLarge {
                actual: pixel_height,
                maximum: MAXIMUM_PIXEL_EXTENT,
            });
        }
        Ok(Self {
            columns,
            rows,
            cell_pixel_width,
            cell_pixel_height,
        })
    }

    #[must_use]
    pub const fn columns(self) -> u16 {
        self.columns
    }

    #[must_use]
    pub const fn rows(self) -> u16 {
        self.rows
    }

    #[must_use]
    pub const fn cell_pixel_width(self) -> u16 {
        self.cell_pixel_width
    }

    #[must_use]
    pub const fn cell_pixel_height(self) -> u16 {
        self.cell_pixel_height
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DecodedGeometry {
    columns: Binary64SafeInteger,
    rows: Binary64SafeInteger,
    cell_pixel_width: Binary64SafeInteger,
    cell_pixel_height: Binary64SafeInteger,
}

impl<'de> Deserialize<'de> for Geometry {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let decoded = DecodedGeometry::deserialize(deserializer)?;
        Self::new(
            decoded.columns.try_into_u16().map_err(de::Error::custom)?,
            decoded.rows.try_into_u16().map_err(de::Error::custom)?,
            decoded
                .cell_pixel_width
                .try_into_u16()
                .map_err(de::Error::custom)?,
            decoded
                .cell_pixel_height
                .try_into_u16()
                .map_err(de::Error::custom)?,
        )
        .map_err(de::Error::custom)
    }
}

fn validate_value(field: GeometryField, value: u16) -> Result<(), GeometryError> {
    if value == 0 || value > MAXIMUM_GEOMETRY_VALUE {
        return Err(GeometryError::ValueOutOfRange { field, value });
    }
    Ok(())
}

fn validate_cell_pixel_value(field: GeometryField, value: u16) -> Result<(), GeometryError> {
    if value > MAXIMUM_GEOMETRY_VALUE {
        return Err(GeometryError::ValueOutOfRange { field, value });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
