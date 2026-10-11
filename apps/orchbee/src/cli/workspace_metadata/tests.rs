use super::*;
use orchbee_workbench_protocol::{MetadataReportDisposition, ProtocolVersion, WorkbenchRevision};
#[test]
fn workspace_metadata_receipts_survive_late_interrupt_and_validate_the_public_schema() {
    let mut answer = Answer {
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        site_id: SiteId::parse(&format!("{:032x}", 2)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
        observation: Observation::Unknown("response-deadline"),
        interrupted: true,
        cleanup_failed: true,
    };
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.workspace.metadata.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    assert_eq!(answer.exit_code(), 8);
    assert!(validator.is_valid(&answer.document()));
    answer.observation = Observation::Server(WorkspaceMetadataResult {
        protocol_version: ProtocolVersion,
        request_id: answer.request_id.clone(),
        site_id: answer.site_id.clone(),
        outcome: WorkspaceMetadataOutcome::Reported {
            workspace_id: answer.workspace_id.clone(),
            workbench_revision: WorkbenchRevision::new(5),
            metadata_revision: WorkbenchRevision::new(9),
            disposition: MetadataReportDisposition::Applied,
        },
    });
    assert_eq!(answer.exit_code(), 1);
    answer.cleanup_failed = false;
    assert_eq!(answer.exit_code(), 0);
    assert!(validator.is_valid(&answer.document()));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let emitted = answer.emit(OutputFormat::Text, locale);
        assert_eq!(emitted.status, 0);
        assert!(emitted.stderr.is_empty());
        assert!(!emitted.stdout.contains(&0x1b));
        assert!(
            String::from_utf8(emitted.stdout)
                .unwrap()
                .contains(answer.request_id.as_str())
        );
    }
    let mut invalid = answer.document();
    invalid["observation"]["reply"]["outcome"]["disposition"] = serde_json::json!("invented");
    assert!(!validator.is_valid(&invalid));
}
