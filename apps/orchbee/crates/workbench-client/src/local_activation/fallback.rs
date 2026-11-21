//! What activation records for the fallback after a failed move: the start of each generation it
//! publishes, just before its Server starts, and that generation's failure when its Server cannot
//! start, ends before its endpoint appears, or is later proved dead by crash recovery without
//! having recorded an `exit`. A Server still running at the deadline counts nothing, and neither
//! does a generation that never started its Server, since its start was never recorded. One
//! exception comes before any generation: a version whose own files fail the cohort check as
//! damaged can start no Server at all, so the activation records a fresh generation's start and
//! its failure at once. Each record is written synchronously on its path, within the recorder's 2 s
//! lock bound and only while a move counts; the recorder owns every other rule.
use orchbee_local_runtime::{FallbackEvent, FallbackRecorder, activation_text};
use std::io;

/// What activation saw of one generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Seen {
    /// `begin_activation` published it, and its Server starts next.
    Started,
    /// Its Server could not be started, or ended before its endpoint appeared.
    FailedToStart,
    /// Crash recovery proved it dead.
    Crashed,
    /// This version's own files failed the cohort check as [`damaged`], so no Server could start
    /// from them: a generation that starts and fails at once, before anything is published.
    Damaged,
}

/// Whether a cohort check that failed with `kind` found this version's files damaged: one changed,
/// truncated or unreadable as a release file (`InvalidData`), or missing (`NotFound`). A deadline or
/// a refused open says nothing about the version and counts nothing.
pub(super) fn damaged(kind: io::ErrorKind) -> bool {
    matches!(kind, io::ErrorKind::InvalidData | io::ErrorKind::NotFound)
}

/// Hands `record` the events that `seen` makes of the generation `activation`, in order.
pub(super) fn observe(
    activation: &[u8; 16],
    seen: Seen,
    mut record: impl FnMut(FallbackEvent<'_>),
) {
    let text = activation_text(activation);
    let activation = text.as_str();
    match seen {
        Seen::Started => record(FallbackEvent::SessionStart { activation }),
        Seen::FailedToStart | Seen::Crashed => {
            record(FallbackEvent::SessionFailure { activation });
        }
        Seen::Damaged => {
            record(FallbackEvent::SessionStart { activation });
            record(FallbackEvent::SessionFailure { activation });
        }
    }
}

/// Records `event` for the store this process runs from; a process outside a store version folder
/// records nothing.
pub(super) fn record(event: FallbackEvent<'_>) {
    if let Some(recorder) = FallbackRecorder::for_current_executable() {
        recorder.record(event);
    }
}

#[cfg(test)]
mod tests;
