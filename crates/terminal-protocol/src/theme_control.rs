//! Runtime terminal theme commits. Terminal Host assigns revisions; a caller only names the
//! revision it observed and the complete theme it wants committed.
use serde::{Deserialize, Serialize};

use crate::{
    MessageRole, ProtocolError, ProtocolErrorCode, TerminalId, TerminalTheme,
    TerminalThemeOperationId, TerminalThemeState,
    control::{decode_wire, preflight_json, serialize_wire},
    inventory::{
        TerminalThemeStateWire, TerminalThemeWire, theme_from_wire, theme_state_from_wire,
        theme_state_to_wire, theme_to_wire, validate_theme,
    },
    terminal_fact::canonical_u64,
};

const APPLY_TERMINAL_THEME_KIND: u16 = 77;
const TERMINAL_THEME_RESULT_KIND: u16 = 78;

/// Compare-and-set request for one terminal's complete theme.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyTerminalTheme {
    pub expected_theme_revision: u64,
    pub operation_id: TerminalThemeOperationId,
    pub terminal: TerminalTheme,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalThemeTransition {
    AlreadyApplied,
    Applied,
    Stale,
}

/// Host's answer; `current` is the committed state after the request, including for `stale`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalThemeResult {
    pub current: TerminalThemeState,
    pub operation_id: TerminalThemeOperationId,
    pub terminal_id: TerminalId,
    pub transition: TerminalThemeTransition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeControl {
    ApplyTerminalTheme(ApplyTerminalTheme),
    TerminalThemeResult(TerminalThemeResult),
}

pub struct EncodedThemeControl {
    pub kind: u16,
    pub role: MessageRole,
    pub payload: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyWire {
    expected_theme_revision: String,
    operation_id: String,
    terminal: TerminalThemeWire,
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResultWire {
    current: TerminalThemeStateWire,
    operation_id: String,
    terminal_id: String,
    transition: TerminalThemeTransition,
}

pub fn decode_theme_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ThemeControl, ProtocolError> {
    preflight_json(payload)?;
    Ok(match (kind, role) {
        (APPLY_TERMINAL_THEME_KIND, MessageRole::Request) => {
            let wire: ApplyWire = decode_wire(payload)?;
            let terminal = theme_from_wire(wire.terminal)?;
            validate_theme(&terminal)?;
            ThemeControl::ApplyTerminalTheme(ApplyTerminalTheme {
                expected_theme_revision: canonical_u64(&wire.expected_theme_revision)?,
                operation_id: operation(&wire.operation_id)?,
                terminal,
                terminal_id: terminal_id(&wire.terminal_id)?,
            })
        }
        (TERMINAL_THEME_RESULT_KIND, MessageRole::Response) => {
            let wire: ResultWire = decode_wire(payload)?;
            let result = TerminalThemeResult {
                current: theme_state_from_wire(wire.current)?,
                operation_id: operation(&wire.operation_id)?,
                terminal_id: terminal_id(&wire.terminal_id)?,
                transition: wire.transition,
            };
            validate_result(&result)?;
            ThemeControl::TerminalThemeResult(result)
        }
        _ => return Err(invalid("kind/role do not select terminal theme control")),
    })
}

pub fn encode_theme_control(value: &ThemeControl) -> Result<EncodedThemeControl, ProtocolError> {
    let (kind, role, payload) = match value {
        ThemeControl::ApplyTerminalTheme(request) => {
            validate_theme(&request.terminal)?;
            (
                APPLY_TERMINAL_THEME_KIND,
                MessageRole::Request,
                serialize_wire(&ApplyWire {
                    expected_theme_revision: request.expected_theme_revision.to_string(),
                    operation_id: request.operation_id.to_text(),
                    terminal: theme_to_wire(&request.terminal),
                    terminal_id: request.terminal_id.to_text(),
                })?,
            )
        }
        ThemeControl::TerminalThemeResult(result) => {
            validate_result(result)?;
            validate_theme(&result.current.terminal)?;
            (
                TERMINAL_THEME_RESULT_KIND,
                MessageRole::Response,
                serialize_wire(&ResultWire {
                    current: theme_state_to_wire(&result.current),
                    operation_id: result.operation_id.to_text(),
                    terminal_id: result.terminal_id.to_text(),
                    transition: result.transition,
                })?,
            )
        }
    };
    Ok(EncodedThemeControl {
        kind,
        role,
        payload,
    })
}

fn validate_result(result: &TerminalThemeResult) -> Result<(), ProtocolError> {
    let current_matches = result.current.operation_id == Some(result.operation_id);
    if (result.transition == TerminalThemeTransition::Stale) == current_matches {
        return Err(invalid(
            "theme transition disagrees with the current operation identity",
        ));
    }
    Ok(())
}

fn operation(value: &str) -> Result<TerminalThemeOperationId, ProtocolError> {
    TerminalThemeOperationId::parse(value).map_err(|_| invalid("invalid theme operation identity"))
}

fn terminal_id(value: &str) -> Result<TerminalId, ProtocolError> {
    TerminalId::parse(value).map_err(|_| invalid("invalid theme terminal identity"))
}

const fn invalid(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
