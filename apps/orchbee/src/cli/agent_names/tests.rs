use super::*;

#[test]
fn launch_protocol_and_pending_names_match_published_schemas() {
    use orchbee_agent_observation_protocol::{parse_agent_request, parse_agent_result};
    let wire = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../../../../../schemas/agent-observation/v1/agent-observation.schema.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let id = "11111111111111111111111111111111";
    let reservation = json!({"reservationId":id,"name":"alpha","providerId":"codex","binding":{"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id},"remainingMs":10000,"settleRemainingMs":3000});
    for (operation, fields, outcome) in [
        (
            "reserve",
            json!({"name":"alpha","providerId":"codex","presentationId":id,"hostEpoch":id,"hostTerminalId":id,"ttlMs":10000}),
            json!({"kind":"reserved","reservation":reservation,"runToken":id}),
        ),
        (
            "inspect",
            json!({"reservationId":id}),
            json!({"kind":"pending","reservation":reservation,"candidateSessionId":null,"receipt":{"phase":"finished","succeeded":true}}),
        ),
        (
            "complete",
            json!({"reservationId":id,"sessionId":"s1"}),
            json!({"kind":"completed","reservationId":id,"name":"alpha","sessionId":"s1"}),
        ),
        (
            "cancel",
            json!({"reservationId":id}),
            json!({"kind":"released","reservationId":id}),
        ),
    ] {
        let mut request = fields;
        request["kind"] = json!(format!("agent.launch.{operation}.request"));
        request["requestId"] = json!(id);
        assert!(wire.is_valid(&request), "{request}");
        assert!(parse_agent_request(&serde_json::to_vec(&request).unwrap()).is_ok());
        let result = json!({"kind":format!("agent.launch.{operation}.result"),"requestId":id,"result":{"outcome":outcome}});
        assert!(wire.is_valid(&result), "{result}");
        assert!(parse_agent_result(&serde_json::to_vec(&result).unwrap()).is_ok());
        let mut invalid = request;
        invalid["surprise"] = json!(true);
        assert!(!wire.is_valid(&invalid));
        assert!(parse_agent_request(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let listed = json!({"kind":"agent.session.names.list.result","requestId":id,"result":{"outcome":{"kind":"listed","bindings":[],"pending":[reservation]}}});
    assert!(wire.is_valid(&listed));
    assert!(parse_agent_result(&serde_json::to_vec(&listed).unwrap()).is_ok());
    let result = serde_json::from_value(listed).unwrap();
    let schema = jsonschema::validator_for(
        &serde_json::from_str(include_str!("../schemas/orchbee.agent.names.json")).unwrap(),
    )
    .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let answer = render(&result, OutputFormat::Json, locale);
        let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
        assert!(schema.is_valid(&document), "{document}");
        assert_eq!(document["version"], 1);
        let answer = render(&result, OutputFormat::Text, locale);
        assert!(
            String::from_utf8(answer.stdout)
                .unwrap()
                .contains("alpha\tcodex\t10000")
        );
    }
}

#[test]
fn semantic_refusals_preserve_the_correlated_owner_and_validate_the_failure_schema() {
    let validator = jsonschema::validator_for(
        &serde_json::from_str(include_str!(
            "../schemas/orchbee.cli.failure.agent-names.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for (operation, outcome, kind) in [
        (
            "resolve",
            json!({"kind":"rejected","reason":"launch-pending"}),
            "rejected",
        ),
        (
            "name.assign",
            json!({"kind":"rejected","reason":"name-in-use","conflictSessionId":"incumbent"}),
            "rejected",
        ),
        (
            "name.assign",
            json!({"kind":"rejected","reason":"manage-denied","conflictSessionId":null}),
            "denied",
        ),
        (
            "name.release",
            json!({"kind":"rejected","reason":"name-not-found"}),
            "not-found",
        ),
        (
            "resolve",
            json!({"kind":"unavailable","reason":"runtime-unavailable"}),
            "service-unavailable",
        ),
    ] {
        let result: AgentNamesResult = serde_json::from_value(json!({"kind":format!("agent.session.{operation}.result"),"requestId":"11111111111111111111111111111111","result":{"outcome":outcome}})).unwrap();
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output = render(&result, OutputFormat::Json, locale);
            assert_ne!(output.status, 0);
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(validator.is_valid(&document), "{document}");
            assert_eq!(document["failure"]["kind"], kind);
            assert_eq!(
                document["source"]["requestId"],
                "11111111111111111111111111111111"
            );
            if operation == "name.assign" && kind == "rejected" {
                assert_eq!(
                    document["failure"]["details"]["conflictTarget"]["sessionId"],
                    "incumbent"
                );
            }
            let text = render(&result, OutputFormat::Text, locale);
            assert!(text.stdout.is_empty());
            assert!(!text.stderr.is_empty());
        }
    }
}
