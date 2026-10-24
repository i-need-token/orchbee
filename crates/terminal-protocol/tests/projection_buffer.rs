//! Independent TS/Zig corpus and real Host witnesses for the Rust strict boundary.
#[path = "support/encoded_fixture.rs"]
mod encoded_fixture;
use encoded_fixture::base64;
use orchbee_terminal_protocol::{ProtocolErrorCode, decode_terminal_projection_buffer};
use std::error::Error;

const CORPUS: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/packed-projection-conformance.json");
const PROJECTION: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/terminal-projection-buffer.bin");

#[test]
fn shared_obpb_corpus_has_identical_admission_and_observations() -> Result<(), Box<dyn Error>> {
    let corpus: serde_json::Value = serde_json::from_str(CORPUS)?;
    let mut count = 0;
    for case in corpus["cases"].as_array().ok_or("cases")? {
        if case["codec"] != "OBPB" {
            continue;
        }
        count += 1;
        let name = case["name"].as_str().ok_or("name")?;
        let bytes = base64(case["bytesBase64"].as_str().ok_or("bytes")?)?;
        let expected = &case["expected"];
        let decoded = decode_terminal_projection_buffer(&bytes);
        if expected["result"] == "failure" {
            let error = decoded
                .err()
                .ok_or_else(|| format!("{name}: accepted invalid data"))?;
            assert_eq!(
                error.code().to_string(),
                expected["code"].as_str().ok_or("code")?,
                "{name}"
            );
        } else {
            assert_eq!(expected["result"], "success");
            let decoded = decoded.map_err(|error| format!("{name}: {error}"))?;
            let observations = &expected["observations"];
            assert_eq!(
                serde_json::json!(decoded.rows().map(|row| row.index()).collect::<Vec<_>>()),
                observations["rowIndices"],
                "{name}"
            );
            assert_eq!(
                serde_json::json!(decoded.rows().len()),
                observations["rowCount"],
                "{name}"
            );
            assert_eq!(
                serde_json::json!(decoded.ignored_optional_flags()),
                observations["ignoredOptionalFlags"],
                "{name}"
            );
            assert_eq!(
                if decoded.route().base_revision.is_some() {
                    "delta"
                } else {
                    "snapshot"
                },
                observations["kind"].as_str().ok_or("kind")?,
                "{name}"
            );
            // Every public cell accessor remains safe after strict admission.
            for row in decoded.rows() {
                assert_eq!(row.cells().count(), usize::from(decoded.geometry().columns));
            }
        }
    }
    assert_eq!(count, 23);
    Ok(())
}

#[test]
fn canonical_views_borrow_utf8_and_reject_all_strict_prefixes() -> Result<(), Box<dyn Error>> {
    let decoded = decode_terminal_projection_buffer(PROJECTION)?;
    assert!(decoded.route().base_revision.is_some());
    assert!(decoded.rows().len() <= usize::from(decoded.geometry().rows));
    let lower = PROJECTION.as_ptr() as usize;
    let upper = lower + PROJECTION.len();
    let mut saw_unicode = false;
    for row in decoded.rows() {
        for cell in row.cells() {
            let address = cell.text.as_ptr() as usize;
            assert!(address >= lower && address + cell.text.len() <= upper);
            saw_unicode |= !cell.text.is_ascii();
        }
    }
    assert!(saw_unicode);
    for length in 0..PROJECTION.len() {
        assert!(
            decode_terminal_projection_buffer(&PROJECTION[..length]).is_err(),
            "prefix {length}"
        );
    }
    let mut unknown_pixels = PROJECTION.to_vec();
    unknown_pixels[100..104].fill(0);
    let unknown = decode_terminal_projection_buffer(&unknown_pixels)?;
    assert_eq!(
        (
            unknown.geometry().cell_pixel_width,
            unknown.geometry().cell_pixel_height
        ),
        (0, 0)
    );
    unknown_pixels[100] = 8;
    assert_eq!(
        decode_terminal_projection_buffer(&unknown_pixels)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::InvalidField
    );
    let mut extra = PROJECTION.to_vec();
    extra.push(0);
    assert_eq!(
        decode_terminal_projection_buffer(&extra)
            .unwrap_err()
            .code(),
        ProtocolErrorCode::LengthMismatch
    );
    Ok(())
}
