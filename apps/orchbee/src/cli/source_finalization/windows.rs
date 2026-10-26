//! Windows cancellation retains the original console guard until the emit finalizer releases it.
#[must_use = "the owed finalization must be released by the common emit finalizer"]
pub(crate) struct SourceFinalization {
    interrupt: crate::windows_signal::ControlInterrupt,
}

impl SourceFinalization {
    pub(in crate::cli) fn new(interrupt: crate::windows_signal::ControlInterrupt) -> Self {
        Self { interrupt }
    }
    pub(crate) fn release(self) -> std::io::Result<()> {
        drop(self.interrupt);
        Ok(())
    }
}
