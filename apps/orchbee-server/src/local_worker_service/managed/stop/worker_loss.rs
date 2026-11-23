//! The Worker loss that the fallback after a failed move counts: a Worker that exits while no stop
//! is recorded fails this Server's generation. It is recorded once, synchronously on the stop
//! loop, within the recorder's 2 s lock bound and only while a move counts; the recorder owns every
//! other rule, so a Server outside a store version folder records nothing.
use orchbee_local_runtime::{
    FallbackEvent, FallbackRecorder, RetainedSessionRecord, activation_text,
};

/// Whether the stop loop has seen its Worker exit.
#[derive(Default)]
pub(super) struct WorkerLoss {
    seen: bool,
}

impl WorkerLoss {
    /// One look of the stop loop: `exited` once the Worker has exited, `stopping` once a stop is
    /// recorded. The first look that sees the exit calls `record` when no stop is recorded; no
    /// later look calls it, since a Worker exits once.
    pub(super) fn look(&mut self, exited: bool, stopping: bool, record: impl FnOnce()) {
        if self.seen || !exited {
            return;
        }
        self.seen = true;
        if !stopping {
            record();
        }
    }

    /// The Worker ended because a restart released it to a successor, which fails nothing.
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) fn released(&mut self) {
        self.seen = true;
    }
}

/// Records the generation `session` retains as failed, for the store this Server runs from.
pub(super) fn record(session: &RetainedSessionRecord) {
    let Ok(activation) = session.current_activation_id() else {
        return;
    };
    if let Some(recorder) = FallbackRecorder::for_current_executable() {
        recorder.record(FallbackEvent::SessionFailure {
            activation: &activation_text(&activation),
        });
    }
}

#[cfg(test)]
mod tests;
