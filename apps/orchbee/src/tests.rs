#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_output;

#[cfg(windows)]
mod windows_conpty_pointer;
