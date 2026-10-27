//! Serves one bounded stdio exchange for device preferences or read-only system-shell
//! discovery through shared runtime capabilities. It starts no Session or shell; the parent
//! owns executable admission, exchange deadlines, cancellation, and reaping.

use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    MAXIMUM_USER_PREFERENCES_BYTES, PreferencesFailure, UserPreferencesResponse,
    parse_user_preferences_request,
};
use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
};

fn serve() -> UserPreferencesResponse {
    let arguments: Vec<_> = env::args_os().skip(1).take(2).collect();
    if arguments.len() != 1 || arguments[0] != "--stdio-v1" || io::stdin().is_terminal() {
        return refused(PreferencesFailure::InvalidRequest);
    }
    let mut bytes = Vec::new();
    if io::stdin()
        .take(MAXIMUM_USER_PREFERENCES_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return refused(PreferencesFailure::InvalidRequest);
    }
    let request = match parse_user_preferences_request(&bytes) {
        Ok(request) => request,
        Err(reason) => return refused(reason),
    };
    let scope = match UserPreferencesScope::from_environment(|name| env::var_os(name)) {
        Ok(scope) => scope,
        Err(reason) => return refused(reason),
    };
    UserPreferencesStore::new(scope).execute(&request)
}
fn refused(reason: PreferencesFailure) -> UserPreferencesResponse {
    UserPreferencesResponse::Refused { reason }
}
fn main() -> ExitCode {
    if env::args_os().skip(1).collect::<Vec<_>>() == ["--system-shell-v1"] {
        return serve_system_shell();
    }
    let response = serve();
    let Ok(mut bytes) = serde_json::to_vec(&response) else {
        return ExitCode::from(1);
    };
    if bytes.len() > MAXIMUM_USER_PREFERENCES_BYTES {
        return ExitCode::from(1);
    }
    bytes.push(b'\n');
    if io::stdout().lock().write_all(&bytes).is_err() {
        return ExitCode::from(1);
    }
    match response {
        UserPreferencesResponse::Snapshot { .. } => ExitCode::SUCCESS,
        UserPreferencesResponse::Refused { .. } | UserPreferencesResponse::OutcomeUnknown {} => {
            ExitCode::from(1)
        }
    }
}

fn serve_system_shell() -> ExitCode {
    let mut request = Vec::new();
    if io::stdin().is_terminal()
        || io::stdin().take(3).read_to_end(&mut request).is_err()
        || request != b"{}"
    {
        return ExitCode::from(1);
    }
    let executable = orchbee_local_runtime::resolve_default_interactive_launch()
        .ok()
        .map(|launch| launch.executable);
    let Ok(mut bytes) = serde_json::to_vec(&executable) else {
        return ExitCode::from(1);
    };
    bytes.push(b'\n');
    if io::stdout().lock().write_all(&bytes).is_err() || executable.is_none() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
