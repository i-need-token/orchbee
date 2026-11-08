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
    CatalogRecord, Directory, Lifecycle, RECORD_FILES, Record, is_record_guard,
};
