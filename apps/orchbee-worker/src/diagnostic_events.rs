//! The Worker's diagnostic log records (ORC-397): its own start and end, each Terminal Host start it
//! attempts, so Host diagnostics reach the log through the Worker, its removal of day-old editor
//! documents, and each Server request it took a second or longer to answer. A failure carries only
//! closed tokens and numbers: its error kind and, for a Host start, its stage and codes; no record
//! holds terminal content, input, launch commands or paths.
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, TerminalControlAction, TerminalViewerAction,
};
use orchbee_local_runtime::{LogField, LogLevel, LogValue, log_event};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

/// The Worker serves one Server request at a time, and typing for every terminal it carries waits
/// behind the request in hand. A request held this long is recorded.
const SLOW_REQUEST: Duration = Duration::from_secs(1);

/// One Server request being answered: its kind, the input bytes it carries, and when it arrived.
pub(crate) struct RequestTiming {
    kind: &'static str,
    action: &'static str,
    bytes: u64,
    started: Instant,
}

impl RequestTiming {
    pub(crate) fn start(request: &CapabilityNodeMessage) -> Self {
        let (kind, action, bytes) = request_kind(request);
        Self {
            kind,
            action,
            bytes,
            started: Instant::now(),
        }
    }

    /// Records the request when answering it took `SLOW_REQUEST` or longer.
    pub(crate) fn finish(self) {
        let elapsed = self.started.elapsed();
        if elapsed < SLOW_REQUEST {
            return;
        }
        log_event(
            LogLevel::Warning,
            "request-slow",
            &[
                ("request", LogValue::Token(self.kind)),
                ("action", LogValue::Token(self.action)),
                ("inputBytes", LogValue::Count(self.bytes)),
                (
                    "milliseconds",
                    LogValue::Count(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)),
                ),
            ],
        );
    }
}

fn request_kind(request: &CapabilityNodeMessage) -> (&'static str, &'static str, u64) {
    match request {
        CapabilityNodeMessage::TerminalControlRequest(request) => {
            let (action, bytes) = match &request.action {
                TerminalControlAction::Input { bytes, .. } => (
                    "input",
                    u64::try_from(bytes.as_bytes().len()).unwrap_or(u64::MAX),
                ),
                TerminalControlAction::SemanticInput { .. } => ("semantic-input", 0),
                TerminalControlAction::Resize { .. } => ("resize", 0),
                TerminalControlAction::Paste { .. } => ("paste", 0),
                TerminalControlAction::HostColors { .. } => ("host-colors", 0),
                TerminalControlAction::Acquire
                | TerminalControlAction::Takeover { .. }
                | TerminalControlAction::Release => ("controller", 0),
                TerminalControlAction::Clear | TerminalControlAction::Reset => ("clear", 0),
            };
            ("control", action, bytes)
        }
        CapabilityNodeMessage::TerminalViewerRequest(request) => {
            let action = match &request.action {
                TerminalViewerAction::Attach { .. } => "attach",
                TerminalViewerAction::Poll => "poll",
                TerminalViewerAction::ReadProcessInfo => "read-process-info",
                TerminalViewerAction::ReadRows { .. }
                | TerminalViewerAction::ReadViewportRows { .. } => "read-rows",
                _ => "other",
            };
            ("viewer", action, 0)
        }
        CapabilityNodeMessage::TerminalSendRequest(_) => ("send", "none", 0),
        CapabilityNodeMessage::TerminalLifecycleRequest(_) => ("lifecycle", "none", 0),
        CapabilityNodeMessage::HostQueryRequest(_) => ("host-query", "none", 0),
        CapabilityNodeMessage::TerminalTitlesRequest(_)
        | CapabilityNodeMessage::TerminalTitleChangesRequest(_) => ("titles", "none", 0),
        CapabilityNodeMessage::AgentProbeJobRequest(_)
        | CapabilityNodeMessage::AgentIntegrationJobRequest(_)
        | CapabilityNodeMessage::AgentDrainRequest(_)
        | CapabilityNodeMessage::PluginJobRequest(_)
        | CapabilityNodeMessage::WorktreeJobRequest(_)
        | CapabilityNodeMessage::WorktreeMutationRequest(_) => ("job", "none", 0),
        _ => ("other", "none", 0),
    }
}

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
