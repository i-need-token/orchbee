//! Shared activation generations bind catalog admission, direct-child registration and clean-stop publication.
#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub use unsupported::RetainedSessionRecord;
#[cfg(windows)]
pub use windows::RetainedSessionRecord;
#[cfg(windows)]
pub(crate) use windows::{
    CHECKPOINT_DIRECTORY, CatalogRecord, Directory, Lifecycle, Record, checkpoint_path,
    supports_scope,
};
