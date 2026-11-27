//! Behavior of the Git engine against real temporary repositories and the installed Git.

// The private folders a data root needs are admitted beside the temporary folder on Windows only,
// as the Workbench Client's device record tests are.
#[cfg(windows)]
mod consents;
mod discovery;
mod fixture;
mod status;
