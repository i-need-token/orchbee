//! No unavailable source adapter can construct an owed native finalization.
#[must_use = "the owed finalization must be released by the common emit finalizer"]
pub(crate) struct SourceFinalization {}

impl SourceFinalization {
    pub(crate) fn release(self) -> Result<(), std::convert::Infallible> {
        Ok(())
    }
}
