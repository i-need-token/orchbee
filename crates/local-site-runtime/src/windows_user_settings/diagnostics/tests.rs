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
