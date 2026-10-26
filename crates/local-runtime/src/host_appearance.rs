//! Read-only host appearance capability; precedence remains with each caller.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::read_windows_apps_use_light_theme;
