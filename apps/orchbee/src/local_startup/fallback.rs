//! The TUI start that the fallback after a failed move counts: the first TUI of the moved version
//! opens the Client window. The record is written on a background thread, so it never delays the
//! TUI, and the TUI's process waits for that thread before it exits. The recorder owns every rule,
//! so a TUI outside a store version folder, or long after a move, records nothing and takes no
//! lock.
use orchbee_local_runtime::{FallbackEvent, FallbackRecorder};

/// Runs `tui` while `record` runs on its own thread, and waits for that thread before answering
/// `tui`'s outcome. When no thread can start, nothing is recorded.
pub(super) fn with_start_recorded<T>(
    record: impl FnOnce() + Send + 'static,
    tui: impl FnOnce() -> T,
) -> T {
    let recording = std::thread::Builder::new()
        .name("tui-start-record".into())
        .spawn(record)
        .ok();
    let outcome = tui();
    if let Some(recording) = recording {
        let _ = recording.join();
    }
    outcome
}

/// Records this TUI's start for the store it runs from.
pub(super) fn record_start() {
    if let Some(recorder) = FallbackRecorder::for_current_executable() {
        recorder.record(FallbackEvent::TuiStart);
    }
}

#[cfg(test)]
mod tests;
