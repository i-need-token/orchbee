#[cfg(target_os = "linux")]
mod linux_output;

#[cfg(windows)]
mod windows_conpty_pointer;
#[cfg(windows)]
pub(crate) mod windows_endpoints;
