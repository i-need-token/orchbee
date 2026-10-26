//! Platform-neutral command settlement vocabulary; execution adapters own process effects.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowsCommandFailure {
    ExitFailure,
    SpawnUnavailable,
    SpawnFailed,
    Io,
    Cancelled,
    Timeout,
    PhysicalUnknown,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsCommandOutput {
    pub exit_code: Option<i32>,
    pub failure: Option<WindowsCommandFailure>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}
