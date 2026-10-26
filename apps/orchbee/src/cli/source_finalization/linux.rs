//! Linux source cancellation holds the inherited mask through output, then restores it exactly.
#[must_use = "the owed finalization must be released by the common emit finalizer"]
pub(crate) struct SourceFinalization {
    old_mask: nix::sys::signal::SigSet,
    #[cfg(test)]
    fail_release: bool,
}

impl SourceFinalization {
    pub(crate) fn new(old_mask: nix::sys::signal::SigSet) -> Self {
        Self {
            old_mask,
            #[cfg(test)]
            fail_release: false,
        }
    }

    /// A token whose `release` always reports a kernel failure — mock
    /// evidence for the emit finalizer's restore-failure routing.
    #[cfg(test)]
    pub(crate) fn failing() -> Self {
        Self {
            old_mask: nix::sys::signal::SigSet::empty(),
            fail_release: true,
        }
    }

    /// Restore the captured thread mask exactly; a failure is a terminal
    /// internal failure — reported by the caller, never retried.
    pub(crate) fn release(self) -> Result<(), nix::errno::Errno> {
        #[cfg(test)]
        if self.fail_release {
            return Err(nix::errno::Errno::EIO);
        }
        self.old_mask
            .thread_swap_mask(nix::sys::signal::SigmaskHow::SIG_SETMASK)
            .map(drop)
    }
}
