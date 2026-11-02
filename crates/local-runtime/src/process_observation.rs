//! Exact registered-process observations are supplied by the Windows process adapter.
mod windows;
pub(crate) use windows::{ProcessSnapshot, current_identity, identity, role_absent};
