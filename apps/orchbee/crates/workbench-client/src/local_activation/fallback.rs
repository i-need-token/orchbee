//! What activation records for the fallback after a failed move: the start of each generation it
//! publishes, just before its Server starts, and that generation's failure when its Server cannot
//! start, ends before its endpoint appears, or is later proved dead by crash recovery. A Server
//! still running at the deadline counts nothing, and neither does a generation that never started
//! its Server, since its start was never recorded. Each record is written synchronously on its
//! path, within the recorder's 2 s lock bound and only while a move counts; the recorder owns every
//! other rule.
use orchbee_local_runtime::{FallbackEvent, FallbackRecorder, activation_text};

/// What activation saw of one generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Seen {
    /// `begin_activation` published it, and its Server starts next.
    Started,
    /// Its Server could not be started, or ended before its endpoint appeared.
    FailedToStart,
    /// Crash recovery proved it dead.
    Crashed,
}

/// Hands `record` the event that `seen` makes of the generation `activation`.
pub(super) fn observe(activation: &[u8; 16], seen: Seen, record: impl FnOnce(FallbackEvent<'_>)) {
    let text = activation_text(activation);
    let activation = text.as_str();
    record(match seen {
        Seen::Started => FallbackEvent::SessionStart { activation },
        Seen::FailedToStart | Seen::Crashed => FallbackEvent::SessionFailure { activation },
    });
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
