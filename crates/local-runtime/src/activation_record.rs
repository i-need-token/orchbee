//! Activation receipt vocabulary and retained completion capability; process liveness belongs to the runtime owner.
/// Strict existing-file observation; Stopped is a receipt, not independent process-liveness proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationState {
    Absent,
    Active,
    Crashed { activation_id: [u8; 16] },
    Stopped { request_id: [u8; 16] },
}

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub use unsupported::RetainedActivationRecord;
#[cfg(windows)]
pub use windows::RetainedActivationRecord;
#[cfg(windows)]
pub(crate) use windows::{crash_matches, publish_crashed, read};
