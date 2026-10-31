//! Deterministic OBTF 0.1.0 text frames for the benchmarks, wrapped in transport envelopes. The
//! encoder mirrors the Terminal Host's table shape (one deduplicated string table, a small style
//! table, full-width rows) and relies on the real decoder to reject any drift from the contract.

use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, Envelope, HostEpoch, MessageRole, PayloadEncoding,
    TRANSPORT_ENVELOPE_HEADER_BYTES, encode_transport_envelope,
};
use orchbee_workbench_client::application::terminal_device::WindowDimensions;
use ratatui::layout::Rect;
use std::collections::HashMap;

const HEADER_BYTES: usize = 160;
const CELL_PIXEL_WIDTH: u16 = 9;
const CELL_PIXEL_HEIGHT: u16 = 18;
const DEFAULT_COLOR: u32 = 0x100_0000;
const NO_LINK: u32 = u32::MAX;
const SEQUENCE: usize = TRANSPORT_ENVELOPE_HEADER_BYTES + 48;
const BASE_REVISION: usize = TRANSPORT_ENVELOPE_HEADER_BYTES + 56;
const REVISION: usize = TRANSPORT_ENVELOPE_HEADER_BYTES + 64;
const SNAPSHOT_KIND: u16 = 41;
const DELTA_KIND: u16 = 42;

const ASCII: &[u8] =
    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ._-/:=()[]{}<>,;'\"!?#$%&*+|~`^@";
const WIDE: [&str; 8] = ["界", "终", "端", "代", "理", "工", "作", "区"];
/// Default text plus seven SGR combinations: colors, bold, italic, underline, and inverse.
const STYLES: [(u32, u32, u16, u8); 8] = [
    (DEFAULT_COLOR, DEFAULT_COLOR, 0, 0),
    (0x00d7_5f5f, DEFAULT_COLOR, 0, 0),
    (0x005f_d75f, DEFAULT_COLOR, 1, 0),
    (0x005f_87d7, DEFAULT_COLOR, 0, 0),
    (0x00d7_af5f, DEFAULT_COLOR, 2, 0),
    (DEFAULT_COLOR, DEFAULT_COLOR, 0, 1),
    (0x00af_afaf, 0x0030_3030, 0, 0),
    (DEFAULT_COLOR, DEFAULT_COLOR, 1, 0),
];

pub fn attachment() -> AttachmentId {
    AttachmentId::from_bytes([0x22; 16]).expect("nonzero benchmark attachment")
}

pub fn epoch() -> HostEpoch {
    HostEpoch::from_bytes([0x11; 16]).expect("nonzero benchmark Host epoch")
}

#[derive(Clone, Copy)]
pub struct Size {
    pub columns: u16,
    pub rows: u16,
}

impl Size {
    pub fn label(self) -> String {
        format!("{}x{}", self.columns, self.rows)
    }

    pub fn geometry(self) -> orchbee_client_server_protocol::Geometry {
        orchbee_client_server_protocol::Geometry::new(
            self.columns,
            self.rows,
            CELL_PIXEL_WIDTH,
            CELL_PIXEL_HEIGHT,
        )
        .expect("benchmark geometry within protocol bounds")
    }

    /// The host window this many cells fill, at the frames' cell pixel size.
    pub const fn window(self) -> WindowDimensions {
        WindowDimensions {
            columns: self.columns,
            rows: self.rows,
            pixel_width: self.columns * CELL_PIXEL_WIDTH,
            pixel_height: self.rows * CELL_PIXEL_HEIGHT,
        }
    }

    pub const fn area(self) -> Rect {
        Rect::new(0, 0, self.columns, self.rows)
    }
}

struct Cell {
    text: &'static str,
    style: u16,
    width: u8,
}

/// Shell or Agent output: rows filled to a varying length with ASCII, a few double-width CJK
/// graphemes, and SGR style runs; the remainder of each row is blank.
fn row(columns: u16, seed: u64) -> Vec<Cell> {
    let mut random = Random(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let columns = usize::from(columns);
    let filled = columns / 4 + random.below(columns - columns / 4 + 1);
    let mut cells = Vec::with_capacity(columns);
    let mut style = 0;
    while cells.len() < columns {
        if cells.len() % 12 == 0 {
            style = if random.below(10) < 7 {
                0
            } else {
                1 + random.below(STYLES.len() - 1) as u16
            };
        }
        if cells.len() >= filled {
            cells.push(Cell {
                text: "",
                style: 0,
                width: 0,
            });
        } else if cells.len() + 1 < columns && random.below(100) < 3 {
            cells.push(Cell {
                text: WIDE[random.below(WIDE.len())],
                style,
                width: 1,
            });
            cells.push(Cell {
                text: "",
                style,
                width: 2,
            });
        } else {
            let index = random.below(ASCII.len());
            let text = std::str::from_utf8(&ASCII[index..=index]).expect("ASCII grapheme");
            cells.push(Cell {
                text,
                style,
                width: 0,
            });
        }
    }
    cells
}

/// One encoded frame record whose route counters the benchmark may advance in place.
pub struct Record {
    pub bytes: Vec<u8>,
}

impl Record {
    /// A full snapshot of `size` at `sequence`/`revision`, with row content selected by `salt`.
    pub fn snapshot(size: Size, sequence: u64, revision: u64, salt: u64) -> Self {
        let indexes: Vec<u16> = (0..size.rows).collect();
        Self::encode(size, &indexes, None, sequence, revision, salt)
    }

    /// A delta that replaces `dirty` evenly spaced rows of a frame at `base`.
    pub fn delta(size: Size, dirty: u16, sequence: u64, base: u64, salt: u64) -> Self {
        let dirty = dirty.clamp(1, size.rows);
        let indexes: Vec<u16> = (0..dirty)
            .map(|ordinal| {
                let index = u32::from(ordinal) * u32::from(size.rows) / u32::from(dirty);
                u16::try_from(index).expect("row index below the row count")
            })
            .collect();
        Self::encode(size, &indexes, Some(base), sequence, base + 1, salt)
    }

    pub fn payload(&self) -> &[u8] {
        &self.bytes[TRANSPORT_ENVELOPE_HEADER_BYTES..]
    }

    /// Re-address this record as the next one in a live stream; the decoded content is unchanged.
    pub fn advance(&mut self, sequence: u64, base: Option<u64>, revision: u64) {
        self.bytes[SEQUENCE..SEQUENCE + 8].copy_from_slice(&sequence.to_le_bytes());
        self.bytes[BASE_REVISION..BASE_REVISION + 8]
            .copy_from_slice(&base.unwrap_or(u64::MAX).to_le_bytes());
        self.bytes[REVISION..REVISION + 8].copy_from_slice(&revision.to_le_bytes());
    }

    fn encode(
        size: Size,
        indexes: &[u16],
        base: Option<u64>,
        sequence: u64,
        revision: u64,
        salt: u64,
    ) -> Self {
        let rows: Vec<(u16, Vec<Cell>)> = indexes
            .iter()
            .map(|index| {
                let seed = salt.wrapping_mul(1_000_003).wrapping_add(u64::from(*index));
                (*index, row(size.columns, seed))
            })
            .collect();
        let mut strings: Vec<&str> = vec!["Agent · build", "C:\\work\\orchbee"];
        let mut string_index: HashMap<&str, u32> = HashMap::new();
        for (_, cells) in &rows {
            for cell in cells {
                string_index.entry(cell.text).or_insert_with(|| {
                    strings.push(cell.text);
                    u32::try_from(strings.len() - 1).expect("bounded string table")
                });
            }
        }

        let mut out = Vec::new();
        out.extend_from_slice(b"OBTF");
        out.extend_from_slice(&[0, 1, if base.is_some() { 2 } else { 1 }, 0]);
        out.extend_from_slice(&0_u32.to_le_bytes()); // Total length, patched below.
        put16(&mut out, u16::try_from(HEADER_BYTES).expect("header size"));
        put16(&mut out, 0);
        out.extend_from_slice(&[0x11; 16]);
        out.extend_from_slice(&[0x22; 16]);
        put64(&mut out, sequence);
        put64(&mut out, base.unwrap_or(u64::MAX));
        put64(&mut out, revision);
        put64(&mut out, 1); // Generation.
        put64(&mut out, 2_000); // Retained scrollback rows.
        put16(&mut out, size.columns);
        put16(&mut out, size.rows);
        put16(&mut out, 2); // Cursor column.
        put16(&mut out, size.rows - 1); // Cursor row.
        out.extend_from_slice(&[1, 1, 1, 0]); // Block cursor, visible, primary screen, reserved.
        put32(&mut out, 0); // Captured interaction.
        put32(&mut out, 0x00d0_d0d0);
        put32(&mut out, 0x0010_1010);
        put32(&mut out, DEFAULT_COLOR);
        put32(&mut out, 0); // Title string.
        put32(&mut out, 1); // Current-directory string.
        put32(
            &mut out,
            u32::try_from(strings.len()).expect("bounded strings"),
        );
        put16(
            &mut out,
            u16::try_from(STYLES.len()).expect("bounded styles"),
        );
        let encoded_rows = u16::try_from(rows.len()).expect("bounded rows");
        put16(&mut out, encoded_rows);
        put16(&mut out, 0); // Extensions.
        put16(&mut out, 0); // Reserved tables.
        put16(&mut out, CELL_PIXEL_WIDTH);
        put16(&mut out, CELL_PIXEL_HEIGHT);
        put32(&mut out, u32::from(encoded_rows) * u32::from(size.columns));
        out.push(0); // Patch version.
        put64(&mut out, 1); // Theme revision.
        out.extend_from_slice(&[0; 7]);
        assert_eq!(out.len(), HEADER_BYTES);

        for text in &strings {
            put32(&mut out, u32::try_from(text.len()).expect("short grapheme"));
            out.extend_from_slice(text.as_bytes());
        }
        for (foreground, background, flags, underline) in STYLES {
            put32(&mut out, foreground);
            put32(&mut out, background);
            put32(&mut out, DEFAULT_COLOR);
            put32(&mut out, NO_LINK);
            put16(&mut out, flags);
            out.extend_from_slice(&[underline, 0]);
        }
        for (index, cells) in &rows {
            put16(&mut out, *index);
            out.extend_from_slice(&[0, 0]); // Row flags, prompt class.
            put16(&mut out, size.columns);
            put16(&mut out, 0);
            for cell in cells {
                put32(&mut out, string_index[cell.text]);
                put16(&mut out, cell.style);
                out.extend_from_slice(&[cell.width, 0]);
            }
        }
        let length = u32::try_from(out.len()).expect("frame below the record limit");
        out[8..12].copy_from_slice(&length.to_le_bytes());

        let bytes = encode_transport_envelope(&Envelope {
            correlation_id: None,
            host_epoch: Some(epoch()),
            kind: if base.is_some() {
                DELTA_KIND
            } else {
                SNAPSHOT_KIND
            },
            optional_message: false,
            payload: &out,
            payload_encoding: PayloadEncoding::TerminalFrame,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Event,
        })
        .expect("benchmark frame fits one transport record");
        Self { bytes }
    }
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// xorshift64: reproducible content without a dependency or platform entropy.
struct Random(u64);

impl Random {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % bound as u64).expect("bounded below usize")
    }
}
