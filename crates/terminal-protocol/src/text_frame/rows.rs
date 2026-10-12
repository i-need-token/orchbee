use super::{Reader, TerminalTextCell, TerminalTextRow, TerminalTextStyle, error};
use crate::{ProtocolError, ProtocolErrorCode, TerminalPixelGeometry};
use std::sync::Arc;

pub(super) fn read(
    reader: &mut Reader<'_>,
    geometry: TerminalPixelGeometry,
    count: u16,
    snapshot: bool,
    strings: &[Arc<str>],
    styles: &[TerminalTextStyle],
) -> Result<Vec<TerminalTextRow>, ProtocolError> {
    let mut rows: Vec<TerminalTextRow> = Vec::new();
    for ordinal in 0..count {
        let index = reader.u16()?;
        let flags = reader.u8()?;
        let prompt = reader.u8()?;
        let cells_count = reader.u16()?;
        if reader.u16()? != 0 || flags & !3 != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        if index >= geometry.rows
            || cells_count != geometry.columns
            || prompt > 2
            || (snapshot && index != ordinal)
            || rows.last().is_some_and(|previous| index <= previous.index)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        let mut cells: Vec<TerminalTextCell> = Vec::new();
        for column in 0..cells_count {
            let string = reader.u32()?;
            let style = reader.u16()?;
            let cell_flags = reader.u8()?;
            if reader.u8()? != 0 || cell_flags & !31 != 0 {
                return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
            }
            let width = cell_flags & 3;
            let previous_wide = cells.last().is_some_and(|cell| cell.width == 1);
            if ((cell_flags >> 2) & 3) > 2
                || (width == 2) != previous_wide
                || (width == 3 && (column != cells_count - 1 || flags & 1 == 0))
            {
                return Err(error(ProtocolErrorCode::InvalidField));
            }
            cells.push(TerminalTextCell {
                text: strings
                    .get(string as usize)
                    .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?
                    .clone(),
                style: *styles
                    .get(usize::from(style))
                    .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
                width,
            });
        }
        if cells.last().is_some_and(|cell| cell.width == 1) {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        if let Some(previous) = rows.last()
            && index == previous.index + 1
            && ((previous.flags & 1 != 0) != (flags & 2 != 0)
                || (previous.cells.last().is_some_and(|cell| cell.width == 3)
                    && !cells.first().is_some_and(|cell| cell.width == 1)))
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        rows.push(TerminalTextRow {
            index,
            flags,
            prompt_class: prompt,
            cells,
        });
    }
    Ok(rows)
}
