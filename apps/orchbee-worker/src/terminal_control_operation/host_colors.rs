//! Applies the controlling Client's effective query colors to its terminal, independently of the
//! launch's display mode. Unknown outer ANSI colors keep the current fallback.
use super::{
    ConnectionControl, CorrelationId, Error, HostBrokerConnection, Outcome, PayloadEncoding,
    TerminalId, UnixStream, decode_connection_control, decode_transport_envelope, exchange,
    fresh_identity, invalid,
};
use orchbee_capability_node_protocol::{HostDefaultColors, HostTerminalId};
use orchbee_local_runtime::UserTerminalColors;
use orchbee_terminal_protocol::{
    ApplyTerminalTheme, TerminalThemeOperationId, TerminalThemeTransition, ThemeControl,
    decode_theme_control, encode_theme_control,
};

/// Best effort: a rejected or exited target stops following and keeps its current colors.
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
        let response = exchange(
            host,
            server,
            CorrelationId::from_bytes(fresh_identity()?)?,
            command.kind,
            &command.payload,
            PayloadEncoding::ControlJson,
        )?;
        let envelope = decode_transport_envelope(&response)?;
        if matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        ) {
            host.host_colors.borrow_mut().remove(key);
            break;
        }
        let ThemeControl::TerminalThemeResult(result) =
            decode_theme_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            return Err(invalid("Host did not answer the theme update"));
        };
        if result.terminal_id != terminal_id {
            return Err(invalid("Host theme result names another terminal"));
        }
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
