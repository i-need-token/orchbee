//! Unix provider process lookup is unsupported and never launches a probe.
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};
pub(super) fn directory() -> std::io::Result<PathBuf> {
    Err(
        crate::tui_capabilities::Unsupported(crate::tui_capabilities::Capability::AgentLaunch)
            .into(),
    )
}
pub(super) fn query(
    _executable: &Path,
    _system: &Path,
    _path: Option<&OsStr>,
    _name: &str,
    _cancelled: &AtomicBool,
    _run_deadline: Instant,
    _slot_deadline: Instant,
) -> Result<bool, ()> {
    Err(())
}
