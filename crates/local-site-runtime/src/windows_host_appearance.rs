//! Read the current user's documented app-theme fallback without mutating Windows appearance.

use winsafe::{self as w, co};

const PERSONALIZE_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const APPS_USE_LIGHT_THEME: &str = "AppsUseLightTheme";

/// Reads `AppsUseLightTheme`; missing, unreadable, and non-boolean values remain unknown.
pub fn read_windows_apps_use_light_theme() -> Option<bool> {
    match w::HKEY::CURRENT_USER
        .RegGetValue(
            Some(PERSONALIZE_KEY),
            Some(APPS_USE_LIGHT_THEME),
            co::RRF::RT_REG_DWORD,
        )
        .ok()?
    {
        w::RegistryValue::Dword(0) => Some(false),
        w::RegistryValue::Dword(1) => Some(true),
        _ => None,
    }
}
