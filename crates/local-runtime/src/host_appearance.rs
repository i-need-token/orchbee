//! Read-only host appearance capability; precedence remains with each caller.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
pub use macos::read_macos_dark_appearance;
#[cfg(windows)]
pub use windows::read_windows_apps_use_light_theme;
