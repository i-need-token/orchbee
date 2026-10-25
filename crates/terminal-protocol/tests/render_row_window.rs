//! Render admission is checked against the Host and shared TS/Zig fixtures.
#[path = "support/encoded_fixture.rs"]
mod encoded_fixture;
use encoded_fixture::base64;
use orchbee_terminal_protocol::{
    ProtocolErrorCode, RowWindowDetail, decode_terminal_render_row_window,
    decode_terminal_text_row_window,
};
use std::error::Error;

const WINDOW: &[u8] =
    include_bytes!("../../../schemas/terminal/v1/fixtures/ghostty-render-row-window.bin");
const PACKED: &str =
    include_str!("../../../schemas/terminal/v1/fixtures/packed-projection-conformance.json");
const MALFORMED: &str =
    include_str!("../../../schemas/terminal/v1/fixtures/malformed-row-window.json");

#[test]
fn host_render_snapshot_is_borrowed_and_all_prefixes_reject() -> Result<(), Box<dyn Error>> {
    let window = decode_terminal_render_row_window(WINDOW)?;
    let route = window.route();
    assert_eq!(route.detail, RowWindowDetail::Render);
    assert_eq!(
        (route.columns, route.row_count, route.total_rows),
        (8, 6, 6)
    );
    assert_eq!(
        (route.sequence.get(), route.revision, route.generation.get()),
        (7, 11, 13)
    );
    assert!(window.projection().route().base_revision.is_none());
    assert_eq!(window.projection().rows().len(), 6);
    assert_eq!(
        (
            window.graphics_image_count(),
            window.graphics_placement_count()
        ),
        (0, 0)
    );
    let mut unicode = false;
    let lower = WINDOW.as_ptr() as usize;
    for row in window.projection().rows() {
        for cell in row.cells() {
            let address = cell.text.as_ptr() as usize;
            assert!(address >= lower && address + cell.text.len() <= lower + WINDOW.len());
            unicode |= !cell.text.is_ascii();
        }
    }
    assert!(unicode);
    for length in 0..WINDOW.len() {
        assert!(
            decode_terminal_render_row_window(&WINDOW[..length]).is_err(),
            "{length}"
        );
    }
    assert!(decode_terminal_text_row_window(WINDOW).is_err());
    Ok(())
}

#[test]
fn shared_complete_row_window_corpora_keep_their_error_codes() -> Result<(), Box<dyn Error>> {
    let mut render_cases = 0;
    for corpus in [PACKED, MALFORMED] {
        let corpus: serde_json::Value = serde_json::from_str(corpus)?;
        for case in corpus["cases"].as_array().ok_or("cases")? {
            if case.get("codec").is_some_and(|codec| codec != "OBRW") {
                continue;
            }
            let name = case["name"].as_str().ok_or("name")?;
            let bytes = base64(case["bytesBase64"].as_str().ok_or("bytes")?)?;
            let expected = case.get("expected").unwrap_or(case);
            let result = if bytes.get(6) == Some(&1) {
                render_cases += 1;
                decode_terminal_render_row_window(&bytes).map(|_| ())
            } else {
                decode_terminal_text_row_window(&bytes).map(|_| ())
            };
            if expected["result"] == "success" {
                result.map_err(|error| format!("{name}: {error}"))?;
            } else {
                let error = result.err().ok_or_else(|| format!("{name}: accepted"))?;
                assert_eq!(
                    error.code().to_string(),
                    expected["code"].as_str().ok_or("code")?,
                    "{name}"
                );
            }
        }
    }
    assert_eq!(render_cases, 6);
    Ok(())
}

#[test]
fn every_nested_route_pin_must_agree_with_the_outer_snapshot() {
    // Mutate only the outer header so the nested snapshot remains independently valid.
    for offset in [16, 32, 48, 56, 64] {
        let mut wrong = WINDOW.to_vec();
        wrong[offset] ^= 1;
        assert_eq!(
            decode_terminal_render_row_window(&wrong)
                .unwrap_err()
                .code(),
            ProtocolErrorCode::InvalidField,
            "offset {offset}"
        );
    }
    let mut wrong_origin = WINDOW.to_vec();
    wrong_origin[72..80].copy_from_slice(&7_u64.to_le_bytes());
    wrong_origin[80..88].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        decode_terminal_render_row_window(&wrong_origin)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::InvalidField
    );
    let text = include_bytes!("../../../schemas/terminal/v1/fixtures/terminal-row-window.bin");
    assert_eq!(
        decode_terminal_render_row_window(text).unwrap_err().code(),
        ProtocolErrorCode::InvalidField
    );
}

fn with_graphics(graphics: &[u8]) -> Vec<u8> {
    let projection_length =
        u32::from_le_bytes(WINDOW[101..105].try_into().expect("fixture")) as usize;
    let mut record = WINDOW[..112 + projection_length].to_vec();
    record.extend_from_slice(graphics);
    let total = record.len() as u32;
    record[8..12].copy_from_slice(&total.to_le_bytes());
    record[96..100].copy_from_slice(&(total - 112).to_le_bytes());
    record[105..109].copy_from_slice(&(graphics.len() as u32).to_le_bytes());
    record
}

fn graphics_snapshot() -> Vec<u8> {
    // One 2x1 RGBA image and two placements sharing the legal zero placement ID.
    // Fields are authored from the wire contract, independently of the decoder.
    let mut bytes = vec![0; 40];
    bytes[..8].copy_from_slice(b"OBKG\0\x01\0\x01");
    bytes[12..20].copy_from_slice(&1_u64.to_le_bytes());
    bytes[28..30].copy_from_slice(&1_u16.to_le_bytes());
    bytes[30..32].copy_from_slice(&2_u16.to_le_bytes());
    bytes[32..36].copy_from_slice(&8_u32.to_le_bytes());
    bytes.extend_from_slice(&9_u32.to_le_bytes());
    bytes.extend_from_slice(&[4, 0, 0, 0]);
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    for field in [2_u32, 1, 8, 0] {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
    bytes.extend_from_slice(&[255, 0, 0, 255, 0, 0, 255, 255]);
    for _ in 0..2 {
        for field in [
            9_u32,
            0,
            u32::MAX,
            u32::MAX,
            0,
            0,
            0,
            0,
            2,
            1,
            20,
            10,
            u32::MAX,
            1,
        ] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
    }
    let total = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&total.to_le_bytes());
    bytes
}

#[test]
fn graphics_resources_validate_even_though_the_consumer_only_uses_text()
-> Result<(), Box<dyn Error>> {
    let graphics = graphics_snapshot();
    let record = with_graphics(&graphics);
    let window = decode_terminal_render_row_window(&record)?;
    assert_eq!(
        (
            window.graphics_image_count(),
            window.graphics_placement_count()
        ),
        (1, 2)
    );
    // Header, image and placement faults all fail closed, including ignored pixels.
    for (offset, value, code) in [
        (7, 2, ProtocolErrorCode::InvalidField),
        (12, 0, ProtocolErrorCode::InvalidField),
        (36, 1, ProtocolErrorCode::UnknownRequiredFlag),
        (40, 0, ProtocolErrorCode::InvalidField),
        (44, 5, ProtocolErrorCode::InvalidField),
        (45, 1, ProtocolErrorCode::UnknownRequiredFlag),
        (48, 0, ProtocolErrorCode::InvalidField),
        (56, 3, ProtocolErrorCode::LengthMismatch),
        (68, 1, ProtocolErrorCode::UnknownRequiredFlag),
        (80, 7, ProtocolErrorCode::InvalidField),
        (104, 1, ProtocolErrorCode::InvalidField),
        (112, 0, ProtocolErrorCode::InvalidField),
        (120, 0, ProtocolErrorCode::InvalidField),
        (132, 2, ProtocolErrorCode::UnknownRequiredFlag),
        (32, 7, ProtocolErrorCode::LengthMismatch),
    ] {
        let mut bad = graphics.clone();
        bad[offset] = value;
        assert_eq!(
            decode_terminal_render_row_window(&with_graphics(&bad))
                .unwrap_err()
                .code(),
            code,
            "offset {offset}"
        );
    }
    let mut duplicate = graphics.clone();
    duplicate.splice(80..80, graphics[40..80].iter().copied());
    duplicate[28..30].copy_from_slice(&2_u16.to_le_bytes());
    duplicate[32..36].copy_from_slice(&16_u32.to_le_bytes());
    let total = duplicate.len() as u32;
    duplicate[8..12].copy_from_slice(&total.to_le_bytes());
    assert_eq!(
        decode_terminal_render_row_window(&with_graphics(&duplicate))
            .unwrap_err()
            .code(),
        ProtocolErrorCode::InvalidField
    );
    for end in 40..graphics.len() {
        let mut truncated = graphics[..end].to_vec();
        truncated[8..12].copy_from_slice(&(end as u32).to_le_bytes());
        assert!(decode_terminal_render_row_window(&with_graphics(&truncated)).is_err());
    }
    Ok(())
}
