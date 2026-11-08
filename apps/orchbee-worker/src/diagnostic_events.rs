//! The Worker's diagnostic log records (ORC-397): its own start and end, and each Terminal Host
//! start it attempts, so Host diagnostics reach the log through the Worker. A failure carries only
//! its error kind; no record holds terminal content, input, launch commands or paths.
use orchbee_local_runtime::{LogField, LogLevel, LogValue, log_event};
use std::{error::Error, io};

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

pub(crate) fn host_start_failed(error: &(dyn Error + 'static)) {
    log_event(LogLevel::Error, "host-start-failed", &[error_kind(error)]);
}

fn error_kind(error: &(dyn Error + 'static)) -> LogField {
    (
        "error",
        error
            .downcast_ref::<io::Error>()
            .map_or(LogValue::Token("other"), |error| error.kind().into()),
    )
}
