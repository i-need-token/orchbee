//! The assembled Windows `agent doctor` machine document stays inside its
//! published schema bounds.

use serde_json::{Value, json};

use crate::agent_doctor::Report;
use crate::invocation::grammar::values::DOCTOR_PROVIDER_IDS;

#[test]
fn all_windows_provider_rows_fit_the_current_bounded_schema() {
    let report = Report::new(
        DOCTOR_PROVIDER_IDS
            .iter()
            .map(|id| {
                json!({
                    "id": crate::agent_integrations::IntegrationId::parse(id).unwrap().provider(), "executable":{"state":"absent"},
                    "runtime":{"node":{"state":"absent"}},
                    "hook":{"state":"unknown","reason":"unreadable","detail":{}}
                })
            })
            .collect(),
        vec![],
        json!({"socket":{"state":"not-inspected","reason":"windows-endpoint-not-inspected"}}),
        json!({"observationSocketEnv":{"state":"unset"}}),
        true,
    );
    assert_eq!(report.providers.len(), 20);
    let output = super::emit(
        report,
        crate::invocation::OutputFormat::Json,
        crate::localization::Locale::EnUs,
    );
    let mut document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let definition: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.agent.doctor.json")).unwrap();
    let validator = jsonschema::validator_for(&definition).unwrap();
    validator.validate(&document).unwrap();
    let row = document["result"]["providers"][0].clone();
    document["result"]["providers"] = json!(vec![row; 21]);
    assert!(!validator.is_valid(&document));
}
