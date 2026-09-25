//! Complete OBKG resource-snapshot validation without copying decoded pixels.
use super::super::Reader;
use crate::{ProtocolError, ProtocolErrorCode};
use std::collections::HashMap;

pub(super) fn validate_snapshot(bytes: &[u8]) -> Result<(u16, u16), ProtocolError> {
    if bytes.len() < 40 {
        return Err(error(ProtocolErrorCode::Truncated));
    }
    if bytes.len() > 33 * 1024 * 1024 {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    if &bytes[..4] != b"OBKG" {
        return Err(error(ProtocolErrorCode::InvalidMagic));
    }
    let mut reader = Reader::new(&bytes[4..]);
    if reader.take(3)? != [0, 0, 2] {
        return Err(error(ProtocolErrorCode::UnsupportedVersion));
    }
    let kind = reader.u8()?;
    if reader.u32()? as usize != bytes.len() {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    let resource_revision = reader.u64()?;
    reader.u64()?; // animation tick, an unrestricted unsigned wire value
    let image_count = reader.u16()?;
    let placement_count = reader.u16()?;
    let declared_bytes = reader.u32()?;
    if reader.u32()? != 0 {
        return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
    }
    if image_count > 256 || placement_count > 1024 || declared_bytes > 32 * 1024 * 1024 {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    // A row window always carries all resources, never a placement-only update.
    if kind != 1 {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    let mut images = HashMap::new();
    let mut actual_bytes = 0_u64;
    for _ in 0..image_count {
        let id = reader.u32()?;
        let channels = reader.u8()?;
        let reserved = reader.take(3)?;
        let generation = reader.u64()?;
        let width = reader.u32()?;
        let height = reader.u32()?;
        let data_length = reader.u32()?;
        if reader.u32()? != 0 || reserved.iter().any(|byte| *byte != 0) {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        if !(1..=4).contains(&channels)
            || id == 0
            || generation == 0
            || !(1..=4096).contains(&width)
            || !(1..=4096).contains(&height)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        reader.take(data_length as usize)?;
        if u64::from(width) * u64::from(height) * u64::from(channels) != u64::from(data_length) {
            return Err(error(ProtocolErrorCode::LengthMismatch));
        }
        if images.insert(id, (width, height)).is_some() {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        actual_bytes += u64::from(data_length);
        if actual_bytes > 32 * 1024 * 1024 {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
    }
    if actual_bytes != u64::from(declared_bytes) {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    if image_count != 0 && resource_revision == 0 {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    for _ in 0..placement_count {
        let image = reader.u32()?;
        reader.u32()?; // placement ID, zero and duplicates are legal
        reader.u32()?; // signed column: every 32-bit representation is valid
        reader.u32()?; // signed row
        reader.u32()?; // unsigned offset X
        reader.u32()?; // unsigned offset Y
        let source_x = reader.u32()?;
        let source_y = reader.u32()?;
        let source_width = reader.u32()?;
        let source_height = reader.u32()?;
        let destination_width = reader.u32()?;
        let destination_height = reader.u32()?;
        reader.u32()?; // signed Z
        if reader.u32()? & !1 != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        if !(1..=4096).contains(&source_width)
            || !(1..=4096).contains(&source_height)
            || !(1..=65535).contains(&destination_width)
            || !(1..=65535).contains(&destination_height)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        let Some((width, height)) = images.get(&image) else {
            return Err(error(ProtocolErrorCode::InvalidField));
        };
        if u64::from(source_x) + u64::from(source_width) > u64::from(*width)
            || u64::from(source_y) + u64::from(source_height) > u64::from(*height)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
    }
    if !reader.bytes.is_empty() {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    Ok((image_count, placement_count))
}

fn error(code: ProtocolErrorCode) -> ProtocolError {
    ProtocolError::new(code, "invalid OBKG render resource snapshot")
}
