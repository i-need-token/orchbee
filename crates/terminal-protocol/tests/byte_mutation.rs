//! Byte mutation of the canonical binary corpus. Each decoder must reject or accept without
//! panicking, and a codec that owns an encoder must re-encode every accepted value to the exact
//! mutated bytes, so malformed input is never silently normalized. Cases derive from a fixed seed;
//! `ORCHBEE_PROTOCOL_MUTATION_SEED` explores another seed, and a failure prints its minimal edits.

use std::panic::{AssertUnwindSafe, catch_unwind};

use orchbee_terminal_protocol::{
    Envelope, MessageRole, PayloadEncoding, decode_input_message, decode_selection_export_chunk,
    decode_terminal_projection_buffer, decode_terminal_render_row_window,
    decode_terminal_text_frame, decode_terminal_text_row_window, decode_transport_envelope,
    encode_input_message, encode_selection_export_chunk, encode_transport_envelope,
};

const DEFAULT_SEED: u64 = 0x0b7d_0238_5eed_0001;
const RANDOM_CASES_PER_FIXTURE: u64 = 256;
const BOUNDARY_BYTES: [u8; 6] = [0x00, 0x01, 0x7f, 0x80, 0xfe, 0xff];

#[derive(Clone, Copy)]
enum Codec {
    Envelope,
    TextFrame,
    ProjectionBuffer,
    TextRowWindow,
    RenderRowWindow,
    SelectionExportChunk,
    Input(u16),
}

struct Fixture {
    name: &'static str,
    bytes: &'static [u8],
    codec: Codec,
    /// `(offset, base)` pairs whose little-endian u32 must equal `length - base`.
    length_fields: &'static [(usize, usize)],
}

macro_rules! fixture {
    ($name:literal, $codec:expr, $length_fields:expr) => {
        Fixture {
            name: $name,
            bytes: include_bytes!(concat!("../../../schemas/terminal/v1/fixtures/", $name)),
            codec: $codec,
            length_fields: $length_fields,
        }
    };
}

const ROW_WINDOW_LENGTHS: &[(usize, usize)] = &[(8, 0), (96, 112)];

const CORPUS: &[Fixture] = &[
    // The envelope carries the terminal frame, so length repair also keeps the nested frame whole.
    fixture!(
        "transport-envelope.bin",
        Codec::Envelope,
        &[(8, 0), (12, 64), (72, 64)]
    ),
    fixture!("terminal-frame.bin", Codec::TextFrame, &[(8, 0)]),
    fixture!("ghostty-surface.bin", Codec::TextFrame, &[(8, 0)]),
    fixture!("ghostty-alternate-surface.bin", Codec::TextFrame, &[(8, 0)]),
    fixture!(
        "terminal-projection-buffer.bin",
        Codec::ProjectionBuffer,
        &[]
    ),
    fixture!(
        "terminal-row-window.bin",
        Codec::TextRowWindow,
        ROW_WINDOW_LENGTHS
    ),
    fixture!(
        "ghostty-render-row-window.bin",
        Codec::RenderRowWindow,
        ROW_WINDOW_LENGTHS
    ),
    fixture!(
        "selection-export-chunk.bin",
        Codec::SelectionExportChunk,
        &[]
    ),
    fixture!("write-input.bin", Codec::Input(30), &[]),
    fixture!("key-input.bin", Codec::Input(34), &[]),
    fixture!("text-input.bin", Codec::Input(35), &[]),
    fixture!("mouse-input.bin", Codec::Input(36), &[]),
    fixture!("focus-input.bin", Codec::Input(37), &[]),
    fixture!("paste-input.bin", Codec::Input(38), &[]),
    fixture!("paste-auto-input.bin", Codec::Input(39), &[]),
    fixture!("prompt-line-input.bin", Codec::Input(84), &[]),
];

#[derive(Clone, Copy, Debug)]
enum Edit {
    Set(usize, u8),
    Xor(usize, u8),
    Insert(usize, u8),
    Delete(usize),
    Truncate(usize),
    RepairLengths,
}

#[test]
fn every_single_byte_mutation_is_rejected_or_canonical_without_panicking() {
    for fixture in CORPUS {
        assert_eq!(
            judge(fixture.codec, fixture.bytes),
            Ok(true),
            "{} must decode before mutation",
            fixture.name
        );
        for position in 0..fixture.bytes.len() {
            let flips = (0..8).map(|bit| Edit::Xor(position, 1 << bit));
            let values = BOUNDARY_BYTES
                .iter()
                .map(|value| Edit::Set(position, *value));
            for edit in flips.chain(values) {
                run_case(fixture, &[edit], "single-byte sweep");
            }
        }
    }
}

#[test]
fn seeded_multi_edit_mutations_are_rejected_or_canonical_without_panicking() {
    let seed = seed();
    for (index, fixture) in CORPUS.iter().enumerate() {
        for case in 0..RANDOM_CASES_PER_FIXTURE {
            let mut random = SplitMix64(seed ^ ((index as u64) << 32) ^ case);
            let edits = random_edits(&mut random, fixture);
            run_case(
                fixture,
                &edits,
                &format!("seed {seed:#018x}, fixture {index}, case {case}"),
            );
        }
    }
}

fn judge(codec: Codec, bytes: &[u8]) -> Result<bool, String> {
    match codec {
        Codec::Envelope => judge_envelope(bytes),
        Codec::TextFrame => Ok(decode_terminal_text_frame(bytes).is_ok()),
        Codec::ProjectionBuffer => Ok(decode_terminal_projection_buffer(bytes).is_ok()),
        Codec::TextRowWindow => Ok(decode_terminal_text_row_window(bytes).is_ok()),
        Codec::RenderRowWindow => Ok(decode_terminal_render_row_window(bytes).is_ok()),
        Codec::SelectionExportChunk => {
            let Ok(chunk) = decode_selection_export_chunk(65, MessageRole::Event, bytes) else {
                return Ok(false);
            };
            let encoded = encode_selection_export_chunk(&chunk)
                .map_err(|error| format!("accepted chunk cannot re-encode: {error:?}"))?;
            canonical(encoded.payload == bytes, "selection-export chunk")
        }
        Codec::Input(kind) => {
            let Ok(input) = decode_input_message(kind, MessageRole::Request, bytes) else {
                return Ok(false);
            };
            let encoded = encode_input_message(&input)
                .map_err(|error| format!("accepted input cannot re-encode: {error:?}"))?;
            canonical(
                encoded.kind == kind && encoded.payload == bytes,
                "input message",
            )
        }
    }
}

fn judge_envelope(bytes: &[u8]) -> Result<bool, String> {
    let Ok(decoded) = decode_transport_envelope(bytes) else {
        return Ok(false);
    };
    let encoded = encode_transport_envelope(&Envelope {
        correlation_id: decoded.correlation_id,
        host_epoch: decoded.host_epoch,
        kind: decoded.kind,
        optional_message: decoded.optional_message,
        payload: decoded.payload,
        payload_encoding: decoded.payload_encoding,
        protocol: decoded.protocol,
        role: decoded.role,
    })
    .map_err(|error| format!("accepted envelope cannot re-encode: {error:?}"))?;
    // Unknown optional flags are reported and ignored; every other header bit is retained.
    let mut expected = bytes.to_vec();
    if let Some(flags) = expected.get_mut(6) {
        *flags &= !decoded.ignored_optional_flags;
    }
    canonical(encoded == expected, "transport envelope")?;
    match decoded.payload_encoding {
        PayloadEncoding::TerminalFrame => {
            let _frame = decode_terminal_text_frame(decoded.payload);
        }
        PayloadEncoding::RowWindow => {
            let _text = decode_terminal_text_row_window(decoded.payload);
            let _render = decode_terminal_render_row_window(decoded.payload);
        }
        PayloadEncoding::ControlJson
        | PayloadEncoding::RawBytes
        | PayloadEncoding::GraphicsImage => {}
    }
    Ok(true)
}

fn canonical(matches: bool, codec: &str) -> Result<bool, String> {
    if matches {
        Ok(true)
    } else {
        Err(format!("accepted {codec} does not re-encode to its input"))
    }
}

fn run_case(fixture: &Fixture, edits: &[Edit], origin: &str) {
    if let Err(failure) = check(fixture, edits) {
        let (edits, failure) = shrink(fixture, edits.to_vec(), failure);
        let bytes = apply(fixture, &edits);
        panic!(
            "{} ({origin}): {failure}\nminimal edits: {edits:?}\nmutated length {}: {}",
            fixture.name,
            bytes.len(),
            hex_preview(&bytes)
        );
    }
}

fn check(fixture: &Fixture, edits: &[Edit]) -> Result<(), String> {
    let bytes = apply(fixture, edits);
    match catch_unwind(AssertUnwindSafe(|| judge(fixture.codec, &bytes))) {
        Ok(result) => result.map(|_accepted| ()),
        Err(panic) => Err(format!("decoder panicked: {}", panic_text(&*panic))),
    }
}

/// Greedily removes edits while the same case still fails, yielding a locally minimal sample.
fn shrink(fixture: &Fixture, mut edits: Vec<Edit>, mut failure: String) -> (Vec<Edit>, String) {
    let mut index = 0;
    while index < edits.len() {
        let mut candidate = edits.clone();
        candidate.remove(index);
        match check(fixture, &candidate) {
            Err(message) => {
                edits = candidate;
                failure = message;
            }
            Ok(()) => index += 1,
        }
    }
    (edits, failure)
}

fn apply(fixture: &Fixture, edits: &[Edit]) -> Vec<u8> {
    let mut bytes = fixture.bytes.to_vec();
    for edit in edits {
        let length = bytes.len();
        match *edit {
            Edit::Set(position, value) => {
                if let Some(byte) = bytes.get_mut(position % length.max(1)) {
                    *byte = value;
                }
            }
            Edit::Xor(position, mask) => {
                if let Some(byte) = bytes.get_mut(position % length.max(1)) {
                    *byte ^= mask;
                }
            }
            Edit::Insert(position, value) => bytes.insert(position % (length + 1), value),
            Edit::Delete(position) => {
                if length > 0 {
                    bytes.remove(position % length);
                }
            }
            Edit::Truncate(retained) => bytes.truncate(retained % (length + 1)),
            Edit::RepairLengths => {
                for (offset, base) in fixture.length_fields {
                    let value = u32::try_from(length.saturating_sub(*base)).unwrap_or(u32::MAX);
                    if let Some(field) = bytes.get_mut(*offset..offset + 4) {
                        field.copy_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
    }
    bytes
}

fn random_edits(random: &mut SplitMix64, fixture: &Fixture) -> Vec<Edit> {
    let length = fixture.bytes.len();
    let mut edits = (0..=random.below(4))
        .map(|_| {
            let position = random.below(length);
            match random.below(9) {
                0..=2 => Edit::Set(position, random.byte()),
                3..=5 => Edit::Xor(position, 1 << random.below(8)),
                6 => Edit::Insert(position, random.byte()),
                7 => Edit::Delete(position),
                _ => Edit::Truncate(random.below(length + 1)),
            }
        })
        .collect::<Vec<_>>();
    if !fixture.length_fields.is_empty() && random.below(2) == 0 {
        edits.push(Edit::RepairLengths);
    }
    edits
}

fn seed() -> u64 {
    std::env::var("ORCHBEE_PROTOCOL_MUTATION_SEED")
        .ok()
        .and_then(|value| {
            let value = value.trim();
            value.strip_prefix("0x").map_or_else(
                || value.parse().ok(),
                |hex| u64::from_str_radix(hex, 16).ok(),
            )
        })
        .unwrap_or(DEFAULT_SEED)
}

fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
        .unwrap_or_else(|| "non-text panic payload".to_owned())
}

fn hex_preview(bytes: &[u8]) -> String {
    let shown = bytes.get(..256).unwrap_or(bytes);
    let mut text = shown
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if shown.len() < bytes.len() {
        text.push_str("...");
    }
    text
}

/// Deterministic SplitMix64 stream; reproducibility needs no external generator.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }

    fn byte(&mut self) -> u8 {
        if self.below(2) == 0 {
            BOUNDARY_BYTES[self.below(BOUNDARY_BYTES.len())]
        } else {
            self.next().to_le_bytes()[0]
        }
    }
}
