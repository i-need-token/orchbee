//! Styled buffer rows require the whole nested projection and graphics snapshot.
use super::{Detail, TERMINAL_ROW_WINDOW_HEADER_BYTES, decode_header};
use crate::{
    ProtocolError, ProtocolErrorCode, TerminalProjectionBuffer, TerminalRowWindowRoute,
    decode_terminal_projection_buffer,
};

mod graphics;

/// Borrowed styles with completely validated, unmaterialized graphics resources.
#[derive(Debug)]
pub struct TerminalRenderRowWindow<'a> {
    route: TerminalRowWindowRoute,
    projection: TerminalProjectionBuffer<'a>,
    ignored_optional_flags: u8,
    graphics_images: u16,
    graphics_placements: u16,
}

impl<'a> TerminalRenderRowWindow<'a> {
    #[must_use]
    pub const fn route(&self) -> TerminalRowWindowRoute {
        self.route
    }
    #[must_use]
    pub const fn projection(&self) -> &TerminalProjectionBuffer<'a> {
        &self.projection
    }
    #[must_use]
    pub const fn ignored_optional_flags(&self) -> u8 {
        self.ignored_optional_flags
    }
    #[must_use]
    pub const fn graphics_image_count(&self) -> u16 {
        self.graphics_images
    }
    #[must_use]
    pub const fn graphics_placement_count(&self) -> u16 {
        self.graphics_placements
    }
}

pub fn decode_terminal_render_row_window(
    bytes: &[u8],
) -> Result<TerminalRenderRowWindow<'_>, ProtocolError> {
    let header = decode_header(bytes)?;
    if !matches!(header.detail, Detail::Render) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidField,
            "OBRW text detail is outside the render decoder",
        ));
    }
    // The shared header proved both section lengths and their exact complete body.
    let split = TERMINAL_ROW_WINDOW_HEADER_BYTES + header.projection_length as usize;
    let projection =
        decode_terminal_projection_buffer(&bytes[TERMINAL_ROW_WINDOW_HEADER_BYTES..split])?;
    let (graphics_images, graphics_placements) = graphics::validate_snapshot(&bytes[split..])?;
    let route = header.route()?;
    let nested = projection.route();
    let geometry = projection.geometry();
    if nested.base_revision.is_some()
        || nested.attachment_id != route.attachment_id
        || nested.host_epoch != route.host_epoch
        || nested.sequence != route.sequence
        || nested.revision != route.revision
        || nested.generation != route.generation
        || geometry.columns != route.columns
        || geometry.rows != route.row_count
        || projection.scrollback_rows() != route.start_row
        || header.entry_count != u32::from(geometry.columns) * u32::from(geometry.rows)
    {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidField,
            "OBRW render snapshot disagrees with outer window",
        ));
    }
    Ok(TerminalRenderRowWindow {
        route,
        projection,
        ignored_optional_flags: header.ignored_optional_flags,
        graphics_images,
        graphics_placements,
    })
}
