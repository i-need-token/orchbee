//! The Client-failure helper `--record-client-failure-v1 <version>`, which either launcher runs
//! through the previous version's Client after a Client of `<version>` failed to start or to join.
//! It records one Client failure of `<version>` in the store it runs from, counted only while
//! `<version>` is the entry of the counting move, and that write may commit the fallback. It opens
//! no data root, writes nothing on either stream and always exits 0, so a launcher only waits for
//! it. A build that does not run automatic updates does not know it. Its grammar never changes;
//! help, completion and the Agent guide leave it out.
use super::Output;
use orchbee_local_runtime::{FallbackEvent, FallbackRecorder};

/// Runs the helper. A Client outside a store version folder records nothing.
pub(crate) fn run(version: &str) -> Output {
    record(version, |event| {
        if let Some(recorder) = FallbackRecorder::for_current_executable() {
            recorder.record(event);
        }
    });
    Output {
        status: 0,
        stdout: Vec::new(),
        stderr: String::new(),
        finalization: None,
    }
}

/// Hands `recorder` the one event the helper records.
fn record(version: &str, recorder: impl FnOnce(FallbackEvent<'_>)) {
    recorder(FallbackEvent::ClientFailure { version });
}

#[cfg(test)]
mod tests;
