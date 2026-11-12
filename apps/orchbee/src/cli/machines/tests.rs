use super::*;
#[test]
fn receipts_preserve_committed_and_remote_effects_on_cancellation() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.machine.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let id = ProfileId::parse("11111111111111111111111111111111").unwrap();
    let command = MachineCommand::Remove(id.as_str().to_owned());
    let report = Report {
        catalog: Some(Catalog::default()),
        id: Some(id),
        changed: true,
        committed: true,
        remote: Remote::Ready,
        remote_version: Some("0.0.0-remote".into()),
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let output = emit(
            &command,
            &report,
            Some(Error::Cancelled),
            true,
            OutputFormat::Json,
            locale,
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        validator.validate(&value).unwrap();
        assert_eq!(output.status, 130);
        assert_eq!(value["result"]["committed"], true);
        assert_eq!(value["result"]["remote"], "ready");
        assert!(!output.stderr.is_empty());
        let text = emit(
            &command,
            &report,
            Some(Error::Cancelled),
            true,
            OutputFormat::Text,
            locale,
        );
        assert!(text.stdout.is_empty());
        assert!(!text.stderr.is_empty());
    }
    let report = Report {
        catalog: None,
        id: None,
        changed: false,
        committed: false,
        remote: Remote::NotContacted,
        remote_version: None,
    };
    for error in [
        Error::Catalog(CatalogError::NotFound),
        Error::SshFailed,
        Error::PhysicalUnknown,
        Error::RemoteUnsupported,
        Error::RemoteOtherEnvironment,
        Error::Timeout,
        Error::Catalog(CatalogError::InvalidCatalog),
    ] {
        let output = emit(
            &command,
            &report,
            Some(error),
            false,
            OutputFormat::Json,
            Locale::EnUs,
        );
        validator
            .validate(&serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap())
            .unwrap();
        assert_eq!(output.status, error.status());
    }
}
