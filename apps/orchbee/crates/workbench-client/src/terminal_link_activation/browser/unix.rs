//! Unix browser process dispatch is unsupported.
pub(super) fn open(_url: &str) -> Result<(), ()> {
    Err(())
}
