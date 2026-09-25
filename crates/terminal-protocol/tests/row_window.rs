use orchbee_terminal_protocol::{
    MAXIMUM_ROW_WINDOW_BYTES, ProtocolErrorCode, TERMINAL_ROW_WINDOW_HEADER_BYTES,
    TerminalRowPromptClass, decode_terminal_text_row_window,
};
use std::error::Error;
#[path = "support/encoded_fixture.rs"]
mod encoded_fixture;
use encoded_fixture::base64;

const WINDOW: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/terminal-row-window-v0.0.3.bin");
const CANONICAL: &str = include_str!("../../../packages/terminal-protocol/fixtures/canonical.json");
const MALFORMED: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/malformed-row-window-v0.0.3.json");
const PACKED: &str = include_str!(
    "../../../packages/terminal-protocol/fixtures/packed-projection-conformance-v0.0.3.json"
);

#[test]
fn canonical_text_window_matches_the_encoding_owner() -> Result<(), Box<dyn Error>> {
    let expected: serde_json::Value = serde_json::from_str(CANONICAL)?;
    let expected = &expected["rowWindow"];
    let window = decode_terminal_text_row_window(WINDOW)?;
    assert_eq!(
        window.attachment_id.to_text(),
        expected["attachmentId"].as_str().ok_or("attachmentId")?
    );
    assert_eq!(
        window.host_epoch.to_text(),
        expected["hostEpoch"].as_str().ok_or("hostEpoch")?
    );
    assert_eq!(
        window.sequence.get(),
        expected["sequence"]
            .as_str()
            .ok_or("sequence")?
            .parse::<u64>()?
    );
    assert_eq!(
        window.revision,
        expected["revision"]
            .as_str()
            .ok_or("revision")?
            .parse::<u64>()?
    );
    assert_eq!(
        window.generation.get(),
        expected["generation"]
            .as_str()
            .ok_or("generation")?
            .parse::<u64>()?
    );
    assert_eq!(
        window.total_rows,
        expected["totalRows"]
            .as_str()
            .ok_or("totalRows")?
            .parse::<u64>()?
    );
    assert_eq!(
        window.start_row,
        expected["startRow"]
            .as_str()
            .ok_or("startRow")?
            .parse::<u64>()?
    );
    assert_eq!(
        u64::from(window.columns),
        expected["columns"].as_u64().ok_or("columns")?
    );
    assert_eq!(window.ignored_optional_flags, 0);
    let expected_rows = expected["rows"].as_array().ok_or("rows")?;
    assert_eq!(window.rows.len(), expected_rows.len());
    for (row, expected_row) in window.rows.iter().zip(expected_rows) {
        assert_eq!(
            row.index,
            expected_row["index"]
                .as_str()
                .ok_or("index")?
                .parse::<u64>()?
        );
        assert_eq!(
            row.prompt_class,
            match expected_row["promptClass"].as_str().ok_or("promptClass")? {
                "none" => TerminalRowPromptClass::None,
                "prompt" => TerminalRowPromptClass::Prompt,
                "prompt-continuation" => TerminalRowPromptClass::PromptContinuation,
                other => return Err(format!("unknown prompt class {other}").into()),
            }
        );
        assert_eq!(&*row.text, expected_row["text"].as_str().ok_or("text")?);
        assert_eq!(row.wrap, expected_row["wrap"].as_bool().ok_or("wrap")?);
        assert_eq!(
            row.wrap_continuation,
            expected_row["wrapContinuation"]
                .as_bool()
                .ok_or("wrapContinuation")?
        );
        assert!(!row.identity_ascii);
        let expected_mappings = expected_row["mappings"].as_array().ok_or("mappings")?;
        assert_eq!(row.mappings.len(), expected_mappings.len());
        for (mapping, expected_mapping) in row.mappings.iter().zip(expected_mappings) {
            assert_eq!(
                u64::from(mapping.byte_offset),
                expected_mapping["byteOffset"]
                    .as_u64()
                    .ok_or("byteOffset")?
            );
            assert_eq!(
                u64::from(mapping.utf16_offset),
                expected_mapping["utf16Offset"]
                    .as_u64()
                    .ok_or("utf16Offset")?
            );
            assert_eq!(
                u64::from(mapping.column),
                expected_mapping["column"].as_u64().ok_or("column")?
            );
            assert_eq!(
                u64::from(mapping.width),
                expected_mapping["width"].as_u64().ok_or("width")?
            );
        }
    }
    assert_eq!(window.rows[0].text.as_ref(), "A\u{754c}e\u{0301} ");
    assert_eq!(
        window.rows[1].text.as_ref(),
        "\u{1f469}\u{1f3fd}\u{200d}\u{1f4bb}B"
    );
    Ok(())
}

#[test]
fn malformed_owner_corpus_and_every_truncation_reject_without_partial_rows()
-> Result<(), Box<dyn Error>> {
    let corpus: serde_json::Value = serde_json::from_str(MALFORMED)?;
    for case in corpus["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let bytes = base64(case["bytesBase64"].as_str().ok_or("bytes")?)?;
        let error = decode_terminal_text_row_window(&bytes)
            .err()
            .ok_or("invalid row window accepted")?;
        // The shared expectation is full-decoder semantics: the projection
        // section mismatch lives inside the render payload, which a text-only
        // decoder never parses, so the window reaches the render rejection.
        let expected = if name == "render-projection-section-length-mismatch" {
            "invalid-field"
        } else {
            case["code"].as_str().ok_or("code")?
        };
        assert_eq!(error.code().to_string(), expected, "{name}");
    }
    for end in 0..WINDOW.len() {
        assert!(decode_terminal_text_row_window(&WINDOW[..end]).is_err());
    }
    Ok(())
}

#[test]
fn packed_conformance_obrw_cases_match_shared_observations() -> Result<(), Box<dyn Error>> {
    let corpus: serde_json::Value = serde_json::from_str(PACKED)?;
    let mut observed = 0_usize;
    for case in corpus["cases"].as_array().ok_or("cases")? {
        if case["codec"].as_str() != Some("OBRW") {
            continue;
        }
        observed += 1;
        let name = case["name"].as_str().ok_or("name")?;
        let bytes = base64(case["bytesBase64"].as_str().ok_or("bytes")?)?;
        let expected = &case["expected"];
        match expected["result"].as_str().ok_or("result")? {
            "success" => {
                let window = decode_terminal_text_row_window(&bytes)
                    .map_err(|error| format!("{name}: {error}"))?;
                let observations = &expected["observations"];
                assert_eq!(
                    observations["detail"].as_str().ok_or("detail")?,
                    "text",
                    "{name}"
                );
                assert_eq!(
                    u64::from(window.ignored_optional_flags),
                    observations["ignoredOptionalFlags"]
                        .as_u64()
                        .ok_or("ignoredOptionalFlags")?,
                    "{name}"
                );
                assert_eq!(
                    window.rows.len() as u64,
                    observations["rowCount"].as_u64().ok_or("rowCount")?,
                    "{name}"
                );
                match name {
                    "obrw-text-identity-ascii" => {
                        assert!(window.rows[0].identity_ascii, "{name}");
                        assert!(window.rows[0].mappings.is_empty(), "{name}");
                        assert_eq!(&*window.rows[0].text, "alpha", "{name}");
                    }
                    "obrw-text-aggregate-row-over-16k" => {
                        assert_eq!(window.rows[0].mappings.len(), 3, "{name}");
                        assert!(window.rows[0].text.len() > 16 * 1024, "{name}");
                    }
                    "obrw-text-empty-row" => {
                        assert!(window.rows[0].text.is_empty(), "{name}");
                        assert!(window.rows[0].mappings.is_empty(), "{name}");
                    }
                    "obrw-text-wrap-chain-three-rows" => {
                        assert!(window.rows[0].wrap, "{name}");
                        assert!(
                            window.rows[1].wrap && window.rows[1].wrap_continuation,
                            "{name}"
                        );
                        assert!(window.rows[2].wrap_continuation, "{name}");
                    }
                    "obrw-text-utf8-bom-leading" => {
                        assert!(window.rows[0].text.starts_with('\u{feff}'), "{name}");
                    }
                    _ => {}
                }
            }
            "failure" => {
                let error = decode_terminal_text_row_window(&bytes)
                    .err()
                    .ok_or("invalid row window accepted")?;
                // Same full-decoder boundary as the malformed corpus: the
                // projection section fault is render-payload semantics.
                let expected_code = if name == "obrw-projection-section-length-mismatch" {
                    "invalid-field"
                } else {
                    expected["code"].as_str().ok_or("code")?
                };
                assert_eq!(error.code().to_string(), expected_code, "{name}");
            }
            other => return Err(format!("unknown result {other}").into()),
        }
    }
    assert_eq!(observed, 11);
    Ok(())
}

#[test]
fn utf8_and_mapping_boundaries_reject_on_scalar_splits() -> Result<(), Box<dyn Error>> {
    // Byte 125 sits inside the three-byte UTF-8 sequence for U+4F60 (row 0 text
    // starts at 124): corrupting it breaks UTF-8, not the mapping table.
    let mut invalid_utf8 = WINDOW.to_vec();
    invalid_utf8[125] = 0xff;
    assert_eq!(
        decode_terminal_text_row_window(&invalid_utf8)
            .err()
            .ok_or("accepted")?
            .code(),
        ProtocolErrorCode::InvalidUtf8
    );
    // Row 0 mapping 1 byte offset lives at 144: pointing it inside U+4F60 must
    // fail as a non-scalar-boundary mapping, independent of fixture corpora.
    let mut mid_scalar = WINDOW.to_vec();
    mid_scalar[144..148].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        decode_terminal_text_row_window(&mid_scalar)
            .err()
            .ok_or("accepted")?
            .code(),
        ProtocolErrorCode::InvalidField
    );
    Ok(())
}

#[test]
fn size_bound_rejects_before_magic_is_read() -> Result<(), Box<dyn Error>> {
    // Past the 44 MiB row-window bound the length guard fires before the magic
    // bytes are inspected; at exactly the bound the same invalid magic is
    // reached and named.
    let mut bytes = vec![0_u8; MAXIMUM_ROW_WINDOW_BYTES + 1];
    assert_eq!(
        decode_terminal_text_row_window(&bytes)
            .err()
            .ok_or("over-limit window accepted")?
            .code(),
        ProtocolErrorCode::LimitExceeded
    );
    bytes.truncate(MAXIMUM_ROW_WINDOW_BYTES);
    assert_eq!(
        decode_terminal_text_row_window(&bytes)
            .err()
            .ok_or("invalid magic accepted")?
            .code(),
        ProtocolErrorCode::InvalidMagic
    );
    Ok(())
}

#[test]
fn legal_render_window_fails_closed_as_text_only() -> Result<(), Box<dyn Error>> {
    // The real Host-produced render window completes full header validation,
    // then fails closed: the text decoder never interprets render payloads.
    let render = include_bytes!(
        "../../../packages/terminal-protocol/fixtures/ghostty-render-row-window-v0.0.3.bin"
    );
    let error = decode_terminal_text_row_window(render)
        .err()
        .ok_or("render window accepted by text decoder")?;
    assert_eq!(error.code(), ProtocolErrorCode::InvalidField);
    assert!(error.detail().contains("text"), "{}", error.detail());
    // Independent negative control: a render window with inconsistent section
    // lengths is a length mismatch before the text-only rejection is reached.
    let malformed_render = assembled_render_window(4, 39);
    assert_eq!(
        decode_terminal_text_row_window(&malformed_render)
            .err()
            .ok_or("accepted")?
            .code(),
        ProtocolErrorCode::LengthMismatch
    );
    Ok(())
}

fn assembled_render_window(projection_length: u32, graphics_length: u32) -> Vec<u8> {
    let body_length = projection_length + graphics_length;
    let mut bytes = vec![0_u8; TERMINAL_ROW_WINDOW_HEADER_BYTES + body_length as usize];
    bytes[0..4].copy_from_slice(b"OBRW");
    bytes[6] = 1;
    let total_length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&total_length.to_le_bytes());
    bytes[12..14].copy_from_slice(&(TERMINAL_ROW_WINDOW_HEADER_BYTES as u16).to_le_bytes());
    bytes[14..16].copy_from_slice(&6_u16.to_le_bytes());
    bytes[16..32].fill(0x11);
    bytes[32..48].fill(0x22);
    bytes[48..56].copy_from_slice(&8_u64.to_le_bytes());
    bytes[56..64].copy_from_slice(&42_u64.to_le_bytes());
    bytes[64..72].copy_from_slice(&3_u64.to_le_bytes());
    bytes[72..80].copy_from_slice(&125_u64.to_le_bytes());
    bytes[80..88].copy_from_slice(&100_u64.to_le_bytes());
    bytes[88..90].copy_from_slice(&24_u16.to_le_bytes());
    bytes[92..96].copy_from_slice(&144_u32.to_le_bytes());
    bytes[96..100].copy_from_slice(&body_length.to_le_bytes());
    bytes[100] = 3;
    bytes[101..105].copy_from_slice(&projection_length.to_le_bytes());
    bytes[105..109].copy_from_slice(&graphics_length.to_le_bytes());
    bytes
}
