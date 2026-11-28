//! Behavior of the Git engine against real temporary repositories and the installed Git.

mod blame;
// The private folders a data root needs are admitted beside the temporary folder on Windows only,
// as the Workbench Client's device record tests are.
#[cfg(windows)]
mod consents;
mod discovery;
mod engine;
pub(super) mod fixture;
mod history;
mod mutations;
mod reads;
mod relay;
mod remote;
mod serve;
mod status;
