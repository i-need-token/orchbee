use super::*;
use orchbee_agent_observation_protocol::AgentReportProvider as Provider;

fn explain_request(a: &WorkbenchAuthority) -> PanelAgentRequest {
    let Action::Observe { binding, .. } = observation(4, None) else {
        unreachable!()
    };
    PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"7".repeat(32)).unwrap(),
        site_id: a.site_id().clone(),
        action: Action::Explain { binding },
    }
}
fn detect(a: &mut WorkbenchAuthority, facts: &[TerminalAgentBinding], provider: Provider) {
    let now = Instant::now();
    a.observe_terminal_agent(
        &facts[0],
        facts,
        TerminalAgentProcessObservation {
            observed_at: now,
            agent: Some(provider),
            fallback: None,
            exited: false,
        },
        now,
    )
    .unwrap();
}
#[test]
fn physical_explanation_uses_detected_provider_behind_custom_label_and_refuses_rebound_host() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    invoke(&mut a, state("custom", "1"), &facts);
    let request = explain_request(&a);
    assert_eq!(
        a.prepare_terminal_agent_explain(&request, &facts, Instant::now()),
        Err(Failure::Unavailable)
    );
    detect(&mut a, &facts, Provider::Codex);
    let before = a
        .prepare_terminal_agent_explain(&request, &facts, Instant::now())
        .unwrap();
    assert_eq!(before.provider, Provider::Codex);
    assert_eq!(before.view.agent.as_deref(), Some("Team Agent"));
    assert!(before.skipped().is_none());
    let again = a
        .prepare_terminal_agent_explain(&request, &facts, Instant::now())
        .unwrap();
    assert_eq!(
        again, before,
        "diagnostic preparation does not publish a screen state"
    );
    invoke(
        &mut a,
        report(
            json!({"kind":"state","source":"custom","agent":"Team Agent","state":"idle","sequence":"2"}),
        ),
        &facts,
    );
    assert_ne!(
        a.prepare_terminal_agent_explain(&request, &facts, Instant::now())
            .unwrap(),
        before
    );
    let mut replaced = physical(1, 2);
    replaced.identity = HostTerminalIdentity::new(
        HostCreationId::from_bytes([4; 16]).unwrap(),
        HostEpoch::from_bytes([5; 16]).unwrap(),
        HostTerminalId::from_bytes([6; 16]).unwrap(),
    );
    assert_eq!(
        a.prepare_terminal_agent_explain(&request, &[replaced], Instant::now()),
        Err(Failure::BindingChanged)
    );
}
#[test]
fn physical_explanation_full_lifecycle_context_emits_valid_explicit_screen_skip() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    invoke(
        &mut a,
        report(
            json!({"kind":"state","source":"herdr:pi","agent":"pi","state":"working","sequence":"11","agentSessionId":"current"}),
        ),
        &facts,
    );
    invoke(
        &mut a,
        report(
            json!({"kind":"session","source":"herdr:pi","agent":"pi","sequence":"10","agentSessionId":"current","sessionStartSource":"startup"}),
        ),
        &facts,
    );
    detect(&mut a, &facts, Provider::Pi);
    let request = explain_request(&a);
    let context = a
        .prepare_terminal_agent_explain(&request, &facts, Instant::now())
        .unwrap();
    let skipped = context
        .skipped()
        .expect("live full lifecycle skips Host capture");
    assert!(skipped.sample.is_none());
    assert!(skipped.explanation.evaluated_rules.is_empty());
    let result = PanelAgentResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: Outcome::Explained {
            workbench_revision: context.workbench_revision,
            report_revision: context.report_revision,
            explanation: Box::new(skipped),
        },
    };
    assert!(result.validate().is_ok());
    assert!(result.answers(&request));
}
