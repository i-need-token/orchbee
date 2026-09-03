mod invocation;
#[cfg(unix)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the local Site rendezvous primitive is staged before activation integration"
    )
)]
mod local_site_rendezvous;
mod localization;
mod presentation;

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use invocation::parse_invocation;
use localization::select_locale;
use presentation::present;

fn main() -> ExitCode {
    let locale = select_locale(|name| env::var_os(name));
    let result = present(parse_invocation(env::args_os().skip(1)), locale);

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
