//! Sparse Herdr configuration conversion and explicit new-file publication; no discovery or reload.
mod conversion;
mod files_windows;
mod paths;
use super::{
    DiagnosticsCollector, WindowsSettingsDiagnosticDisposition, WindowsSettingsDiagnostics,
    WindowsSettingsError, WindowsUserSettings, parse,
};
pub use files_windows::{
    WindowsHerdrImportWriteFailure, read_windows_herdr_source, write_windows_herdr_import,
};
pub use paths::{normalize_windows_herdr_file_operand, valid_windows_herdr_file_operand};
use std::{collections::BTreeMap, ffi::OsString, path::Path};

/// Treatment of one explicitly configured source field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsHerdrImportDisposition {
    /// The supported explicit field was converted.
    Mapped,
    /// An invalid field was omitted so its existing safe default remains.
    Defaulted,
    /// An invalid known action was retained as disabled.
    Disabled,
    /// An unknown or shadowed field was omitted.
    Ignored,
    /// A recognized field outside the migration scope was omitted.
    Unsupported,
}

impl WindowsHerdrImportDisposition {
    /// Stable locale-neutral receipt token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mapped => "mapped",
            Self::Defaulted => "defaulted",
            Self::Disabled => "disabled",
            Self::Ignored => "ignored",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Bounded value-free field identity and its conversion outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsHerdrImportEntry {
    /// Canonical escaped source identity, bounded by the existing diagnostic owner.
    pub field: String,
    /// Candidate identities written for this source, absent for omitted fields.
    pub target_fields: Vec<String>,
    /// Explicit conversion treatment.
    pub disposition: WindowsHerdrImportDisposition,
    /// Canonical nonlocalized reason without a source value.
    pub reason: &'static str,
}

/// Stable field report with exact omitted count beyond its 128-entry limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsHerdrImportReport {
    items: Vec<WindowsHerdrImportEntry>,
    omitted_count: usize,
}

impl WindowsHerdrImportReport {
    /// Visible fields in canonical field/reason order.
    pub fn items(&self) -> &[WindowsHerdrImportEntry] {
        &self.items
    }
    /// Exact count excluded by the report limit.
    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }
}

/// Fully encoded and readmitted candidate; construction performs no filesystem effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedWindowsHerdrImport {
    candidate_toml: String,
    report: WindowsHerdrImportReport,
}

impl PreparedWindowsHerdrImport {
    /// Complete sparse UTF-8 TOML, at most 64 KiB, with exactly one trailing newline.
    pub fn candidate_toml(&self) -> &str {
        &self.candidate_toml
    }
    /// Value-free conversion report.
    pub const fn report(&self) -> &WindowsHerdrImportReport {
        &self.report
    }
}

#[derive(Default)]
struct ReportCollector {
    fields: DiagnosticsCollector,
    targets: BTreeMap<String, Vec<String>>,
}

impl ReportCollector {
    fn mapped(&mut self, field: &str, targets: &[&str]) {
        self.targets.insert(
            field.to_owned(),
            targets.iter().map(|s| (*s).to_owned()).collect(),
        );
        self.fields.push(
            field,
            "mapped",
            WindowsSettingsDiagnosticDisposition::Ignored,
        );
    }
    fn disabled_target(&mut self, field: &str, target: &str) {
        self.targets
            .insert(field.to_owned(), vec![target.to_owned()]);
    }
    fn absorb(&mut self, diagnostics: &WindowsSettingsDiagnostics) {
        // Each leaf/action admission is bounded independently before the shared report is bounded.
        for entry in diagnostics.items() {
            self.fields
                .push(&entry.field, entry.reason, entry.disposition);
        }
    }
    fn finish(self) -> WindowsHerdrImportReport {
        let diagnostics = self.fields.finish();
        let items = diagnostics
            .items()
            .iter()
            .map(|entry| WindowsHerdrImportEntry {
                field: entry.field.clone(),
                target_fields: self.targets.get(&entry.field).cloned().unwrap_or_default(),
                disposition: if entry.reason == "mapped" {
                    WindowsHerdrImportDisposition::Mapped
                } else {
                    match entry.disposition {
                        WindowsSettingsDiagnosticDisposition::Defaulted => {
                            WindowsHerdrImportDisposition::Defaulted
                        }
                        WindowsSettingsDiagnosticDisposition::Disabled => {
                            WindowsHerdrImportDisposition::Disabled
                        }
                        WindowsSettingsDiagnosticDisposition::Ignored => {
                            WindowsHerdrImportDisposition::Ignored
                        }
                        WindowsSettingsDiagnosticDisposition::Unsupported => {
                            WindowsHerdrImportDisposition::Unsupported
                        }
                    }
                },
                reason: entry.reason,
            })
            .collect();
        WindowsHerdrImportReport {
            items,
            omitted_count: diagnostics.omitted_count(),
        }
    }
}

/// Convert the admitted explicit subset without discovering files or applying environment overrides.
///
/// # Errors
/// Rejects source syntax/size, oversized output or a failed relocation round-trip invariant.
pub fn prepare_windows_herdr_import(
    text: &str,
    source: &Path,
    destination: &Path,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<PreparedWindowsHerdrImport, WindowsSettingsError> {
    if text.len() > 64 * 1024 {
        return Err(WindowsSettingsError::new("source", "too-large"));
    }
    let source_table: toml::Table =
        toml::from_str(text).map_err(|_| WindowsSettingsError::new("source", "invalid-toml"))?;
    let mut report = ReportCollector::default();
    let table = conversion::convert(&source_table, source, &env, &mut report)?;
    let candidate_toml = conversion::encode(&table);
    if candidate_toml.len() > 64 * 1024 {
        return Err(WindowsSettingsError::new("candidate", "too-large"));
    }
    let admitted = parse(&candidate_toml, source, &|_| None)?;
    // Only deliberately disabled actions may warn after whole-candidate admission.
    // In particular, a global profile bound must not silently discard mapped bindings.
    if admitted.diagnostics.omitted_count() != 0
        || admitted.diagnostics.items().iter().any(|item| {
            let disabled = item
                .field
                .strip_prefix("keys.")
                .and_then(|action| table.get("keys")?.get(action))
                .and_then(toml::Value::as_array)
                .is_some_and(Vec::is_empty);
            !disabled || item.disposition != WindowsSettingsDiagnosticDisposition::Disabled
        })
    {
        return Err(WindowsSettingsError::new("candidate", "round-trip-failed"));
    }
    let mut original_context = admitted.settings;
    let mut destination_context = parse(&candidate_toml, destination, &|_| None)?.settings;
    clear_location(&mut original_context);
    clear_location(&mut destination_context);
    if original_context != destination_context {
        return Err(WindowsSettingsError::new("candidate", "round-trip-failed"));
    }
    Ok(PreparedWindowsHerdrImport {
        candidate_toml,
        report: report.finish(),
    })
}

fn clear_location(settings: &mut WindowsUserSettings) {
    settings.path = None;
    settings.exists = false;
}

#[cfg(test)]
mod tests;
