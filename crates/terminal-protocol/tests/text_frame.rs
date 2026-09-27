use orchbee_terminal_protocol::{MouseFormat, MouseTracking, decode_terminal_text_frame};
use std::{error::Error, io};
const FRAME: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin");

#[test]
fn canonical_text_matches_the_encoding_owner_and_real_ghostty() -> Result<(), Box<dyn Error>> {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/terminal-protocol/fixtures/canonical.json"
    ))?;
    let frame = decode_terminal_text_frame(FRAME)?;
    assert_eq!(
        u64::from(frame.geometry.columns),
        expected["frame"]["columns"].as_u64().ok_or("columns")?
    );
    let rows = expected["frame"]["rowData"].as_array().ok_or("rows")?;
    for (actual, expected) in frame.rows.iter().zip(rows) {
        for (cell, expected) in actual
            .cells
            .iter()
            .zip(expected["cells"].as_array().ok_or("cells")?)
        {
            assert_eq!(&*cell.text, expected["grapheme"].as_str().ok_or("text")?);
        }
    }
    for bytes in [
        include_bytes!("../../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin")
            .as_slice(),
        include_bytes!(
            "../../../packages/terminal-protocol/fixtures/ghostty-alternate-surface-v0.0.3.bin"
        )
        .as_slice(),
    ] {
        let frame = decode_terminal_text_frame(bytes)?;
        assert_eq!(frame.rows.len(), usize::from(frame.geometry.rows));
        assert!(
            frame
                .rows
                .iter()
                .any(|row| row.cells.iter().any(|cell| !cell.text.trim().is_empty()))
        );
    }
    Ok(())
}

#[test]
fn malformed_owner_corpus_and_every_truncation_reject_without_partial_rows()
-> Result<(), Box<dyn Error>> {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/terminal-protocol/fixtures/malformed-frame-v0.0.3.json"
    ))?;
    for case in corpus["cases"].as_array().ok_or("cases")? {
        let bytes = base64(case["bytesBase64"].as_str().ok_or("bytes")?)?;
        let error = decode_terminal_text_frame(&bytes)
            .err()
            .ok_or("invalid frame accepted")?;
        assert_eq!(
            error.code().to_string(),
            case["code"].as_str().ok_or("code")?,
            "{}",
            case["name"]
        );
    }
    for end in 0..FRAME.len() {
        assert!(decode_terminal_text_frame(&FRAME[..end]).is_err());
    }
    for (offset, value) in [(99, 1), (153, 1), (128, 255), (129, 255), (132, 65)] {
        let mut bytes = FRAME.to_vec();
        bytes[offset] = value;
        assert!(decode_terminal_text_frame(&bytes).is_err());
    }
    Ok(())
}
/// The `captured_interaction` word survives validated decode and exposes the
/// mutually exclusive tracking/format facts through narrow queries; invalid
/// combinations keep failing at the boundary.
#[test]
fn interaction_facts_survive_validated_decode() -> Result<(), Box<dyn Error>> {
    // Wire offset: 88-byte fixed prefix + columns/rows/cursor/screen fields.
    const INTERACTION: std::ops::Range<usize> = 100..104;
    let with_interaction = |bits: u32| {
        let mut bytes = FRAME.to_vec();
        bytes[INTERACTION].copy_from_slice(&bits.to_le_bytes());
        decode_terminal_text_frame(&bytes)
    };
    for (bits, tracking, format, cursor) in [
        (0x0, MouseTracking::None, MouseFormat::Default, false),
        (0x1, MouseTracking::None, MouseFormat::Default, true),
        (0x10, MouseTracking::X10, MouseFormat::Default, false),
        (0x20, MouseTracking::Normal, MouseFormat::Default, false),
        (0x40, MouseTracking::Button, MouseFormat::Default, false),
        (0x80, MouseTracking::Any, MouseFormat::Default, false),
        (0x100, MouseTracking::None, MouseFormat::Utf8, false),
        (0x200, MouseTracking::None, MouseFormat::Sgr, false),
        (0x400, MouseTracking::None, MouseFormat::Urxvt, false),
        (0x800, MouseTracking::None, MouseFormat::SgrPixels, false),
        (0x221, MouseTracking::Normal, MouseFormat::Sgr, true),
    ] {
        let frame = with_interaction(bits)?;
        assert_eq!(
            frame.interaction.mouse_tracking(),
            tracking,
            "bits {bits:#x}"
        );
        assert_eq!(frame.interaction.mouse_format(), format, "bits {bits:#x}");
        assert_eq!(
            frame.interaction.application_cursor_keys(),
            cursor,
            "bits {bits:#x}"
        );
    }
    for bits in [0, 1, 4, 5, 0x224] {
        assert_eq!(
            with_interaction(bits)?.interaction.bracketed_paste(),
            bits & 4 != 0
        );
    }
    for bits in [0, 4, 8, 12, 0x228] {
        assert_eq!(
            with_interaction(bits)?.interaction.focus_reporting(),
            bits & 8 != 0
        );
    }
    // Multiple bits in one exclusive group, and unknown bits, still reject.
    for bits in [0x30, 0x300, 0x40000] {
        assert!(with_interaction(bits).is_err(), "bits {bits:#x} accepted");
    }
    Ok(())
}

fn base64(value: &str) -> io::Result<Vec<u8>> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0_u32;
    let mut count = 0;
    let mut out = Vec::new();
    for byte in value.bytes().take_while(|b| *b != b'=') {
        let value = alphabet
            .iter()
            .position(|x| *x == byte)
            .ok_or_else(|| io::Error::other("invalid fixture base64"))? as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Ok(out)
}

#[test]
fn string_byte_boundary_preserves_shared_text_and_rejects_oversize_before_allocation()
-> Result<(), Box<dyn Error>> {
    let old_length = u32::from_le_bytes(FRAME[160..164].try_into()?) as usize;
    for length in [16 * 1024, 16 * 1024 + 1] {
        let mut bytes = FRAME.to_vec();
        bytes.splice(164..164 + old_length, std::iter::repeat_n(b'a', length));
        bytes[160..164].copy_from_slice(&(length as u32).to_le_bytes());
        let total = bytes.len() as u32;
        bytes[8..12].copy_from_slice(&total.to_le_bytes());
        assert_eq!(
            decode_terminal_text_frame(&bytes).is_ok(),
            length == 16 * 1024
        );
    }
    let frame = decode_terminal_text_frame(FRAME)?;
    let cells: Vec<_> = frame.rows.iter().flat_map(|row| &row.cells).collect();
    assert!(cells.iter().enumerate().any(|(index, cell)| {
        cells
            .iter()
            .skip(index + 1)
            .any(|other| std::sync::Arc::ptr_eq(&cell.text, &other.text))
    }));
    Ok(())
}
