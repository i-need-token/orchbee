//! Incremental bounded reference header/raw-body grammar. An incomplete frame is never submitted.
use base64::Engine;
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_STREAM_BYTES, PaneGraphicsDamage, PaneGraphicsFormat, PaneGraphicsFrame,
    PaneGraphicsPlacement, PaneGraphicsStreamImage, WorkbenchRevision, graphics_rgba_bytes,
};
use serde::Deserialize;
const MAXIMUM_HEADER: usize = 64 * 1024;
fn zero() -> WorkbenchRevision {
    WorkbenchRevision::new(0)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header {
    format: PaneGraphicsFormat,
    width: u32,
    height: u32,
    #[serde(default)]
    data_length: Option<usize>,
    #[serde(default)]
    file: Option<File>,
    #[serde(default, deserialize_with = "deserialize_sequence")]
    sequence: Option<WorkbenchRevision>,
    #[serde(default = "zero", deserialize_with = "deserialize_revision")]
    revision: WorkbenchRevision,
    #[serde(default)]
    placement: Placement,
    #[serde(default, deserialize_with = "deserialize_damage")]
    damage: Vec<PaneGraphicsDamage>,
    // Upstream browser headers carry this ignored hint; only Client observations select transports.
    #[serde(default, rename = "transport")]
    _transport: Option<TransportHint>,
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TransportHint {
    DirectKitty,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum DamageInput {
    One(PaneGraphicsDamage),
    Many(Vec<PaneGraphicsDamage>),
}
#[derive(Deserialize)]
#[serde(untagged)]
enum RevisionInput {
    Decimal(WorkbenchRevision),
    Number(u64),
}
impl RevisionInput {
    fn canonical(self) -> Result<WorkbenchRevision, &'static str> {
        match self {
            Self::Decimal(revision) => Ok(revision),
            Self::Number(number) if number <= 9_007_199_254_740_991 => {
                Ok(WorkbenchRevision::new(number))
            }
            Self::Number(_) => Err("unsafe JSON integer revision"),
        }
    }
}
fn deserialize_revision<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<WorkbenchRevision, D::Error> {
    RevisionInput::deserialize(decoder)?
        .canonical()
        .map_err(serde::de::Error::custom)
}
fn deserialize_sequence<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<WorkbenchRevision>, D::Error> {
    Option::<RevisionInput>::deserialize(decoder)?
        .map(RevisionInput::canonical)
        .transpose()
        .map_err(serde::de::Error::custom)
}
fn deserialize_damage<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<PaneGraphicsDamage>, D::Error> {
    Ok(match DamageInput::deserialize(decoder)? {
        DamageInput::One(rectangle) => vec![rectangle],
        DamageInput::Many(rectangles) => rectangles,
    })
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Placement {
    #[serde(default)]
    viewport_column: i32,
    #[serde(default)]
    viewport_row: i32,
    #[serde(default)]
    grid_columns: u32,
    #[serde(default)]
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
mod tests;
