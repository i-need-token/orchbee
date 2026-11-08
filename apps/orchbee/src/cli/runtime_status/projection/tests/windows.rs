use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt};

#[test]
fn local_metadata_preserves_non_unicode_paths_and_closed_unavailable_states() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/orchbee.status.client.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let units = [67, 58, 92, 0xd800, 92, 120];
    for issue in [
        CohortIssue::Missing,
        CohortIssue::Invalid,
        CohortIssue::Denied,
        CohortIssue::TimedOut,
        CohortIssue::Io,
    ] {
        let make_report = || Report::Client {
            executable: PathBuf::from(OsString::from_wide(&units)),
            cohort: Cohort::Unavailable(
                issue,
                Some(CohortDiagnostic {
                    io_kind: CohortIoKind::NotFound,
                    os_error: Some(2),
                    missing_artifact: Some("orchbee-server.exe".to_owned()),
                }),
            ),
            installation: crate::cli::installation::Installation::Unavailable,
        };
        let report = make_report();
        let doc = report.document();
        validator.validate(&doc).unwrap();
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["result"]["executable"], json!({"pathUtf16":units}));
        assert_eq!(
            doc["result"]["cohort"]["diagnostic"],
            json!({"ioKind":"not-found","osError":2,"missingArtifact":"orchbee-server.exe"})
        );
        let en_json = emit(make_report(), OutputFormat::Json, Locale::EnUs);
        let zh_json = emit(make_report(), OutputFormat::Json, Locale::ZhCn);
        assert_eq!(en_json.stdout, zh_json.stdout);
        assert_eq!(en_json.status, 0);
        validator
            .validate(&serde_json::from_slice::<Value>(&en_json.stdout).unwrap())
            .unwrap();
        let en = emit(report, OutputFormat::Text, Locale::EnUs);
        assert_eq!(en.status, 0);
        let text = String::from_utf8(en.stdout).unwrap();
        assert!(!text.contains('\u{fffd}'));
        assert!(!text.contains("NotFound"));
        assert!(!text.contains("missingArtifact"));
        let mut guessed = doc.clone();
        guessed["result"]["cohort"]["identity"] = json!("a".repeat(64));
        assert!(!validator.is_valid(&guessed));
        let mut missing_diagnostic = doc.clone();
        missing_diagnostic["result"]["cohort"]
            .as_object_mut()
            .unwrap()
            .remove("diagnostic");
        assert!(!validator.is_valid(&missing_diagnostic));
        let mut debug_kind = doc.clone();
        debug_kind["result"]["cohort"]["diagnostic"]["ioKind"] = json!("NotFound");
        assert!(!validator.is_valid(&debug_kind));
        let mut unknown_kind = doc.clone();
        unknown_kind["result"]["cohort"]["diagnostic"]["ioKind"] = json!("network-failure");
        assert!(!validator.is_valid(&unknown_kind));
        let mut oversized_os_error = doc.clone();
        oversized_os_error["result"]["cohort"]["diagnostic"]["osError"] = json!(2_147_483_648_i64);
        assert!(!validator.is_valid(&oversized_os_error));
        let mut invented_artifact = doc.clone();
        invented_artifact["result"]["cohort"]["diagnostic"]["missingArtifact"] =
            json!("orchbee-unexpected.exe");
        assert!(!validator.is_valid(&invented_artifact));
        let mut invented_channel = doc;
        invented_channel["result"]["updateChannel"] = json!("stable");
        assert!(!validator.is_valid(&invented_channel));
    }

    let unknown_kind = Report::Client {
        executable: PathBuf::from(r"C:\orchbee.exe"),
        cohort: Cohort::Unavailable(
            CohortIssue::Io,
            Some(CohortDiagnostic {
                io_kind: CohortIoKind::Other,
                os_error: None,
                missing_artifact: None,
            }),
        ),
        installation: crate::cli::installation::Installation::Unavailable,
    }
    .document();
    validator.validate(&unknown_kind).unwrap();
    assert_eq!(
        unknown_kind["result"]["cohort"]["diagnostic"],
        json!({"ioKind":"other","osError":null})
    );
}
