//! Applies the controlling Client's effective query colors to its terminal, independently of the
//! launch's display mode. Unknown outer ANSI colors keep the current fallback.
use super::{
    ConnectionControl, CorrelationId, Error, HostBrokerConnection, NotStarted, Outcome,
    PayloadEncoding, TerminalId, UnexpectedAnswer, UnixStream, decode_connection_control,
    decode_transport_envelope, diagnostic_events, exchange, fresh_identity,
};
use crate::diagnostic_events::Site;
use orchbee_capability_node_protocol::{HostDefaultColors, HostTerminalId};
use orchbee_local_runtime::UserTerminalColors;
use orchbee_terminal_protocol::{
    ApplyTerminalTheme, TerminalThemeOperationId, TerminalThemeTransition, ThemeControl,
    decode_theme_control, encode_theme_control,
};

/// Best effort: a rejected or exited target stops following and keeps its current colors, and so
/// does a target whose Host answer this Worker cannot accept. A change the request has no room for
/// waits for the next colors.
pub(super) fn apply(
    host: &HostBrokerConnection,
    server: &UnixStream,
    target: HostTerminalId,
    colors: HostDefaultColors,
) -> Result<Outcome, Box<dyn Error>> {
    host.host_colors
        .borrow_mut()
        .observe(UserTerminalColors::from_rgb(
            colors.foreground(),
            colors.background(),
        ));
    let key = target.as_bytes();
    let Some(mut state) = host.host_colors.borrow().follower(key).cloned() else {
        return Ok(Outcome::HostColorsAccepted);
    };
    let terminal_id = TerminalId::from_bytes(key)?;
    // One retry absorbs a revision this Worker did not observe; a second stale result yields.
    for _attempt in 0..2 {
        let next = crate::terminal_theme::reported(colors, &state.terminal);
        if next == state.terminal {
            break;
        }
        let command =
            encode_theme_control(&ThemeControl::ApplyTerminalTheme(ApplyTerminalTheme {
                expected_theme_revision: state.revision,
                operation_id: TerminalThemeOperationId::from_bytes(fresh_identity()?)?,
                terminal: next,
                terminal_id,
            }))?;
        let response = match NotStarted::optional(exchange(
            host,
            server,
            CorrelationId::from_bytes(fresh_identity()?)?,
            command.kind,
            &command.payload,
            PayloadEncoding::ControlJson,
        )) {
            Ok(Some(response)) => response,
            Ok(None) => break,
            // An answer out of contract arrived whole: this terminal stops following, as below.
            Err(error) => {
                let answer = error.downcast::<UnexpectedAnswer>()?;
                diagnostic_events::host_unexpected(
                    Site::Control,
                    answer.kind,
                    diagnostic_events::Anomaly::Answer,
                );
                host.host_colors.borrow_mut().remove(key);
                break;
            }
        };
        let envelope = decode_transport_envelope(&response)?;
        if matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        ) {
            host.host_colors.borrow_mut().remove(key);
            break;
        }
        let result = match decode_theme_control(envelope.kind, envelope.role, envelope.payload) {
            Ok(ThemeControl::TerminalThemeResult(result)) if result.terminal_id == terminal_id => {
                result
            }
            _ => {
                diagnostic_events::host_unexpected(
                    Site::Control,
                    envelope.kind,
                    diagnostic_events::Anomaly::Answer,
                );
                host.host_colors.borrow_mut().remove(key);
                break;
            }
        };
        host.host_colors
            .borrow_mut()
            .commit(key, result.current.clone());
        if result.transition != TerminalThemeTransition::Stale {
            break;
        }
        state = result.current;
    }
    Ok(Outcome::HostColorsAccepted)
}
