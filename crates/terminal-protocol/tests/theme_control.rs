use std::error::Error;

use orchbee_terminal_protocol::{
    MessageRole, TerminalThemeTransition, ThemeControl, decode_theme_control, encode_theme_control,
};
use serde_json::Value;

const CONTROL_FIXTURE: &str =
    include_str!("../../../packages/terminal-protocol/fixtures/control-conformance.json");

#[test]
fn canonical_theme_payloads_round_trip_byte_for_byte() -> Result<(), Box<dyn Error>> {
    let fixture: Value = serde_json::from_str(CONTROL_FIXTURE)?;
    let command = fixture
        .pointer("/commands/apply_terminal_theme")
        .and_then(Value::as_str)
        .ok_or("missing apply fixture")?;
    let decoded = decode_theme_control(77, MessageRole::Request, command.as_bytes())?;
    assert!(matches!(decoded, ThemeControl::ApplyTerminalTheme(_)));
    assert_eq!(encode_theme_control(&decoded)?.payload, command.as_bytes());
    assert!(decode_theme_control(77, MessageRole::Response, command.as_bytes()).is_err());

    let results = fixture
        .pointer("/outputs/terminal_theme_result")
        .and_then(Value::as_array)
        .ok_or("missing result fixture")?;
    assert!(!results.is_empty());
    for result in results {
        let payload = result.as_str().ok_or("result fixture is not text")?;
        let decoded = decode_theme_control(78, MessageRole::Response, payload.as_bytes())?;
        let encoded = encode_theme_control(&decoded)?;
        assert_eq!((encoded.kind, encoded.role), (78, MessageRole::Response));
        assert_eq!(encoded.payload, payload.as_bytes());
    }
    Ok(())
}

#[test]
fn theme_result_transition_must_agree_with_current_operation() -> Result<(), Box<dyn Error>> {
    let fixture: Value = serde_json::from_str(CONTROL_FIXTURE)?;
    let payload = fixture
        .pointer("/outputs/terminal_theme_result/0")
        .and_then(Value::as_str)
        .ok_or("missing result fixture")?;
    let ThemeControl::TerminalThemeResult(result) =
        decode_theme_control(78, MessageRole::Response, payload.as_bytes())?
    else {
        return Err("fixture did not decode as a theme result".into());
    };
    let flipped = match result.transition {
        TerminalThemeTransition::Stale => TerminalThemeTransition::Applied,
        TerminalThemeTransition::Applied | TerminalThemeTransition::AlreadyApplied => {
            TerminalThemeTransition::Stale
        }
    };
    let invalid =
        ThemeControl::TerminalThemeResult(orchbee_terminal_protocol::TerminalThemeResult {
            transition: flipped,
            ..result
        });
    assert!(encode_theme_control(&invalid).is_err());
    Ok(())
}
