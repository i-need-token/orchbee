mod client_view;
mod invocation;
#[cfg(unix)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the accepted private foundations are staged before TUI and activation integration"
    )
)]
mod local_client;
#[cfg(target_os = "linux")]
mod local_startup;
mod localization;
mod presentation;
#[cfg(unix)]
mod terminal_device;
#[cfg(unix)]
mod terminal_input;
#[cfg(unix)]
mod terminal_lifecycle;
#[cfg(unix)]
mod terminal_projection;
#[cfg(unix)]
mod terminal_session;
#[cfg(unix)]
mod terminal_text;
#[cfg(unix)]
mod termination_signal;
#[cfg(unix)]
mod tui_backend;
#[cfg(unix)]
mod tui_layout;
#[cfg(unix)]
mod tui_render;
#[cfg(unix)]
mod tui_shell;

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use invocation::parse_invocation;
use localization::select_locale;
use presentation::{ProcessResult, present};

// Keep the additive shell integration seam type-checked without making it an invocation route.
#[cfg(unix)]
const _: fn(localization::Locale) -> Result<(), tui_shell::TuiFailure> = tui_shell::run;

#[cfg(unix)]
const _: fn(
    localization::Locale,
    local_client::LocalClient,
    terminal_session::TerminalIntent,
) -> Result<(), tui_shell::TuiFailure> = tui_shell::run_connected;

fn main() -> ExitCode {
    let locale = select_locale(|name| env::var_os(name));
    let invocation = parse_invocation(env::args_os().skip(1));
    #[cfg(target_os = "linux")]
    if invocation == Ok(invocation::Invocation::Tui) {
        return match local_startup::run(locale) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => emit(ProcessResult {
                status: 1,
                stdout: "",
                stderr: locale.startup_failure(failure),
            }),
        };
    }
    emit(present(invocation, locale))
}

fn emit(result: ProcessResult) -> ExitCode {
    if write_stream(io::stdout().lock(), result.stdout).is_err()
        || write_stream(io::stderr().lock(), result.stderr).is_err()
    {
        return ExitCode::FAILURE;
    }

    ExitCode::from(result.status)
}

fn write_stream(mut writer: impl Write, payload: &str) -> io::Result<()> {
    if payload.is_empty() {
        return Ok(());
    }
    writer.write_all(payload.as_bytes())?;
    writer.flush()
}
