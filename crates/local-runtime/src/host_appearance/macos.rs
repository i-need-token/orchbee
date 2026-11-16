//! Read the current user's appearance without mutating it: `AppleInterfaceStyle` in the global
//! preferences domain names `Dark` while the system appears dark, including under the automatic
//! appearance, and is absent while it appears light. One bounded `/usr/bin/defaults` child reads
//! it, so no foreign-code boundary or new dependency is needed.
use crate::CommandOutput;
use std::{
    process::Command,
    time::{Duration, Instant},
};

const DEFAULTS: &str = "/usr/bin/defaults";
/// A slow preferences daemon must not hold the caller long; past it the appearance stays unknown.
const DEADLINE: Duration = Duration::from_millis(300);
/// `Dark`, or the absent-key message, fits well inside this.
const OUTPUT_LIMIT: usize = 512;

/// Reads `AppleInterfaceStyle`: `Some(true)` for Dark, `Some(false)` when the key is absent, the
/// light appearance, and `None` for every other answer or failure.
pub fn read_macos_dark_appearance() -> Option<bool> {
    let mut command = Command::new(DEFAULTS);
    command.args(["read", "-g", "AppleInterfaceStyle"]);
    let output = crate::run_unix_command(
        command,
        OUTPUT_LIMIT,
        Some(Instant::now() + DEADLINE),
        || Ok(false),
    );
    classify(&output)
}

fn classify(output: &CommandOutput) -> Option<bool> {
    if output.stdout_truncated || output.stderr_truncated {
        return None;
    }
    match (output.failure, output.exit_code) {
        (None, Some(0)) => (output.stdout.trim_ascii() == b"Dark").then_some(true),
        // `defaults` names the missing pair on standard error and exits with status 1.
        (Some(crate::CommandFailure::ExitFailure), Some(1))
            if output
                .stderr
                .windows(b"does not exist".len())
                .any(|window| window == b"does not exist") =>
        {
            Some(false)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
