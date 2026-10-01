//! Incremental bounded reference header/raw-body grammar. An incomplete frame is never submitted.
use base64::Engine;
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_STREAM_BYTES, PaneGraphicsDamage, PaneGraphicsFormat, PaneGraphicsFrame,
    PaneGraphicsPlacement, PaneGraphicsStreamImage, WorkbenchRevision, graphics_rgba_bytes,
};
use serde::Deserialize;
const MAXIMUM_HEADER: usize = 8192;
fn zero() -> WorkbenchRevision {
    WorkbenchRevision::new(0)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header {
    format: PaneGraphicsFormat,
    #[serde(alias = "image_width", alias = "imageWidth")]
    width: u32,
    #[serde(alias = "image_height", alias = "imageHeight")]
    height: u32,
    #[serde(default, alias = "data_length")]
    data_length: Option<usize>,
    #[serde(default)]
    file: Option<File>,
    #[serde(default)]
    sequence: Option<WorkbenchRevision>,
    #[serde(default = "zero")]
    revision: WorkbenchRevision,
    #[serde(default)]
    placement: Placement,
    #[serde(default)]
    damage: Vec<PaneGraphicsDamage>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    #[serde(default, alias = "viewport_col", alias = "viewportCol")]
    viewport_column: i32,
    #[serde(default, alias = "viewport_row")]
    viewport_row: i32,
    #[serde(default, alias = "grid_cols", alias = "gridCols")]
    grid_columns: u32,
    #[serde(default, alias = "grid_rows")]
    grid_rows: u32,
}
impl From<Placement> for PaneGraphicsPlacement {
    fn from(p: Placement) -> Self {
        Self {
            viewport_column: p.viewport_column,
            viewport_row: p.viewport_row,
            grid_columns: p.grid_columns,
            grid_rows: p.grid_rows,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    path: String,
}
pub(super) struct Frame {
    pub sequence: WorkbenchRevision,
    pub revision: WorkbenchRevision,
    pub image: PaneGraphicsStreamImage,
    pub damage: Vec<PaneGraphicsDamage>,
}
struct Body {
    header: Header,
    sequence: WorkbenchRevision,
    bytes: Vec<u8>,
    length: usize,
}
#[derive(Default)]
pub(super) struct Parser {
    pending: Vec<u8>,
    header: Vec<u8>,
    body: Option<Body>,
    accepted: u64,
    first: bool,
}
impl Parser {
    pub(super) fn new() -> Self {
        Self {
            first: true,
            ..Self::default()
        }
    }
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        // Consumers call next until pending is drained before polling another fixed-size chunk.
        if !self.pending.is_empty() || bytes.len() > 8192 {
            return Err("invalid-frame");
        }
        self.pending.extend_from_slice(bytes);
        Ok(())
    }
    pub(super) fn accept(&mut self, sequence: WorkbenchRevision) {
        self.accepted = sequence.value();
    }
    pub(super) fn in_progress(&self) -> bool {
        self.body.is_some() || !self.pending.is_empty() || !self.header.is_empty()
    }
    pub(super) fn finish(&mut self) -> Result<(), &'static str> {
        if self.body.is_some() || !self.pending.is_empty() || !self.header.is_empty() {
            Err("truncated-frame")
        } else {
            Ok(())
        }
    }
    pub(super) fn next(&mut self) -> Result<Option<Frame>, &'static str> {
        loop {
            if let Some(mut body) = self.body.take() {
                let take = (body.length - body.bytes.len()).min(self.pending.len());
                body.bytes.extend(self.pending.drain(..take));
                if body.bytes.len() != body.length {
                    self.body = Some(body);
                    return Ok(None);
                }
                let image = PaneGraphicsFrame {
                    format: body.header.format,
                    width: body.header.width,
                    height: body.header.height,
                    placement: body.header.placement.into(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(body.bytes),
                };
                image
                    .validate(MAXIMUM_GRAPHICS_STREAM_BYTES)
                    .map_err(|_| "invalid-frame")?;
                return Ok(Some(Frame {
                    sequence: body.sequence,
                    revision: body.header.revision,
                    image: PaneGraphicsStreamImage::Inline { image },
                    damage: body.header.damage,
                }));
            }
            let newline = self.pending.iter().position(|byte| *byte == b'\n');
            let count = newline.unwrap_or(self.pending.len());
            if self.header.len() + count > MAXIMUM_HEADER {
                return Err("header-too-large");
            }
            self.header.extend(self.pending.drain(..count));
            if newline.is_none() {
                return Ok(None);
            }
            self.pending.remove(0);
            let mut header = std::mem::take(&mut self.header);
            if self.first {
                self.first = false;
                if header.starts_with(&[0xef, 0xbb, 0xbf]) {
                    header.drain(..3);
                }
            }
            if header.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let mut header: Header =
                serde_json::from_slice(&header).map_err(|_| "invalid-frame")?;
            let sequence = header
                .sequence
                .map_or_else(
                    || self.accepted.checked_add(1).map(WorkbenchRevision::new),
                    Some,
                )
                .ok_or("invalid-sequence")?;
            if sequence.value() == 0 || sequence.value() <= self.accepted {
                return Err("invalid-sequence");
            }
            let rgba =
                graphics_rgba_bytes(header.width, header.height).map_err(|_| "invalid-frame")?;
            if header.damage.len() > 64
                || header.damage.iter().any(|d| {
                    d.width == 0
                        || d.height == 0
                        || u64::from(d.x) + u64::from(d.width) > u64::from(header.width)
                        || u64::from(d.y) + u64::from(d.height) > u64::from(header.height)
                })
            {
                return Err("invalid-frame");
            }
            if let Some(file) = header.file.take() {
                if header.data_length.is_some()
                    || !matches!(
                        header.format,
                        PaneGraphicsFormat::Rgba | PaneGraphicsFormat::Bgra
                    )
                    || rgba > MAXIMUM_GRAPHICS_STREAM_BYTES
                    || file.path.len() > 4096
                {
                    return Err("invalid-frame");
                }
                return Ok(Some(Frame {
                    sequence,
                    revision: header.revision,
                    image: PaneGraphicsStreamImage::File {
                        format: header.format,
                        width: header.width,
                        height: header.height,
                        path: file.path,
                        placement: header.placement.into(),
                    },
                    damage: header.damage,
                }));
            }
            let length = header.data_length.ok_or("invalid-frame")?;
            if length == 0 || length > MAXIMUM_GRAPHICS_STREAM_BYTES {
                return Err("image-too-large");
            }
            let valid = match header.format {
                PaneGraphicsFormat::Png => length >= 33,
                PaneGraphicsFormat::Rgb => length == rgba / 4 * 3,
                PaneGraphicsFormat::Rgba | PaneGraphicsFormat::Bgra => length == rgba,
            };
            if !valid {
                return Err("invalid-frame");
            }
            self.body = Some(Body {
                header,
                sequence,
                bytes: Vec::with_capacity(length),
                length,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_binary_frames_pin_full_sequences_and_reject_truncation_without_partial_image() {
        let mut parser = Parser::new();
        let mut bytes = b"{\"format\":\"rgba\",\"image_width\":1,\"image_height\":1,\"data_length\":4,\"sequence\":\"18446744073709551614\",\"revision\":\"18446744073709551615\",\"placement\":{\"grid_cols\":4294967295,\"grid_rows\":4294967295}}\r\n".to_vec();
        bytes.extend([0, 10, 255, 128]);
        let mut frames = Vec::new();
        for byte in bytes {
            parser.feed(&[byte]).unwrap();
            if let Some(frame) = parser.next().unwrap() {
                parser.accept(frame.sequence);
                frames.push(frame);
            }
        }
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].sequence.value(), u64::MAX - 1);
        assert_eq!(frames[0].revision.value(), u64::MAX);
        let PaneGraphicsStreamImage::Inline { image } = &frames[0].image else {
            panic!("inline image");
        };
        assert_eq!(image.placement.grid_columns, u32::MAX);
        assert_eq!(image.placement.grid_rows, u32::MAX);
        parser.finish().unwrap();
        parser
            .feed(b"{\"format\":\"rgba\",\"width\":1,\"height\":1,\"dataLength\":4}\n\0")
            .unwrap();
        assert!(parser.next().unwrap().is_none());
        assert_eq!(parser.finish(), Err("truncated-frame"));
    }
    #[test]
    fn header_bounds_and_sizes_are_checked_before_body_allocation() {
        for raw in [
            r#"{"format":"rgba","width":1,"height":1,"dataLength":5}"#,
            r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"format":"bgra"}"#,
            r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"unknown":0}"#,
            r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"sequence":"0"}"#,
            r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"sequence":9007199254740992}"#,
            r#"{"format":"png","width":4096,"height":4096,"dataLength":16777217}"#,
            r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"damage":[{"x":1,"y":0,"width":1,"height":1}]}"#,
        ] {
            let mut parser = Parser::new();
            parser.feed(format!("{raw}\n").as_bytes()).unwrap();
            assert!(parser.next().is_err(), "{raw}");
            assert!(parser.body.is_none());
        }
        let mut parser = Parser::new();
        parser.feed(&vec![b'x'; MAXIMUM_HEADER]).unwrap();
        assert!(parser.next().unwrap().is_none());
        parser.feed(b"x\n").unwrap();
        assert_eq!(parser.next().err(), Some("header-too-large"));
    }
}
