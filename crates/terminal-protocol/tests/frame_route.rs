use orchbee_terminal_protocol::{ProtocolErrorCode, inspect_terminal_frame_route};

const FRAME: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/terminal-frame.bin");
const GHOSTTY: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/ghostty-surface.bin");

#[test]
fn reads_existing_typescript_and_real_ghostty_routes_without_touching_payload()
-> Result<(), Box<dyn std::error::Error>> {
    for bytes in [FRAME, GHOSTTY] {
        let route = inspect_terminal_frame_route(bytes)?;
        assert_eq!(route.host_epoch.as_bytes(), &bytes[16..32]);
        assert_eq!(route.attachment_id.as_bytes(), &bytes[32..48]);
        assert_eq!(
            route.sequence.get(),
            u64::from_le_bytes(bytes[48..56].try_into()?)
        );
        let mut opaque = bytes.to_vec();
        opaque[160..].fill(255);
        assert_eq!(inspect_terminal_frame_route(&opaque)?, route);
    }
    Ok(())
}

#[test]
fn rejects_unroutable_headers_without_claiming_to_validate_projection_content() {
    for (offset, value, error) in [
        (0, 0, ProtocolErrorCode::InvalidMagic),
        (144, 2, ProtocolErrorCode::UnsupportedVersion),
        (7, 128, ProtocolErrorCode::UnknownRequiredFlag),
        (6, 255, ProtocolErrorCode::InvalidField),
        (8, 0, ProtocolErrorCode::LengthMismatch),
    ] {
        let mut bytes = FRAME.to_vec();
        bytes[offset] = value;
        assert_eq!(
            inspect_terminal_frame_route(&bytes).map_err(|error| error.code()),
            Err(error)
        );
    }
    for range in [16..32, 32..48, 48..56, 72..80] {
        let mut bytes = FRAME.to_vec();
        bytes[range].fill(0);
        assert!(inspect_terminal_frame_route(&bytes).is_err());
    }
    assert_eq!(
        inspect_terminal_frame_route(&FRAME[..159]).map_err(|error| error.code()),
        Err(ProtocolErrorCode::Truncated)
    );
}
