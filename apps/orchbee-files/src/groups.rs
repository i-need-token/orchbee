//! The report, on standard output, of each process group this process runs Git in, which the Worker
//! that started it reads to end the groups this process leaves running when it is killed. Every
//! other end settles the groups first; a killed process cannot.

use std::{
    io,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use orchbee_local_runtime::{
    CommandChild, GroupReport, LogLevel, LogValue, ReportedGroup, log_event,
};

/// The report, once the Server admitted this process; none in a process that never reports, such
/// as a test's.
static REPORT: OnceLock<GroupReport> = OnceLock::new();
/// A failure of the report was logged already.
static FAILED: AtomicBool = AtomicBool::new(false);

/// Starts the report, once the Server admitted this process and before any child starts.
pub(crate) fn install() {
    match GroupReport::standard_output() {
        // Only `main` installs it, once.
        Ok(report) => drop(REPORT.set(report)),
        Err(error) => failed(&error),
    }
}

/// Reports `child`'s group to the Worker until the returned guard drops, once its run settled it;
/// nothing before [`install`], for a child whose root already ended, or when the line cannot be
/// written.
pub(crate) fn report(child: &CommandChild) -> Option<ReportedGroup<'static>> {
    match REPORT.get()?.report(child) {
        Ok(reported) => reported,
        Err(error) => {
            failed(&error);
            None
        }
    }
}

/// Logs the first failure only: one that lasts, as once the Worker stopped reading, would repeat
/// for every command.
fn failed(error: &io::Error) {
    if !FAILED.swap(true, Ordering::Relaxed) {
        log_event(
            LogLevel::Warning,
            "group-report-failed",
            &[("error", LogValue::from(error.kind()))],
        );
    }
}
