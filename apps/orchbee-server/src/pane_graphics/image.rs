//! Complete RGBA staging before layer mutation; PNG decoding is supplied only by the admitted Host adapter.
use base64::Engine;
use orchbee_workbench_protocol::{
    PaneGraphicsFailure as Failure, PaneGraphicsFormat, PaneGraphicsFrame, PaneGraphicsPlacement,
    graphics_rgba_bytes,
};
use std::sync::Arc;
/// An immutable, fully dimension-validated image; construction cannot publish a partially decoded layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedGraphicsImage {
    width: u32,
    height: u32,
    placement: PaneGraphicsPlacement,
    rgba: Arc<[u8]>,
}
impl DecodedGraphicsImage {
    /// Consumes a complete private RGBA/BGRA file copy without encoding it again; malformed input stays uncommitted.
    pub fn from_file_pixels(
        format: PaneGraphicsFormat,
        width: u32,
        height: u32,
        placement: PaneGraphicsPlacement,
        mut bytes: Vec<u8>,
    ) -> Result<Self, Failure> {
        let expected = graphics_rgba_bytes(width, height).map_err(|_| Failure::InvalidImage)?;
        if bytes.len() > orchbee_workbench_protocol::MAXIMUM_GRAPHICS_STREAM_BYTES {
            return Err(Failure::ImageTooLarge);
        }
        if bytes.len() != expected
            || !matches!(format, PaneGraphicsFormat::Rgba | PaneGraphicsFormat::Bgra)
        {
            return Err(Failure::InvalidImage);
        }
        if format == PaneGraphicsFormat::Bgra {
            for pixel in bytes.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        Ok(Self {
            width,
            height,
            placement,
            rgba: Arc::from(bytes),
        })
    }
    /// Normalizes raw formats, or consumes a complete correlated Host PNG decode, within exact image bounds.
    pub fn decode(
        image: &PaneGraphicsFrame,
        maximum_input: usize,
        png: impl FnOnce(&[u8], u32, u32) -> Result<Vec<u8>, Failure>,
    ) -> Result<Self, Failure> {
        if image.data_base64.len() > maximum_input.div_ceil(3).saturating_mul(4) {
            return Err(Failure::ImageTooLarge);
        }
        image
            .validate(maximum_input)
            .map_err(|_| Failure::InvalidImage)?;
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(&image.data_base64)
            .map_err(|_| Failure::InvalidImage)?;
        let expected =
            graphics_rgba_bytes(image.width, image.height).map_err(|_| Failure::InvalidImage)?;
        let rgba = match image.format {
            PaneGraphicsFormat::Png => {
                if bytes.len() < 33
                    || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
                    || bytes[8..16] != [0, 0, 0, 13, b'I', b'H', b'D', b'R']
                    || bytes[16..20] != image.width.to_be_bytes()
                    || bytes[20..24] != image.height.to_be_bytes()
                {
                    return Err(Failure::InvalidImage);
                }
                png(&bytes, image.width, image.height)?
            }
            PaneGraphicsFormat::Rgba => bytes,
            PaneGraphicsFormat::Bgra => {
                for pixel in bytes.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                bytes
            }
            PaneGraphicsFormat::Rgb => {
                let mut rgba = Vec::new();
                rgba.try_reserve_exact(expected)
                    .map_err(|_| Failure::MemoryLimit)?;
                for pixel in bytes.chunks_exact(3) {
                    rgba.extend_from_slice(pixel);
                    rgba.push(255);
                }
                rgba
            }
        };
        if rgba.len() != expected {
            return Err(Failure::InvalidImage);
        }
        Ok(Self {
            width: image.width,
            height: image.height,
            placement: image.placement,
            rgba: Arc::from(rgba),
        })
    }
    /// Returns exact declared dimensions, not renderer-estimated geometry.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    /// Returns exact declared dimensions, not renderer-estimated geometry.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    /// Returns the validated requested placement; the Client clips it to its real Panel surface.
    #[must_use]
    pub const fn placement(&self) -> PaneGraphicsPlacement {
        self.placement
    }
    /// Borrowed complete RGBA pixels; no caller receives mutable ownership.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.rgba
    }
}
