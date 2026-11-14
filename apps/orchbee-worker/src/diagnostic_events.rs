//! The Worker's diagnostic log records (ORC-397): its own start and end, and each Terminal Host
//! start it attempts, so Host diagnostics reach the log through the Worker. A failure carries only
//! closed tokens and numbers: its error kind and, for a Host start, its stage and codes; no record
//! holds terminal content, input, launch commands or paths.
use orchbee_local_runtime::{LogField, LogLevel, LogValue, log_event};
use std::{error::Error, io, time::Duration};

pub(crate) fn started() {
    log_event(
        LogLevel::Info,
        "started",
        &[("version", LogValue::Token(env!("CARGO_PKG_VERSION")))],
    );
}

pub(crate) fn exited(outcome: &Result<(), Box<dyn Error>>) {
    match outcome {
        Ok(()) => log_event(
            LogLevel::Info,
            "exited",
            &[("outcome", LogValue::Token("ok"))],
        ),
        Err(error) => log_event(
            LogLevel::Error,
            "exited",
            &[
                ("outcome", LogValue::Token("failed")),
                error_kind(error.as_ref()),
            ],
        ),
    }
}

pub(crate) fn host_started() {
    log_event(LogLevel::Info, "host-started", &[]);
}

/// Names where a Host start failed: its `stage` token, the operating-system error code `error`
/// carries, the Host's exit code when it had exited, and the time since the start began.
pub(crate) fn host_start_failed(
    stage: &'static str,
    error: &(dyn Error + 'static),
    host_exit: Option<u32>,
    elapsed: Duration,
) {
    let mut fields = vec![
        error_kind(error),
        ("stage", LogValue::Token(stage)),
        (
            "elapsedMs",
            LogValue::Count(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)),
        ),
    ];
    if let Some(code) = os_error(error) {
        fields.push(("osError", LogValue::Count(u64::from(code))));
    }
    if let Some(code) = host_exit {
        fields.push(("hostExit", LogValue::Count(u64::from(code))));
    }
    log_event(LogLevel::Error, "host-start-failed", &fields);
}

/// The first operating-system error code in `error`'s chain, through wrapped I/O errors and
/// sources, as the unsigned value the system reports.
fn os_error(error: &(dyn Error + 'static)) -> Option<u32> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(error) = error.downcast_ref::<io::Error>() {
            if let Some(code) = error.raw_os_error() {
                return Some(u32::from_ne_bytes(code.to_ne_bytes()));
            }
            if let Some(inner) = error.get_ref() {
                current = Some(inner);
                continue;
            }
        }
        current = error.source();
    }
    None
}

fn error_kind(error: &(dyn Error + 'static)) -> LogField {
    (
        "error",
        error
            .downcast_ref::<io::Error>()
            .map_or(LogValue::Token("other"), |error| error.kind().into()),
    )
}
