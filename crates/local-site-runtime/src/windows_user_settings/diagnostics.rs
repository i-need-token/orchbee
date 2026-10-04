//! Bounded, value-free diagnostics emitted while admitting Windows user settings.

use std::collections::{BTreeMap, BTreeSet};

use super::WindowsSettingsError;

/// Maximum visible diagnostics in one admitted settings result.
pub const MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS: usize = 128;
const MAXIMUM_DIAGNOSTIC_FIELD_BYTES: usize = 256;

/// The effective treatment applied to one invalid, unknown, or unsupported field.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WindowsSettingsDiagnosticDisposition {
    /// The field's documented safe default was used.
    Defaulted,
    /// The explicitly configured behavior was retained as disabled.
    Disabled,
    /// The field is unknown and has no effect.
    Ignored,
    /// The field is recognized from the pinned Herdr configuration but is not implemented.
    Unsupported,
}

impl WindowsSettingsDiagnosticDisposition {
    /// Return the stable machine token used by public CLI diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Defaulted => "defaulted",
            Self::Disabled => "disabled",
            Self::Ignored => "ignored",
            Self::Unsupported => "unsupported",
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
mod tests {
    use super::*;

    #[test]
    fn bounds_sorting_and_pre_truncation_deduplication_are_stable() {
        let mut collector = DiagnosticsCollector::default();
        collector.push(
            "z",
            "wrong-type",
            WindowsSettingsDiagnosticDisposition::Defaulted,
        );
        collector.push(
            "a",
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
        collector.push(
            "a",
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
        for index in 0..MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS {
            collector.push(
                format!("x.{index:03}"),
                "unknown-key",
                WindowsSettingsDiagnosticDisposition::Ignored,
            );
        }
        let diagnostics = collector.finish();
        assert_eq!(
            diagnostics.items().len(),
            MAXIMUM_WINDOWS_SETTINGS_DIAGNOSTICS
        );
        assert_eq!(diagnostics.omitted_count(), 2);
        assert_eq!(diagnostics.items()[0].field, "a");
    }

    #[test]
    fn unknown_segments_are_escaped_and_truncated_without_invalid_utf8() {
        let first = unknown_field("theme", &format!("{}a", "界".repeat(100)));
        let second = unknown_field("theme", &format!("{}b", "界".repeat(100)));
        assert_ne!(
            first, second,
            "source fields remain distinct until collection finishes"
        );
        assert!(std::str::from_utf8(first.as_bytes()).is_ok());
        assert_eq!(unknown_field("", ""), "\\u{empty}");
        assert_eq!(unknown_field("ui", ""), "ui.\\u{empty}");

        let mut collector = DiagnosticsCollector::default();
        collector.push(
            format!("field.{}", "界".repeat(MAXIMUM_DIAGNOSTIC_FIELD_BYTES)),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
        let diagnostics = collector.finish();
        let field = &diagnostics.items()[0].field;
        assert!(field.len() <= MAXIMUM_DIAGNOSTIC_FIELD_BYTES);
        assert!(std::str::from_utf8(field.as_bytes()).is_ok());
        assert!(field.ends_with("~000"));
    }

    #[test]
    fn display_truncation_keeps_distinct_source_fields_distinct() {
        let prefix = "x".repeat(MAXIMUM_DIAGNOSTIC_FIELD_BYTES + 20);
        let mut collector = DiagnosticsCollector::default();
        collector.push(
            format!("{prefix}a"),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
        collector.push(
            format!("{prefix}b"),
            "unknown-key",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
        let diagnostics = collector.finish();
        assert_eq!(diagnostics.items().len(), 2);
        assert_ne!(diagnostics.items()[0].field, diagnostics.items()[1].field);
        assert!(
            diagnostics
                .items()
                .iter()
                .all(|item| item.field.len() <= MAXIMUM_DIAGNOSTIC_FIELD_BYTES)
        );
    }
}
