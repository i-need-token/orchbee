#[cfg(target_os = "linux")]
mod linux_output;

#[cfg(windows)]
pub(crate) mod windows_endpoints;
