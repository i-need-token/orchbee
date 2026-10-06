//! Three-field candidates and one observed-source save; applications own live publication.
mod document;
#[cfg(test)]
mod tests;
use super::{
    LIMIT, WindowsNotificationDelivery, WindowsSettingsDiagnostics, WindowsThemeName,
    WindowsUserSettings, WindowsUserSettingsLoad, apply_resume_environment, parse,
};
use crate::WindowsPrivateCache;
use sha2::{Digest, Sha256};
use std::{ffi::OsString, path::Path};

/// Only explicitly changed leaves are edited; no unrelated invalid section is repaired.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowsNativeSettingsChanges {
    /// New shipped chrome theme, absent when unchanged in the draft.
    pub theme: Option<WindowsThemeName>,
    /// Automatic reminders only; enabled delivery channels retain their identity.
    pub background_reminders: Option<bool>,
    /// Global sound switch, without changing per-Agent overrides.
    pub sound_enabled: Option<bool>,
}
impl WindowsNativeSettingsChanges {
    /// An empty patch cannot authorize a save.
    pub const fn is_empty(self) -> bool {
        self.theme.is_none() && self.background_reminders.is_none() && self.sound_enabled.is_none()
    }
}

/// Exact bounded source content, or explicit absence; never a timestamp guess.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsSettingsSource(String);
impl WindowsSettingsSource {
    /// Admit the closed absence token or a lowercase SHA-256 digest.
    pub fn parse(value: &str) -> Option<Self> {
        (value == "missing"
            || (value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))))
        .then(|| Self(value.to_owned()))
    }
    /// Locale-neutral token; never contains configuration values or paths.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    fn of(bytes: Option<&[u8]>) -> Self {
        Self(bytes.map_or_else(
            || "missing".to_owned(),
            |bytes| format!("{:x}", Sha256::digest(bytes)),
        ))
    }
}

/// A readonly observation whose complete settings and token derive from the same bytes.
pub struct WindowsNativeSettingsObservation {
    /// Exact source identity to be compared again for an explicit save.
    pub source: WindowsSettingsSource,
    /// Fully admitted settings and diagnostics, including absence/environment semantics.
    pub load: WindowsUserSettingsLoad,
}

/// Definite pre-save refusals; live application and lost transport belong to the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsNativeSettingsFailure {
    /// Source bytes or absence no longer match the observation.
    SourceChanged,
    /// Unsafe, malformed, unreadable, absent explicit or over-limit source.
    InvalidSource,
    /// Empty patch, invalid target section or unadmitted complete candidate.
    InvalidChanges,
    /// The single flushed publication did not succeed.
    WriteFailed,
}

/// Observe without creating a parent, lock, file or pending candidate.
pub fn read_windows_native_settings(
    path: &Path,
    explicit: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsNativeSettingsObservation, WindowsNativeSettingsFailure> {
    let read = WindowsPrivateCache::read_configuration_existing_pinned(path, LIMIT as usize)
        .map_err(|_| WindowsNativeSettingsFailure::InvalidSource)?;
    let load = load_bytes(read.bytes(), path, explicit, &env)?;
    Ok(WindowsNativeSettingsObservation {
        source: WindowsSettingsSource::of(read.bytes()),
        load,
    })
}

/// Owns the cooperating-writer lock and a fully parsed candidate until one commit or drop.
pub struct PreparedWindowsNativeSettings {
    cache: WindowsPrivateCache,
    name: String,
    before: Option<Vec<u8>>,
    previous: WindowsUserSettingsLoad,
    candidate: WindowsUserSettingsLoad,
    text: String,
}
impl PreparedWindowsNativeSettings {
    /// The complete observed baseline for the Server's publication conflict check.
    pub const fn previous(&self) -> &WindowsUserSettingsLoad {
        &self.previous
    }
    /// The complete candidate for admission and exact outbound preflight before disk effects.
    pub const fn candidate(&self) -> &WindowsUserSettingsLoad {
        &self.candidate
    }
    /// Byte-identical candidates are intentional no-ops.
    pub fn changed(&self) -> bool {
        self.before.as_deref() != Some(self.text.as_bytes())
    }
    /// Consumes the capability, rechecks the source and attempts at most one publication.
    /// An absent target is created without replacement; a failure never retries or repairs it.
    pub fn commit(mut self) -> Result<bool, WindowsNativeSettingsFailure> {
        self.cache
            .publish_configuration(&self.name, self.before.as_deref(), self.text.as_bytes())
            .map_err(|error| match error {
                crate::private_cache_windows::ConfigurationWriteFailure::SourceChanged => {
                    WindowsNativeSettingsFailure::SourceChanged
                }
                crate::private_cache_windows::ConfigurationWriteFailure::Io => {
                    WindowsNativeSettingsFailure::WriteFailed
                }
            })
    }
}

/// Freeze a complete guarded candidate; creation is limited to the private writer boundary.
/// Callers must admit and preflight live publication before consuming its commit capability.
pub fn prepare_windows_native_settings(
    path: &Path,
    explicit: bool,
    expected: &WindowsSettingsSource,
    changes: WindowsNativeSettingsChanges,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PreparedWindowsNativeSettings, WindowsNativeSettingsFailure> {
    if changes.is_empty() {
        return Err(WindowsNativeSettingsFailure::InvalidChanges);
    }
    let observed = read_windows_native_settings(path, explicit, &env)?;
    if observed.source != *expected {
        return Err(WindowsNativeSettingsFailure::SourceChanged);
    }
    let parent = path
        .parent()
        .ok_or(WindowsNativeSettingsFailure::InvalidSource)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(WindowsNativeSettingsFailure::InvalidSource)?
        .to_owned();
    let cache = WindowsPrivateCache::acquire(parent)
        .map_err(|_| WindowsNativeSettingsFailure::WriteFailed)?;
    let before = cache
        .read_configuration(&name, LIMIT as usize)
        .map_err(|_| WindowsNativeSettingsFailure::InvalidSource)?;
    if WindowsSettingsSource::of(before.as_deref()) != *expected {
        return Err(WindowsNativeSettingsFailure::SourceChanged);
    }
    let previous = load_bytes(before.as_deref(), path, explicit, &env)?;
    let text = std::str::from_utf8(before.as_deref().unwrap_or_default())
        .map_err(|_| WindowsNativeSettingsFailure::InvalidSource)?;
    let text = document::edit(text, changes, &previous.settings)?;
    if text.len() as u64 > LIMIT {
        return Err(WindowsNativeSettingsFailure::InvalidChanges);
    }
    let candidate = load_bytes(Some(text.as_bytes()), path, explicit, &env)
        .map_err(|_| WindowsNativeSettingsFailure::InvalidChanges)?;
    if changes
        .theme
        .is_some_and(|theme| candidate.settings.theme != theme)
        || changes.background_reminders.is_some_and(|enabled| {
            (candidate.settings.notifications.agent_delivery != WindowsNotificationDelivery::Off)
                != enabled
        })
        || changes
            .sound_enabled
            .is_some_and(|enabled| candidate.settings.notifications.sound_enabled != enabled)
    {
        return Err(WindowsNativeSettingsFailure::InvalidChanges);
    }
    Ok(PreparedWindowsNativeSettings {
        cache,
        name,
        before,
        previous,
        candidate,
        text,
    })
}

fn load_bytes(
    bytes: Option<&[u8]>,
    path: &Path,
    explicit: bool,
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<WindowsUserSettingsLoad, WindowsNativeSettingsFailure> {
    let mut load = match bytes {
        Some(bytes) => {
            if bytes.len() as u64 > LIMIT {
                return Err(WindowsNativeSettingsFailure::InvalidSource);
            }
            let text = std::str::from_utf8(bytes)
                .map_err(|_| WindowsNativeSettingsFailure::InvalidSource)?;
            let mut load =
                parse(text, path, env).map_err(|_| WindowsNativeSettingsFailure::InvalidSource)?;
            load.settings.explicit = explicit;
            load
        }
        None if !explicit => WindowsUserSettingsLoad {
            settings: WindowsUserSettings {
                path: Some(path.to_owned()),
                ..Default::default()
            },
            diagnostics: WindowsSettingsDiagnostics::default(),
        },
        None => return Err(WindowsNativeSettingsFailure::InvalidSource),
    };
    apply_resume_environment(&mut load, env)
        .map_err(|_| WindowsNativeSettingsFailure::InvalidSource)?;
    Ok(load)
}
