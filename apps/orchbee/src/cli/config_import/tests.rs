use super::*;
#[test]
fn all_import_receipts_obey_the_closed_schema_without_disclosing_candidate_values() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.config.import-herdr.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let plan = prepare_windows_herdr_import(
        "[ui]\nwindow_title='sensitive-value'",
        Path::new("C:/source.toml"),
        Path::new("C:/target.toml"),
        |_| None,
    )
    .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut receipt = Receipt {
            source: Some("C:/source.toml"),
            destination: Some("C:/target.toml"),
            plan: Some(&plan),
            preview: false,
            created: true,
            error: None,
        };
        for case in 0..WRITE_FAILURES.len() + 2 {
            receipt.preview = case == 1;
            receipt.created = case == 0;
            receipt.error = None;
            if let Some(failure) = case.checked_sub(2).and_then(|i| WRITE_FAILURES.get(i)) {
                receipt.created = failure.created();
                receipt.error = Some((failure.stage(), failure.reason()));
            }
            let answer = encode(&receipt, OutputFormat::Json, locale).unwrap();
            let document: serde_json::Value = serde_json::from_slice(&answer.stdout).unwrap();
            assert!(validator.is_valid(&document), "{document}");
            assert_eq!(
                document["result"].get("candidateToml").is_some(),
                receipt.preview
            );
            assert!(!answer.stderr.contains("sensitive-value"));
            let text = encode(&receipt, OutputFormat::Text, locale).unwrap();
            assert!(
                !String::from_utf8(text.stdout)
                    .unwrap()
                    .contains("sensitive-value")
            );
            let mut invalid = document.clone();
            invalid["result"]["created"] = (!receipt.created).into();
            assert!(!validator.is_valid(&invalid));
        }
    }
}
