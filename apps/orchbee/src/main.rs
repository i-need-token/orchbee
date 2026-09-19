mod cli;
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
mod startup_wait;
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
mod tui_pointer;
#[cfg(unix)]
mod tui_render;
#[cfg(unix)]
mod tui_shell;
#[cfg(unix)]
mod tui_theme;

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use invocation::{Invocation, parse_request};
use localization::{Locale, select_locale};
use presentation::{ProcessResult, present};

// Keep the additive shell integration seam type-checked without making it an invocation route.
#[cfg(unix)]
const _: fn(localization::Locale, bool) -> Result<(), tui_shell::TuiFailure> = tui_shell::run;

fn main() -> ExitCode {
    let locale = select_locale(|name| env::var_os(name));
    let parsed = parse_request(env::args_os().skip(1));
    #[cfg(target_os = "linux")]
    if let Ok(invocation::Request::Tui { mouse }) = &parsed {
        return match local_startup::run(locale, *mouse) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => emit(
                cli::process_result(
                    ProcessResult {
                        status: 1,
                        stdout: "",
                        stderr: locale.startup_failure(failure),
                    },
                    locale,
                ),
                locale,
            ),
        };
    }
    emit(
        match parsed {
            Err(failure) => cli::parse_failure(failure, locale),
            Ok(invocation::Request::Tui { .. }) => {
                cli::process_result(present(Ok(Invocation::Tui), locale), locale)
            }
            Ok(invocation::Request::Version) => {
                cli::process_result(present(Ok(Invocation::Version), locale), locale)
            }
            Ok(invocation::Request::RootHelp) => {
                cli::process_result(present(Ok(Invocation::Help), locale), locale)
            }
            Ok(invocation::Request::Help(topic)) => cli::help(topic, locale),
            Ok(invocation::Request::Completion(shell)) => cli::completion(shell, locale),
            Ok(invocation::Request::Query { command, format }) => {
                cli::query(command, format, locale)
            }
            Ok(invocation::Request::Mutation { command, format }) => {
                cli::mutation(command, format, locale)
            }
        },
        locale,
    )
}

fn emit(result: cli::Output, locale: Locale) -> ExitCode {
    if write_stream(io::stdout().lock(), &result.stdout).is_err() {
        // The prepared document failed at the OS write: a localized diagnostic
        // on stderr is the only remaining channel; no second document follows.
        let _ = write_stream(io::stderr().lock(), locale.cli_output_failed().as_bytes());
        return ExitCode::FAILURE;
    }
    if write_stream(io::stderr().lock(), result.stderr.as_bytes()).is_err() {
        return ExitCode::FAILURE;
    }

    ExitCode::from(result.status)
}

fn write_stream(mut writer: impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.is_empty() {
        return Ok(());
    }
    writer.write_all(payload)?;
    writer.flush()
}
