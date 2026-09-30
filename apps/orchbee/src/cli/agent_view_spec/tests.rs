use super::*;
use orchbee_workbench_protocol::{
    AgentViewSnapshot, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision,
};
fn answer(outcome: AgentViewOutcome) -> Answer {
    let request = AgentViewRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        site_id: SiteId::parse(&"2".repeat(32)).unwrap(),
        action: AgentViewAction::Clear { source: None },
    };
    Answer {
        observation: Observation::Server(AgentViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome,
        }),
        request,
        interrupted: false,
        cleanup_failed: false,
    }
}
#[test]
fn agent_view_receipts_have_closed_schema_status_and_localized_text() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.agent.view.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let outcome=AgentViewOutcome::Available{snapshot:AgentViewSnapshot{revision:WorkbenchRevision::new(1),spec:Some(orchbee_workbench_protocol::parse_agent_view_spec(br#"{"source":"tools","label":"A","filter":{"op":"eq","field":"seen","value":false},"sort":[{"field":{"token":"phase"},"order":"desc"}]}"#).unwrap())}};
    let answer = answer(outcome);
    assert!(validator.is_valid(&answer.document()));
    assert_eq!(answer.status(), 0);
    assert!(
        String::from_utf8(answer.emit(OutputFormat::Text, Locale::EnUs).stdout)
            .unwrap()
            .contains("Agent view")
    );
    assert!(
        String::from_utf8(answer.emit(OutputFormat::Text, Locale::ZhCn).stdout)
            .unwrap()
            .contains("Agent 视图")
    );
    for (reason, code) in [
        (AgentViewFailure::Denied, 3),
        (AgentViewFailure::PluginDisabled, 3),
        (AgentViewFailure::PluginNotFound, 3),
        (AgentViewFailure::Unavailable, 4),
        (AgentViewFailure::SourceUnavailable, 4),
        (AgentViewFailure::InvalidRequest, 5),
        (AgentViewFailure::RevisionExhausted, 1),
    ] {
        let failure = super::tests::answer(AgentViewOutcome::Failed { reason });
        assert!(validator.is_valid(&failure.document()));
        assert_eq!(failure.status(), code);
    }
    let mut unknown = super::tests::answer(AgentViewOutcome::Available {
        snapshot: AgentViewSnapshot {
            revision: WorkbenchRevision::new(0),
            spec: None,
        },
    });
    unknown.observation = Observation::Unknown("response-read");
    unknown.interrupted = true;
    unknown.cleanup_failed = true;
    assert_eq!(unknown.status(), 8);
    assert!(validator.is_valid(&unknown.document()));
    let mut exchange = super::tests::answer(AgentViewOutcome::Available {
        snapshot: AgentViewSnapshot {
            revision: WorkbenchRevision::new(0),
            spec: None,
        },
    });
    exchange.request.action = AgentViewAction::Read {};
    for (reason, status) in [
        (MetadataExchangeReason::SessionUnavailable, 4),
        (MetadataExchangeReason::Encoding, 1),
        (MetadataExchangeReason::Deadline, 7),
        (MetadataExchangeReason::NotSubmitted, 5),
        (MetadataExchangeReason::Write, 5),
        (MetadataExchangeReason::Read, 5),
        (MetadataExchangeReason::ResponseDeadline, 7),
        (MetadataExchangeReason::Correlation, 5),
        (MetadataExchangeReason::Protocol, 5),
    ] {
        exchange.observation = Observation::ExchangeFailed(reason);
        assert!(validator.is_valid(&exchange.document()));
        assert_eq!(exchange.status(), status);
    }
    assert!(
        String::from_utf8(exchange.emit(OutputFormat::Text, Locale::ZhCn).stdout)
            .unwrap()
            .contains("交换失败")
    );
    let mut impossible = unknown.document();
    impossible["operation"] = serde_json::json!("get");
    assert!(!validator.is_valid(&impossible));
    impossible = exchange.document();
    impossible["operation"] = serde_json::json!("set");
    assert!(!validator.is_valid(&impossible));
    let mut bad = answer.document();
    bad["result"]["snapshot"]["spec"]["filter"]["value"] = serde_json::json!("false");
    assert!(!validator.is_valid(&bad));
    bad = answer.document();
    bad["extra"] = serde_json::json!(1);
    assert!(!validator.is_valid(&bad));
}
