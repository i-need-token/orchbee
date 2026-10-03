//! Single joined clipboard adapter; platform-specific ownership stays outside copy state.
#[cfg(not(windows))]
use super::terminal_copy::Job;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(super) use windows::Writer;

#[cfg(not(windows))]
#[derive(Default)]
pub(super) struct Writer;
#[cfg(not(windows))]
impl Writer {
    pub(super) fn submit(&mut self, job: Job) {
        drop(job);
    }
    pub(super) fn close(&mut self) {}
}
