//! Direct-child registration is a shared lifecycle capability; platform adapters own suspended process creation.
/// A managed process registers only the directly created child owned by its role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionChild {
    Worker,
    Host,
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::spawn_registered;
#[cfg(windows)]
pub use windows::spawn_session_child;
#[cfg(not(windows))]
pub fn spawn_session_child(
    _root: &crate::DataRoot,
    _runtime: &std::path::Path,
    _child: SessionChild,
    _executable: &std::path::Path,
    _arguments: &[&std::ffi::OsStr],
    _deadline: std::time::Instant,
) -> std::io::Result<crate::RetainedChild> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// POSIX registers no process identities, so it creates no registered child and refuses before any
/// effect.
#[cfg(not(windows))]
pub(crate) fn spawn_registered(
    _directory: &crate::durable_sessions::Directory,
    _scope: &str,
    _role: crate::process_registration::ProcessRole,
    _executable: &std::path::Path,
    _arguments: &[&std::ffi::OsStr],
    _deadline: std::time::Instant,
) -> std::io::Result<crate::RetainedChild> {
    Err(std::io::ErrorKind::Unsupported.into())
}
