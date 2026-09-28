use super::*;
use crate::workbench_authority::TerminalAgentBinding;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration,
};
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentOutcome, PanelAgentReport, PanelAgentRequest, PanelAgentState,
    PresentationBinding, TerminalId,
};
use std::{num::NonZeroU64, time::Instant};

fn session(reference: &str) -> Value {
    json!({"source":"herdr:grok","agent":"grok","reference":{"kind":"id","value":reference}})
}
fn rows() -> Value {
    json!([
        {"presentationId":identity(11),"session":session("primary & literal")},
        {"presentationId":identity(15),"session":session("child session")}
    ])
}
fn saved() -> Vec<u8> {
    changed(|v| v["agentSessions"] = rows())
}
fn physical(terminal: u128, generation: u64) -> TerminalAgentBinding {
    TerminalAgentBinding {
        binding: PresentationBinding {
            worker_id: worker(400),
            terminal_id: TerminalId::parse(&identity(terminal)).unwrap(),
        },
        worker_generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes(terminal.to_be_bytes()).unwrap(),
            HostEpoch::from_bytes([2; 16]).unwrap(),
            HostTerminalId::from_bytes(terminal.to_be_bytes()).unwrap(),
        ),
    }
}
fn bind(authority: &mut WorkbenchAuthority) -> Vec<TerminalAgentBinding> {
    let targets = authority.restoration_targets(&worker(400)).unwrap();
    let mut facts = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        let fact = physical(900 + index as u128, 2);
        authority
            .prepare_restored_terminal(target, fact.binding.clone())
            .unwrap()
            .commit();
        facts.push(fact);
    }
    facts
}
fn invoke(
    a: &mut WorkbenchAuthority,
    action: PanelAgentAction,
    facts: &[TerminalAgentBinding],
) -> PanelAgentOutcome {
    a.panel_agent(
        PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&identity(8000)).unwrap(),
            site_id: a.site_id().clone(),
            action,
        },
        facts,
        Instant::now(),
        |p| std::path::Path::new(p).is_absolute(),
    )
    .outcome
}
fn read(
    a: &mut WorkbenchAuthority,
    facts: &[TerminalAgentBinding],
) -> orchbee_workbench_protocol::PanelAgentView {
    let PanelAgentOutcome::Read { panels, .. } = invoke(
        a,
        PanelAgentAction::Read {
            panel_ids: vec![
                orchbee_workbench_protocol::WorkbenchPanelId::parse(&identity(10)).unwrap(),
            ],
        },
        facts,
    ) else {
        panic!("read")
    };
    panels[0].report.clone().unwrap()
}

#[test]
fn v1_remains_readable_but_v2_records_are_required_and_strict() {
    let legacy = changed(|v| {
        v["version"] = json!(1);
        v.as_object_mut().unwrap().remove("agentSessions");
    });
    let upgraded = restore(&legacy).unwrap().checkpoint(&worker(400)).unwrap();
    let upgraded: Value = serde_json::from_slice(&upgraded).unwrap();
    assert_eq!(upgraded["version"], 2);
    assert_eq!(upgraded["agentSessions"], json!([]));
    for fault in 0..10 {
        let bytes = changed(|v| {
            v["agentSessions"] = rows();
            match fault {
                0 => {
                    v.as_object_mut().unwrap().remove("agentSessions");
                }
                1 => v["version"] = json!(1),
                2 => v["agentSessions"] = Value::Null,
                3 => {
                    let r = v["agentSessions"][0].clone();
                    v["agentSessions"].as_array_mut().unwrap().push(r);
                }
                4 => v["agentSessions"][0]["presentationId"] = json!(identity(777)),
                5 => v["agentSessions"][0]["session"]["source"] = json!("custom"),
                6 => v["agentSessions"][0]["session"]["agent"] = json!("GROK"),
                7 => v["agentSessions"][0]["session"]["reference"]["value"] = json!("bad\nref"),
                8 => {
                    v["agentSessions"][0]["session"]["reference"]["value"] = json!("a".repeat(513))
                }
                _ => v["agentSessions"][0]["extra"] = json!(true),
            }
        });
        assert!(WorkbenchCheckpoint::parse(&bytes).is_err(), "fault {fault}");
    }
    let null_legacy = changed(|v| {
        v["version"] = json!(1);
        v["agentSessions"] = Value::Null;
    });
    assert!(WorkbenchCheckpoint::parse(&null_legacy).is_err());
}

#[test]
fn restart_seeds_only_references_and_current_generation_capture_preserves_primary_and_child() {
    let mut a = restore(&saved()).unwrap();
    let targets = a.restoration_targets(&worker(400)).unwrap();
    assert_eq!(
        targets[0].agent_session().unwrap().reference.value,
        "primary & literal"
    );
    assert_eq!(
        targets[1].agent_session().unwrap().reference.value,
        "child session"
    );
    assert!(targets[2].agent_session().is_none());
    let facts = bind(&mut a);
    assert_eq!(
        a.agent_restart_bindings().unwrap(),
        vec![facts[0].binding.clone(), facts[1].binding.clone()]
    );
    let before = a.snapshot().clone();
    assert_eq!(
        a.seed_restored_agent_sessions(&facts, Instant::now())
            .unwrap(),
        2
    );
    assert_eq!(a.snapshot(), &before);
    let view = read(&mut a, &facts);
    assert_eq!(view.agent, None);
    assert_eq!(view.state, PanelAgentState::Unknown);
    assert_eq!(view.authority_source, None);
    assert_eq!(view.session.unwrap().reference.value, "primary & literal");
    let output: Value =
        serde_json::from_slice(&a.checkpoint_with_agents(&worker(400), &facts).unwrap()).unwrap();
    assert_eq!(output["agentSessions"], rows());
    let mut replacement = facts.clone();
    replacement[0].worker_generation = WorkerGeneration::new(NonZeroU64::new(3).unwrap());
    let stale: Value = serde_json::from_slice(
        &a.checkpoint_with_agents(&worker(400), &replacement)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stale["agentSessions"], json!([rows()[1].clone()]));
    assert_eq!(
        a.seed_restored_agent_sessions(&facts, Instant::now())
            .unwrap(),
        0
    );
}

#[test]
fn cleared_reference_never_reappears_from_consumed_restart_intent() {
    let bytes = changed(|v| {
        v["agentSessions"] = rows();
        v["agentSessions"][0]["session"]["agent"] = json!("pi");
        v["agentSessions"][0]["session"]["source"] = json!("herdr:pi");
    });
    let mut a = restore(&bytes).unwrap();
    let facts = bind(&mut a);
    a.seed_restored_agent_sessions(&facts, Instant::now())
        .unwrap();
    let now = Instant::now();
    a.observe_terminal_agent(
        &facts[0],
        &facts,
        crate::workbench_authority::TerminalAgentProcessObservation {
            observed_at: now,
            agent: orchbee_agent_observation_protocol::AgentReportProvider::lookup("pi"),
            fallback: None,
            exited: false,
        },
        now,
    )
    .unwrap();
    for report in [
        json!({"kind":"state","source":"herdr:pi","agent":"pi","state":"idle"}),
        json!({"kind":"clear","source":"herdr:pi"}),
    ] {
        let report: PanelAgentReport = serde_json::from_value(report).unwrap();
        assert!(matches!(
            invoke(
                &mut a,
                PanelAgentAction::Report {
                    panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&identity(10))
                        .unwrap(),
                    expected_binding: facts[0].binding.clone(),
                    report: Box::new(report)
                },
                &facts
            ),
            PanelAgentOutcome::Reported {
                disposition: orchbee_workbench_protocol::PanelAgentDisposition::Applied,
                ..
            }
        ));
    }
    assert!(read(&mut a, &facts).session.is_none());
    assert_eq!(
        a.agent_restart_bindings().unwrap(),
        vec![facts[1].binding.clone()]
    );
    a.seed_restored_agent_sessions(&facts, Instant::now())
        .unwrap();
    let output: Value =
        serde_json::from_slice(&a.checkpoint_with_agents(&worker(400), &facts).unwrap()).unwrap();
    assert_eq!(output["agentSessions"], json!([rows()[1].clone()]));
}

#[test]
fn clean_capture_uses_the_live_report_reference_without_mutating_topology() {
    let mut a = fixture();
    let mut fact = physical(111, 1);
    fact.binding.worker_id = worker(200);
    let facts = [fact];
    let before = a.snapshot().clone();
    let outcome = invoke(&mut a, PanelAgentAction::Report {
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&identity(10)).unwrap(),
        expected_binding: facts[0].binding.clone(),
        report: Box::new(serde_json::from_value(json!({"kind":"session","source":"herdr:grok","agent":"grok","agentSessionId":"primary & literal"})).unwrap()),
    }, &facts);
    assert!(matches!(
        outcome,
        PanelAgentOutcome::Reported {
            disposition: orchbee_workbench_protocol::PanelAgentDisposition::Applied,
            ..
        }
    ));
    let bytes = a.checkpoint_with_agents(&worker(200), &facts).unwrap();
    let output: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(output["agentSessions"], json!([rows()[0].clone()]));
    assert_eq!(a.snapshot(), &before);
}

#[test]
fn seed_failure_is_atomic_and_changed_reference_rejects_prepared_target() {
    let mut a = restore(&saved()).unwrap();
    let target = a.restoration_targets(&worker(400)).unwrap()[0].clone();
    a.pending_agent_sessions
        .get_mut(target.presentation_id())
        .unwrap()
        .reference
        .value = "changed".into();
    assert!(matches!(
        a.prepare_restored_terminal(&target, physical(900, 2).binding),
        Err(WorkbenchRestoreError::TargetChanged)
    ));
    let facts = bind(&mut a);
    let before = a.snapshot().clone();
    a.pending_agent_sessions
        .get_mut(&orchbee_workbench_protocol::TerminalPresentationId::parse(&identity(15)).unwrap())
        .unwrap()
        .reference
        .value = "\n".into();
    assert!(
        a.seed_restored_agent_sessions(&facts, Instant::now())
            .is_err()
    );
    assert_eq!(a.pending_agent_sessions.len(), 2);
    assert_eq!(a.snapshot(), &before);
    assert!(read(&mut a, &facts).session.is_none());
}

#[cfg(windows)]
#[test]
fn native_absolute_paths_are_admitted_only_for_path_capable_official_providers() {
    for (agent, source, path, valid) in [
        ("pi", "herdr:pi", r"C:\sessions\a.jsonl", true),
        ("omp", "herdr:omp", r"\\server\share\a.jsonl", true),
        ("pi", "herdr:pi", r"relative\a.jsonl", false),
        ("pi", "herdr:pi", r"C:a.jsonl", false),
        ("grok", "herdr:grok", r"C:\sessions\a.jsonl", false),
    ] {
        let bytes = changed(
            |v| v["agentSessions"] = json!([{"presentationId":identity(11),"session":{"source":source,"agent":agent,"reference":{"kind":"path","value":path}}}]),
        );
        assert_eq!(WorkbenchCheckpoint::parse(&bytes).is_ok(), valid, "{path}");
    }
}
