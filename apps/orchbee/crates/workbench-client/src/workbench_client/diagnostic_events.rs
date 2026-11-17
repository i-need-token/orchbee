//! The Workbench Client's diagnostic log records: its connection retiring on a failure, so a
//! disconnect leaves a trace, and one request's fault or one refused projection record that the
//! Client contained to its Panel. The retirement names the failure, the kind of command it
//! interrupted and the source location that closed the mailbox, by directory, file stem and line; a
//! contained fault names the command and the answer, and a contained record its family, validation
//! stage and containment. None holds terminal content, input or paths.
use super::mailbox::ClosedSite;
use crate::workbench_state::CarrierFailure;
use orchbee_client_server_protocol::{
    ControlRejectedReason, ControlUnavailableReason, TerminalControlOutcome, TerminalViewerOutcome,
    ViewerRejectedReason, ViewerUnavailableReason,
};
use orchbee_local_runtime::{LogLevel, LogValue, log_event};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A command whose fault the Client contained to one Panel.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ContainedCommand {
    Input,
    Resize,
    HostColors,
    /// A terminal poll the Server refused, which releases the viewer.
    Poll,
    /// Frame feedback the Server refused, which releases the viewer.
    FrameFeedback,
    /// The demand pair that asks for a fresh frame after a takeover.
    ControlFrame,
    Clear,
    /// A projection record the Client refused and contained to its Panel.
    ProjectionRecord,
}

impl ContainedCommand {
    const fn token(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Resize => "resize",
            Self::HostColors => "host-colors",
            Self::Poll => "poll",
            Self::FrameFeedback => "frame-feedback",
            Self::ControlFrame => "control-frame",
            Self::Clear => "clear",
            Self::ProjectionRecord => "projection-record",
        }
    }
}

/// Records that `command` got `outcome` and the Client dropped the command alone, keeping the
/// connection and every Panel. Each command and answer pair writes at most once a minute, with the
/// number of records it suppressed since, so a refused key per keystroke cannot flood the log.
pub(super) fn fault_contained(command: ContainedCommand, outcome: &TerminalControlOutcome) {
    record(command, control_answer_token(outcome));
}

/// Records that a viewer request for `command` got `outcome` and the Client contained it to the
/// Panel, under the same once-a-minute bound.
pub(super) fn viewer_fault_contained(command: ContainedCommand, outcome: &TerminalViewerOutcome) {
    record(command, viewer_answer_token(outcome));
}

fn record(command: ContainedCommand, answer: &'static str) {
    let Some(suppressed) = admit_once_a_minute(command, answer) else {
        return;
    };
    log_event(
        LogLevel::Warning,
        "client-fault-contained",
        &[
            ("command", LogValue::Token(command.token())),
            ("answer", LogValue::Token(answer)),
            ("suppressed", LogValue::Count(suppressed)),
        ],
    );
}

/// What the Client did with one projection record it refused.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum RecordContainment {
    /// The record was dropped alone; a program effect never touches the frame.
    Dropped,
    /// The record was dropped and the Panel's frame invalidated until a snapshot replaces it.
    Resynced,
    /// The Panel alone stopped updating: past its resync bound, or on a record it could not read.
    Stopped,
}

impl RecordContainment {
    const fn token(self) -> &'static str {
        match self {
            Self::Dropped => "dropped",
            Self::Resynced => "resynced",
            Self::Stopped => "stopped",
        }
    }
}

/// Records the class of a projection record the Client refused (the record family and the
/// validation stage) and how it contained the refusal to the Panel. A stop always writes, so every
/// stopped Panel's class stays in the log; drops and resyncs share the once-a-minute bound per stage.
pub(super) fn record_contained(
    operation: &'static str,
    stage: &'static str,
    containment: RecordContainment,
) {
    let suppressed = if containment == RecordContainment::Stopped {
        0
    } else {
        match admit_once_a_minute(ContainedCommand::ProjectionRecord, stage) {
            Some(suppressed) => suppressed,
            None => return,
        }
    };
    log_event(
        LogLevel::Warning,
        "client-record-contained",
        &[
            ("operation", LogValue::Token(operation)),
            ("stage", LogValue::Token(stage)),
            ("containment", LogValue::Token(containment.token())),
            ("suppressed", LogValue::Count(suppressed)),
        ],
    );
}

const fn control_answer_token(outcome: &TerminalControlOutcome) -> &'static str {
    match outcome {
        TerminalControlOutcome::Rejected { reason } => match reason {
            ControlRejectedReason::ViewerUnavailable => "viewer-unavailable",
            ControlRejectedReason::ControllerBusy => "controller-busy",
            ControlRejectedReason::NotController => "not-controller",
            ControlRejectedReason::SequenceMismatch => "sequence-mismatch",
            ControlRejectedReason::HostRejected => "host-rejected",
            ControlRejectedReason::HostClearAlternateScreen => "host-clear-alternate-screen",
            ControlRejectedReason::HostInputInvalid => "host-input-invalid",
            ControlRejectedReason::HostInputLimitExceeded => "host-input-limit-exceeded",
            ControlRejectedReason::HostInputBackpressure => "host-input-backpressure",
            ControlRejectedReason::HostInputUnavailable => "host-input-unavailable",
        },
        TerminalControlOutcome::Unavailable { reason } => match reason {
            ControlUnavailableReason::ServerBusy => "server-busy",
            ControlUnavailableReason::TerminalRuntimeUnavailable => "terminal-runtime-unavailable",
        },
        TerminalControlOutcome::OutcomeUnknown {} => "outcome-unknown",
        TerminalControlOutcome::Observed { .. }
        | TerminalControlOutcome::Acquired { .. }
        | TerminalControlOutcome::InputAccepted {}
        | TerminalControlOutcome::Resized { .. }
        | TerminalControlOutcome::Pasted { .. }
        | TerminalControlOutcome::HostColorsAccepted {}
        | TerminalControlOutcome::Cleared { .. }
        | TerminalControlOutcome::ResetAccepted { .. } => "unexpected-answer",
    }
}

const fn viewer_answer_token(outcome: &TerminalViewerOutcome) -> &'static str {
    match outcome {
        TerminalViewerOutcome::Rejected { reason } => match reason {
            ViewerRejectedReason::AlreadyAttached => "already-attached",
            ViewerRejectedReason::TerminalUnavailable => "terminal-unavailable",
            ViewerRejectedReason::FeedbackMismatch => "feedback-mismatch",
            ViewerRejectedReason::HostRejected => "host-rejected",
        },
        TerminalViewerOutcome::Unavailable { reason } => match reason {
            ViewerUnavailableReason::ServerBusy => "server-busy",
            ViewerUnavailableReason::TerminalRuntimeUnavailable => "terminal-runtime-unavailable",
            ViewerUnavailableReason::WorkbenchUnavailable => "workbench-unavailable",
        },
        TerminalViewerOutcome::RowsRejected { .. } => "rows-rejected",
        TerminalViewerOutcome::Attached { .. }
        | TerminalViewerOutcome::FeedbackAccepted {}
        | TerminalViewerOutcome::Detached {}
        | TerminalViewerOutcome::Empty {}
        | TerminalViewerOutcome::Record(_) => "unexpected-answer",
    }
}

/// One command and answer pair's last record and the records suppressed since.
struct Slot {
    command: ContainedCommand,
    answer: &'static str,
    last: Instant,
    suppressed: u64,
}

/// The pairs are a closed set of a few dozen; a pair past this bound is never recorded.
const SLOT_LIMIT: usize = 64;

/// Admits the pair's record at most once a minute and returns how many of its records were
/// suppressed since the last one admitted.
fn admit_once_a_minute(command: ContainedCommand, answer: &'static str) -> Option<u64> {
    static SLOTS: Mutex<Vec<Slot>> = Mutex::new(Vec::new());
    let now = Instant::now();
    let mut slots = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(slot) = slots
        .iter_mut()
        .find(|slot| slot.command == command && slot.answer == answer)
    {
        if now.saturating_duration_since(slot.last) < Duration::from_secs(60) {
            slot.suppressed = slot.suppressed.saturating_add(1);
            return None;
        }
        slot.last = now;
        return Some(std::mem::take(&mut slot.suppressed));
    }
    if slots.len() >= SLOT_LIMIT {
        return None;
    }
    slots.push(Slot {
        command,
        answer,
        last: now,
        suppressed: 0,
    });
    Some(0)
}

/// Records the failure that retired the connection, the command the worker held then, and where
/// the mailbox closed: the mailbox check that hit the failure, or the Client step that decided it. A
/// process exit's close writes nothing.
pub(super) fn connection_retired(failure: CarrierFailure, site: ClosedSite) {
    let Some(failure) = failure_token(failure) else {
        return;
    };
    let (directory, file) = source(site.location.file());
    let mut fields = vec![("failure", LogValue::Token(failure))];
    if let Some(command) = site.command {
        fields.push(("command", LogValue::Token(command)));
    }
    fields.extend([
        ("sourceDirectory", LogValue::Token(directory)),
        ("sourceFile", LogValue::Token(file)),
        ("line", LogValue::Count(u64::from(site.location.line()))),
    ]);
    log_event(LogLevel::Error, "client-retired", &fields);
}

const fn failure_token(failure: CarrierFailure) -> Option<&'static str> {
    Some(match failure {
        CarrierFailure::LaunchRequestTooLarge => "launch-request-too-large",
        CarrierFailure::AuthorityDivergence => "authority-divergence",
        CarrierFailure::Deadline => "deadline",
        CarrierFailure::IdentityExhausted => "identity-exhausted",
        CarrierFailure::OutcomeUnknown => "outcome-unknown",
        CarrierFailure::Protocol => "protocol",
        CarrierFailure::Transport => "transport",
        CarrierFailure::WorkerExited => "worker-exited",
        CarrierFailure::Closed => return None,
    })
}

/// The parent directory and stem of a source file, which the log grammar admits as tokens where it
/// would refuse the path.
fn source(path: &'static str) -> (&'static str, &'static str) {
    let mut parts = path.rsplit(['/', '\\']);
    let file = parts.next().unwrap_or(path);
    let directory = parts.next().unwrap_or_default();
    (directory, file.strip_suffix(".rs").unwrap_or(file))
}
