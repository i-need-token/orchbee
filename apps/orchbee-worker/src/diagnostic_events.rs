//! The Worker's diagnostic log records (ORC-397): its own start and end, each Terminal Host start it
//! attempts, so Host diagnostics reach the log through the Worker, and its removal of day-old editor
//! documents. A failure carries only its error kind; no record holds terminal content, input,
//! launch commands or paths.
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

/// Records a removal pass over the editor document folder only when it removed a document or
/// failed to remove one.
#[cfg(windows)]
pub(crate) fn editor_documents_removed(removal: orchbee_local_runtime::EditorDocumentRemoval) {
    if removal.removed == 0 && removal.failed == 0 {
        return;
    }
    log_event(
        if removal.failed == 0 {
            LogLevel::Info
        } else {
            LogLevel::Warning
        },
        "editor-documents-removed",
        &[
            ("removed", LogValue::Count(removal.removed)),
            ("failed", LogValue::Count(removal.failed)),
        ],
    );
}

#[cfg(windows)]
pub(crate) fn editor_document_removal_failed(error: &io::Error) {
    log_event(
        LogLevel::Warning,
        "editor-document-removal-failed",
        &[("error", error.kind().into())],
    );
}

fn error_kind(error: &(dyn Error + 'static)) -> LogField {
    (
        "error",
        error
            .downcast_ref::<io::Error>()
            .map_or(LogValue::Token("other"), |error| error.kind().into()),
    )
}
