use orchbee_terminal_protocol::*;
use std::num::NonZeroU64;

#[test]
fn clear_requires_explicit_prompt_policy_and_keeps_reset_closed() {
    let id = AttachmentId::from_bytes([1; 16]).unwrap();
    for preserve_prompt in [false, true] {
        let value = ScreenControl::ClearTerminal(ClearTerminal {
            attachment_id: id,
            preserve_prompt,
        });
        let wire = encode_screen_control(value).unwrap();
        let expected = if preserve_prompt {
            br#"{"attachmentId":"01010101010101010101010101010101","preservePrompt":true}"#
                .as_slice()
        } else {
            br#"{"attachmentId":"01010101010101010101010101010101","preservePrompt":false}"#
                .as_slice()
        };
        assert_eq!(wire.payload, expected);
        assert_eq!(
            decode_screen_control(wire.kind, wire.role, &wire.payload).unwrap(),
            value
        );
    }
    let missing = br#"{"attachmentId":"01010101010101010101010101010101"}"#;
    assert!(decode_screen_control(74, MessageRole::Request, missing).is_err());
    let invalid = br#"{"attachmentId":"01010101010101010101010101010101","preservePrompt":true}"#;
    assert!(decode_screen_control(75, MessageRole::Request, invalid).is_err());
    assert!(decode_screen_control(74, MessageRole::Response, invalid).is_err());
}

#[test]
fn clear_acknowledgements_require_positive_generation_canonical_revision_and_full_identity() {
    let value = ScreenControl::TerminalControlAccepted(TerminalControlAccepted {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        generation: NonZeroU64::MAX,
        revision: 1,
        operation: ScreenControlOperation::Clear,
    });
    let wire = encode_screen_control(value).unwrap();
    assert_eq!(
        decode_screen_control(76, MessageRole::Response, &wire.payload).unwrap(),
        value
    );
    let original = String::from_utf8(wire.payload).unwrap();
    for bad in ["01", "18446744073709551616", "+1", "1.0"] {
        let payload = original.replace("\"revision\":\"1\"", &format!("\"revision\":\"{bad}\""));
        assert!(decode_screen_control(76, MessageRole::Response, payload.as_bytes()).is_err());
    }
    let zero = original.replace("\"revision\":\"1\"", "\"revision\":\"0\"");
    assert!(decode_screen_control(76, MessageRole::Response, zero.as_bytes()).is_ok());
    let extra = original.replace("\"operation\":\"clear\"", "\"operation\":\"clear-prompt\"");
    assert!(decode_screen_control(76, MessageRole::Response, extra.as_bytes()).is_err());
}
