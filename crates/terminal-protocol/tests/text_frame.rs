use orchbee_terminal_protocol::decode_terminal_text_frame;
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
