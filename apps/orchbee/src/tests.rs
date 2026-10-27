#[cfg(target_os = "linux")]
mod linux_output;

#[cfg(windows)]
mod windows_conpty_pointer;
