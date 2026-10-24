//! Bounded, value-free diagnostics emitted while admitting Windows user settings.

use std::collections::{BTreeMap, BTreeSet};

use super::WindowsSettingsError;

/// Maximum visible diagnostics in one admitted settings result.
pub const MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS: usize = 128;
const MAXIMUM_DIAGNOSTIC_FIELD_BYTES: usize = 256;

/// The effective treatment applied to one invalid or unknown field.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WindowsSettingsDiagnosticDisposition {
    /// The field's documented safe default was used.
    Defaulted,
    /// The explicitly configured behavior was retained as disabled.
    Disabled,
    /// The field is unknown and has no effect.
    Ignored,
}

impl WindowsSettingsDiagnosticDisposition {
    /// Return the stable machine token used by public CLI diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Defaulted => "defaulted",
            Self::Disabled => "disabled",
            Self::Ignored => "ignored",
        }
    }
}

/// One bounded diagnostic without the rejected value or command text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsSettingsDiagnostic {
    /// Canonical escaped dotted field path.
    pub field: String,
    /// Bounded canonical lowercase kebab reason token.
    pub reason: &'static str,
    /// How admission handled the field.
    pub disposition: WindowsSettingsDiagnosticDisposition,
}

/// Visible sorted diagnostics plus the exact number omitted by the public bound.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WindowsSettingsDiagnostics {
    items: Vec<WindowsSettingsDiagnostic>,
    omitted_count: usize,
}

impl WindowsSettingsDiagnostics {
    /// Return the visible diagnostics in canonical field/reason order.
    pub fn items(&self) -> &[WindowsSettingsDiagnostic] {
        &self.items
    }

    /// Return the exact number of diagnostics beyond the visible bound.
    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }

    /// Return whether neither visible nor omitted diagnostics exist.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.omitted_count == 0
    }
}

#[derive(Default)]
pub(super) struct DiagnosticsCollector {
    // Deduplicate before display truncation so two long source fields never collapse.
    entries: BTreeMap<(String, &'static str), WindowsSettingsDiagnosticDisposition>,
}

impl DiagnosticsCollector {
    pub(super) fn push(
        &mut self,
        field: impl Into<String>,
        reason: &'static str,
        disposition: WindowsSettingsDiagnosticDisposition,
    ) {
        let field = field.into();
        self.entries
            .entry((field, reason))
            .and_modify(|current| *current = (*current).min(disposition))
            .or_insert(disposition);
    }

    pub(super) fn push_error(
        &mut self,
        error: &WindowsSettingsError,
        disposition: WindowsSettingsDiagnosticDisposition,
    ) {
        self.push(error.field, error.reason, disposition);
    }

    pub(super) fn finish(self) -> WindowsSettingsDiagnostics {
        let omitted_count = self
            .entries
            .len()
            .saturating_sub(MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS);
        let mut used = BTreeSet::new();
        let mut items: Vec<_> = self
            .entries
            .into_iter()
            .take(MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS)
            .enumerate()
            .map(|(index, ((source_field, reason), disposition))| {
                let mut attempt = index;
                let field = loop {
                    let field = bounded_field(&source_field, attempt);
                    if used.insert((field.clone(), reason)) {
                        break field;
                    }
                    attempt += MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS;
                };
                WindowsSettingsDiagnostic {
                    field,
                    reason,
                    disposition,
                }
            })
            .collect();
        items.sort_by(|left, right| {
            (left.field.as_str(), left.reason).cmp(&(right.field.as_str(), right.reason))
        });
        WindowsSettingsDiagnostics {
            items,
            omitted_count,
        }
    }
}

pub(super) fn unknown_field(parent: &str, segment: &str) -> String {
    let mut escaped = segment
        .chars()
        .fold(String::new(), |mut output, character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                output.push(character);
            } else {
                use std::fmt::Write;
                let _ = write!(output, "\\u{{{:x}}}", character as u32);
            }
            output
        });
    if escaped.is_empty() {
        escaped.push_str("\\u{empty}");
    }
    if parent.is_empty() {
        escaped
    } else {
        format!("{parent}.{escaped}")
    }
}

fn truncate_utf8(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    const SUFFIX: &str = "...";
    let mut end = maximum.saturating_sub(SUFFIX.len()).min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = String::with_capacity(maximum);
    result.push_str(&value[..end]);
    result.push_str(SUFFIX);
    result
}

fn bounded_field(value: &str, sequence: usize) -> String {
    if value.len() <= MAXIMUM_DIAGNOSTIC_FIELD_BYTES {
        return value.to_owned();
    }
    let suffix = format!("~{sequence:03}");
    let mut result = truncate_utf8(
        value,
        MAXIMUM_DIAGNOSTIC_FIELD_BYTES.saturating_sub(suffix.len()),
    );
    result.push_str(&suffix);
    result
}

#[cfg(test)]
mod tests;
